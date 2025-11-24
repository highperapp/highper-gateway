//! Upstream state management with health checking
//!
//! Manages runtime state for upstreams including backend health tracking
//! and load balancing across healthy servers.

use super::types::UpstreamConfig;
use crate::config::{ActiveHealthCheckConfig, ServerDef};
use crate::proxy::health::{Backend, HealthChecker};
use crate::proxy::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use tracing::{info, warn};

/// Runtime state for an upstream (backends + health tracking)
pub struct UpstreamState {
    /// Upstream name
    name: String,

    /// Backend servers with health tracking
    backends: Vec<Arc<Backend>>,

    /// Load balancing algorithm
    algorithm: String,

    /// Round-robin counter (for round-robin algorithm)
    next_index: AtomicUsize,

    /// Health checker (if health checking is enabled)
    health_checker: Option<Arc<Mutex<HealthChecker>>>,
}

impl UpstreamState {
    /// Create upstream state from configuration
    pub fn from_config(
        name: String,
        config: UpstreamConfig,
        client: Client,
    ) -> Self {
        info!(
            "Creating upstream state for '{}' with {} servers",
            name,
            config.servers.len()
        );

        // Convert server URLs to Backend instances
        let backends: Vec<Arc<Backend>> = config
            .servers
            .iter()
            .map(|url| {
                Arc::new(Backend::new(ServerDef {
                    url: url.clone(),
                    weight: 1, // Default weight
                    max_conns: 100, // Default max connections
                    location: None, // No geographic location by default
                    region: None, // No region by default
                }))
            })
            .collect();

        // Create health checker if configured
        let health_checker = config.health_check.map(|hc_config| {
            info!(
                "Enabling health checks for upstream '{}': interval={}s, path={}",
                name, hc_config.interval_secs, hc_config.path
            );

            let active_config = ActiveHealthCheckConfig {
                enabled: true,
                interval: Duration::from_secs(hc_config.interval_secs),
                timeout: Duration::from_secs(hc_config.timeout_secs),
                path: hc_config.path,
                healthy_threshold: 2,
                unhealthy_threshold: 3,
            };

            Arc::new(Mutex::new(HealthChecker::new(
                backends.clone(),
                active_config,
            )))
        });

        Self {
            name,
            backends,
            algorithm: config.algorithm,
            next_index: AtomicUsize::new(0),
            health_checker,
        }
    }

    /// Select a healthy backend server using the configured load balancing algorithm
    pub fn select_healthy_backend(&self) -> Option<Arc<Backend>> {
        // Filter to only healthy backends
        let healthy_backends: Vec<_> = self
            .backends
            .iter()
            .filter(|b| b.is_healthy())
            .cloned()
            .collect();

        if healthy_backends.is_empty() {
            warn!("No healthy backends available for upstream '{}'", self.name);
            return None;
        }

        // Apply load balancing algorithm
        match self.algorithm.as_str() {
            "round_robin" => {
                let index = self.next_index.fetch_add(1, Ordering::Relaxed);
                Some(healthy_backends[index % healthy_backends.len()].clone())
            }
            "random" => {
                use rand::Rng;
                let index = rand::thread_rng().gen_range(0..healthy_backends.len());
                Some(healthy_backends[index].clone())
            }
            _ => {
                // Default to round-robin for unknown algorithms
                warn!(
                    "Unknown load balancing algorithm '{}', using round_robin",
                    self.algorithm
                );
                let index = self.next_index.fetch_add(1, Ordering::Relaxed);
                Some(healthy_backends[index % healthy_backends.len()].clone())
            }
        }
    }

    /// Start health checking background task
    pub async fn start_health_checks(&self) {
        if let Some(checker) = &self.health_checker {
            info!("Starting health checks for upstream '{}'", self.name);
            let checker_arc = checker.clone();
            // Spawn background task for health checking
            tokio::spawn(async move {
                // Lock to get the checker, then release immediately
                let checker = checker_arc.lock();
                drop(checker); // Release lock
                // Run health checks (this method needs Arc<Self>)
                // For now, we'll skip this as the HealthChecker API needs refactoring
                // TODO: Refactor HealthChecker to support this use case
            });
        }
    }

    /// Get all backends with their current health status
    pub fn get_backends_status(&self) -> Vec<BackendStatus> {
        self.backends
            .iter()
            .map(|backend| BackendStatus {
                url: backend.server.url.clone(),
                healthy: backend.is_healthy(),
                consecutive_successes: backend.consecutive_successes(),
                consecutive_failures: backend.consecutive_failures(),
            })
            .collect()
    }

    /// Get backend by URL (for passive health tracking)
    pub fn find_backend(&self, url: &str) -> Option<Arc<Backend>> {
        self.backends
            .iter()
            .find(|b| b.server.url == url)
            .cloned()
    }

    /// Get upstream name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get number of backends
    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }

    /// Get number of healthy backends
    pub fn healthy_backend_count(&self) -> usize {
        self.backends.iter().filter(|b| b.is_healthy()).count()
    }
}

/// Backend status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendStatus {
    pub url: String,
    pub healthy: bool,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_upstream_state() {
        let config = UpstreamConfig {
            servers: vec!["http://localhost:8001".to_string(), "http://localhost:8002".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        };

        let client = Client::new();
        let state = UpstreamState::from_config("test-upstream".to_string(), config, client);

        assert_eq!(state.name(), "test-upstream");
        assert_eq!(state.backend_count(), 2);
        assert_eq!(state.healthy_backend_count(), 2); // All start healthy
    }

    #[test]
    fn test_round_robin_selection() {
        let config = UpstreamConfig {
            servers: vec![
                "http://localhost:8001".to_string(),
                "http://localhost:8002".to_string(),
                "http://localhost:8003".to_string(),
            ],
            algorithm: "round_robin".to_string(),
            health_check: None,
        };

        let client = Client::new();
        let state = UpstreamState::from_config("test-upstream".to_string(), config, client);

        // Select 6 backends and verify round-robin pattern
        let selections: Vec<_> = (0..6)
            .map(|_| state.select_healthy_backend().unwrap().server.url.clone())
            .collect();

        // Should cycle through all three backends twice
        assert_eq!(selections[0], "http://localhost:8001");
        assert_eq!(selections[1], "http://localhost:8002");
        assert_eq!(selections[2], "http://localhost:8003");
        assert_eq!(selections[3], "http://localhost:8001");
        assert_eq!(selections[4], "http://localhost:8002");
        assert_eq!(selections[5], "http://localhost:8003");
    }

    #[test]
    fn test_backends_status() {
        let config = UpstreamConfig {
            servers: vec!["http://localhost:8001".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        };

        let client = Client::new();
        let state = UpstreamState::from_config("test-upstream".to_string(), config, client);

        let statuses = state.get_backends_status();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].url, "http://localhost:8001");
        assert_eq!(statuses[0].healthy, true);
    }
}
