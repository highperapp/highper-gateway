//! Service Registry
//!
//! Manages service instances discovered from Consul/etcd and integrates with load balancing
//!
//! Note: This module requires configuration types that are defined elsewhere.
//! The implementation is present but temporarily not actively used.

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::{create_discovery, DiscoveryConfig, ServiceDiscovery, ServiceInstance};

/// Service registry that integrates discovery with proxy upstreams
pub struct ServiceRegistry {
    discovery: Arc<dyn ServiceDiscovery>,
    upstreams: Arc<RwLock<DashMap<String, Vec<ServiceInstance>>>>,
    last_update: Arc<RwLock<DashMap<String, Instant>>>,
    config: DiscoveryConfig,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub async fn new(config: DiscoveryConfig) -> Result<Self> {
        let discovery = create_discovery(config.clone()).await?;

        Ok(Self {
            discovery,
            upstreams: Arc::new(RwLock::new(DashMap::new())),
            last_update: Arc::new(RwLock::new(DashMap::new())),
            config,
        })
    }

    /// Get upstream servers for a service
    pub async fn get_upstreams(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        // Check if we need to refresh - simplified to always refresh for now
        let should_refresh = true;

        if should_refresh {
            self.refresh_service(service_name).await?;
        }

        // Get from cache
        let upstreams = self.upstreams.read().await;
        upstreams
            .get(service_name)
            .map(|servers| servers.clone())
            .ok_or_else(|| anyhow::anyhow!("Service not found: {}", service_name))
    }

    /// Refresh service instances from discovery backend
    pub async fn refresh_service(&self, service_name: &str) -> Result<()> {
        info!("Refreshing service instances for: {}", service_name);

        let instances = if self.config.only_healthy {
            self.discovery.get_healthy_instances(service_name).await?
        } else {
            self.discovery.get_service_instances(service_name).await?
        };

        if instances.is_empty() {
            warn!("No instances found for service: {}", service_name);
            return Ok(());
        }

        // Service instances
        let servers = instances;

        info!(
            "Discovered {} servers for service: {}",
            servers.len(),
            service_name
        );

        // Update cache
        {
            let upstreams = self.upstreams.read().await;
            upstreams.insert(service_name.to_string(), servers);
        }

        {
            let last_update = self.last_update.read().await;
            last_update.insert(service_name.to_string(), Instant::now());
        }

        Ok(())
    }

    /// Register self as a service instance
    pub async fn register_self(&self, instance: ServiceInstance) -> Result<()> {
        info!("Registering self with service discovery: {}", instance.id);
        self.discovery.register_service(instance).await
    }

    /// Deregister self from service discovery
    pub async fn deregister_self(&self, service_id: &str) -> Result<()> {
        info!("Deregistering self from service discovery: {}", service_id);
        self.discovery.deregister_service(service_id).await
    }

    /// Start watching a service for changes
    pub async fn watch_service(&self, service_name: &str) -> Result<()> {
        info!("Starting watch for service: {}", service_name);

        self.discovery.watch_service(service_name).await?;

        // Also start periodic refresh as backup
        let registry = Arc::new(self.clone());
        let service = service_name.to_string();

        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(Duration::from_secs(registry.config.refresh_interval));
            loop {
                ticker.tick().await;
                if let Err(e) = registry.refresh_service(&service).await {
                    error!("Failed to refresh service {}: {}", service, e);
                }
            }
        });

        Ok(())
    }

    /// Get all service names
    pub async fn get_service_names(&self) -> Vec<String> {
        let upstreams = self.upstreams.read().await;
        upstreams.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Clear cache for a service
    pub async fn clear_cache(&self, service_name: &str) {
        let upstreams = self.upstreams.read().await;
        upstreams.remove(service_name);

        let last_update = self.last_update.read().await;
        last_update.remove(service_name);
    }
}

impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            discovery: self.discovery.clone(),
            upstreams: self.upstreams.clone(),
            last_update: self.last_update.clone(),
            config: self.config.clone(),
        }
    }
}

// Upstream conversion logic omitted - requires integration with main config types

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::DiscoveryBackend;

    #[tokio::test]
    async fn test_service_registry_creation() {
        let config = DiscoveryConfig {
            backend_type: DiscoveryBackend::Consul,
            addresses: vec!["http://localhost:8500".to_string()],
            refresh_interval: 30,
            service_name: Some("test-service".to_string()),
            health_check_enabled: true,
            only_healthy: true,
            static_backends: vec![],
            static_health_check_interval: 10,
            static_health_check_timeout: 3,
        };

        // This will fail if Consul is not running
        let _result = ServiceRegistry::new(config).await;
        // We don't assert since discovery backend may not be available
    }
}
