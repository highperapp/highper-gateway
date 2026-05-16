//! etcd service discovery
//!
//! Integration with etcd for service discovery and configuration management

use anyhow::{Context, Result};
use async_trait::async_trait;
use etcd_client::{Client, GetOptions, PutOptions};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{DiscoveryConfig, HealthStatus, ServiceDiscovery, ServiceInstance};

/// etcd-based service discovery
pub struct EtcdDiscovery {
    client: Client,
    config: DiscoveryConfig,
    cache: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
    key_prefix: String,
}

impl EtcdDiscovery {
    /// Create a new etcd discovery client
    pub async fn new(config: DiscoveryConfig) -> Result<Self> {
        let endpoints = if config.addresses.is_empty() {
            vec!["http://127.0.0.1:2379".to_string()]
        } else {
            config.addresses.clone()
        };

        let client = Client::connect(&endpoints, None)
            .await
            .context("Failed to connect to etcd")?;

        let discovery = Self {
            client,
            config: config.clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            key_prefix: "/services".to_string(),
        };

        // Start background refresh task
        if let Some(service_name) = &config.service_name {
            let cache = discovery.cache.clone();
            let mut client = discovery.client.clone();
            let service = service_name.clone();
            let interval = config.refresh_interval;
            let key_prefix = discovery.key_prefix.clone();

            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(interval));
                loop {
                    ticker.tick().await;

                    match Self::fetch_services(&mut client, &key_prefix, &service).await {
                        Ok(instances) => {
                            let mut cache_write = cache.write().await;
                            cache_write.insert(service.clone(), instances);
                            debug!("Refreshed etcd service cache for {}", service);
                        }
                        Err(e) => {
                            error!("Failed to refresh etcd cache for {}: {}", service, e);
                        }
                    }
                }
            });
        }

        Ok(discovery)
    }

    async fn fetch_services(
        client: &mut Client,
        key_prefix: &str,
        service_name: &str,
    ) -> Result<Vec<ServiceInstance>> {
        let key = format!("{}/{}/", key_prefix, service_name);

        let options = GetOptions::new().with_prefix();
        let response = client
            .get(key.clone(), Some(options))
            .await
            .context("Failed to query etcd services")?;

        let mut instances = Vec::new();

        for kv in response.kvs() {
            let value = kv.value_str().context("Invalid UTF-8 in etcd value")?;

            // Parse service instance from JSON
            match serde_json::from_str::<ServiceInstance>(value) {
                Ok(instance) => instances.push(instance),
                Err(e) => {
                    warn!("Failed to parse service instance from etcd: {}", e);
                }
            }
        }

        Ok(instances)
    }
}

#[async_trait]
impl ServiceDiscovery for EtcdDiscovery {
    async fn get_service_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        // Check cache first
        {
            let cache_read = self.cache.read().await;
            if let Some(instances) = cache_read.get(service_name) {
                debug!("Returning cached etcd instances for {}", service_name);
                return Ok(instances.clone());
            }
        }

        // Fetch from etcd
        info!("Fetching service instances from etcd: {}", service_name);
        let mut client = self.client.clone();
        let instances = Self::fetch_services(&mut client, &self.key_prefix, service_name).await?;

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
        info!("Registering service with etcd: {}", instance.id);

        let key = format!("{}/{}/{}", self.key_prefix, instance.name, instance.id);
        let value =
            serde_json::to_string(&instance).context("Failed to serialize service instance")?;

        let mut client = self.client.clone();
        client
            .put(key.clone(), value.clone(), None)
            .await
            .context("Failed to register service with etcd")?;

        // Set a TTL lease for automatic cleanup
        let lease_grant_response = client
            .lease_grant(60, None)
            .await
            .context("Failed to create lease")?;

        let lease_id = lease_grant_response.id();

        let options = PutOptions::new().with_lease(lease_id);
        client
            .put(key, value, Some(options))
            .await
            .context("Failed to register service with lease")?;

        // Start lease keep-alive
        let (mut keeper, mut stream) = client
            .lease_keep_alive(lease_id)
            .await
            .context("Failed to start lease keep-alive")?;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(20));
            loop {
                ticker.tick().await;
                if let Err(e) = keeper.keep_alive().await {
                    error!("Failed to keep lease alive: {}", e);
                    break;
                }
                // Drain responses
                while let Ok(Some(_)) = stream.message().await {}
            }
        });

        info!("Successfully registered service: {}", instance.id);
        Ok(())
    }

    async fn deregister_service(&self, service_id: &str) -> Result<()> {
        info!("Deregistering service from etcd: {}", service_id);

        // Find and delete the key
        let key_pattern = format!("{}/*/{}", self.key_prefix, service_id);

        let options = GetOptions::new().with_prefix();
        let mut client = self.client.clone();
        let response = client
            .get(key_pattern, Some(options))
            .await
            .context("Failed to find service in etcd")?;

        for kv in response.kvs() {
            let key = kv.key_str().context("Invalid UTF-8 in etcd key")?;
            client
                .delete(key, None)
                .await
                .context("Failed to delete service from etcd")?;
        }

        info!("Successfully deregistered service: {}", service_id);
        Ok(())
    }

    async fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()> {
        debug!("Updating health status for {}: {:?}", service_id, status);

        // Find the service instance
        let key_pattern = format!("{}/*/{}", self.key_prefix, service_id);

        let options = GetOptions::new().with_prefix();
        let mut client = self.client.clone();
        let response = client
            .get(key_pattern, Some(options))
            .await
            .context("Failed to find service in etcd")?;

        for kv in response.kvs() {
            let value = kv.value_str().context("Invalid UTF-8 in etcd value")?;
            let mut instance: ServiceInstance =
                serde_json::from_str(value).context("Failed to parse service instance")?;

            instance.health = status;

            let key = kv.key_str().context("Invalid UTF-8 in etcd key")?;
            let updated_value =
                serde_json::to_string(&instance).context("Failed to serialize updated instance")?;

            client
                .put(key, updated_value, None)
                .await
                .context("Failed to update health status in etcd")?;
        }

        Ok(())
    }

    async fn watch_service(&self, service_name: &str) -> Result<()> {
        info!("Watching etcd service: {}", service_name);

        let key = format!("{}/{}/", self.key_prefix, service_name);
        let mut client = self.client.clone();
        let cache = self.cache.clone();
        let service = service_name.to_string();
        let key_prefix = self.key_prefix.clone();

        tokio::spawn(async move {
            let options = etcd_client::WatchOptions::new().with_prefix();
            let (mut watcher, mut stream) = match client.watch(key, Some(options)).await {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create etcd watcher: {}", e);
                    return;
                }
            };

            while let Ok(Some(response)) = stream.message().await {
                debug!("Received etcd watch event for {}", service);

                // Refresh cache on any change
                match Self::fetch_services(&mut client, &key_prefix, &service).await {
                    Ok(instances) => {
                        let mut cache_write = cache.write().await;
                        cache_write.insert(service.clone(), instances);
                        info!("Updated service cache from etcd watch event");
                    }
                    Err(e) => {
                        error!("Failed to refresh cache on watch event: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_etcd_discovery_creation() {
        let config = DiscoveryConfig {
            backend_type: super::super::DiscoveryBackend::Etcd,
            addresses: vec!["http://localhost:2379".to_string()],
            refresh_interval: 30,
            service_name: None,
            health_check_enabled: true,
            only_healthy: true,
        };

        // This will fail if etcd is not running, but tests basic construction
        let result = EtcdDiscovery::new(config).await;
        // We don't assert success since etcd may not be running in test env
    }
}
