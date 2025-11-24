//! Service Discovery
//!
//! Integrates with Consul and etcd for dynamic service discovery and health checking

#[cfg(feature = "consul")]
pub mod consul;
#[cfg(feature = "etcd-client")]
pub mod etcd;
pub mod registry;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "consul")]
pub use consul::ConsulDiscovery;
#[cfg(feature = "etcd-client")]
pub use etcd::EtcdDiscovery;
pub use registry::ServiceRegistry;

/// Service instance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    /// Service ID
    pub id: String,

    /// Service name
    pub name: String,

    /// Instance address
    pub address: String,

    /// Instance port
    pub port: u16,

    /// Health check status
    pub health: HealthStatus,

    /// Metadata tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ServiceInstance {
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }

    pub fn url(&self, scheme: &str) -> String {
        format!("{}://{}:{}", scheme, self.address, self.port)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Passing,
    Warning,
    Critical,
    Unknown,
}

/// Service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Discovery backend type
    #[serde(rename = "type")]
    pub backend_type: DiscoveryBackend,

    /// Backend addresses
    pub addresses: Vec<String>,

    /// Refresh interval in seconds
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,

    /// Service name to discover
    pub service_name: Option<String>,

    /// Enable health checking
    #[serde(default = "default_true")]
    pub health_check_enabled: bool,

    /// Only return healthy instances
    #[serde(default = "default_true")]
    pub only_healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryBackend {
    Consul,
    Etcd,
    Static,
}

fn default_refresh_interval() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

/// Service discovery trait
#[async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Get all instances of a service
    async fn get_service_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;

    /// Get healthy instances of a service
    async fn get_healthy_instances(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;

    /// Register a service instance
    async fn register_service(&self, instance: ServiceInstance) -> Result<()>;

    /// Deregister a service instance
    async fn deregister_service(&self, service_id: &str) -> Result<()>;

    /// Update service health status
    async fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()>;

    /// Watch for service changes (returns a stream of service updates)
    async fn watch_service(&self, service_name: &str) -> Result<()>;
}

/// Create a service discovery backend
pub async fn create_discovery(config: DiscoveryConfig) -> Result<Arc<dyn ServiceDiscovery>> {
    match config.backend_type {
        #[cfg(feature = "consul")]
        DiscoveryBackend::Consul => {
            let discovery = ConsulDiscovery::new(config).await?;
            Ok(Arc::new(discovery) as Arc<dyn ServiceDiscovery>)
        }
        #[cfg(not(feature = "consul"))]
        DiscoveryBackend::Consul => {
            Err(anyhow::anyhow!("Consul support not enabled. Enable the 'consul' feature to use Consul discovery."))
        }
        #[cfg(feature = "etcd-client")]
        DiscoveryBackend::Etcd => {
            let discovery = EtcdDiscovery::new(config).await?;
            Ok(Arc::new(discovery) as Arc<dyn ServiceDiscovery>)
        }
        #[cfg(not(feature = "etcd-client"))]
        DiscoveryBackend::Etcd => {
            Err(anyhow::anyhow!("etcd support not enabled. Enable the 'etcd-client' feature to use etcd discovery."))
        }
        DiscoveryBackend::Static => {
            Err(anyhow::anyhow!("Static discovery not yet implemented"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_instance_endpoint() {
        let instance = ServiceInstance {
            id: "test-1".to_string(),
            name: "test-service".to_string(),
            address: "192.168.1.100".to_string(),
            port: 8080,
            health: HealthStatus::Passing,
            tags: vec![],
            metadata: HashMap::new(),
        };

        assert_eq!(instance.endpoint(), "192.168.1.100:8080");
        assert_eq!(instance.url("http"), "http://192.168.1.100:8080");
    }

    #[test]
    fn test_health_status() {
        assert_eq!(HealthStatus::Passing, HealthStatus::Passing);
        assert_ne!(HealthStatus::Passing, HealthStatus::Critical);
    }
}
