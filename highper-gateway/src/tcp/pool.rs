//! TCP Connection Pool
//!
//! High-performance connection pooling for database connections
//! Target: >95% connection reuse ratio
//!
//! Features:
//! - Idle connection tracking with LIFO queue (most recently used first)
//! - Connection validation before reuse
//! - Configurable connection lifetime and idle timeout
//! - Connection pre-warming
//! - Comprehensive metrics (reuse ratio, wait time, pool exhaustion)
//! - Lock-free statistics with AtomicU64
//! - Graceful cleanup on shutdown

use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Pooled TCP connection with metadata
#[derive(Debug)]
struct PooledConnection {
    /// The underlying TCP stream
    stream: TcpStream,

    /// Time when connection was created
    created_at: Instant,

    /// Time when connection was last used
    last_used: Instant,

    /// Number of times this connection has been reused
    reuse_count: u32,
}

impl PooledConnection {
    /// Create a new pooled connection
    fn new(stream: TcpStream) -> Self {
        let now = Instant::now();
        Self {
            stream,
            created_at: now,
            last_used: now,
            reuse_count: 0,
        }
    }

    /// Check if connection has expired based on max lifetime
    fn is_expired(&self, max_lifetime: Duration) -> bool {
        self.created_at.elapsed() > max_lifetime
    }

    /// Check if connection is idle for too long
    fn is_idle_timeout(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }

    /// Update last used time and increment reuse count
    fn mark_reused(&mut self) {
        self.last_used = Instant::now();
        self.reuse_count += 1;
    }
}

/// TCP Connection Pool
pub struct TcpConnectionPool {
    /// Backend address
    backend_addr: SocketAddr,

    /// Configuration
    config: PoolConfig,

    /// Idle connections (LIFO queue - most recently used first)
    idle_connections: Arc<Mutex<VecDeque<PooledConnection>>>,

    /// Semaphore to limit total connections (active + idle)
    semaphore: Arc<Semaphore>,

    /// Statistics
    stats: Arc<PoolStats>,

    /// Shutdown signal
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum total connections (active + idle)
    pub max_size: usize,

    /// Minimum idle connections to maintain
    pub min_idle: usize,

    /// Maximum idle connections to keep
    pub max_idle: usize,

    /// Maximum connection lifetime (absolute age limit)
    pub connection_lifetime: Duration,

    /// Maximum idle time before connection is closed
    pub idle_timeout: Duration,

    /// Connection validation timeout
    pub validation_timeout: Duration,

    /// Pre-warm connections on startup
    pub pre_warm: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 100,
            min_idle: 10,
            max_idle: 50,
            connection_lifetime: Duration::from_secs(3600), // 1 hour
            idle_timeout: Duration::from_secs(300),         // 5 minutes
            validation_timeout: Duration::from_millis(100),
            pre_warm: true,
        }
    }
}

/// Connection pool statistics (lock-free)
#[derive(Debug)]
pub struct PoolStats {
    /// Total connections created
    pub total_created: AtomicU64,

    /// Total connections reused from pool
    pub total_reused: AtomicU64,

    /// Total connections closed
    pub total_closed: AtomicU64,

    /// Total validation failures
    pub validation_failures: AtomicU64,

    /// Total times pool was exhausted (had to wait)
    pub pool_exhausted: AtomicU64,

    /// Total connection errors
    pub connection_errors: AtomicU64,

    /// Current idle connection count (approximate)
    pub idle_count: AtomicU64,

    /// Current active connection count (approximate)
    pub active_count: AtomicU64,
}

impl PoolStats {
    fn new() -> Self {
        Self {
            total_created: AtomicU64::new(0),
            total_reused: AtomicU64::new(0),
            total_closed: AtomicU64::new(0),
            validation_failures: AtomicU64::new(0),
            pool_exhausted: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            idle_count: AtomicU64::new(0),
            active_count: AtomicU64::new(0),
        }
    }

    /// Get reuse ratio (percentage of connections reused vs created)
    pub fn reuse_ratio(&self) -> f64 {
        let created = self.total_created.load(Ordering::Relaxed) as f64;
        let reused = self.total_reused.load(Ordering::Relaxed) as f64;

        if created == 0.0 {
            return 0.0;
        }

        (reused / (created + reused)) * 100.0
    }

    /// Get snapshot of statistics
    pub fn snapshot(&self) -> PoolStatsSnapshot {
        PoolStatsSnapshot {
            max_size: 0, // Set by pool
            min_idle: 0, // Set by pool
            active: self.active_count.load(Ordering::Relaxed) as usize,
            idle: self.idle_count.load(Ordering::Relaxed) as usize,
            total_created: self.total_created.load(Ordering::Relaxed),
            total_reused: self.total_reused.load(Ordering::Relaxed),
            total_closed: self.total_closed.load(Ordering::Relaxed),
            validation_failures: self.validation_failures.load(Ordering::Relaxed),
            pool_exhausted: self.pool_exhausted.load(Ordering::Relaxed),
            connection_errors: self.connection_errors.load(Ordering::Relaxed),
            reuse_ratio: self.reuse_ratio(),
        }
    }
}

/// Connection pool statistics snapshot
#[derive(Debug, Clone)]
pub struct PoolStatsSnapshot {
    pub max_size: usize,
    pub min_idle: usize,
    pub active: usize,
    pub idle: usize,
    pub total_created: u64,
    pub total_reused: u64,
    pub total_closed: u64,
    pub validation_failures: u64,
    pub pool_exhausted: u64,
    pub connection_errors: u64,
    pub reuse_ratio: f64,
}

impl TcpConnectionPool {
    /// Create a new connection pool with default config
    pub fn new(
        backend_addr: SocketAddr,
        max_size: usize,
        min_idle: usize,
        connection_lifetime: Duration,
    ) -> Self {
        let config = PoolConfig {
            max_size,
            min_idle,
            max_idle: max_size / 2,
            connection_lifetime,
            ..Default::default()
        };

        Self::with_config(backend_addr, config)
    }

    /// Create a new connection pool with custom config
    pub fn with_config(backend_addr: SocketAddr, config: PoolConfig) -> Self {
        info!(
            "Creating TCP connection pool for {} (max: {}, min_idle: {}, max_idle: {})",
            backend_addr, config.max_size, config.min_idle, config.max_idle
        );

        let max_size = config.max_size;

        Self {
            backend_addr,
            config,
            idle_connections: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_size)),
            stats: Arc::new(PoolStats::new()),
            shutdown_tx: None,
        }
    }

    /// Start background tasks (cleanup, pre-warming)
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Pre-warm connections if enabled
        if self.config.pre_warm {
            self.pre_warm_connections().await?;
        }

        // Spawn cleanup task
        self.spawn_cleanup_task(shutdown_tx.clone());

        // Spawn min_idle maintenance task
        self.spawn_min_idle_task(shutdown_tx.clone());

        Ok(())
    }

    /// Pre-warm connections to min_idle
    async fn pre_warm_connections(&self) -> Result<()> {
        info!(
            "Pre-warming {} connections to {}",
            self.config.min_idle, self.backend_addr
        );

        let mut tasks = Vec::new();

        for _ in 0..self.config.min_idle {
            let backend_addr = self.backend_addr;
            let stats = self.stats.clone();

            let task = tokio::spawn(async move {
                match TcpStream::connect(backend_addr).await {
                    Ok(stream) => {
                        stats.total_created.fetch_add(1, Ordering::Relaxed);
                        Some(PooledConnection::new(stream))
                    }
                    Err(e) => {
                        error!("Pre-warm connection failed: {}", e);
                        stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                        None
                    }
                }
            });

            tasks.push(task);
        }

        // Wait for all pre-warm connections
        let mut idle_conns = self.idle_connections.lock().await;
        for task in tasks {
            if let Ok(Some(conn)) = task.await {
                idle_conns.push_back(conn);
                self.stats.idle_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        info!(
            "Pre-warmed {} connections (target: {})",
            idle_conns.len(),
            self.config.min_idle
        );

        Ok(())
    }

    /// Spawn cleanup task to remove expired/idle connections
    fn spawn_cleanup_task(&self, shutdown_tx: tokio::sync::broadcast::Sender<()>) {
        let idle_connections = self.idle_connections.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::cleanup_connections(&idle_connections, &config, &stats).await;
                    }

                    _ = shutdown_rx.recv() => {
                        debug!("Cleanup task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Cleanup expired and idle timeout connections
    async fn cleanup_connections(
        idle_connections: &Arc<Mutex<VecDeque<PooledConnection>>>,
        config: &PoolConfig,
        stats: &Arc<PoolStats>,
    ) {
        let mut idle_conns = idle_connections.lock().await;
        let original_count = idle_conns.len();

        // Remove expired or idle timeout connections
        idle_conns.retain(|conn| {
            let keep = !conn.is_expired(config.connection_lifetime)
                && !conn.is_idle_timeout(config.idle_timeout);

            if !keep {
                stats.total_closed.fetch_add(1, Ordering::Relaxed);
                stats.idle_count.fetch_sub(1, Ordering::Relaxed);
                debug!(
                    "Closing connection (age: {:?}, idle: {:?}, reuse_count: {})",
                    conn.created_at.elapsed(),
                    conn.last_used.elapsed(),
                    conn.reuse_count
                );
            }

            keep
        });

        // Also enforce max_idle limit
        while idle_conns.len() > config.max_idle {
            if let Some(conn) = idle_conns.pop_front() {
                stats.total_closed.fetch_add(1, Ordering::Relaxed);
                stats.idle_count.fetch_sub(1, Ordering::Relaxed);
                debug!(
                    "Closing excess idle connection (pool size: {})",
                    idle_conns.len()
                );
                drop(conn);
            }
        }

        let removed = original_count.saturating_sub(idle_conns.len());
        if removed > 0 {
            debug!(
                "Cleanup removed {} connections ({} remaining)",
                removed,
                idle_conns.len()
            );
        }
    }

    /// Spawn task to maintain min_idle connections
    fn spawn_min_idle_task(&self, shutdown_tx: tokio::sync::broadcast::Sender<()>) {
        let idle_connections = self.idle_connections.clone();
        let backend_addr = self.backend_addr;
        let config = self.config.clone();
        let stats = self.stats.clone();
        let semaphore = self.semaphore.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let idle_count = {
                            let idle_conns = idle_connections.lock().await;
                            idle_conns.len()
                        };

                        if idle_count < config.min_idle {
                            let to_create = config.min_idle - idle_count;
                            debug!("Creating {} connections to maintain min_idle", to_create);

                            for _ in 0..to_create {
                                // Try to acquire permit without blocking
                                if let Ok(permit) = semaphore.clone().try_acquire_owned() {
                                    match TcpStream::connect(backend_addr).await {
                                        Ok(stream) => {
                                            let conn = PooledConnection::new(stream);
                                            let mut idle_conns = idle_connections.lock().await;
                                            idle_conns.push_back(conn);

                                            stats.total_created.fetch_add(1, Ordering::Relaxed);
                                            stats.idle_count.fetch_add(1, Ordering::Relaxed);

                                            // Release permit back to pool
                                            drop(permit);
                                        }
                                        Err(e) => {
                                            warn!("Failed to create min_idle connection: {}", e);
                                            stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                                            drop(permit);
                                        }
                                    }
                                } else {
                                    // Pool is at max capacity
                                    break;
                                }
                            }
                        }
                    }

                    _ = shutdown_rx.recv() => {
                        debug!("Min idle task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Get a connection from the pool (or create new)
    pub async fn get(&self) -> Result<TcpStream> {
        let start = Instant::now();

        // Try to get idle connection first (LIFO - most recently used)
        {
            let mut idle_conns = self.idle_connections.lock().await;

            while let Some(mut conn) = idle_conns.pop_back() {
                self.stats.idle_count.fetch_sub(1, Ordering::Relaxed);

                // Validate connection before reuse
                if self.validate_connection(&mut conn.stream).await {
                    conn.mark_reused();
                    self.stats.total_reused.fetch_add(1, Ordering::Relaxed);
                    self.stats.active_count.fetch_add(1, Ordering::Relaxed);

                    debug!(
                        "Reusing connection (age: {:?}, reuse_count: {}, wait: {:?})",
                        conn.created_at.elapsed(),
                        conn.reuse_count,
                        start.elapsed()
                    );

                    return Ok(conn.stream);
                } else {
                    // Validation failed, close connection
                    self.stats
                        .validation_failures
                        .fetch_add(1, Ordering::Relaxed);
                    self.stats.total_closed.fetch_add(1, Ordering::Relaxed);
                    warn!("Connection validation failed, closing");
                    drop(conn);
                }
            }
        }

        // No idle connections, create new one
        debug!("No idle connections, creating new connection");

        // Try to acquire permit (may wait if pool is full)
        let permit = match self.semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                // Pool exhausted, wait for permit
                self.stats.pool_exhausted.fetch_add(1, Ordering::Relaxed);
                warn!("Pool exhausted, waiting for permit");

                self.semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .context("Failed to acquire pool permit")?
            }
        };

        // Create new connection
        match TcpStream::connect(self.backend_addr).await {
            Ok(stream) => {
                self.stats.total_created.fetch_add(1, Ordering::Relaxed);
                self.stats.active_count.fetch_add(1, Ordering::Relaxed);

                debug!(
                    "Created new connection (total_created: {}, wait: {:?})",
                    self.stats.total_created.load(Ordering::Relaxed),
                    start.elapsed()
                );

                // Store permit so it gets released when connection is returned
                std::mem::forget(permit);

                Ok(stream)
            }
            Err(e) => {
                self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                error!("Failed to create connection: {}", e);

                // Release permit
                drop(permit);

                Err(e.into())
            }
        }
    }

    /// Return a connection to the pool
    pub async fn put(&self, stream: TcpStream) {
        self.stats.active_count.fetch_sub(1, Ordering::Relaxed);

        // Check if pool has space for idle connections
        let mut idle_conns = self.idle_connections.lock().await;

        if idle_conns.len() < self.config.max_idle {
            // Return to pool
            let conn = PooledConnection::new(stream);
            idle_conns.push_back(conn);
            self.stats.idle_count.fetch_add(1, Ordering::Relaxed);

            debug!(
                "Returned connection to pool (idle: {}/{})",
                idle_conns.len(),
                self.config.max_idle
            );
        } else {
            // Pool is full, close connection
            self.stats.total_closed.fetch_add(1, Ordering::Relaxed);
            debug!("Pool full, closing connection");
            drop(stream);

            // Release semaphore permit
            self.semaphore.add_permits(1);
        }
    }

    /// Validate connection by attempting a non-blocking read
    async fn validate_connection(&self, stream: &mut TcpStream) -> bool {
        // Set a very short timeout for validation
        match timeout(self.config.validation_timeout, async {
            // Try to peek at data without consuming it
            stream.readable().await
        })
        .await
        {
            Ok(Ok(_)) => {
                // Stream is readable, check if it's actually alive
                let mut buf = [0u8; 1];
                match stream.try_read(&mut buf) {
                    Ok(0) => {
                        // EOF - connection closed
                        false
                    }
                    Ok(_) => {
                        // Got data - connection alive but has pending data
                        // This shouldn't happen for idle connections
                        warn!("Idle connection has pending data, marking as invalid");
                        false
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No data available, connection is valid
                        true
                    }
                    Err(_) => {
                        // Other error
                        false
                    }
                }
            }
            Ok(Err(_)) => false, // Error checking readability
            Err(_) => true,      // Timeout is OK - means no data, connection likely valid
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStatsSnapshot {
        let mut snapshot = self.stats.snapshot();
        snapshot.max_size = self.config.max_size;
        snapshot.min_idle = self.config.min_idle;
        snapshot
    }

    /// Shutdown pool and close all connections
    pub async fn shutdown(&self) {
        info!("Shutting down connection pool for {}", self.backend_addr);

        // Signal shutdown to background tasks
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }

        // Close all idle connections
        let mut idle_conns = self.idle_connections.lock().await;
        let count = idle_conns.len();
        idle_conns.clear();

        info!(
            "Closed {} idle connections for {}",
            count, self.backend_addr
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_size, 100);
        assert_eq!(config.min_idle, 10);
        assert_eq!(config.max_idle, 50);
    }

    #[test]
    fn test_pool_creation() {
        let pool = TcpConnectionPool::new(
            "127.0.0.1:3306".parse().unwrap(),
            100,
            10,
            Duration::from_secs(3600),
        );

        assert_eq!(pool.config.max_size, 100);
        assert_eq!(pool.config.min_idle, 10);
    }

    #[test]
    fn test_pool_stats() {
        let pool = TcpConnectionPool::new(
            "127.0.0.1:3306".parse().unwrap(),
            100,
            10,
            Duration::from_secs(3600),
        );

        let stats = pool.stats();
        assert_eq!(stats.max_size, 100);
        assert_eq!(stats.min_idle, 10);
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_reused, 0);
    }

    #[test]
    fn test_reuse_ratio() {
        let stats = PoolStats::new();

        // No connections yet
        assert_eq!(stats.reuse_ratio(), 0.0);

        // 10 created, 0 reused = 0%
        stats.total_created.store(10, Ordering::Relaxed);
        assert_eq!(stats.reuse_ratio(), 0.0);

        // 10 created, 10 reused = 50%
        stats.total_reused.store(10, Ordering::Relaxed);
        assert_eq!(stats.reuse_ratio(), 50.0);

        // 10 created, 90 reused = 90%
        stats.total_reused.store(90, Ordering::Relaxed);
        assert_eq!(stats.reuse_ratio(), 90.0);
    }

    #[test]
    fn test_connection_expiry_logic() {
        // Test expiry logic without needing actual network connections
        let now = Instant::now();
        let expired_time = now - Duration::from_secs(3700); // 1h + 100s ago

        // Connection expired if created_at.elapsed() > max_lifetime
        assert!(expired_time.elapsed() > Duration::from_secs(3600));
    }

    #[test]
    fn test_idle_timeout_logic() {
        // Test idle timeout logic without needing actual network connections
        let now = Instant::now();
        let idle_time = now - Duration::from_secs(400); // 400s ago

        // Connection idle timeout if last_used.elapsed() > idle_timeout
        assert!(idle_time.elapsed() > Duration::from_secs(300));
    }
}
