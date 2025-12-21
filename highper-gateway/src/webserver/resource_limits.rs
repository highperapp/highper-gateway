/// Resource limits and quotas for production webserver deployment
///
/// This module provides comprehensive resource management to prevent abuse and ensure
/// fair resource allocation across clients and requests.
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use anyhow::{Result, anyhow};
use tracing::{debug, warn};

/// Global resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimitsConfig {
    /// Maximum concurrent connections globally
    pub max_concurrent_connections: usize,
    /// Maximum concurrent connections per IP address
    pub max_connections_per_ip: usize,
    /// Maximum requests per second per IP address
    pub max_requests_per_second_per_ip: u32,
    /// Maximum requests per second globally
    pub max_requests_per_second_global: u32,
    /// Rate limiting window duration
    pub rate_limit_window: Duration,
    /// Maximum open file descriptors for static files
    pub max_open_files: usize,
    /// Maximum memory usage for request buffering (bytes)
    pub max_request_buffer_memory: usize,
}

impl Default for ResourceLimitsConfig {
    fn default() -> Self {
        Self {
            max_concurrent_connections: 10_000,
            max_connections_per_ip: 100,
            max_requests_per_second_per_ip: 100,
            max_requests_per_second_global: 10_000,
            rate_limit_window: Duration::from_secs(1),
            max_open_files: 1_000,
            max_request_buffer_memory: 100 * 1024 * 1024, // 100 MB
        }
    }
}

/// Connection tracker for enforcing connection limits
pub struct ConnectionLimiter {
    config: ResourceLimitsConfig,
    /// Global connection count
    global_connections: Arc<std::sync::atomic::AtomicUsize>,
    /// Per-IP connection counts
    ip_connections: Arc<DashMap<String, usize>>,
}

impl ConnectionLimiter {
    pub fn new(config: ResourceLimitsConfig) -> Self {
        Self {
            config,
            global_connections: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ip_connections: Arc::new(DashMap::new()),
        }
    }

    /// Check if a new connection from the given IP is allowed
    pub fn check_connection(&self, client_ip: &str) -> Result<ConnectionGuard> {
        // Check global limit
        let global_count = self.global_connections.load(std::sync::atomic::Ordering::Relaxed);
        if global_count >= self.config.max_concurrent_connections {
            warn!("Global connection limit reached: {}/{}",
                global_count, self.config.max_concurrent_connections);
            return Err(anyhow!("Too many concurrent connections"));
        }

        // Check per-IP limit
        let mut entry = self.ip_connections.entry(client_ip.to_string()).or_insert(0);
        if *entry >= self.config.max_connections_per_ip {
            warn!("Connection limit reached for IP {}: {}/{}",
                client_ip, *entry, self.config.max_connections_per_ip);
            return Err(anyhow!("Too many connections from this IP"));
        }

        // Increment counters
        self.global_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        *entry += 1;

        debug!("Connection allowed for {}: {}/{} (global: {}/{})",
            client_ip, *entry, self.config.max_connections_per_ip,
            global_count + 1, self.config.max_concurrent_connections);

        Ok(ConnectionGuard {
            limiter: self,
            client_ip: client_ip.to_string(),
        })
    }

    /// Decrement connection counters (called when connection closes)
    fn release_connection(&self, client_ip: &str) {
        self.global_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        if let Some(mut entry) = self.ip_connections.get_mut(client_ip) {
            if *entry > 0 {
                *entry -= 1;
            }

            // Clean up entry if count is 0
            if *entry == 0 {
                drop(entry);
                self.ip_connections.remove(client_ip);
            }
        }
    }

    /// Get current global connection count
    pub fn global_connection_count(&self) -> usize {
        self.global_connections.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get connection count for specific IP
    pub fn ip_connection_count(&self, client_ip: &str) -> usize {
        self.ip_connections.get(client_ip).map(|e| *e).unwrap_or(0)
    }
}

/// RAII guard for automatic connection cleanup
pub struct ConnectionGuard<'a> {
    limiter: &'a ConnectionLimiter,
    client_ip: String,
}

impl<'a> Drop for ConnectionGuard<'a> {
    fn drop(&mut self) {
        self.limiter.release_connection(&self.client_ip);
    }
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    config: ResourceLimitsConfig,
    /// Per-IP rate limiting buckets
    ip_buckets: Arc<DashMap<String, TokenBucket>>,
    /// Global rate limiting bucket
    global_bucket: Arc<parking_lot::Mutex<TokenBucket>>,
}

impl RateLimiter {
    pub fn new(config: ResourceLimitsConfig) -> Self {
        Self {
            global_bucket: Arc::new(parking_lot::Mutex::new(
                TokenBucket::new(config.max_requests_per_second_global, config.rate_limit_window)
            )),
            ip_buckets: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Check if request is allowed for the given IP
    pub fn check_rate_limit(&self, client_ip: &str) -> Result<()> {
        // Check global rate limit
        let mut global_bucket = self.global_bucket.lock();
        if !global_bucket.consume(1) {
            warn!("Global rate limit exceeded");
            return Err(anyhow!("Too many requests globally"));
        }
        drop(global_bucket);

        // Check per-IP rate limit
        let mut entry = self.ip_buckets.entry(client_ip.to_string()).or_insert_with(|| {
            TokenBucket::new(
                self.config.max_requests_per_second_per_ip,
                self.config.rate_limit_window
            )
        });

        if !entry.consume(1) {
            warn!("Rate limit exceeded for IP: {}", client_ip);
            return Err(anyhow!("Too many requests from this IP"));
        }

        debug!("Rate limit check passed for IP: {}", client_ip);
        Ok(())
    }

    /// Cleanup expired buckets (should be called periodically)
    pub fn cleanup_expired_buckets(&self, max_age: Duration) {
        let now = Instant::now();
        self.ip_buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < max_age
        });
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    /// Current number of tokens
    tokens: u32,
    /// Maximum capacity
    capacity: u32,
    /// Last refill time
    last_refill: Instant,
    /// Refill interval
    refill_interval: Duration,
}

impl TokenBucket {
    fn new(capacity: u32, refill_interval: Duration) -> Self {
        Self {
            tokens: capacity,
            capacity,
            last_refill: Instant::now(),
            refill_interval,
        }
    }

    /// Try to consume tokens
    fn consume(&mut self, amount: u32) -> bool {
        self.refill();

        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        if elapsed >= self.refill_interval {
            // Refill to capacity
            self.tokens = self.capacity;
            self.last_refill = now;
        }
    }
}

/// File descriptor limiter for static file serving
pub struct FileDescriptorLimiter {
    config: ResourceLimitsConfig,
    open_files: Arc<std::sync::atomic::AtomicUsize>,
}

impl FileDescriptorLimiter {
    pub fn new(config: ResourceLimitsConfig) -> Self {
        Self {
            config,
            open_files: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Check if opening a new file is allowed
    pub fn check_file_open(&self) -> Result<FileGuard> {
        let current = self.open_files.load(std::sync::atomic::Ordering::Relaxed);

        if current >= self.config.max_open_files {
            warn!("File descriptor limit reached: {}/{}",
                current, self.config.max_open_files);
            return Err(anyhow!("Too many open files"));
        }

        self.open_files.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(FileGuard {
            limiter: self,
        })
    }

    /// Get current open file count
    pub fn open_file_count(&self) -> usize {
        self.open_files.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Decrement open file counter
    fn release_file(&self) {
        self.open_files.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// RAII guard for file descriptor cleanup
pub struct FileGuard<'a> {
    limiter: &'a FileDescriptorLimiter,
}

impl<'a> Drop for FileGuard<'a> {
    fn drop(&mut self) {
        self.limiter.release_file();
    }
}

/// Memory usage tracker for request buffering
pub struct MemoryLimiter {
    config: ResourceLimitsConfig,
    used_memory: Arc<std::sync::atomic::AtomicUsize>,
}

impl MemoryLimiter {
    pub fn new(config: ResourceLimitsConfig) -> Self {
        Self {
            config,
            used_memory: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Check if allocating memory is allowed
    pub fn check_memory_allocation(&self, size: usize) -> Result<MemoryGuard> {
        let current = self.used_memory.load(std::sync::atomic::Ordering::Relaxed);

        if current + size > self.config.max_request_buffer_memory {
            warn!("Memory limit would be exceeded: {} + {} > {}",
                current, size, self.config.max_request_buffer_memory);
            return Err(anyhow!("Memory limit exceeded"));
        }

        self.used_memory.fetch_add(size, std::sync::atomic::Ordering::Relaxed);

        Ok(MemoryGuard {
            limiter: self,
            size,
        })
    }

    /// Get current memory usage
    pub fn used_memory(&self) -> usize {
        self.used_memory.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Release allocated memory
    fn release_memory(&self, size: usize) {
        self.used_memory.fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
    }
}

/// RAII guard for memory cleanup
pub struct MemoryGuard<'a> {
    limiter: &'a MemoryLimiter,
    size: usize,
}

impl<'a> Drop for MemoryGuard<'a> {
    fn drop(&mut self) {
        self.limiter.release_memory(self.size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_connection_limiter() {
        let config = ResourceLimitsConfig {
            max_concurrent_connections: 10,
            max_connections_per_ip: 3,
            ..Default::default()
        };

        let limiter = ConnectionLimiter::new(config);

        // Allow first 3 connections from same IP
        let _guard1 = limiter.check_connection("192.168.1.1").unwrap();
        let _guard2 = limiter.check_connection("192.168.1.1").unwrap();
        let _guard3 = limiter.check_connection("192.168.1.1").unwrap();

        // 4th connection should fail
        assert!(limiter.check_connection("192.168.1.1").is_err());

        // Different IP should work
        let _guard4 = limiter.check_connection("192.168.1.2").unwrap();

        // After dropping guards, connections should be released
        drop(_guard1);
        let _guard5 = limiter.check_connection("192.168.1.1").unwrap();
    }

    #[test]
    fn test_rate_limiter() {
        let config = ResourceLimitsConfig {
            max_requests_per_second_per_ip: 5,
            max_requests_per_second_global: 100,
            rate_limit_window: Duration::from_secs(1),
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);

        // Allow first 5 requests
        for _ in 0..5 {
            limiter.check_rate_limit("192.168.1.1").unwrap();
        }

        // 6th request should fail
        assert!(limiter.check_rate_limit("192.168.1.1").is_err());

        // Different IP should work
        limiter.check_rate_limit("192.168.1.2").unwrap();

        // After waiting, tokens should refill
        thread::sleep(Duration::from_millis(1100));
        limiter.check_rate_limit("192.168.1.1").unwrap();
    }

    #[test]
    fn test_file_descriptor_limiter() {
        let config = ResourceLimitsConfig {
            max_open_files: 3,
            ..Default::default()
        };

        let limiter = FileDescriptorLimiter::new(config);

        let _guard1 = limiter.check_file_open().unwrap();
        let _guard2 = limiter.check_file_open().unwrap();
        let _guard3 = limiter.check_file_open().unwrap();

        // 4th file should fail
        assert!(limiter.check_file_open().is_err());

        // After dropping guard, should work again
        drop(_guard1);
        let _guard4 = limiter.check_file_open().unwrap();

        assert_eq!(limiter.open_file_count(), 3);
    }

    #[test]
    fn test_memory_limiter() {
        let config = ResourceLimitsConfig {
            max_request_buffer_memory: 1024,
            ..Default::default()
        };

        let limiter = MemoryLimiter::new(config);

        let _guard1 = limiter.check_memory_allocation(512).unwrap();
        let _guard2 = limiter.check_memory_allocation(400).unwrap();

        // This should fail (512 + 400 + 200 = 1112 > 1024)
        assert!(limiter.check_memory_allocation(200).is_err());

        // After dropping guard, should work
        drop(_guard1);
        let _guard3 = limiter.check_memory_allocation(500).unwrap();

        assert_eq!(limiter.used_memory(), 900);
    }

    #[test]
    fn test_global_connection_limit() {
        let config = ResourceLimitsConfig {
            max_concurrent_connections: 5,
            max_connections_per_ip: 10,
            ..Default::default()
        };

        let limiter = ConnectionLimiter::new(config);

        let _g1 = limiter.check_connection("1.1.1.1").unwrap();
        let _g2 = limiter.check_connection("2.2.2.2").unwrap();
        let _g3 = limiter.check_connection("3.3.3.3").unwrap();
        let _g4 = limiter.check_connection("4.4.4.4").unwrap();
        let _g5 = limiter.check_connection("5.5.5.5").unwrap();

        // Global limit reached
        assert!(limiter.check_connection("6.6.6.6").is_err());

        assert_eq!(limiter.global_connection_count(), 5);
    }
}
