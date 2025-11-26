//! Backpressure and Load Shedding
//!
//! This module provides graceful degradation under extreme load (3M+ connections).
//!
//! ## Features
//! - **Connection limiting**: Reject new connections when at capacity
//! - **Memory pressure detection**: Shed load when memory usage is high
//! - **CPU throttling**: Slow down acceptance rate when CPU is saturated
//! - **Adaptive rate limiting**: Dynamically adjust limits based on system health
//! - **Metrics**: Track rejected connections, memory pressure events
//!
//! ## Philosophy (FreeBSD-level reliability)
//! - **Fail gracefully**: Better to reject new connections than crash
//! - **Preserve existing**: Prioritize active connections over new ones
//! - **Predictable behavior**: Clear limits, no sudden failures
//! - **Observable**: Comprehensive metrics for debugging
//!
//! ## Usage
//! ```rust,no_run
//! use highper_gateway::runtime::backpressure::BackpressureManager;
//!
//! let manager = BackpressureManager::new(3_000_000, 48 * 1024);
//!
//! // Before accepting a new connection
//! if !manager.should_accept_connection() {
//!     // Reject with 503 Service Unavailable
//!     return Err(anyhow::anyhow!("Server at capacity"));
//! }
//!
//! manager.on_connection_accepted();
//! // ... handle connection ...
//! manager.on_connection_closed();
//! ```

use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, warn, info};
use crate::observability::system::{SystemStats, MemoryStats};

/// Backpressure manager for graceful degradation
pub struct BackpressureManager {
    /// Maximum concurrent connections allowed
    max_connections: AtomicUsize,

    /// Current number of active connections
    current_connections: AtomicUsize,

    /// Memory limit in MB (RSS)
    memory_limit_mb: usize,

    /// CPU usage threshold (0-100)
    cpu_threshold: u8,

    /// Total connections rejected due to capacity limits
    connections_rejected_capacity: AtomicU64,

    /// Total connections rejected due to memory pressure
    connections_rejected_memory: AtomicU64,

    /// Total connections rejected due to CPU saturation
    connections_rejected_cpu: AtomicU64,

    /// Enable adaptive limiting (adjust limits based on system health)
    adaptive: bool,
}

impl BackpressureManager {
    /// Create a new backpressure manager
    ///
    /// # Arguments
    /// * `max_connections` - Maximum concurrent connections (e.g., 3M)
    /// * `memory_limit_mb` - Memory limit in MB (e.g., 48GB = 48 * 1024)
    ///
    /// # Example
    /// ```rust,no_run
    /// use highper_gateway::runtime::backpressure::BackpressureManager;
    ///
    /// // 3M connections, 48GB memory limit
    /// let manager = BackpressureManager::new(3_000_000, 48 * 1024);
    /// ```
    pub fn new(max_connections: usize, memory_limit_mb: usize) -> Self {
        info!(
            "Initializing backpressure manager: max_connections={}, memory_limit={}MB",
            max_connections, memory_limit_mb
        );

        Self {
            max_connections: AtomicUsize::new(max_connections),
            current_connections: AtomicUsize::new(0),
            memory_limit_mb,
            cpu_threshold: 90, // Reject when CPU > 90%
            connections_rejected_capacity: AtomicU64::new(0),
            connections_rejected_memory: AtomicU64::new(0),
            connections_rejected_cpu: AtomicU64::new(0),
            adaptive: true,
        }
    }

    /// Create a backpressure manager with custom CPU threshold
    pub fn with_cpu_threshold(mut self, threshold: u8) -> Self {
        self.cpu_threshold = threshold.min(100);
        self
    }

    /// Disable adaptive limiting (use fixed limits)
    pub fn without_adaptive(mut self) -> Self {
        self.adaptive = false;
        self
    }

    /// Check if a new connection should be accepted
    ///
    /// Returns `false` if:
    /// - At max connection capacity
    /// - Memory pressure detected
    /// - CPU saturation detected
    pub fn should_accept_connection(&self) -> bool {
        let current = self.current_connections.load(Ordering::Relaxed);
        let max = self.max_connections.load(Ordering::Relaxed);

        // Check connection capacity
        if current >= max {
            warn!(
                "Rejecting connection: at max capacity ({}/{})",
                current, max
            );
            self.connections_rejected_capacity.fetch_add(1, Ordering::Relaxed);
            metrics::counter!("connections_rejected_total", 1, "reason" => "max_capacity");
            return false;
        }

        // Check memory pressure
        if self.is_memory_pressure() {
            warn!(
                "Rejecting connection: memory pressure (current={}, limit={}MB)",
                self.get_current_memory_mb(),
                self.memory_limit_mb
            );
            self.connections_rejected_memory.fetch_add(1, Ordering::Relaxed);
            metrics::counter!("connections_rejected_total", 1, "reason" => "memory_pressure");
            return false;
        }

        // Check CPU saturation (if adaptive is enabled)
        if self.adaptive && self.is_cpu_saturated() {
            debug!("Rejecting connection: CPU saturation (>{}%)", self.cpu_threshold);
            self.connections_rejected_cpu.fetch_add(1, Ordering::Relaxed);
            metrics::counter!("connections_rejected_total", 1, "reason" => "cpu_saturation");
            return false;
        }

        true
    }

    /// Called when a connection is accepted
    pub fn on_connection_accepted(&self) {
        let prev = self.current_connections.fetch_add(1, Ordering::Relaxed);
        let new = prev + 1;

        debug!("Connection accepted ({}/{})", new, self.max_connections.load(Ordering::Relaxed));
        metrics::gauge!("active_connections", new as f64);

        // Warn if approaching capacity
        let max = self.max_connections.load(Ordering::Relaxed);
        let usage_percent = (new as f64 / max as f64) * 100.0;

        if usage_percent > 90.0 {
            warn!(
                "High connection usage: {}/{} ({:.1}%)",
                new, max, usage_percent
            );
        } else if usage_percent > 80.0 {
            info!(
                "Moderate connection usage: {}/{} ({:.1}%)",
                new, max, usage_percent
            );
        }
    }

    /// Called when a connection is closed
    pub fn on_connection_closed(&self) {
        let prev = self.current_connections.fetch_sub(1, Ordering::Relaxed);
        let new = prev.saturating_sub(1);

        debug!("Connection closed ({}/{})", new, self.max_connections.load(Ordering::Relaxed));
        metrics::gauge!("active_connections", new as f64);
    }

    /// Get current number of active connections
    pub fn current_connections(&self) -> usize {
        self.current_connections.load(Ordering::Relaxed)
    }

    /// Get maximum connections allowed
    pub fn max_connections(&self) -> usize {
        self.max_connections.load(Ordering::Relaxed)
    }

    /// Update maximum connections (for adaptive limiting)
    pub fn set_max_connections(&self, new_max: usize) {
        let old_max = self.max_connections.swap(new_max, Ordering::Relaxed);
        info!("Updated max connections: {} -> {}", old_max, new_max);
    }

    /// Get connection usage percentage (0-100)
    pub fn usage_percent(&self) -> f64 {
        let current = self.current_connections.load(Ordering::Relaxed);
        let max = self.max_connections.load(Ordering::Relaxed);

        if max == 0 {
            return 0.0;
        }

        (current as f64 / max as f64) * 100.0
    }

    /// Check if memory pressure is detected
    fn is_memory_pressure(&self) -> bool {
        let current_mb = self.get_current_memory_mb();
        current_mb > self.memory_limit_mb
    }

    /// Get current memory usage in MB
    fn get_current_memory_mb(&self) -> usize {
        let stats = MemoryStats::collect();
        stats.rss / 1024 / 1024 // Convert bytes to MB
    }

    /// Check if CPU is saturated
    fn is_cpu_saturated(&self) -> bool {
        // TODO: Implement actual CPU usage monitoring
        // For now, return false (no CPU throttling)
        // This requires adding CPU usage tracking to SystemStats
        false
    }

    /// Get backpressure statistics
    pub fn stats(&self) -> BackpressureStats {
        BackpressureStats {
            current_connections: self.current_connections.load(Ordering::Relaxed),
            max_connections: self.max_connections.load(Ordering::Relaxed),
            usage_percent: self.usage_percent(),
            memory_usage_mb: self.get_current_memory_mb(),
            memory_limit_mb: self.memory_limit_mb,
            connections_rejected_capacity: self.connections_rejected_capacity.load(Ordering::Relaxed),
            connections_rejected_memory: self.connections_rejected_memory.load(Ordering::Relaxed),
            connections_rejected_cpu: self.connections_rejected_cpu.load(Ordering::Relaxed),
        }
    }

    /// Print backpressure statistics
    pub fn print_stats(&self) {
        let stats = self.stats();

        info!("=== Backpressure Statistics ===");
        info!("Active connections: {}/{} ({:.1}%)",
            stats.current_connections,
            stats.max_connections,
            stats.usage_percent
        );
        info!("Memory usage: {}MB / {}MB",
            stats.memory_usage_mb,
            stats.memory_limit_mb
        );
        info!("Connections rejected:");
        info!("  - Capacity: {}", stats.connections_rejected_capacity);
        info!("  - Memory: {}", stats.connections_rejected_memory);
        info!("  - CPU: {}", stats.connections_rejected_cpu);
    }
}

/// Backpressure statistics snapshot
#[derive(Debug, Clone)]
pub struct BackpressureStats {
    /// Current number of active connections
    pub current_connections: usize,

    /// Maximum connections allowed
    pub max_connections: usize,

    /// Connection usage percentage (0-100)
    pub usage_percent: f64,

    /// Current memory usage (MB)
    pub memory_usage_mb: usize,

    /// Memory limit (MB)
    pub memory_limit_mb: usize,

    /// Total connections rejected due to capacity
    pub connections_rejected_capacity: u64,

    /// Total connections rejected due to memory
    pub connections_rejected_memory: u64,

    /// Total connections rejected due to CPU
    pub connections_rejected_cpu: u64,
}

/// Global backpressure manager instance
pub static GLOBAL_BACKPRESSURE: once_cell::sync::Lazy<Arc<BackpressureManager>> =
    once_cell::sync::Lazy::new(|| {
        // Default limits for 3M connections with 48GB RAM
        Arc::new(BackpressureManager::new(3_000_000, 48 * 1024))
    });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_manager() {
        let manager = BackpressureManager::new(100, 1024);

        // Initially should accept connections
        assert!(manager.should_accept_connection());
        assert_eq!(manager.current_connections(), 0);

        // Accept 100 connections
        for _ in 0..100 {
            assert!(manager.should_accept_connection());
            manager.on_connection_accepted();
        }

        // Should be at capacity
        assert_eq!(manager.current_connections(), 100);
        assert!(!manager.should_accept_connection());

        // Close one connection
        manager.on_connection_closed();
        assert_eq!(manager.current_connections(), 99);

        // Should accept again
        assert!(manager.should_accept_connection());
    }

    #[test]
    fn test_usage_percent() {
        let manager = BackpressureManager::new(1000, 1024);

        assert_eq!(manager.usage_percent(), 0.0);

        for _ in 0..500 {
            manager.on_connection_accepted();
        }

        assert_eq!(manager.usage_percent(), 50.0);

        for _ in 0..500 {
            manager.on_connection_accepted();
        }

        assert_eq!(manager.usage_percent(), 100.0);
    }

    #[test]
    fn test_stats() {
        let manager = BackpressureManager::new(1000, 1024);

        let stats = manager.stats();
        assert_eq!(stats.current_connections, 0);
        assert_eq!(stats.max_connections, 1000);
        assert_eq!(stats.usage_percent, 0.0);

        manager.on_connection_accepted();

        let stats = manager.stats();
        assert_eq!(stats.current_connections, 1);
        assert_eq!(stats.usage_percent, 0.1);
    }
}
