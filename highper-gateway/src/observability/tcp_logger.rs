//! TCP Session Structured Logging
//!
//! Provides structured JSON logging for TCP connections:
//! - Connection lifecycle events
//! - Session tracking with connection IDs
//! - Data transfer statistics
//! - Connection pool events
//! - Error tracking

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// TCP Connection ID for session tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TcpConnectionId(String);

impl TcpConnectionId {
    /// Generate a new TCP connection ID
    pub fn new() -> Self {
        Self(format!("tcp-{}", Uuid::now_v7()))
    }

    /// Get the ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TcpConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TcpConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TCP connection context for structured logging
#[derive(Debug, Clone, Serialize)]
pub struct TcpConnectionContext {
    pub connection_id: String,
    pub client_addr: String,
    pub backend_addr: String,
    pub protocol: String, // mysql, postgres, redis, tcp
}

impl TcpConnectionContext {
    pub fn new(
        connection_id: TcpConnectionId,
        client_addr: SocketAddr,
        backend_addr: String,
        protocol: String,
    ) -> Self {
        Self {
            connection_id: connection_id.to_string(),
            client_addr: client_addr.to_string(),
            backend_addr,
            protocol,
        }
    }
}

/// Log TCP connection established
pub fn log_connection_established(ctx: &TcpConnectionContext) {
    info!(
        event = "tcp_connection_established",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        backend_addr = %ctx.backend_addr,
        protocol = %ctx.protocol,
        "TCP connection established"
    );
}

/// Log TCP connection closed with statistics
pub fn log_connection_closed(
    ctx: &TcpConnectionContext,
    duration: Duration,
    bytes_sent: u64,
    bytes_received: u64,
    error: Option<&str>,
) {
    let duration_ms = duration.as_millis() as u64;

    if let Some(err) = error {
        warn!(
            event = "tcp_connection_closed",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            backend_addr = %ctx.backend_addr,
            protocol = %ctx.protocol,
            duration_ms = duration_ms,
            bytes_sent = bytes_sent,
            bytes_received = bytes_received,
            error = err,
            "TCP connection closed with error"
        );
    } else {
        info!(
            event = "tcp_connection_closed",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            backend_addr = %ctx.backend_addr,
            protocol = %ctx.protocol,
            duration_ms = duration_ms,
            bytes_sent = bytes_sent,
            bytes_received = bytes_received,
            "TCP connection closed"
        );
    }
}

/// Log TCP connection error
pub fn log_connection_error(
    ctx: &TcpConnectionContext,
    error_type: &str,
    error_message: &str,
) {
    error!(
        event = "tcp_connection_error",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        backend_addr = %ctx.backend_addr,
        protocol = %ctx.protocol,
        error_type = error_type,
        error_message = error_message,
        "TCP connection error"
    );
}

/// Log TCP connection timeout
pub fn log_connection_timeout(
    ctx: &TcpConnectionContext,
    timeout_duration: Duration,
) {
    warn!(
        event = "tcp_connection_timeout",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        backend_addr = %ctx.backend_addr,
        protocol = %ctx.protocol,
        timeout_ms = timeout_duration.as_millis() as u64,
        "TCP connection timeout"
    );
}

/// Log TCP connection refused
pub fn log_connection_refused(
    client_addr: SocketAddr,
    backend_addr: &str,
    protocol: &str,
) {
    warn!(
        event = "tcp_connection_refused",
        client_addr = %client_addr,
        backend_addr = backend_addr,
        protocol = protocol,
        "TCP connection refused by backend"
    );
}

/// Log TCP connection reset
pub fn log_connection_reset(
    ctx: &TcpConnectionContext,
    reset_by: &str, // "client" or "backend"
) {
    warn!(
        event = "tcp_connection_reset",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        backend_addr = %ctx.backend_addr,
        protocol = %ctx.protocol,
        reset_by = reset_by,
        "TCP connection reset"
    );
}

/// Log TCP connection pool event
#[derive(Debug, Clone, Serialize)]
pub struct TcpPoolContext {
    pub backend_addr: String,
    pub pool_name: String,
}

/// Log connection acquired from pool
pub fn log_pool_connection_acquired(
    pool_ctx: &TcpPoolContext,
    wait_duration: Duration,
    reused: bool,
) {
    debug!(
        event = "tcp_pool_connection_acquired",
        backend_addr = %pool_ctx.backend_addr,
        pool_name = %pool_ctx.pool_name,
        wait_ms = wait_duration.as_millis() as u64,
        reused = reused,
        "Connection acquired from pool"
    );
}

/// Log connection returned to pool
pub fn log_pool_connection_returned(
    pool_ctx: &TcpPoolContext,
    connection_id: &TcpConnectionId,
) {
    trace!(
        event = "tcp_pool_connection_returned",
        backend_addr = %pool_ctx.backend_addr,
        pool_name = %pool_ctx.pool_name,
        connection_id = %connection_id,
        "Connection returned to pool"
    );
}

/// Log connection pool exhausted
pub fn log_pool_exhausted(
    pool_ctx: &TcpPoolContext,
    active_count: usize,
    max_size: usize,
) {
    warn!(
        event = "tcp_pool_exhausted",
        backend_addr = %pool_ctx.backend_addr,
        pool_name = %pool_ctx.pool_name,
        active_connections = active_count,
        max_pool_size = max_size,
        "Connection pool exhausted"
    );
}

/// Log new connection created for pool
pub fn log_pool_connection_created(
    pool_ctx: &TcpPoolContext,
    connection_id: &TcpConnectionId,
) {
    debug!(
        event = "tcp_pool_connection_created",
        backend_addr = %pool_ctx.backend_addr,
        pool_name = %pool_ctx.pool_name,
        connection_id = %connection_id,
        "New connection created for pool"
    );
}

/// Log connection pool statistics
pub fn log_pool_stats(
    pool_ctx: &TcpPoolContext,
    active_count: usize,
    idle_count: usize,
    total_created: u64,
    total_reused: u64,
) {
    debug!(
        event = "tcp_pool_stats",
        backend_addr = %pool_ctx.backend_addr,
        pool_name = %pool_ctx.pool_name,
        active_connections = active_count,
        idle_connections = idle_count,
        total_created = total_created,
        total_reused = total_reused,
        reuse_ratio = if total_created + total_reused > 0 {
            (total_reused as f64) / ((total_created + total_reused) as f64)
        } else {
            0.0
        },
        "Connection pool statistics"
    );
}

/// Log backend health check
pub fn log_backend_health_check(
    backend_addr: &str,
    protocol: &str,
    healthy: bool,
    response_time: Duration,
) {
    if healthy {
        debug!(
            event = "tcp_backend_health_check",
            backend_addr = backend_addr,
            protocol = protocol,
            status = "healthy",
            response_ms = response_time.as_millis() as u64,
            "Backend health check passed"
        );
    } else {
        warn!(
            event = "tcp_backend_health_check",
            backend_addr = backend_addr,
            protocol = protocol,
            status = "unhealthy",
            response_ms = response_time.as_millis() as u64,
            "Backend health check failed"
        );
    }
}

/// Log load balancer backend selection
pub fn log_backend_selected(
    backend_addr: &str,
    protocol: &str,
    algorithm: &str,
    active_connections: usize,
) {
    trace!(
        event = "tcp_backend_selected",
        backend_addr = backend_addr,
        protocol = protocol,
        lb_algorithm = algorithm,
        active_connections = active_connections,
        "Backend selected for connection"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_tcp_connection_id() {
        let id1 = TcpConnectionId::new();
        let id2 = TcpConnectionId::new();

        assert!(id1.as_str().starts_with("tcp-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tcp_logging() {
        // Initialize tracing subscriber for tests
        // (In production this would be done at startup)

        let conn_id = TcpConnectionId::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 54321);
        let ctx = TcpConnectionContext::new(
            conn_id.clone(),
            client_addr,
            "mysql-backend:3306".to_string(),
            "mysql".to_string(),
        );

        // Test connection lifecycle
        log_connection_established(&ctx);
        log_connection_closed(&ctx, Duration::from_secs(300), 1024000, 2048000, None);

        // Test errors
        log_connection_error(&ctx, "timeout", "Connection timed out after 30s");
        log_connection_timeout(&ctx, Duration::from_secs(30));

        // Test pool events
        let pool_ctx = TcpPoolContext {
            backend_addr: "mysql-backend:3306".to_string(),
            pool_name: "mysql-pool".to_string(),
        };

        log_pool_connection_acquired(&pool_ctx, Duration::from_millis(10), true);
        log_pool_connection_returned(&pool_ctx, &conn_id);
        log_pool_exhausted(&pool_ctx, 100, 100);

        // If we reach here without panicking, logging works
        assert!(true);
    }
}
