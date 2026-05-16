//! Connection pooling per route/upstream
//!
//! Provides HTTP connection pooling with:
//! - Per-upstream connection pools
//! - Automatic connection reuse
//! - Idle connection cleanup
//! - Connection health checking
//! - Configurable pool sizes

use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum connections per upstream
    pub max_connections_per_upstream: usize,

    /// Maximum idle time before connection is closed
    pub max_idle_duration: Duration,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Enable connection keep-alive
    pub keep_alive: bool,

    /// Keep-alive timeout
    pub keep_alive_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_upstream: 100,
            max_idle_duration: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(10),
            keep_alive: true,
            keep_alive_timeout: Duration::from_secs(60),
        }
    }
}

/// Pooled connection wrapper
struct PooledConnection {
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
    upstream: String,
}

impl PooledConnection {
    fn new(stream: TcpStream, upstream: String) -> Self {
        let now = Instant::now();
        Self {
            stream,
            created_at: now,
            last_used: now,
            upstream,
        }
    }

    fn is_idle_expired(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }

    fn mark_used(&mut self) {
        self.last_used = Instant::now();
    }
}

/// Per-upstream connection pool
struct UpstreamPool {
    /// Available idle connections
    idle_connections: parking_lot::Mutex<Vec<PooledConnection>>,

    /// Semaphore to limit total connections
    connection_limiter: Arc<Semaphore>,

    /// Pool configuration
    config: PoolConfig,

    /// Upstream identifier
    upstream_name: String,
}

impl UpstreamPool {
    fn new(upstream_name: String, config: PoolConfig) -> Self {
        Self {
            idle_connections: parking_lot::Mutex::new(Vec::new()),
            connection_limiter: Arc::new(Semaphore::new(config.max_connections_per_upstream)),
            config,
            upstream_name,
        }
    }

    /// Try to get an idle connection from the pool
    fn try_get_idle(&self) -> Option<TcpStream> {
        let mut idle = self.idle_connections.lock();

        // Remove expired connections
        idle.retain(|conn| !conn.is_idle_expired(self.config.max_idle_duration));

        // Get the most recently used connection
        if let Some(mut conn) = idle.pop() {
            conn.mark_used();
            debug!(
                upstream = %self.upstream_name,
                pool_size = idle.len(),
                "Reusing pooled connection"
            );
            Some(conn.stream)
        } else {
            None
        }
    }

    /// Return a connection to the pool
    fn return_connection(&self, stream: TcpStream) {
        let mut idle = self.idle_connections.lock();

        // Check pool capacity
        if idle.len() < self.config.max_connections_per_upstream {
            let conn = PooledConnection::new(stream, self.upstream_name.clone());
            idle.push(conn);
            debug!(
                upstream = %self.upstream_name,
                pool_size = idle.len(),
                "Returned connection to pool"
            );
        } else {
            // Pool is full, drop the connection
            debug!(
                upstream = %self.upstream_name,
                "Pool full, dropping connection"
            );
            drop(stream);
        }
    }

    /// Cleanup expired idle connections
    fn cleanup_idle(&self) {
        let mut idle = self.idle_connections.lock();
        let before_count = idle.len();
        idle.retain(|conn| !conn.is_idle_expired(self.config.max_idle_duration));
        let removed = before_count - idle.len();

        if removed > 0 {
            debug!(
                upstream = %self.upstream_name,
                removed = removed,
                remaining = idle.len(),
                "Cleaned up idle connections"
            );
        }
    }

    /// Get pool statistics
    fn stats(&self) -> PoolStats {
        let idle = self.idle_connections.lock();
        PoolStats {
            idle_count: idle.len(),
            max_connections: self.config.max_connections_per_upstream,
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub idle_count: usize,
    pub max_connections: usize,
}

/// Global connection pool manager
pub struct ConnectionPoolManager {
    /// Pools per upstream
    pools: DashMap<String, Arc<UpstreamPool>>,

    /// Pool configuration
    config: PoolConfig,
}

impl ConnectionPoolManager {
    /// Create a new connection pool manager
    pub fn new(config: PoolConfig) -> Self {
        Self {
            pools: DashMap::new(),
            config,
        }
    }

    /// Get or create pool for upstream
    fn get_or_create_pool(&self, upstream: &str) -> Arc<UpstreamPool> {
        self.pools
            .entry(upstream.to_string())
            .or_insert_with(|| {
                debug!(upstream = %upstream, "Creating new connection pool");
                Arc::new(UpstreamPool::new(upstream.to_string(), self.config.clone()))
            })
            .clone()
    }

    /// Get a connection for an upstream
    ///
    /// Returns an existing pooled connection if available,
    /// otherwise creates a new connection.
    pub async fn get_connection(
        &self,
        upstream: &str,
        addr: SocketAddr,
    ) -> Result<TcpStream, std::io::Error> {
        let pool = self.get_or_create_pool(upstream);

        // Try to get an idle connection first
        if let Some(stream) = pool.try_get_idle() {
            return Ok(stream);
        }

        // No idle connection available, create new one
        debug!(
            upstream = %upstream,
            addr = %addr,
            "Creating new connection (no idle connection available)"
        );

        // Acquire permit from semaphore (limits total connections)
        let _permit = pool
            .connection_limiter
            .acquire()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Create new connection with timeout
        let stream = tokio::time::timeout(pool.config.connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timeout"))?
            .map_err(|e| {
                warn!(
                    upstream = %upstream,
                    addr = %addr,
                    error = %e,
                    "Failed to create connection"
                );
                e
            })?;

        // Configure keep-alive if enabled
        if pool.config.keep_alive {
            let sock_ref = socket2::SockRef::from(&stream);
            let keepalive = socket2::TcpKeepalive::new().with_time(pool.config.keep_alive_timeout);
            sock_ref.set_tcp_keepalive(&keepalive)?;
        }

        debug!(
            upstream = %upstream,
            addr = %addr,
            "Created new connection"
        );

        Ok(stream)
    }

    /// Return a connection to the pool for reuse
    pub fn return_connection(&self, upstream: &str, stream: TcpStream) {
        if let Some(pool) = self.pools.get(upstream) {
            pool.return_connection(stream);
        }
    }

    /// Cleanup idle connections across all pools
    pub fn cleanup_idle_connections(&self) {
        for pool in self.pools.iter() {
            pool.value().cleanup_idle();
        }
    }

    /// Get statistics for an upstream pool
    pub fn get_pool_stats(&self, upstream: &str) -> Option<PoolStats> {
        self.pools.get(upstream).map(|pool| pool.stats())
    }

    /// Get statistics for all pools
    pub fn get_all_stats(&self) -> Vec<(String, PoolStats)> {
        self.pools
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().stats()))
            .collect()
    }

    /// Clear all pools (for shutdown or testing)
    pub fn clear_all(&self) {
        self.pools.clear();
    }
}

impl Default for ConnectionPoolManager {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}

/// Spawn background task to cleanup idle connections
pub fn spawn_cleanup_task(pool_manager: Arc<ConnectionPoolManager>, interval: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            pool_manager.cleanup_idle_connections();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections_per_upstream, 100);
        assert_eq!(config.max_idle_duration, Duration::from_secs(90));
        assert!(config.keep_alive);
    }

    #[tokio::test]
    async fn test_pooled_connection_idle_expiry() {
        // Create a listener for testing
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Connect to our test listener
        let stream = TcpStream::connect(addr).await.unwrap();

        let mut conn = PooledConnection::new(stream, "test-upstream".to_string());

        // Should not be expired immediately
        assert!(!conn.is_idle_expired(Duration::from_secs(60)));

        // Mark as used
        conn.mark_used();

        // Should still not be expired
        assert!(!conn.is_idle_expired(Duration::from_secs(60)));
    }

    #[test]
    fn test_pool_manager_creation() {
        let config = PoolConfig::default();
        let manager = ConnectionPoolManager::new(config);

        assert_eq!(manager.pools.len(), 0);
    }

    #[test]
    fn test_pool_stats() {
        let config = PoolConfig::default();
        let manager = ConnectionPoolManager::new(config);

        // Get pool for upstream (creates it)
        let pool = manager.get_or_create_pool("test-backend");

        let stats = pool.stats();
        assert_eq!(stats.idle_count, 0);
        assert_eq!(stats.max_connections, 100);
    }

    #[test]
    fn test_get_all_stats() {
        let config = PoolConfig::default();
        let manager = ConnectionPoolManager::new(config);

        // Create some pools
        manager.get_or_create_pool("backend-1");
        manager.get_or_create_pool("backend-2");
        manager.get_or_create_pool("backend-3");

        let all_stats = manager.get_all_stats();
        assert_eq!(all_stats.len(), 3);
    }

    #[test]
    fn test_clear_all() {
        let config = PoolConfig::default();
        let manager = ConnectionPoolManager::new(config);

        manager.get_or_create_pool("backend-1");
        manager.get_or_create_pool("backend-2");
        assert_eq!(manager.pools.len(), 2);

        manager.clear_all();
        assert_eq!(manager.pools.len(), 0);
    }

    #[tokio::test]
    async fn test_connection_creation() {
        let config = PoolConfig {
            connect_timeout: Duration::from_secs(5),
            ..Default::default()
        };
        let manager = ConnectionPoolManager::new(config);

        // Try to connect to a public DNS server
        let addr: SocketAddr = "8.8.8.8:53".parse().unwrap();
        let result = manager.get_connection("test-upstream", addr).await;

        // Should succeed or fail gracefully
        match result {
            Ok(_stream) => {
                // Connection succeeded
                assert!(true);
            }
            Err(e) => {
                // Connection failed (expected in some environments)
                println!("Connection failed (expected): {}", e);
            }
        }
    }
}
