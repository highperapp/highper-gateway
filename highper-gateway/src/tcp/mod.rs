//! High-Performance TCP Proxy
//!
//! This module provides a production-grade TCP proxy for database load balancing
//! with HAProxy-level performance. Designed for MySQL, PostgreSQL, and Redis.
//!
//! ## Features
//! - Zero-copy bidirectional forwarding
//! - Sub-millisecond P99 latency overhead
//! - Protocol-aware health checks
//! - Connection pooling with >95% reuse ratio
//! - Load balancing (round-robin, least-conn, consistent hash)
//! - Session persistence (IP-hash, cookie-based for HTTP)
//! - Graceful shutdown with connection draining
//!
//! ## Performance Targets
//! - P50 overhead: < 0.1ms
//! - P95 overhead: < 0.3ms
//! - P99 overhead: < 0.5ms
//! - Throughput: > 1M connections/sec
//! - Connection reuse: > 95%

pub mod server;
pub mod proxy;
pub mod protocol;
pub mod health;
pub mod pool;
pub mod circuit_breaker;

pub use server::TcpProxyServer;
pub use proxy::TcpProxy;
pub use protocol::{Protocol, ProtocolDetector};
pub use health::TcpHealthChecker;
pub use pool::TcpConnectionPool;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// TCP proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    /// Bind address for TCP proxy
    pub bind: SocketAddr,

    /// Upstream configuration
    pub upstreams: Vec<TcpUpstream>,

    /// Load balancing algorithm
    #[serde(default = "default_lb_algorithm")]
    pub load_balancing: LoadBalancingAlgorithm,

    /// Connection timeout
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: Duration,

    /// Read timeout
    #[serde(default = "default_read_timeout")]
    pub read_timeout: Duration,

    /// Write timeout
    #[serde(default = "default_write_timeout")]
    pub write_timeout: Duration,

    /// Enable connection pooling
    #[serde(default = "default_true")]
    pub enable_pooling: bool,

    /// Maximum connections per backend
    #[serde(default = "default_max_conns")]
    pub max_connections_per_backend: usize,

    /// Minimum idle connections in pool
    #[serde(default = "default_min_idle")]
    pub min_idle_connections: usize,

    /// Maximum idle connections in pool
    #[serde(default = "default_max_idle")]
    pub max_idle_connections: usize,

    /// Connection lifetime
    #[serde(default = "default_conn_lifetime")]
    pub connection_lifetime: Duration,

    /// Enable health checks
    #[serde(default = "default_true")]
    pub enable_health_checks: bool,

    /// Health check interval
    #[serde(default = "default_health_interval")]
    pub health_check_interval: Duration,

    /// Health check timeout
    #[serde(default = "default_health_timeout")]
    pub health_check_timeout: Duration,

    /// Protocol (mysql, postgresql, redis, generic)
    #[serde(default)]
    pub protocol: Protocol,

    /// Enable SO_REUSEPORT for multi-threaded accept
    #[serde(default = "default_true")]
    pub reuseport: bool,

    /// TCP buffer size
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Enable TCP_NODELAY (disable Nagle's algorithm)
    #[serde(default = "default_true")]
    pub nodelay: bool,

    /// Enable SO_KEEPALIVE
    #[serde(default = "default_true")]
    pub keepalive: bool,

    /// Keepalive time (seconds)
    #[serde(default = "default_keepalive_time")]
    pub keepalive_time: u64,

    /// Maximum concurrent connections
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_connections: usize,
}

/// TCP upstream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpUpstream {
    /// Upstream name
    pub name: String,

    /// Backend addresses
    pub backends: Vec<TcpBackend>,

    /// Weight for weighted load balancing
    #[serde(default = "default_weight")]
    pub weight: u32,
}

/// TCP backend server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpBackend {
    /// Backend address
    pub addr: SocketAddr,

    /// Backend weight
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Maximum connections
    pub max_conns: Option<usize>,

    /// Health check configuration override
    pub health_check: Option<TcpHealthCheckConfig>,
}

/// TCP health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpHealthCheckConfig {
    /// Health check type
    pub check_type: HealthCheckType,

    /// Interval between checks
    #[serde(default = "default_health_interval")]
    pub interval: Duration,

    /// Timeout for health check
    #[serde(default = "default_health_timeout")]
    pub timeout: Duration,

    /// Healthy threshold (consecutive successes)
    #[serde(default = "default_threshold")]
    pub healthy_threshold: u32,

    /// Unhealthy threshold (consecutive failures)
    #[serde(default = "default_threshold")]
    pub unhealthy_threshold: u32,
}

/// Health check type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthCheckType {
    /// TCP connection check (just connect and disconnect)
    Tcp,

    /// MySQL protocol check (send ping)
    Mysql,

    /// PostgreSQL protocol check (send query)
    Postgresql,

    /// Redis protocol check (send PING)
    Redis,

    /// Custom command
    Custom,
}

/// Load balancing algorithm
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingAlgorithm {
    /// Round-robin
    RoundRobin,

    /// Least connections
    LeastConnections,

    /// Least response time (route to fastest backend)
    LeastResponseTime,

    /// Consistent hashing (IP-based)
    ConsistentHash,

    /// Source IP hash
    IpHash,

    /// Weighted round-robin
    WeightedRoundRobin,

    /// Random
    Random,
}

// Default value functions
fn default_lb_algorithm() -> LoadBalancingAlgorithm {
    LoadBalancingAlgorithm::RoundRobin
}

fn default_connect_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_read_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_write_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_true() -> bool {
    true
}

fn default_max_conns() -> usize {
    10000
}

fn default_min_idle() -> usize {
    10
}

fn default_max_idle() -> usize {
    100
}

fn default_conn_lifetime() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

fn default_health_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_health_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_threshold() -> u32 {
    2
}

fn default_buffer_size() -> usize {
    8192
}

fn default_keepalive_time() -> u64 {
    60
}

fn default_max_concurrent() -> usize {
    100000
}

fn default_weight() -> u32 {
    1
}

/// TCP proxy statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TcpStats {
    /// Total connections accepted
    pub total_connections: u64,

    /// Active connections
    pub active_connections: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Connection errors
    pub connection_errors: u64,

    /// Backend connection failures
    pub backend_failures: u64,

    /// Total requests proxied
    pub requests_proxied: u64,

    /// Average connection duration (ms)
    pub avg_connection_duration_ms: f64,

    /// P50 latency (ms)
    pub p50_latency_ms: f64,

    /// P95 latency (ms)
    pub p95_latency_ms: f64,

    /// P99 latency (ms)
    pub p99_latency_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_config_defaults() {
        let config = TcpConfig {
            bind: "127.0.0.1:3306".parse().unwrap(),
            upstreams: vec![],
            load_balancing: default_lb_algorithm(),
            connect_timeout: default_connect_timeout(),
            read_timeout: default_read_timeout(),
            write_timeout: default_write_timeout(),
            enable_pooling: default_true(),
            max_connections_per_backend: default_max_conns(),
            min_idle_connections: default_min_idle(),
            max_idle_connections: default_max_idle(),
            connection_lifetime: default_conn_lifetime(),
            enable_health_checks: default_true(),
            health_check_interval: default_health_interval(),
            health_check_timeout: default_health_timeout(),
            protocol: Protocol::Generic,
            reuseport: default_true(),
            buffer_size: default_buffer_size(),
            nodelay: default_true(),
            keepalive: default_true(),
            keepalive_time: default_keepalive_time(),
            max_concurrent_connections: default_max_concurrent(),
        };

        assert_eq!(config.load_balancing, LoadBalancingAlgorithm::RoundRobin);
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert!(config.enable_pooling);
        assert_eq!(config.max_connections_per_backend, 10000);
    }

    #[test]
    fn test_load_balancing_algorithms() {
        let algos = vec![
            LoadBalancingAlgorithm::RoundRobin,
            LoadBalancingAlgorithm::LeastConnections,
            LoadBalancingAlgorithm::ConsistentHash,
            LoadBalancingAlgorithm::IpHash,
            LoadBalancingAlgorithm::WeightedRoundRobin,
            LoadBalancingAlgorithm::Random,
        ];

        for algo in algos {
            let serialized = serde_json::to_string(&algo).unwrap();
            let deserialized: LoadBalancingAlgorithm = serde_json::from_str(&serialized).unwrap();
            assert_eq!(algo, deserialized);
        }
    }

    #[test]
    fn test_health_check_types() {
        let types = vec![
            HealthCheckType::Tcp,
            HealthCheckType::Mysql,
            HealthCheckType::Postgresql,
            HealthCheckType::Redis,
            HealthCheckType::Custom,
        ];

        for check_type in types {
            let serialized = serde_json::to_string(&check_type).unwrap();
            let deserialized: HealthCheckType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(check_type, deserialized);
        }
    }
}
