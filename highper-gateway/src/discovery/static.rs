// Copyright 2024-2026 Highper Gateway Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Static Service Discovery
//!
//! Simple service discovery using a static list of backends configured in TOML/DSL.
//! Useful for simple deployments or when service discovery infrastructure is not available.

use super::{DiscoveryConfig, HealthStatus, ServiceDiscovery, ServiceInstance};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Static backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticBackend {
    /// Backend ID (unique identifier)
    pub id: String,

    /// Service name (for grouping backends)
    #[serde(default)]
    pub service_name: String,

    /// Backend address (IP or hostname)
    pub address: String,

    /// Backend port
    pub port: u16,

    /// Tags for filtering
    #[serde(default)]
    pub tags: Vec<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Initial health status (defaults to Passing)
    #[serde(default = "default_health_passing")]
    pub health: HealthStatus,
}

fn default_health_passing() -> HealthStatus {
    HealthStatus::Passing
}

/// Static service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticDiscoveryConfig {
    /// List of static backends
    pub backends: Vec<StaticBackend>,

    /// Enable health checking (TCP connect probes)
    #[serde(default = "default_true")]
    pub health_check_enabled: bool,

    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,

    /// Health check timeout in seconds
    #[serde(default = "default_health_check_timeout")]
    pub health_check_timeout: u64,
}

fn default_true() -> bool {
    true
}

fn default_health_check_interval() -> u64 {
    10
}

fn default_health_check_timeout() -> u64 {
    3
}

/// Static service discovery implementation
#[derive(Debug)]
pub struct StaticDiscovery {
    /// Static backends grouped by service name
    services: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,

    /// Configuration
    config: StaticDiscoveryConfig,
}

impl StaticDiscovery {
    /// Create a new static service discovery instance
    pub async fn new(config: StaticDiscoveryConfig) -> Result<Self> {
        if config.backends.is_empty() {
            return Err(anyhow!("Static discovery requires at least one backend"));
        }

        let mut services: HashMap<String, Vec<ServiceInstance>> = HashMap::new();

        // Group backends by service name
        for backend in &config.backends {
            let service_name = if backend.service_name.is_empty() {
                "default".to_string()
            } else {
                backend.service_name.clone()
            };

            let instance = ServiceInstance {
                id: backend.id.clone(),
                name: service_name.clone(),
                address: backend.address.clone(),
                port: backend.port,
                health: backend.health,
                tags: backend.tags.clone(),
                metadata: backend.metadata.clone(),
            };

            services.entry(service_name).or_insert_with(Vec::new).push(instance);
        }

        let discovery = Self {
            services: Arc::new(RwLock::new(services)),
            config,
        };

        // Start health checking if enabled
        if discovery.config.health_check_enabled {
            discovery.start_health_checker().await;
        }

        Ok(discovery)
    }

    /// Create from base discovery config with static backends
    pub async fn from_discovery_config(
        config: DiscoveryConfig,
        static_config: StaticDiscoveryConfig,
    ) -> Result<Self> {
        Self::new(static_config).await
    }

    /// Start background health checker
    async fn start_health_checker(&self) {
        let services = Arc::clone(&self.services);
        let interval = self.config.health_check_interval;
        let timeout = self.config.health_check_timeout;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(
                tokio::time::Duration::from_secs(interval)
            );

            loop {
                interval_timer.tick().await;

                let services_snapshot = {
                    let services_guard = services.read().await;
                    services_guard.clone()
                };

                for (service_name, instances) in services_snapshot {
                    for instance in instances {
                        // Perform TCP health check
                        let health_status = match tokio::time::timeout(
                            tokio::time::Duration::from_secs(timeout),
                            tokio::net::TcpStream::connect(format!("{}:{}", instance.address, instance.port))
                        ).await {
                            Ok(Ok(_)) => HealthStatus::Passing,
                            Ok(Err(_)) => HealthStatus::Critical,
                            Err(_) => HealthStatus::Critical, // Timeout
                        };

                        // Update health status
                        let mut services_guard = services.write().await;
                        if let Some(instances) = services_guard.get_mut(&service_name) {
                            if let Some(inst) = instances.iter_mut().find(|i| i.id == instance.id) {
                                if inst.health != health_status {
                                    tracing::info!(
                                        "Static discovery: Health status changed for {} ({}:{}) - {:?} -> {:?}",
                                        instance.id,
                                        instance.address,
                                        instance.port,
                                        inst.health,
                                        health_status
                                    );
                                    inst.health = health_status;
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

#[async_trait]
impl ServiceDiscovery for StaticDiscovery {
    async fn get_service_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let services = self.services.read().await;

        match services.get(service_name) {
            Some(instances) => Ok(instances.clone()),
            None => {
                // Try to find services with matching prefix or default
                if let Some(instances) = services.get("default") {
                    Ok(instances.clone())
                } else {
                    Err(anyhow!("Service '{}' not found in static discovery", service_name))
                }
            }
        }
    }

    async fn get_healthy_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let instances = self.get_service_instances(service_name).await?;

        Ok(instances
            .into_iter()
            .filter(|i| i.health == HealthStatus::Passing)
            .collect())
    }

    async fn register_service(&self, instance: ServiceInstance) -> Result<()> {
        let mut services = self.services.write().await;

        services
            .entry(instance.name.clone())
            .or_insert_with(Vec::new)
            .push(instance);

        Ok(())
    }

    async fn deregister_service(&self, service_id: &str) -> Result<()> {
        let mut services = self.services.write().await;

        for instances in services.values_mut() {
            instances.retain(|i| i.id != service_id);
        }

        Ok(())
    }

    async fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()> {
        let mut services = self.services.write().await;

        for instances in services.values_mut() {
            if let Some(instance) = instances.iter_mut().find(|i| i.id == service_id) {
                instance.health = status;
                return Ok(());
            }
        }

        Err(anyhow!("Service instance '{}' not found", service_id))
    }

    async fn watch_service(&self, _service_name: &str) -> Result<()> {
        // Static discovery doesn't support watching (no changes expected)
        tracing::debug!("Static discovery does not support watch_service (backends are static)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_backend(id: &str, service: &str, address: &str, port: u16) -> StaticBackend {
        StaticBackend {
            id: id.to_string(),
            service_name: service.to_string(),
            address: address.to_string(),
            port,
            tags: vec!["test".to_string()],
            metadata: HashMap::new(),
            health: HealthStatus::Passing,
        }
    }

    #[tokio::test]
    async fn test_static_discovery_basic() {
        let config = StaticDiscoveryConfig {
            backends: vec![
                create_test_backend("backend-1", "test-service", "192.168.1.10", 8080),
                create_test_backend("backend-2", "test-service", "192.168.1.11", 8080),
            ],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Get all instances
        let instances = discovery.get_service_instances("test-service").await.unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].address, "192.168.1.10");
        assert_eq!(instances[1].address, "192.168.1.11");
    }

    #[tokio::test]
    async fn test_static_discovery_multiple_services() {
        let config = StaticDiscoveryConfig {
            backends: vec![
                create_test_backend("user-1", "user-service", "192.168.1.10", 8080),
                create_test_backend("user-2", "user-service", "192.168.1.11", 8080),
                create_test_backend("order-1", "order-service", "192.168.1.20", 9000),
            ],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Get user-service instances
        let user_instances = discovery.get_service_instances("user-service").await.unwrap();
        assert_eq!(user_instances.len(), 2);

        // Get order-service instances
        let order_instances = discovery.get_service_instances("order-service").await.unwrap();
        assert_eq!(order_instances.len(), 1);
        assert_eq!(order_instances[0].port, 9000);
    }

    #[tokio::test]
    async fn test_static_discovery_healthy_instances() {
        let mut config = StaticDiscoveryConfig {
            backends: vec![
                create_test_backend("backend-1", "test-service", "192.168.1.10", 8080),
                create_test_backend("backend-2", "test-service", "192.168.1.11", 8080),
            ],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        // Make one backend unhealthy
        config.backends[1].health = HealthStatus::Critical;

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Get only healthy instances
        let healthy = discovery.get_healthy_instances("test-service").await.unwrap();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].id, "backend-1");
    }

    #[tokio::test]
    async fn test_static_discovery_register_deregister() {
        let config = StaticDiscoveryConfig {
            backends: vec![create_test_backend("backend-1", "test-service", "192.168.1.10", 8080)],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Register new service
        let new_instance = ServiceInstance {
            id: "backend-2".to_string(),
            name: "test-service".to_string(),
            address: "192.168.1.11".to_string(),
            port: 8080,
            health: HealthStatus::Passing,
            tags: vec![],
            metadata: HashMap::new(),
        };

        discovery.register_service(new_instance).await.unwrap();

        // Verify registration
        let instances = discovery.get_service_instances("test-service").await.unwrap();
        assert_eq!(instances.len(), 2);

        // Deregister service
        discovery.deregister_service("backend-2").await.unwrap();

        // Verify deregistration
        let instances = discovery.get_service_instances("test-service").await.unwrap();
        assert_eq!(instances.len(), 1);
    }

    #[tokio::test]
    async fn test_static_discovery_update_health() {
        let config = StaticDiscoveryConfig {
            backends: vec![create_test_backend("backend-1", "test-service", "192.168.1.10", 8080)],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Update health status
        discovery.update_health("backend-1", HealthStatus::Critical).await.unwrap();

        // Verify health updated
        let instances = discovery.get_service_instances("test-service").await.unwrap();
        assert_eq!(instances[0].health, HealthStatus::Critical);

        // Get healthy instances (should be empty)
        let healthy = discovery.get_healthy_instances("test-service").await.unwrap();
        assert_eq!(healthy.len(), 0);
    }

    #[tokio::test]
    async fn test_static_discovery_default_service() {
        let config = StaticDiscoveryConfig {
            backends: vec![
                StaticBackend {
                    id: "backend-1".to_string(),
                    service_name: "".to_string(), // Empty service name -> "default"
                    address: "192.168.1.10".to_string(),
                    port: 8080,
                    tags: vec![],
                    metadata: HashMap::new(),
                    health: HealthStatus::Passing,
                },
            ],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let discovery = StaticDiscovery::new(config).await.unwrap();

        // Should be accessible via "default" service name
        let instances = discovery.get_service_instances("default").await.unwrap();
        assert_eq!(instances.len(), 1);

        // Unknown service should fall back to "default"
        let instances = discovery.get_service_instances("unknown-service").await.unwrap();
        assert_eq!(instances.len(), 1);
    }

    #[tokio::test]
    async fn test_static_discovery_empty_backends() {
        let config = StaticDiscoveryConfig {
            backends: vec![],
            health_check_enabled: false,
            health_check_interval: 10,
            health_check_timeout: 3,
        };

        let result = StaticDiscovery::new(config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one backend"));
    }

    #[tokio::test]
    async fn test_service_instance_endpoint() {
        let backend = create_test_backend("test-1", "test-service", "192.168.1.100", 8080);

        let instance = ServiceInstance {
            id: backend.id,
            name: backend.service_name,
            address: backend.address,
            port: backend.port,
            health: backend.health,
            tags: backend.tags,
            metadata: backend.metadata,
        };

        assert_eq!(instance.endpoint(), "192.168.1.100:8080");
        assert_eq!(instance.url("http"), "http://192.168.1.100:8080");
    }
}
