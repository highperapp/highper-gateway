//! Health check system for upstream servers

use crate::config::{ActiveHealthCheckConfig, ServerDef};
use crate::proxy::Client;
use crate::state::ProxyState;
use hyper::{HeaderMap, Method, StatusCode};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Health status of a backend server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Backend server with health tracking
#[derive(Debug)]
pub struct Backend {
    pub server: ServerDef,
    healthy: AtomicBool,
    consecutive_successes: AtomicU32,
    consecutive_failures: AtomicU32,
    last_check: parking_lot::Mutex<Option<Instant>>,
    // Passive monitoring
    passive_failures: AtomicU32,
    passive_successes: AtomicU32,
    last_passive_check: parking_lot::Mutex<Option<Instant>>,
}

impl Backend {
    /// Create a new backend
    pub fn new(server: ServerDef) -> Self {
        Self {
            server,
            healthy: AtomicBool::new(true), // Start as healthy
            consecutive_successes: AtomicU32::new(0),
            consecutive_failures: AtomicU32::new(0),
            last_check: parking_lot::Mutex::new(None),
            passive_failures: AtomicU32::new(0),
            passive_successes: AtomicU32::new(0),
            last_passive_check: parking_lot::Mutex::new(None),
        }
    }

    /// Check if the backend is healthy
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Get health status
    pub fn status(&self) -> HealthStatus {
        if self.is_healthy() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Mark check success
    pub fn mark_success(&self, healthy_threshold: u32) {
        self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);

        let successes = self.consecutive_successes.load(Ordering::Relaxed);
        if successes >= healthy_threshold && !self.is_healthy() {
            info!("Backend {} is now healthy", self.server.url);
            self.healthy.store(true, Ordering::Relaxed);
        }

        *self.last_check.lock() = Some(Instant::now());
    }

    /// Mark check failure
    pub fn mark_failure(&self, unhealthy_threshold: u32) {
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);

        let failures = self.consecutive_failures.load(Ordering::Relaxed);
        if failures >= unhealthy_threshold && self.is_healthy() {
            warn!("Backend {} is now unhealthy", self.server.url);
            self.healthy.store(false, Ordering::Relaxed);
        }

        *self.last_check.lock() = Some(Instant::now());
    }

    /// Get time since last check
    pub fn time_since_last_check(&self) -> Option<Duration> {
        self.last_check.lock().map(|instant| instant.elapsed())
    }

    /// Get consecutive success count
    pub fn consecutive_successes(&self) -> u32 {
        self.consecutive_successes.load(Ordering::Relaxed)
    }

    /// Get consecutive failure count
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Record a passive success (from actual proxy request)
    pub fn record_passive_success(&self) {
        self.passive_successes.fetch_add(1, Ordering::Relaxed);
        self.passive_failures.store(0, Ordering::Relaxed);
        *self.last_passive_check.lock() = Some(Instant::now());

        // If backend was unhealthy, a successful request might indicate recovery
        debug!("Backend {} passive success recorded", self.server.url);
    }

    /// Record a passive failure (from actual proxy request)
    pub fn record_passive_failure(&self, max_failures: u32) {
        self.passive_failures.fetch_add(1, Ordering::Relaxed);
        self.passive_successes.store(0, Ordering::Relaxed);
        *self.last_passive_check.lock() = Some(Instant::now());

        let failures = self.passive_failures.load(Ordering::Relaxed);
        if failures >= max_failures && self.is_healthy() {
            warn!(
                "Backend {} marked unhealthy after {} passive failures",
                self.server.url, failures
            );
            self.healthy.store(false, Ordering::Relaxed);
        }
    }

    /// Get passive monitoring statistics
    pub fn passive_stats(&self) -> (u32, u32) {
        (
            self.passive_successes.load(Ordering::Relaxed),
            self.passive_failures.load(Ordering::Relaxed),
        )
    }

    /// Reset passive monitoring counters
    pub fn reset_passive_counters(&self) {
        self.passive_successes.store(0, Ordering::Relaxed);
        self.passive_failures.store(0, Ordering::Relaxed);
    }
}

/// Active health checker for upstream servers
pub struct HealthChecker {
    backends: Vec<Arc<Backend>>,
    config: ActiveHealthCheckConfig,
    client: Client,
    upstream_name: String,
    proxy_state: Option<Arc<ProxyState>>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(backends: Vec<Arc<Backend>>, config: ActiveHealthCheckConfig) -> Self {
        let client = Client::new();

        Self {
            backends,
            config,
            client,
            upstream_name: "default".to_string(),
            proxy_state: None,
        }
    }

    /// Create a new health checker with ProxyState integration
    pub fn with_state(
        upstream_name: String,
        backends: Vec<Arc<Backend>>,
        config: ActiveHealthCheckConfig,
        proxy_state: Arc<ProxyState>,
    ) -> Self {
        let client = Client::new();

        Self {
            backends,
            config,
            client,
            upstream_name,
            proxy_state: Some(proxy_state),
        }
    }

    /// Start health checking loop
    pub async fn run(self: Arc<Self>) {
        info!(
            "Starting health checker: interval={:?}, timeout={:?}",
            self.config.interval, self.config.timeout
        );

        loop {
            // Check all backends
            let mut tasks = Vec::new();
            for backend in &self.backends {
                let backend = backend.clone();
                let checker = self.clone();

                tasks.push(tokio::spawn(async move {
                    checker.check_backend(backend).await;
                }));
            }

            // Wait for all checks to complete
            for task in tasks {
                let _ = task.await;
            }

            // Sleep until next check interval
            sleep(self.config.interval).await;
        }
    }

    /// Check a single backend
    async fn check_backend(&self, backend: Arc<Backend>) {
        debug!("Health checking backend: {}", backend.server.url);

        // Prepare headers
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "highper-gateway-health-checker/0.1.0".parse().unwrap());

        // Perform health check with timeout
        let result = tokio::time::timeout(
            self.config.timeout,
            self.client.forward(
                &backend.server.url,
                Method::GET,
                &self.config.path,
                headers,
                None, // No body for GET request
            ),
        )
        .await;

        let health_status = match result {
            Ok(Ok(response)) => {
                let status = response.status();
                if status.is_success() || status == StatusCode::OK {
                    debug!("Backend {} health check succeeded: {}", backend.server.url, status);
                    backend.mark_success(self.config.healthy_threshold);
                    backend.status()
                } else {
                    warn!(
                        "Backend {} health check failed with status: {}",
                        backend.server.url, status
                    );
                    backend.mark_failure(self.config.unhealthy_threshold);
                    backend.status()
                }
            }
            Ok(Err(e)) => {
                warn!("Backend {} health check failed: {}", backend.server.url, e);
                backend.mark_failure(self.config.unhealthy_threshold);
                backend.status()
            }
            Err(_) => {
                warn!(
                    "Backend {} health check timed out after {:?}",
                    backend.server.url, self.config.timeout
                );
                backend.mark_failure(self.config.unhealthy_threshold);
                backend.status()
            }
        };

        // Update ProxyState if available
        if let Some(ref state) = self.proxy_state {
            let backend_id = format!("{}_{}", self.upstream_name,
                self.backends.iter().position(|b| Arc::ptr_eq(b, &backend)).unwrap_or(0));

            let state_health = match health_status {
                HealthStatus::Healthy => crate::state::HealthStatus::Healthy,
                HealthStatus::Unhealthy => crate::state::HealthStatus::Unhealthy,
                HealthStatus::Unknown => crate::state::HealthStatus::Unknown,
            };

            let _ = state.set_backend_health(&backend_id, state_health).await;
        }
    }

    /// Get all healthy backends
    pub fn healthy_backends(&self) -> Vec<Arc<Backend>> {
        self.backends
            .iter()
            .filter(|b| b.is_healthy())
            .cloned()
            .collect()
    }

    /// Get health status of all backends
    pub fn backend_statuses(&self) -> Vec<(String, HealthStatus, u32, u32)> {
        self.backends
            .iter()
            .map(|b| {
                (
                    b.server.url.clone(),
                    b.status(),
                    b.consecutive_successes(),
                    b.consecutive_failures(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_health_transitions() {
        let server = ServerDef {
            url: "http://localhost:8080".to_string(),
            weight: 1,
            max_conns: 100,
            location: None,
            region: None,
        };

        let backend = Backend::new(server);

        // Start as healthy
        assert!(backend.is_healthy());
        assert_eq!(backend.status(), HealthStatus::Healthy);

        // Mark failures (threshold 3)
        backend.mark_failure(3);
        assert!(backend.is_healthy()); // Still healthy (1 failure)

        backend.mark_failure(3);
        assert!(backend.is_healthy()); // Still healthy (2 failures)

        backend.mark_failure(3);
        assert!(!backend.is_healthy()); // Now unhealthy (3 failures)
        assert_eq!(backend.status(), HealthStatus::Unhealthy);

        // Mark successes (threshold 2)
        backend.mark_success(2);
        assert!(!backend.is_healthy()); // Still unhealthy (1 success)

        backend.mark_success(2);
        assert!(backend.is_healthy()); // Now healthy again (2 successes)
    }

    #[test]
    fn test_backend_counters_reset() {
        let server = ServerDef {
            url: "http://localhost:8080".to_string(),
            weight: 1,
            max_conns: 100,
            location: None,
            region: None,
        };

        let backend = Backend::new(server);

        // Mark some failures
        backend.mark_failure(3);
        backend.mark_failure(3);
        assert_eq!(backend.consecutive_failures(), 2);
        assert_eq!(backend.consecutive_successes(), 0);

        // One success should reset failure counter
        backend.mark_success(2);
        assert_eq!(backend.consecutive_successes(), 1);
        assert_eq!(backend.consecutive_failures(), 0);

        // Mark some successes
        backend.mark_success(2);
        assert_eq!(backend.consecutive_successes(), 2);

        // One failure should reset success counter
        backend.mark_failure(3);
        assert_eq!(backend.consecutive_failures(), 1);
        assert_eq!(backend.consecutive_successes(), 0);
    }
}
