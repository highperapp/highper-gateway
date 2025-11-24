//! Consul service discovery
//!
//! Integration with HashiCorp Consul for service discovery and health checking

use anyhow::{Context, Result};
use async_trait::async_trait;
use consul::{Client, Config};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{DiscoveryConfig, HealthStatus, ServiceDiscovery, ServiceInstance};

/// Consul-based service discovery
pub struct ConsulDiscovery {
    client: Client,
    config: DiscoveryConfig,
    cache: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
}

impl ConsulDiscovery {
    /// Create a new Consul discovery client
    pub async fn new(config: DiscoveryConfig) -> Result<Self> {
        // Use first address or default to localhost
        let address = config
            .addresses
            .first()
            .map(|s| s.as_str())
            .unwrap_or("http://127.0.0.1:8500");

        let consul_config = Config {
            address: address.to_string(),
            token: None,
        };

        let client = Client::new(consul_config);

        let discovery = Self {
            client,
            config: config.clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start background refresh task
        if let Some(service_name) = &config.service_name {
            let cache = discovery.cache.clone();
            let client = discovery.client.clone();
            let service = service_name.clone();
            let interval = config.refresh_interval;
            let only_healthy = config.only_healthy;

            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(interval));
                loop {
                    ticker.tick().await;

                    match Self::fetch_services(&client, &service, only_healthy).await {
                        Ok(instances) => {
                            let mut cache_write = cache.write().await;
                            cache_write.insert(service.clone(), instances);
                            debug!("Refreshed Consul service cache for {}", service);
                        }
                        Err(e) => {
                            error!("Failed to refresh Consul cache for {}: {}", service, e);
                        }
                    }
                }
            });
        }

        Ok(discovery)
    }

    async fn fetch_services(
        client: &Client,
        service_name: &str,
        only_healthy: bool,
    ) -> Result<Vec<ServiceInstance>> {
        let services = client
            .service(service_name, None, only_healthy)
            .await
            .context("Failed to query Consul services")?;

        let instances: Vec<ServiceInstance> = services
            .iter()
            .map(|service| {
                let health = if service.checks.iter().all(|c| c.status == "passing") {
                    HealthStatus::Passing
                } else if service.checks.iter().any(|c| c.status == "critical") {
                    HealthStatus::Critical
                } else if service.checks.iter().any(|c| c.status == "warning") {
                    HealthStatus::Warning
                } else {
                    HealthStatus::Unknown
                };

                ServiceInstance {
                    id: service.service.id.clone(),
                    name: service.service.service.clone(),
                    address: service.service.address.clone(),
                    port: service.service.port,
                    health,
                    tags: service.service.tags.clone(),
                    metadata: service.service.meta.clone().unwrap_or_default(),
                }
            })
            .collect();

        Ok(instances)
    }
}

#[async_trait]
impl ServiceDiscovery for ConsulDiscovery {
    async fn get_service_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        // Check cache first
        {
            let cache_read = self.cache.read().await;
            if let Some(instances) = cache_read.get(service_name) {
                debug!("Returning cached Consul instances for {}", service_name);
                return Ok(instances.clone());
            }
        }

        // Fetch from Consul
        info!("Fetching service instances from Consul: {}", service_name);
        let instances = Self::fetch_services(&self.client, service_name, false).await?;

        // Update cache
        {
            let mut cache_write = self.cache.write().await;
            cache_write.insert(service_name.to_string(), instances.clone());
        }

        Ok(instances)
    }

    async fn get_healthy_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let instances = self.get_service_instances(service_name).await?;
        Ok(instances
            .into_iter()
            .filter(|i| i.health == HealthStatus::Passing)
            .collect())
    }

    async fn register_service(&self, instance: ServiceInstance) -> Result<()> {
        info!("Registering service with Consul: {}", instance.id);

        let registration = consul::types::RegisterRequest {
            id: Some(instance.id.clone()),
            name: instance.name.clone(),
            address: Some(instance.address.clone()),
            port: Some(instance.port),
            tags: Some(instance.tags.clone()),
            meta: if instance.metadata.is_empty() {
                None
            } else {
                Some(instance.metadata.clone())
            },
            check: None,
            checks: None,
            enable_tag_override: None,
        };

        self.client
            .register(&registration)
            .await
            .context("Failed to register service with Consul")?;

        info!("Successfully registered service: {}", instance.id);
        Ok(())
    }

    async fn deregister_service(&self, service_id: &str) -> Result<()> {
        info!("Deregistering service from Consul: {}", service_id);

        self.client
            .deregister(service_id)
            .await
            .context("Failed to deregister service from Consul")?;

        info!("Successfully deregistered service: {}", service_id);
        Ok(())
    }

    async fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()> {
        debug!("Updating health status for {}: {:?}", service_id, status);

        let check_status = match status {
            HealthStatus::Passing => "passing",
            HealthStatus::Warning => "warning",
            HealthStatus::Critical => "critical",
            HealthStatus::Unknown => "unknown",
        };

        // Update TTL check
        let check_id = format!("service:{}", service_id);
        self.client
            .pass_ttl(&check_id, Some(check_status))
            .await
            .context("Failed to update health check")?;

        Ok(())
    }

    async fn watch_service(&self, service_name: &str) -> Result<()> {
        info!("Watching Consul service: {}", service_name);
        // Consul watch is implemented via the background refresh task
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consul_discovery_creation() {
        let config = DiscoveryConfig {
            backend_type: super::super::DiscoveryBackend::Consul,
            addresses: vec!["http://localhost:8500".to_string()],
            refresh_interval: 30,
            service_name: None,
            health_check_enabled: true,
            only_healthy: true,
        };

        // This will fail if Consul is not running, but tests basic construction
        let result = ConsulDiscovery::new(config).await;
        // We don't assert success since Consul may not be running in test env
    }
}
