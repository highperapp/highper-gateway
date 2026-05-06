//! Database Connection Pooling
//!
//! Specialized connection pooling for database load balancing with:
//! - TCP connection pooling for MySQL, PostgreSQL, Redis
//! - Connection pre-warming (maintain minimum idle connections)
//! - Connection validation (ping/health check before reuse)
//! - Lifecycle management (max lifetime, idle timeout)
//! - Metrics integration (pool stats, wait times, reuse ratio)
//! - Protocol-specific optimizations

use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Semaphore, Notify};
use tracing::{debug, info, warn, error};

use crate::observability::tcp_logger::TcpConnectionId;
use crate::observability::tcp_metrics;

/// Database protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseProtocol {
    MySQL,
    PostgreSQL,
    Redis,
    Generic,
}

impl std::fmt::Display for DatabaseProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseProtocol::MySQL => write!(f, "mysql"),
            DatabaseProtocol::PostgreSQL => write!(f, "postgres"),
            DatabaseProtocol::Redis => write!(f, "redis"),
            DatabaseProtocol::Generic => write!(f, "tcp"),
        }
    }
}

/// Database connection pool configuration
#[derive(Debug, Clone)]
pub struct DatabasePoolConfig {
    /// Maximum connections per backend
    pub max_connections_per_backend: usize,

    /// Minimum idle connections to maintain (pre-warming)
    pub min_idle_connections: usize,

    /// Maximum connection lifetime before forced recycling
    pub max_connection_lifetime: Option<Duration>,

    /// Maximum idle time before connection is closed
    pub max_idle_duration: Duration,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Enable connection validation before reuse
    pub validate_on_checkout: bool,

    /// Validation timeout
    pub validation_timeout: Duration,

    /// Enable TCP keep-alive
    pub keep_alive: bool,

    /// Keep-alive timeout
    pub keep_alive_timeout: Duration,

    /// Enable connection pre-warming
    pub prewarm_enabled: bool,

    /// Database protocol
    pub protocol: DatabaseProtocol,
}

impl Default for DatabasePoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_backend: 100,
            min_idle_connections: 0,
            max_connection_lifetime: Some(Duration::from_secs(3600)), // 1 hour
            max_idle_duration: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(5),
            validate_on_checkout: true,
            validation_timeout: Duration::from_secs(1),
            keep_alive: true,
            keep_alive_timeout: Duration::from_secs(60),
            prewarm_enabled: false,
            protocol: DatabaseProtocol::Generic,
        }
    }
}

/// Pooled database connection wrapper
struct PooledDatabaseConnection {
    stream: TcpStream,
    connection_id: TcpConnectionId,
    created_at: Instant,
    last_used: Instant,
    backend: String,
    protocol: DatabaseProtocol,
    times_reused: u32,
}

impl PooledDatabaseConnection {
    fn new(stream: TcpStream, backend: String, protocol: DatabaseProtocol) -> Self {
        let now = Instant::now();
        Self {
            stream,
            connection_id: TcpConnectionId::new(),
            created_at: now,
            last_used: now,
            backend,
            protocol,
            times_reused: 0,
        }
    }

    /// Check if connection has expired (idle timeout or max lifetime)
    fn is_expired(&self, config: &DatabasePoolConfig) -> bool {
        // Check idle timeout
        if self.last_used.elapsed() > config.max_idle_duration {
            return true;
        }

        // Check max lifetime
        if let Some(max_lifetime) = config.max_connection_lifetime {
            if self.created_at.elapsed() > max_lifetime {
                return true;
            }
        }

        false
    }

    fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.times_reused += 1;
    }

    /// Validate connection is still alive
    async fn validate(&mut self, timeout: Duration) -> bool {
        match self.protocol {
            DatabaseProtocol::MySQL => self.validate_mysql(timeout).await,
            DatabaseProtocol::PostgreSQL => self.validate_postgres(timeout).await,
            DatabaseProtocol::Redis => self.validate_redis(timeout).await,
            DatabaseProtocol::Generic => self.validate_tcp(timeout).await,
        }
    }

    /// Validate MySQL connection with a ping
    async fn validate_mysql(&mut self, timeout: Duration) -> bool {
        // MySQL ping command: COM_PING (0x0e)
        let ping = [0x01, 0x00, 0x00, 0x00, 0x0e];

        match tokio::time::timeout(timeout, async {
            self.stream.write_all(&ping).await?;
            let mut response = [0u8; 7];
            self.stream.read_exact(&mut response).await?;
            Ok::<_, std::io::Error>(())
        }).await {
            Ok(Ok(())) => true,
            _ => false,
        }
    }

    /// Validate PostgreSQL connection (B6 — Workstream 0.E).
    ///
    /// Issues a `SELECT 1` simple query and drains the response message
    /// stream until `ReadyForQuery` (`Z`). Returns `true` only when the
    /// final transaction status byte is `'I'` (idle) — connections that
    /// are still inside a transaction (`'T'`) or in a failed transaction
    /// (`'E'`) are unsafe to hand back to the pool and are rejected.
    ///
    /// Wire format (per
    /// <https://www.postgresql.org/docs/current/protocol-message-formats.html>):
    /// - Frontend `Query`: `Q` + length(big-endian u32, includes itself) + `SELECT 1\0`.
    /// - Backend response sequence: `RowDescription` (`T`) + `DataRow` (`D`)
    ///   + `CommandComplete` (`C`) + `ReadyForQuery` (`Z`). Every message has
    ///   a 1-byte type + 4-byte length header followed by `length - 4`
    ///   body bytes. We read each header, skip the body, and stop on `Z`.
    async fn validate_postgres(&mut self, timeout: Duration) -> bool {
        // `Q` + len=13 (4 + 9 bytes for "SELECT 1\0") + body. 14 bytes total.
        const SELECT_ONE_QUERY: &[u8] = b"Q\x00\x00\x00\x0dSELECT 1\x00";
        // Defensive cap on a single response-message body. Real responses
        // for `SELECT 1` are tiny (~30 bytes for RowDescription); a 1 MiB
        // ceiling rejects pathological / malformed responses without
        // refusing legitimate ones.
        const MAX_MESSAGE_BODY: u32 = 1 << 20;

        let result = tokio::time::timeout(timeout, async {
            self.stream.write_all(SELECT_ONE_QUERY).await?;

            loop {
                let mut header = [0u8; 5];
                self.stream.read_exact(&mut header).await?;
                let msg_type = header[0];
                let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
                if length < 4 || length > MAX_MESSAGE_BODY {
                    return Ok::<bool, std::io::Error>(false);
                }
                let body_len = (length - 4) as usize;
                // We don't need to inspect intermediate-message bodies;
                // just drain them to keep the stream byte-aligned for the
                // next pool user.
                let mut body = vec![0u8; body_len];
                if body_len > 0 {
                    self.stream.read_exact(&mut body).await?;
                }
                if msg_type == b'Z' {
                    // ReadyForQuery: body is exactly 1 byte — the
                    // transaction status. Only `'I'` (idle) is safe to
                    // return to the pool.
                    return Ok(body.first().copied() == Some(b'I'));
                }
                // Other messages (T/D/C/N/...) — keep reading until Z.
            }
        })
        .await;

        matches!(result, Ok(Ok(true)))
    }

    /// Validate Redis connection with PING command
    async fn validate_redis(&mut self, timeout: Duration) -> bool {
        let ping = b"*1\r\n$4\r\nPING\r\n";

        match tokio::time::timeout(timeout, async {
            self.stream.write_all(ping).await?;
            let mut response = [0u8; 7]; // +PONG\r\n
            self.stream.read_exact(&mut response).await?;
            Ok::<_, std::io::Error>(response.starts_with(b"+PONG"))
        }).await {
            Ok(Ok(true)) => true,
            _ => false,
        }
    }

    /// Validate generic TCP connection (just check if readable)
    async fn validate_tcp(&mut self, timeout: Duration) -> bool {
        match tokio::time::timeout(timeout, async {
            let mut buf = [0u8; 1];
            self.stream.peek(&mut buf).await
        }).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }
}

/// Per-backend database connection pool
struct DatabaseBackendPool {
    /// Available idle connections
    idle_connections: parking_lot::Mutex<Vec<PooledDatabaseConnection>>,

    /// Semaphore to limit total connections
    connection_limiter: Arc<Semaphore>,

    /// Notify for waiting tasks when connection becomes available
    connection_available: Arc<Notify>,

    /// Pool configuration
    config: DatabasePoolConfig,

    /// Backend identifier
    backend_name: String,

    /// Backend address
    backend_addr: SocketAddr,

    /// Statistics
    stats: PoolStatistics,
}

/// Pool statistics with atomic counters
struct PoolStatistics {
    total_created: AtomicU64,
    total_reused: AtomicU64,
    total_wait_time_ms: AtomicU64,
    total_validation_failures: AtomicU64,
    total_lifetime_expired: AtomicU64,
    total_idle_expired: AtomicU64,
}

impl PoolStatistics {
    fn new() -> Self {
        Self {
            total_created: AtomicU64::new(0),
            total_reused: AtomicU64::new(0),
            total_wait_time_ms: AtomicU64::new(0),
            total_validation_failures: AtomicU64::new(0),
            total_lifetime_expired: AtomicU64::new(0),
            total_idle_expired: AtomicU64::new(0),
        }
    }
}

impl DatabaseBackendPool {
    fn new(backend_name: String, backend_addr: SocketAddr, config: DatabasePoolConfig) -> Self {
        Self {
            idle_connections: parking_lot::Mutex::new(Vec::new()),
            connection_limiter: Arc::new(Semaphore::new(config.max_connections_per_backend)),
            connection_available: Arc::new(Notify::new()),
            config,
            backend_name,
            backend_addr,
            stats: PoolStatistics::new(),
        }
    }

    /// Try to get an idle connection from the pool
    async fn try_get_idle(&self) -> Option<TcpStream> {
        let mut idle = self.idle_connections.lock();

        while let Some(mut conn) = idle.pop() {
            // Check if connection expired
            if conn.is_expired(&self.config) {
                if conn.last_used.elapsed() > self.config.max_idle_duration {
                    self.stats.total_idle_expired.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.stats.total_lifetime_expired.fetch_add(1, Ordering::Relaxed);
                }
                drop(conn);
                continue;
            }

            // Validate connection if enabled
            if self.config.validate_on_checkout {
                if !conn.validate(self.config.validation_timeout).await {
                    self.stats.total_validation_failures.fetch_add(1, Ordering::Relaxed);
                    warn!(
                        backend = %self.backend_name,
                        connection_id = %conn.connection_id,
                        "Connection validation failed, discarding"
                    );
                    drop(conn);
                    continue;
                }
            }

            // Connection is valid, use it
            conn.mark_used();
            self.stats.total_reused.fetch_add(1, Ordering::Relaxed);

            debug!(
                backend = %self.backend_name,
                connection_id = %conn.connection_id,
                times_reused = conn.times_reused,
                pool_size = idle.len(),
                "Reusing pooled database connection"
            );

            // Record metrics
            tcp_metrics::record_pool_connection_reused(&self.backend_name);

            return Some(conn.stream);
        }

        None
    }

    /// Create a new connection to the backend
    async fn create_connection(&self) -> Result<TcpStream, std::io::Error> {
        let wait_start = Instant::now();

        // Acquire permit from semaphore (limits total connections)
        let _permit = self
            .connection_limiter
            .acquire()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let wait_duration = wait_start.elapsed();
        self.stats.total_wait_time_ms.fetch_add(
            wait_duration.as_millis() as u64,
            Ordering::Relaxed
        );

        // Record wait time metrics
        tcp_metrics::record_pool_wait(wait_duration);

        // Create new connection with timeout
        let stream = tokio::time::timeout(
            self.config.connect_timeout,
            TcpStream::connect(self.backend_addr)
        )
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timeout"))?
        .map_err(|e| {
            error!(
                backend = %self.backend_name,
                addr = %self.backend_addr,
                error = %e,
                "Failed to create database connection"
            );
            e
        })?;

        // Configure TCP keep-alive if enabled
        if self.config.keep_alive {
            let sock_ref = socket2::SockRef::from(&stream);
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(self.config.keep_alive_timeout);
            sock_ref.set_tcp_keepalive(&keepalive)?;
        }

        self.stats.total_created.fetch_add(1, Ordering::Relaxed);

        info!(
            backend = %self.backend_name,
            addr = %self.backend_addr,
            protocol = %self.config.protocol,
            "Created new database connection"
        );

        // Record metrics
        tcp_metrics::record_pool_connection_created(&self.backend_name);

        Ok(stream)
    }

    /// Return a connection to the pool
    fn return_connection(&self, stream: TcpStream) {
        let mut idle = self.idle_connections.lock();

        // Check pool capacity
        if idle.len() < self.config.max_connections_per_backend {
            let conn = PooledDatabaseConnection::new(
                stream,
                self.backend_name.clone(),
                self.config.protocol
            );
            idle.push(conn);

            debug!(
                backend = %self.backend_name,
                pool_size = idle.len(),
                "Returned connection to database pool"
            );

            // Notify waiting tasks
            self.connection_available.notify_one();
        } else {
            debug!(
                backend = %self.backend_name,
                "Pool full, dropping connection"
            );
            drop(stream);
        }
    }

    /// Cleanup expired idle connections
    fn cleanup_idle(&self) {
        let mut idle = self.idle_connections.lock();
        let before_count = idle.len();

        let config = &self.config;
        let stats = &self.stats;
        idle.retain(|conn| {
            if conn.is_expired(config) {
                if conn.last_used.elapsed() > config.max_idle_duration {
                    stats.total_idle_expired.fetch_add(1, Ordering::Relaxed);
                } else {
                    stats.total_lifetime_expired.fetch_add(1, Ordering::Relaxed);
                }
                false
            } else {
                true
            }
        });

        let removed = before_count - idle.len();
        if removed > 0 {
            debug!(
                backend = %self.backend_name,
                removed = removed,
                remaining = idle.len(),
                "Cleaned up expired database connections"
            );
        }
    }

    /// Pre-warm the pool by creating minimum idle connections
    async fn prewarm(&self) -> Result<(), std::io::Error> {
        let current_idle = self.idle_connections.lock().len();

        if current_idle >= self.config.min_idle_connections {
            return Ok(());
        }

        let needed = self.config.min_idle_connections - current_idle;

        info!(
            backend = %self.backend_name,
            current = current_idle,
            target = self.config.min_idle_connections,
            creating = needed,
            "Pre-warming database connection pool"
        );

        for _ in 0..needed {
            match self.create_connection().await {
                Ok(stream) => {
                    self.return_connection(stream);
                }
                Err(e) => {
                    warn!(
                        backend = %self.backend_name,
                        error = %e,
                        "Failed to pre-warm connection"
                    );
                    // Continue with other connections even if one fails
                }
            }
        }

        Ok(())
    }

    /// Get pool statistics
    fn get_stats(&self) -> DatabasePoolStats {
        let idle = self.idle_connections.lock();
        DatabasePoolStats {
            idle_count: idle.len(),
            max_connections: self.config.max_connections_per_backend,
            min_connections: self.config.min_idle_connections,
            total_created: self.stats.total_created.load(Ordering::Relaxed),
            total_reused: self.stats.total_reused.load(Ordering::Relaxed),
            total_wait_time_ms: self.stats.total_wait_time_ms.load(Ordering::Relaxed),
            total_validation_failures: self.stats.total_validation_failures.load(Ordering::Relaxed),
            total_lifetime_expired: self.stats.total_lifetime_expired.load(Ordering::Relaxed),
            total_idle_expired: self.stats.total_idle_expired.load(Ordering::Relaxed),
            reuse_ratio: {
                let created = self.stats.total_created.load(Ordering::Relaxed);
                let reused = self.stats.total_reused.load(Ordering::Relaxed);
                if created + reused > 0 {
                    reused as f64 / (created + reused) as f64
                } else {
                    0.0
                }
            },
        }
    }
}

/// Database connection pool statistics
#[derive(Debug, Clone)]
pub struct DatabasePoolStats {
    pub idle_count: usize,
    pub max_connections: usize,
    pub min_connections: usize,
    pub total_created: u64,
    pub total_reused: u64,
    pub total_wait_time_ms: u64,
    pub total_validation_failures: u64,
    pub total_lifetime_expired: u64,
    pub total_idle_expired: u64,
    pub reuse_ratio: f64,
}

/// Global database connection pool manager
pub struct DatabaseConnectionPoolManager {
    /// Pools per backend
    pools: DashMap<String, Arc<DatabaseBackendPool>>,

    /// Default pool configuration
    default_config: DatabasePoolConfig,
}

impl DatabaseConnectionPoolManager {
    /// Create a new database connection pool manager
    pub fn new(default_config: DatabasePoolConfig) -> Self {
        Self {
            pools: DashMap::new(),
            default_config,
        }
    }

    /// Get or create pool for backend
    fn get_or_create_pool(
        &self,
        backend: &str,
        addr: SocketAddr,
        config: Option<DatabasePoolConfig>,
    ) -> Arc<DatabaseBackendPool> {
        self.pools
            .entry(backend.to_string())
            .or_insert_with(|| {
                let pool_config = config.unwrap_or_else(|| self.default_config.clone());
                debug!(
                    backend = %backend,
                    protocol = %pool_config.protocol,
                    "Creating new database connection pool"
                );
                Arc::new(DatabaseBackendPool::new(
                    backend.to_string(),
                    addr,
                    pool_config
                ))
            })
            .clone()
    }

    /// Get a connection for a backend
    pub async fn get_connection(
        &self,
        backend: &str,
        addr: SocketAddr,
        config: Option<DatabasePoolConfig>,
    ) -> Result<TcpStream, std::io::Error> {
        let pool = self.get_or_create_pool(backend, addr, config);

        // Try to get an idle connection first
        if let Some(stream) = pool.try_get_idle().await {
            return Ok(stream);
        }

        // No idle connection available, create new one
        pool.create_connection().await
    }

    /// Return a connection to the pool for reuse
    pub fn return_connection(&self, backend: &str, stream: TcpStream) {
        if let Some(pool) = self.pools.get(backend) {
            pool.return_connection(stream);
        }
    }

    /// Cleanup idle connections across all pools
    pub fn cleanup_idle_connections(&self) {
        for pool in self.pools.iter() {
            pool.value().cleanup_idle();
        }
    }

    /// Pre-warm all pools
    pub async fn prewarm_all(&self) -> Result<(), std::io::Error> {
        for pool in self.pools.iter() {
            pool.value().prewarm().await?;
        }
        Ok(())
    }

    /// Pre-warm a specific pool
    pub async fn prewarm_pool(&self, backend: &str) -> Result<(), std::io::Error> {
        if let Some(pool) = self.pools.get(backend) {
            pool.prewarm().await?;
        }
        Ok(())
    }

    /// Get statistics for a backend pool
    pub fn get_pool_stats(&self, backend: &str) -> Option<DatabasePoolStats> {
        self.pools.get(backend).map(|pool| pool.get_stats())
    }

    /// Get statistics for all pools
    pub fn get_all_stats(&self) -> Vec<(String, DatabasePoolStats)> {
        self.pools
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().get_stats()))
            .collect()
    }

    /// Clear all pools (for shutdown or testing)
    pub fn clear_all(&self) {
        self.pools.clear();
    }
}

impl Default for DatabaseConnectionPoolManager {
    fn default() -> Self {
        Self::new(DatabasePoolConfig::default())
    }
}

/// Spawn background task to cleanup idle connections
pub fn spawn_cleanup_task(
    pool_manager: Arc<DatabaseConnectionPoolManager>,
    interval: Duration,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            pool_manager.cleanup_idle_connections();
        }
    });
}

/// Spawn background task to maintain minimum idle connections
pub fn spawn_prewarm_task(
    pool_manager: Arc<DatabaseConnectionPoolManager>,
    interval: Duration,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            if let Err(e) = pool_manager.prewarm_all().await {
                warn!(error = %e, "Failed to pre-warm pools");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_pool_config_default() {
        let config = DatabasePoolConfig::default();
        assert_eq!(config.max_connections_per_backend, 100);
        assert_eq!(config.min_idle_connections, 0);
        assert!(config.validate_on_checkout);
        assert!(config.keep_alive);
    }

    #[test]
    fn test_database_protocol_display() {
        assert_eq!(DatabaseProtocol::MySQL.to_string(), "mysql");
        assert_eq!(DatabaseProtocol::PostgreSQL.to_string(), "postgres");
        assert_eq!(DatabaseProtocol::Redis.to_string(), "redis");
        assert_eq!(DatabaseProtocol::Generic.to_string(), "tcp");
    }

    #[test]
    fn test_pool_manager_creation() {
        let config = DatabasePoolConfig::default();
        let manager = DatabaseConnectionPoolManager::new(config);
        assert_eq!(manager.pools.len(), 0);
    }

    #[test]
    fn test_pool_stats_initial() {
        let stats = PoolStatistics::new();
        assert_eq!(stats.total_created.load(Ordering::Relaxed), 0);
        assert_eq!(stats.total_reused.load(Ordering::Relaxed), 0);
    }

    /// Spin up a one-shot fake PostgreSQL server that:
    /// 1. Reads (and discards) the client's `SELECT 1` query frame
    ///    (14 bytes: `Q\x00\x00\x00\x0dSELECT 1\0`).
    /// 2. Writes the supplied response bytes back.
    /// Returns the listener's bound address so the test can connect.
    async fn spawn_fake_postgres_server(response: Vec<u8>) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut query_buf = [0u8; 14];
                let _ = sock.read_exact(&mut query_buf).await;
                let _ = sock.write_all(&response).await;
                // Hold the socket open briefly so the client can read.
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
        addr
    }

    /// Build a synthetic PostgreSQL response stream for `SELECT 1` ending
    /// in `ReadyForQuery` with the given transaction-status byte.
    fn synth_select_1_response(ready_status: u8) -> Vec<u8> {
        // Each message: 1 type byte + 4 length bytes (big-endian, includes
        // the 4 length bytes) + `length - 4` body bytes.
        let mut out = Vec::new();
        // RowDescription `T` — minimal body: 2-byte field count + per-field
        // metadata. For test, we lie with an empty body of length 4 (length
        // field only). The validator drains by length without parsing.
        out.extend_from_slice(b"T\x00\x00\x00\x04");
        // DataRow `D` — empty body.
        out.extend_from_slice(b"D\x00\x00\x00\x04");
        // CommandComplete `C` — body "SELECT 1\0" (9 bytes), length = 13.
        out.extend_from_slice(b"C\x00\x00\x00\x0dSELECT 1\x00");
        // ReadyForQuery `Z` — 1 status byte, length = 5.
        out.extend_from_slice(&[b'Z', 0, 0, 0, 5, ready_status]);
        out
    }

    #[tokio::test]
    async fn test_validate_postgres_idle_returns_true() {
        let addr = spawn_fake_postgres_server(synth_select_1_response(b'I')).await;
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-pg".to_string(),
            DatabaseProtocol::PostgreSQL,
        );
        assert!(conn.validate(Duration::from_secs(1)).await);
    }

    #[tokio::test]
    async fn test_validate_postgres_failed_tx_returns_false() {
        let addr = spawn_fake_postgres_server(synth_select_1_response(b'E')).await;
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-pg".to_string(),
            DatabaseProtocol::PostgreSQL,
        );
        // Failed-tx state is unsafe to return to the pool.
        assert!(!conn.validate(Duration::from_secs(1)).await);
    }

    #[tokio::test]
    async fn test_validate_postgres_in_tx_returns_false() {
        let addr = spawn_fake_postgres_server(synth_select_1_response(b'T')).await;
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-pg".to_string(),
            DatabaseProtocol::PostgreSQL,
        );
        // A connection inside an open transaction is also unsafe to reuse.
        assert!(!conn.validate(Duration::from_secs(1)).await);
    }

    #[tokio::test]
    async fn test_validate_postgres_truncated_response_returns_false() {
        // Server writes only a partial response header — `read_exact`
        // returns UnexpectedEof. Validator must not block forever.
        let addr = spawn_fake_postgres_server(b"T\x00\x00".to_vec()).await;
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-pg".to_string(),
            DatabaseProtocol::PostgreSQL,
        );
        assert!(!conn.validate(Duration::from_secs(1)).await);
    }

    #[tokio::test]
    async fn test_validate_postgres_oversized_message_returns_false() {
        // Server claims a 100 MiB message body — must be rejected by the
        // 1 MiB cap without attempting the read.
        let mut response = Vec::new();
        response.extend_from_slice(b"T");
        response.extend_from_slice(&100_000_000_u32.to_be_bytes());
        let addr = spawn_fake_postgres_server(response).await;
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-pg".to_string(),
            DatabaseProtocol::PostgreSQL,
        );
        assert!(!conn.validate(Duration::from_secs(1)).await);
    }

    #[tokio::test]
    async fn test_connection_expiry() {
        let config = DatabasePoolConfig {
            max_idle_duration: Duration::from_millis(10),
            ..Default::default()
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stream = TcpStream::connect(addr).await.unwrap();

        let mut conn = PooledDatabaseConnection::new(
            stream,
            "test-backend".to_string(),
            DatabaseProtocol::MySQL
        );

        // Should not be expired immediately
        assert!(!conn.is_expired(&config));

        // Wait for idle timeout
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be expired now
        assert!(conn.is_expired(&config));
    }
}
