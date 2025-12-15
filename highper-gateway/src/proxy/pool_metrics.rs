//! Connection pool metrics tracking
//!
//! Provides metrics for monitoring HTTP connection pool health and performance:
//! - Active connections per host
//! - Idle connections per host
//! - Connection reuse ratio
//! - Pool exhaustion events
//! - Connection creation/destruction rate
//! - Average connection lifetime

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Connection pool metrics tracker
#[derive(Clone)]
pub struct ConnectionPoolMetrics {
    /// Per-host connection statistics
    per_host_stats: Arc<DashMap<String, HostConnectionStats>>,

    /// Global pool statistics
    global_stats: Arc<GlobalPoolStats>,
}

/// Per-host connection statistics
#[derive(Debug, Default)]
struct HostConnectionStats {
    /// Number of active connections
    active_connections: AtomicU64,

    /// Number of idle connections in pool
    idle_connections: AtomicU64,

    /// Total connections created (lifetime)
    total_created: AtomicU64,

    /// Total connections reused from pool
    total_reused: AtomicU64,

    /// Total connection errors
    connection_errors: AtomicU64,

    /// Pool exhaustion events (no idle connections available)
    pool_exhausted_count: AtomicU64,

    /// Last connection creation time
    last_created: parking_lot::Mutex<Option<Instant>>,

    /// Total connection lifetime (for averaging)
    total_lifetime_ms: AtomicU64,

    /// Number of closed connections (for lifetime average)
    closed_count: AtomicU64,
}

/// Global connection pool statistics
#[derive(Debug, Default)]
struct GlobalPoolStats {
    /// Total active connections across all hosts
    total_active: AtomicU64,

    /// Total idle connections across all hosts
    total_idle: AtomicU64,

    /// Total connections created (all hosts)
    total_created: AtomicU64,

    /// Total connections reused (all hosts)
    total_reused: AtomicU64,

    /// Total connection errors (all hosts)
    total_errors: AtomicU64,

    /// Total pool exhaustion events
    total_exhausted: AtomicU64,
}

/// Connection pool metrics snapshot for a specific host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostPoolMetrics {
    /// Host identifier (upstream URL)
    pub host: String,

    /// Number of active connections
    pub active_connections: u64,

    /// Number of idle connections
    pub idle_connections: u64,

    /// Total connections created (lifetime)
    pub total_created: u64,

    /// Total connections reused
    pub total_reused: u64,

    /// Connection reuse ratio (0.0-1.0)
    pub reuse_ratio: f64,

    /// Total connection errors
    pub connection_errors: u64,

    /// Pool exhaustion events
    pub pool_exhausted_count: u64,

    /// Average connection lifetime (milliseconds)
    pub avg_connection_lifetime_ms: f64,

    /// Estimated pool utilization (0.0-1.0)
    pub pool_utilization: f64,
}

/// Global connection pool metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPoolMetrics {
    /// Total active connections
    pub total_active: u64,

    /// Total idle connections
    pub total_idle: u64,

    /// Total connections created
    pub total_created: u64,

    /// Total connections reused
    pub total_reused: u64,

    /// Global connection reuse ratio
    pub global_reuse_ratio: f64,

    /// Total connection errors
    pub total_errors: u64,

    /// Total pool exhaustion events
    pub total_exhausted: u64,

    /// Number of tracked hosts
    pub tracked_hosts: usize,

    /// Per-host metrics
    pub per_host: Vec<HostPoolMetrics>,
}

impl ConnectionPoolMetrics {
    /// Create a new connection pool metrics tracker
    pub fn new() -> Self {
        Self {
            per_host_stats: Arc::new(DashMap::new()),
            global_stats: Arc::new(GlobalPoolStats::default()),
        }
    }

    /// Record a new connection created for a host
    pub fn record_connection_created(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.active_connections.fetch_add(1, Ordering::Relaxed);
        stats.total_created.fetch_add(1, Ordering::Relaxed);
        *stats.last_created.lock() = Some(Instant::now());

        self.global_stats.total_active.fetch_add(1, Ordering::Relaxed);
        self.global_stats.total_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection reused from the pool
    pub fn record_connection_reused(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.total_reused.fetch_add(1, Ordering::Relaxed);

        self.global_stats.total_reused.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection returned to the pool (becomes idle)
    pub fn record_connection_idle(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.active_connections.fetch_sub(1, Ordering::Relaxed);
        stats.idle_connections.fetch_add(1, Ordering::Relaxed);

        self.global_stats.total_active.fetch_sub(1, Ordering::Relaxed);
        self.global_stats.total_idle.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection taken from idle pool for reuse
    pub fn record_connection_activated(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.idle_connections.fetch_sub(1, Ordering::Relaxed);
        stats.active_connections.fetch_add(1, Ordering::Relaxed);

        self.global_stats.total_idle.fetch_sub(1, Ordering::Relaxed);
        self.global_stats.total_active.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection closed
    pub fn record_connection_closed(&self, host: &str, lifetime: Duration) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();

        // Update active count
        let active = stats.active_connections.load(Ordering::Relaxed);
        if active > 0 {
            stats.active_connections.fetch_sub(1, Ordering::Relaxed);
            self.global_stats.total_active.fetch_sub(1, Ordering::Relaxed);
        } else {
            // Connection was idle
            let idle = stats.idle_connections.load(Ordering::Relaxed);
            if idle > 0 {
                stats.idle_connections.fetch_sub(1, Ordering::Relaxed);
                self.global_stats.total_idle.fetch_sub(1, Ordering::Relaxed);
            }
        }

        // Track lifetime for averaging
        stats.total_lifetime_ms.fetch_add(lifetime.as_millis() as u64, Ordering::Relaxed);
        stats.closed_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection error
    pub fn record_connection_error(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.connection_errors.fetch_add(1, Ordering::Relaxed);

        self.global_stats.total_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a pool exhaustion event (no idle connections available)
    pub fn record_pool_exhausted(&self, host: &str) {
        let stats = self.per_host_stats.entry(host.to_string()).or_default();
        stats.pool_exhausted_count.fetch_add(1, Ordering::Relaxed);

        self.global_stats.total_exhausted.fetch_add(1, Ordering::Relaxed);
    }

    /// Get metrics for a specific host
    pub fn get_host_metrics(&self, host: &str) -> Option<HostPoolMetrics> {
        self.per_host_stats.get(host).map(|stats| {
            let active = stats.active_connections.load(Ordering::Relaxed);
            let idle = stats.idle_connections.load(Ordering::Relaxed);
            let created = stats.total_created.load(Ordering::Relaxed);
            let reused = stats.total_reused.load(Ordering::Relaxed);
            let closed = stats.closed_count.load(Ordering::Relaxed);
            let total_lifetime = stats.total_lifetime_ms.load(Ordering::Relaxed);

            // Calculate reuse ratio
            let total_uses = created + reused;
            let reuse_ratio = if total_uses > 0 {
                reused as f64 / total_uses as f64
            } else {
                0.0
            };

            // Calculate average connection lifetime
            let avg_lifetime = if closed > 0 {
                total_lifetime as f64 / closed as f64
            } else {
                0.0
            };

            // Estimate pool utilization (assuming max 100 per host from client.rs)
            let max_idle_per_host = 100;
            let pool_utilization = if max_idle_per_host > 0 {
                (active + idle) as f64 / max_idle_per_host as f64
            } else {
                0.0
            };

            HostPoolMetrics {
                host: host.to_string(),
                active_connections: active,
                idle_connections: idle,
                total_created: created,
                total_reused: reused,
                reuse_ratio,
                connection_errors: stats.connection_errors.load(Ordering::Relaxed),
                pool_exhausted_count: stats.pool_exhausted_count.load(Ordering::Relaxed),
                avg_connection_lifetime_ms: avg_lifetime,
                pool_utilization,
            }
        })
    }

    /// Get global metrics across all hosts
    pub fn get_global_metrics(&self) -> GlobalPoolMetrics {
        let total_created = self.global_stats.total_created.load(Ordering::Relaxed);
        let total_reused = self.global_stats.total_reused.load(Ordering::Relaxed);

        // Calculate global reuse ratio
        let total_uses = total_created + total_reused;
        let global_reuse_ratio = if total_uses > 0 {
            total_reused as f64 / total_uses as f64
        } else {
            0.0
        };

        // Collect per-host metrics
        let per_host = self.per_host_stats
            .iter()
            .filter_map(|entry| {
                let host = entry.key().clone();
                self.get_host_metrics(&host)
            })
            .collect();

        GlobalPoolMetrics {
            total_active: self.global_stats.total_active.load(Ordering::Relaxed),
            total_idle: self.global_stats.total_idle.load(Ordering::Relaxed),
            total_created,
            total_reused,
            global_reuse_ratio,
            total_errors: self.global_stats.total_errors.load(Ordering::Relaxed),
            total_exhausted: self.global_stats.total_exhausted.load(Ordering::Relaxed),
            tracked_hosts: self.per_host_stats.len(),
            per_host,
        }
    }

    /// Reset all metrics (useful for testing)
    pub fn reset(&self) {
        self.per_host_stats.clear();
        self.global_stats.total_active.store(0, Ordering::Relaxed);
        self.global_stats.total_idle.store(0, Ordering::Relaxed);
        self.global_stats.total_created.store(0, Ordering::Relaxed);
        self.global_stats.total_reused.store(0, Ordering::Relaxed);
        self.global_stats.total_errors.store(0, Ordering::Relaxed);
        self.global_stats.total_exhausted.store(0, Ordering::Relaxed);
    }
}

impl Default for ConnectionPoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_pool_metrics_creation() {
        let metrics = ConnectionPoolMetrics::new();
        let global = metrics.get_global_metrics();

        assert_eq!(global.total_active, 0);
        assert_eq!(global.total_idle, 0);
        assert_eq!(global.tracked_hosts, 0);
    }

    #[test]
    fn test_record_connection_lifecycle() {
        let metrics = ConnectionPoolMetrics::new();
        let host = "http://localhost:8080";

        // Create connection
        metrics.record_connection_created(host);
        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        assert_eq!(host_metrics.active_connections, 1);
        assert_eq!(host_metrics.total_created, 1);

        // Return to pool (idle)
        metrics.record_connection_idle(host);
        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        assert_eq!(host_metrics.active_connections, 0);
        assert_eq!(host_metrics.idle_connections, 1);

        // Reuse from pool
        metrics.record_connection_activated(host);
        metrics.record_connection_reused(host);
        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        assert_eq!(host_metrics.active_connections, 1);
        assert_eq!(host_metrics.idle_connections, 0);
        assert_eq!(host_metrics.total_reused, 1);

        // Calculate reuse ratio
        assert!(host_metrics.reuse_ratio > 0.0);
    }

    #[test]
    fn test_reuse_ratio_calculation() {
        let metrics = ConnectionPoolMetrics::new();
        let host = "http://backend.example.com";

        // Create 10 connections
        for _ in 0..10 {
            metrics.record_connection_created(host);
        }

        // Reuse 40 times
        for _ in 0..40 {
            metrics.record_connection_reused(host);
        }

        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        // 40 reuses out of 50 total uses = 0.8 ratio
        assert!((host_metrics.reuse_ratio - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_global_metrics_aggregation() {
        let metrics = ConnectionPoolMetrics::new();

        // Create connections for multiple hosts
        metrics.record_connection_created("http://host1:8080");
        metrics.record_connection_created("http://host2:8080");
        metrics.record_connection_created("http://host3:8080");

        let global = metrics.get_global_metrics();
        assert_eq!(global.total_active, 3);
        assert_eq!(global.total_created, 3);
        assert_eq!(global.tracked_hosts, 3);
    }

    #[test]
    fn test_connection_errors() {
        let metrics = ConnectionPoolMetrics::new();
        let host = "http://failing-host:8080";

        metrics.record_connection_error(host);
        metrics.record_connection_error(host);

        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        assert_eq!(host_metrics.connection_errors, 2);

        let global = metrics.get_global_metrics();
        assert_eq!(global.total_errors, 2);
    }

    #[test]
    fn test_pool_exhaustion_tracking() {
        let metrics = ConnectionPoolMetrics::new();
        let host = "http://busy-host:8080";

        metrics.record_pool_exhausted(host);

        let host_metrics = metrics.get_host_metrics(host).expect("Host metrics should exist after recording");
        assert_eq!(host_metrics.pool_exhausted_count, 1);

        let global = metrics.get_global_metrics();
        assert_eq!(global.total_exhausted, 1);
    }
}
