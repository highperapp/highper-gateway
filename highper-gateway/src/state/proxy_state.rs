//! Shared proxy state for runtime components
//!
//! Provides centralized state management for the proxy, allowing
//! components like the Admin API to interact with runtime state.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::gateway::cache::LocalCache;
use crate::proxy::ConnectionPoolMetrics;
use crate::state::request_metrics::RequestMetrics;

/// Backend state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendState {
    /// Backend ID (upstream_name_index)
    pub id: String,

    /// Upstream name
    pub upstream: String,

    /// Backend URL
    pub url: String,

    /// Whether backend is administratively enabled
    pub enabled: bool,

    /// Whether backend is in drain mode
    pub draining: bool,

    /// Timestamp when drain started (Unix timestamp in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drain_started_at: Option<u64>,

    /// Drain timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drain_timeout_secs: Option<u64>,

    /// Whether drain has completed (all connections closed or timeout reached)
    #[serde(default)]
    pub drain_completed: bool,

    /// Reason for current state (for disable/drain)
    pub reason: Option<String>,

    /// Active connections count
    pub active_connections: usize,

    /// Health status
    pub health_status: HealthStatus,
}

/// Health status of a backend
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Backend is healthy
    Healthy,

    /// Backend is unhealthy
    Unhealthy,

    /// Health status unknown
    Unknown,
}

/// Drain status information for a backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainStatus {
    /// Backend ID
    pub backend_id: String,

    /// Whether backend is currently draining
    pub draining: bool,

    /// Whether drain has completed (all connections closed)
    pub drain_completed: bool,

    /// Current active connections
    pub active_connections: usize,

    /// Seconds elapsed since drain started
    pub elapsed_secs: u64,

    /// Total timeout in seconds
    pub timeout_secs: u64,

    /// Seconds remaining until timeout
    pub remaining_secs: u64,

    /// Whether the drain has timed out
    pub timed_out: bool,
}

/// Basic metrics tracking
#[derive(Debug)]
pub struct MetricsState {
    /// Total requests processed
    pub total_requests: AtomicU64,

    /// Total responses with 2xx status
    pub status_2xx: AtomicU64,

    /// Total responses with 3xx status
    pub status_3xx: AtomicU64,

    /// Total responses with 4xx status
    pub status_4xx: AtomicU64,

    /// Total responses with 5xx status
    pub status_5xx: AtomicU64,
}

impl MetricsState {
    /// Create a new metrics state
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
        }
    }

    /// Increment request counter
    pub fn increment_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a response status code
    pub fn record_status(&self, status: u16) {
        match status {
            200..=299 => self.status_2xx.fetch_add(1, Ordering::Relaxed),
            300..=399 => self.status_3xx.fetch_add(1, Ordering::Relaxed),
            400..=499 => self.status_4xx.fetch_add(1, Ordering::Relaxed),
            500..=599 => self.status_5xx.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    /// Get total requests
    pub fn get_total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get 2xx count
    pub fn get_2xx(&self) -> u64 {
        self.status_2xx.load(Ordering::Relaxed)
    }

    /// Get 3xx count
    pub fn get_3xx(&self) -> u64 {
        self.status_3xx.load(Ordering::Relaxed)
    }

    /// Get 4xx count
    pub fn get_4xx(&self) -> u64 {
        self.status_4xx.load(Ordering::Relaxed)
    }

    /// Get 5xx count
    pub fn get_5xx(&self) -> u64 {
        self.status_5xx.load(Ordering::Relaxed)
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared proxy state
pub struct ProxyState {
    /// Backend states indexed by backend ID
    backends: Arc<RwLock<HashMap<String, BackendState>>>,

    /// Local cache instance (optional)
    local_cache: Option<Arc<LocalCache>>,

    /// Basic metrics tracking
    metrics: Arc<MetricsState>,

    /// Connection pool metrics (optional)
    pool_metrics: Option<Arc<ConnectionPoolMetrics>>,

    /// Enhanced request metrics (optional)
    request_metrics: Option<Arc<RequestMetrics>>,
}

impl ProxyState {
    /// Create a new proxy state
    pub fn new() -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
            local_cache: None,
            metrics: Arc::new(MetricsState::new()),
            pool_metrics: None,
            request_metrics: None,
        }
    }

    /// Create a new proxy state with cache
    pub fn with_cache(cache: Arc<LocalCache>) -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
            local_cache: Some(cache),
            metrics: Arc::new(MetricsState::new()),
            pool_metrics: None,
            request_metrics: None,
        }
    }

    /// Set the local cache instance
    pub fn set_local_cache(&mut self, cache: Arc<LocalCache>) {
        self.local_cache = Some(cache);
    }

    /// Get the local cache instance
    pub fn local_cache(&self) -> Option<Arc<LocalCache>> {
        self.local_cache.clone()
    }

    /// Get the metrics instance
    pub fn metrics(&self) -> Arc<MetricsState> {
        self.metrics.clone()
    }

    /// Set the connection pool metrics instance
    pub fn set_pool_metrics(&mut self, pool_metrics: Arc<ConnectionPoolMetrics>) {
        self.pool_metrics = Some(pool_metrics);
    }

    /// Get the connection pool metrics instance
    pub fn pool_metrics(&self) -> Option<Arc<ConnectionPoolMetrics>> {
        self.pool_metrics.clone()
    }

    /// Set the request metrics instance
    pub fn set_request_metrics(&mut self, request_metrics: Arc<RequestMetrics>) {
        self.request_metrics = Some(request_metrics);
    }

    /// Get the request metrics instance
    pub fn request_metrics(&self) -> Option<Arc<RequestMetrics>> {
        self.request_metrics.clone()
    }

    /// Register a backend
    pub async fn register_backend(&self, backend: BackendState) {
        let mut backends = self.backends.write().await;
        backends.insert(backend.id.clone(), backend);
    }

    /// Get a backend state
    pub async fn get_backend(&self, id: &str) -> Option<BackendState> {
        let backends = self.backends.read().await;
        backends.get(id).cloned()
    }

    /// Get all backends
    pub async fn get_all_backends(&self) -> Vec<BackendState> {
        let backends = self.backends.read().await;
        backends.values().cloned().collect()
    }

    /// Update backend enabled status
    pub async fn set_backend_enabled(&self, id: &str, enabled: bool, reason: Option<String>) -> bool {
        let mut backends = self.backends.write().await;
        if let Some(backend) = backends.get_mut(id) {
            backend.enabled = enabled;
            backend.reason = reason;
            true
        } else {
            false
        }
    }

    /// Update backend drain status
    pub async fn set_backend_draining(&self, id: &str, draining: bool) -> bool {
        self.set_backend_draining_with_timeout(id, draining, None, None).await
    }

    /// Update backend drain status with timeout and reason
    pub async fn set_backend_draining_with_timeout(
        &self,
        id: &str,
        draining: bool,
        timeout_secs: Option<u64>,
        reason: Option<String>,
    ) -> bool {
        let mut backends = self.backends.write().await;
        if let Some(backend) = backends.get_mut(id) {
            backend.draining = draining;
            if draining {
                // Starting drain
                backend.drain_started_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                backend.drain_timeout_secs = timeout_secs;
                backend.drain_completed = false;
                backend.reason = reason;
            } else {
                // Stopping drain (either completed or cancelled)
                backend.drain_started_at = None;
                backend.drain_timeout_secs = None;
                backend.drain_completed = false;
                backend.reason = None;
            }
            true
        } else {
            false
        }
    }

    /// Mark drain as completed
    pub async fn complete_drain(&self, id: &str) -> bool {
        let mut backends = self.backends.write().await;
        if let Some(backend) = backends.get_mut(id) {
            if backend.draining {
                backend.drain_completed = true;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Check if drain has timed out
    pub async fn is_drain_timed_out(&self, id: &str) -> bool {
        let backends = self.backends.read().await;
        if let Some(backend) = backends.get(id) {
            if let (Some(started_at), Some(timeout)) = (backend.drain_started_at, backend.drain_timeout_secs) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                return now >= started_at + timeout;
            }
        }
        false
    }

    /// Get drain status for a backend
    pub async fn get_drain_status(&self, id: &str) -> Option<DrainStatus> {
        let backends = self.backends.read().await;
        backends.get(id).and_then(|backend| {
            if !backend.draining {
                return None;
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let elapsed = backend.drain_started_at.map(|s| now.saturating_sub(s)).unwrap_or(0);
            let remaining = backend.drain_timeout_secs
                .map(|t| t.saturating_sub(elapsed))
                .unwrap_or(0);

            Some(DrainStatus {
                backend_id: id.to_string(),
                draining: backend.draining,
                drain_completed: backend.drain_completed,
                active_connections: backend.active_connections,
                elapsed_secs: elapsed,
                timeout_secs: backend.drain_timeout_secs.unwrap_or(0),
                remaining_secs: remaining,
                timed_out: backend.drain_timeout_secs.map(|t| elapsed >= t).unwrap_or(false),
            })
        })
    }

    /// Update backend health status
    pub async fn set_backend_health(&self, id: &str, health: HealthStatus) -> bool {
        let mut backends = self.backends.write().await;
        if let Some(backend) = backends.get_mut(id) {
            backend.health_status = health;
            true
        } else {
            false
        }
    }

    /// Update backend active connections
    pub async fn set_backend_connections(&self, id: &str, connections: usize) -> bool {
        let mut backends = self.backends.write().await;
        if let Some(backend) = backends.get_mut(id) {
            backend.active_connections = connections;
            true
        } else {
            false
        }
    }

    /// Get a cloned reference to the backends map
    pub fn backends(&self) -> Arc<RwLock<HashMap<String, BackendState>>> {
        self.backends.clone()
    }
}

impl Default for ProxyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get_backend() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 0,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend.clone()).await;

        let retrieved = state.get_backend("test_upstream_0").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test_upstream_0");
    }

    #[tokio::test]
    async fn test_set_backend_enabled() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 0,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend).await;

        let success = state.set_backend_enabled(
            "test_upstream_0",
            false,
            Some("Maintenance".to_string())
        ).await;

        assert!(success);

        let backend = state.get_backend("test_upstream_0").await.unwrap();
        assert_eq!(backend.enabled, false);
        assert_eq!(backend.reason, Some("Maintenance".to_string()));
    }

    #[tokio::test]
    async fn test_get_all_backends() {
        let state = ProxyState::new();

        for i in 0..3 {
            let backend = BackendState {
                id: format!("test_upstream_{}", i),
                upstream: "test_upstream".to_string(),
                url: format!("http://localhost:808{}", i),
                enabled: true,
                draining: false,
                drain_started_at: None,
                drain_timeout_secs: None,
                drain_completed: false,
                reason: None,
                active_connections: 0,
                health_status: HealthStatus::Healthy,
            };
            state.register_backend(backend).await;
        }

        let all_backends = state.get_all_backends().await;
        assert_eq!(all_backends.len(), 3);
    }

    #[tokio::test]
    async fn test_drain_backend() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 5,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend).await;

        // Start draining with 60 second timeout
        let success = state.set_backend_draining_with_timeout(
            "test_upstream_0",
            true,
            Some(60),
            Some("Maintenance".to_string()),
        ).await;
        assert!(success);

        // Verify drain state
        let backend = state.get_backend("test_upstream_0").await.unwrap();
        assert!(backend.draining);
        assert!(backend.drain_started_at.is_some());
        assert_eq!(backend.drain_timeout_secs, Some(60));
        assert!(!backend.drain_completed);
        assert_eq!(backend.reason, Some("Maintenance".to_string()));
    }

    #[tokio::test]
    async fn test_drain_status() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 10,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend).await;

        // Before draining, status should be None
        let status = state.get_drain_status("test_upstream_0").await;
        assert!(status.is_none());

        // Start draining
        state.set_backend_draining_with_timeout(
            "test_upstream_0",
            true,
            Some(30),
            None,
        ).await;

        // Get drain status
        let status = state.get_drain_status("test_upstream_0").await;
        assert!(status.is_some());

        let status = status.unwrap();
        assert!(status.draining);
        assert!(!status.drain_completed);
        assert_eq!(status.active_connections, 10);
        assert_eq!(status.timeout_secs, 30);
        assert!(!status.timed_out);
    }

    #[tokio::test]
    async fn test_complete_drain() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 0,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend).await;

        // Start draining
        state.set_backend_draining_with_timeout(
            "test_upstream_0",
            true,
            Some(60),
            None,
        ).await;

        // Complete drain
        let success = state.complete_drain("test_upstream_0").await;
        assert!(success);

        // Verify drain is completed
        let backend = state.get_backend("test_upstream_0").await.unwrap();
        assert!(backend.draining);
        assert!(backend.drain_completed);
    }

    #[tokio::test]
    async fn test_cancel_drain() {
        let state = ProxyState::new();

        let backend = BackendState {
            id: "test_upstream_0".to_string(),
            upstream: "test_upstream".to_string(),
            url: "http://localhost:8080".to_string(),
            enabled: true,
            draining: false,
            drain_started_at: None,
            drain_timeout_secs: None,
            drain_completed: false,
            reason: None,
            active_connections: 5,
            health_status: HealthStatus::Healthy,
        };

        state.register_backend(backend).await;

        // Start draining
        state.set_backend_draining_with_timeout(
            "test_upstream_0",
            true,
            Some(60),
            Some("Maintenance".to_string()),
        ).await;

        // Cancel drain
        let success = state.set_backend_draining("test_upstream_0", false).await;
        assert!(success);

        // Verify drain is cancelled
        let backend = state.get_backend("test_upstream_0").await.unwrap();
        assert!(!backend.draining);
        assert!(backend.drain_started_at.is_none());
        assert!(backend.drain_timeout_secs.is_none());
        assert!(!backend.drain_completed);
    }
}
