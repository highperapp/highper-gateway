//! Upstream management endpoints for Admin API
//!
//! Provides REST API for managing upstreams at runtime:
//! - List upstreams with details
//! - Create/update/delete upstreams
//! - Add/remove backend servers to upstreams
//! - Update load balancing configuration

use crate::admin::{BackendServer, HealthCheckConfig, LoadBalancingConfig, UpstreamDefinition};
use crate::config::Config;
use crate::state::ProxyState;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Upstream manager for runtime upstream updates
pub struct UpstreamManager {
    /// Runtime upstreams (additions/modifications)
    upstreams: Arc<RwLock<HashMap<String, UpstreamDefinition>>>,
    /// Reference to the main config for base upstreams
    config: Arc<RwLock<Config>>,
    /// Proxy state for live updates
    state: Option<Arc<ProxyState>>,
}

impl UpstreamManager {
    /// Create a new upstream manager
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            upstreams: Arc::new(RwLock::new(HashMap::new())),
            config,
            state: None,
        }
    }

    /// Create with proxy state for live updates
    pub fn with_state(config: Arc<RwLock<Config>>, state: Arc<ProxyState>) -> Self {
        Self {
            upstreams: Arc::new(RwLock::new(HashMap::new())),
            config,
            state: Some(state),
        }
    }

    /// List all upstreams (merged from config and runtime)
    pub async fn list_upstreams(&self) -> Vec<UpstreamSummary> {
        let config = self.config.read().await;
        let runtime_upstreams = self.upstreams.read().await;
        let mut result = Vec::new();

        // Add config-based upstreams
        for upstream in &config.upstreams {
            let server_count = upstream.servers.len();
            let healthy_count = self.count_healthy_servers(&upstream.name).await;

            result.push(UpstreamSummary {
                name: upstream.name.clone(),
                servers_count: server_count,
                healthy_count,
                algorithm: format!("{:?}", upstream.load_balancing.algorithm).to_lowercase(),
                health_check_enabled: upstream.health_check.active.enabled,
                source: UpstreamSource::Config,
            });
        }

        // Add runtime upstreams (overwrite if exists)
        for (name, upstream) in runtime_upstreams.iter() {
            let server_count = upstream.servers.len();
            let healthy_count = self.count_healthy_servers(name).await;

            // Check if already in result (from config)
            if let Some(existing) = result.iter_mut().find(|u| u.name == *name) {
                existing.servers_count = server_count;
                existing.healthy_count = healthy_count;
                existing.algorithm = upstream.load_balancing.algorithm.clone();
                existing.source = UpstreamSource::Runtime;
            } else {
                result.push(UpstreamSummary {
                    name: name.clone(),
                    servers_count: server_count,
                    healthy_count,
                    algorithm: upstream.load_balancing.algorithm.clone(),
                    health_check_enabled: upstream
                        .health_checks
                        .as_ref()
                        .map_or(false, |hc| hc.enabled),
                    source: UpstreamSource::Runtime,
                });
            }
        }

        result
    }

    /// Get upstream details
    pub async fn get_upstream(&self, name: &str) -> Option<UpstreamDefinition> {
        // First check runtime upstreams
        let runtime = self.upstreams.read().await;
        if let Some(upstream) = runtime.get(name) {
            return Some(upstream.clone());
        }

        // Then check config upstreams
        let config = self.config.read().await;
        config
            .upstreams
            .iter()
            .find(|u| u.name == name)
            .map(|u| UpstreamDefinition {
                name: u.name.clone(),
                servers: u
                    .servers
                    .iter()
                    .map(|s| BackendServer {
                        url: s.url.clone(),
                        weight: s.weight,
                        max_conns: Some(s.max_conns as u32),
                    })
                    .collect(),
                load_balancing: LoadBalancingConfig {
                    algorithm: format!("{:?}", u.load_balancing.algorithm).to_lowercase(),
                },
                health_checks: Some(HealthCheckConfig {
                    enabled: u.health_check.active.enabled,
                    interval: u.health_check.active.interval.as_secs(),
                    timeout: u.health_check.active.timeout.as_secs(),
                    healthy_threshold: u.health_check.active.healthy_threshold,
                    unhealthy_threshold: u.health_check.active.unhealthy_threshold,
                }),
            })
    }

    /// Create a new upstream
    pub async fn create_upstream(&self, upstream: UpstreamDefinition) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Check if already exists in runtime
        if upstreams.contains_key(&upstream.name) {
            return Err(format!(
                "Upstream '{}' already exists in runtime",
                upstream.name
            ));
        }

        // Check if exists in config
        let config = self.config.read().await;
        if config.upstreams.iter().any(|u| u.name == upstream.name) {
            return Err(format!(
                "Upstream '{}' already exists in config",
                upstream.name
            ));
        }
        drop(config);

        info!("Creating runtime upstream: {}", upstream.name);
        upstreams.insert(upstream.name.clone(), upstream);

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Update an existing upstream
    pub async fn update_upstream(
        &self,
        name: &str,
        upstream: UpstreamDefinition,
    ) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Check if exists
        let config = self.config.read().await;
        let exists_in_config = config.upstreams.iter().any(|u| u.name == name);
        drop(config);

        let exists_in_runtime = upstreams.contains_key(name);

        if !exists_in_config && !exists_in_runtime {
            return Err(format!("Upstream '{}' not found", name));
        }

        info!("Updating upstream: {} -> {}", name, upstream.name);

        // Remove old entry if name changed
        if name != upstream.name {
            upstreams.remove(name);
        }

        upstreams.insert(upstream.name.clone(), upstream);

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Delete an upstream
    pub async fn delete_upstream(&self, name: &str) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Can only delete runtime upstreams, not config ones
        let config = self.config.read().await;
        if config.upstreams.iter().any(|u| u.name == name) {
            return Err(format!(
                "Cannot delete config-defined upstream '{}'. Use config reload instead.",
                name
            ));
        }
        drop(config);

        if upstreams.remove(name).is_none() {
            return Err(format!("Upstream '{}' not found in runtime", name));
        }

        info!("Deleted runtime upstream: {}", name);

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Add a server to an upstream
    pub async fn add_server(
        &self,
        upstream_name: &str,
        server: BackendServer,
    ) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Get or create runtime upstream
        if !upstreams.contains_key(upstream_name) {
            // Copy from config if exists
            if let Some(upstream) = self.get_upstream(upstream_name).await {
                upstreams.insert(upstream_name.to_string(), upstream);
            } else {
                return Err(format!("Upstream '{}' not found", upstream_name));
            }
        }

        let upstream = upstreams
            .get_mut(upstream_name)
            .ok_or_else(|| format!("Upstream '{}' unexpectedly missing", upstream_name))?;

        // Check if server already exists
        if upstream.servers.iter().any(|s| s.url == server.url) {
            return Err(format!(
                "Server '{}' already exists in upstream '{}'",
                server.url, upstream_name
            ));
        }

        info!("Adding server {} to upstream {}", server.url, upstream_name);
        upstream.servers.push(server);

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Remove a server from an upstream
    pub async fn remove_server(&self, upstream_name: &str, server_url: &str) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Get or create runtime upstream
        if !upstreams.contains_key(upstream_name) {
            if let Some(upstream) = self.get_upstream(upstream_name).await {
                upstreams.insert(upstream_name.to_string(), upstream);
            } else {
                return Err(format!("Upstream '{}' not found", upstream_name));
            }
        }

        let upstream = upstreams
            .get_mut(upstream_name)
            .ok_or_else(|| format!("Upstream '{}' unexpectedly missing", upstream_name))?;

        let initial_len = upstream.servers.len();
        upstream.servers.retain(|s| s.url != server_url);

        if upstream.servers.len() == initial_len {
            return Err(format!(
                "Server '{}' not found in upstream '{}'",
                server_url, upstream_name
            ));
        }

        info!(
            "Removed server {} from upstream {}",
            server_url, upstream_name
        );

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Update load balancing configuration
    pub async fn update_load_balancing(
        &self,
        upstream_name: &str,
        lb_config: LoadBalancingConfig,
    ) -> Result<(), String> {
        let mut upstreams = self.upstreams.write().await;

        // Get or create runtime upstream
        if !upstreams.contains_key(upstream_name) {
            if let Some(upstream) = self.get_upstream(upstream_name).await {
                upstreams.insert(upstream_name.to_string(), upstream);
            } else {
                return Err(format!("Upstream '{}' not found", upstream_name));
            }
        }

        let upstream = upstreams
            .get_mut(upstream_name)
            .ok_or_else(|| format!("Upstream '{}' unexpectedly missing", upstream_name))?;
        upstream.load_balancing = lb_config;

        info!("Updated load balancing for upstream {}", upstream_name);

        // TODO: Trigger live update to proxy

        Ok(())
    }

    /// Count healthy servers in an upstream
    async fn count_healthy_servers(&self, _upstream_name: &str) -> usize {
        // TODO: Get from proxy state
        0
    }

    /// Export all runtime changes as config
    pub async fn export_config(&self) -> Vec<UpstreamDefinition> {
        let upstreams = self.upstreams.read().await;
        upstreams.values().cloned().collect()
    }
}

/// Upstream summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamSummary {
    /// Upstream name
    pub name: String,
    /// Number of backend servers
    pub servers_count: usize,
    /// Number of healthy servers
    pub healthy_count: usize,
    /// Load balancing algorithm
    pub algorithm: String,
    /// Health check enabled
    pub health_check_enabled: bool,
    /// Source of upstream definition
    pub source: UpstreamSource,
}

/// Source of upstream definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamSource {
    /// From configuration file
    Config,
    /// Created at runtime via API
    Runtime,
}

/// Request to add a server to an upstream
#[derive(Debug, Deserialize)]
pub struct AddServerRequest {
    /// Server URL
    pub url: String,
    /// Server weight
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Maximum connections
    pub max_conns: Option<u32>,
}

fn default_weight() -> u32 {
    1
}

/// Request to remove a server from an upstream
#[derive(Debug, Deserialize)]
pub struct RemoveServerRequest {
    /// Server URL to remove
    pub url: String,
}

/// Request to update load balancing
#[derive(Debug, Deserialize)]
pub struct UpdateLoadBalancingRequest {
    /// Algorithm name
    pub algorithm: String,
}

// HTTP handlers

/// List all upstreams
pub async fn list_upstreams_handler(manager: Arc<UpstreamManager>) -> Response<Full<Bytes>> {
    let upstreams = manager.list_upstreams().await;

    json_response(
        StatusCode::OK,
        json!({
            "upstreams": upstreams,
            "total": upstreams.len(),
        }),
    )
}

/// Get upstream details
pub async fn get_upstream_handler(
    manager: Arc<UpstreamManager>,
    name: &str,
) -> Response<Full<Bytes>> {
    match manager.get_upstream(name).await {
        Some(upstream) => match serde_json::to_value(upstream) {
            Ok(value) => json_response(StatusCode::OK, value),
            Err(e) => json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": e.to_string()}),
            ),
        },
        None => json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": format!("Upstream '{}' not found", name)
            }),
        ),
    }
}

/// Create upstream
pub async fn create_upstream_handler(
    manager: Arc<UpstreamManager>,
    upstream: UpstreamDefinition,
) -> Response<Full<Bytes>> {
    match manager.create_upstream(upstream.clone()).await {
        Ok(()) => json_response(
            StatusCode::CREATED,
            json!({
                "status": "ok",
                "message": format!("Upstream '{}' created successfully", upstream.name),
                "upstream": upstream,
            }),
        ),
        Err(e) => json_response(
            StatusCode::CONFLICT,
            json!({
                "error": e
            }),
        ),
    }
}

/// Update upstream
pub async fn update_upstream_handler(
    manager: Arc<UpstreamManager>,
    name: &str,
    upstream: UpstreamDefinition,
) -> Response<Full<Bytes>> {
    match manager.update_upstream(name, upstream.clone()).await {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Upstream '{}' updated successfully", name),
                "upstream": upstream,
            }),
        ),
        Err(e) => json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": e
            }),
        ),
    }
}

/// Delete upstream
pub async fn delete_upstream_handler(
    manager: Arc<UpstreamManager>,
    name: &str,
) -> Response<Full<Bytes>> {
    match manager.delete_upstream(name).await {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Upstream '{}' deleted successfully", name),
            }),
        ),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": e
            }),
        ),
    }
}

/// Add server to upstream
pub async fn add_server_handler(
    manager: Arc<UpstreamManager>,
    upstream_name: &str,
    request: AddServerRequest,
) -> Response<Full<Bytes>> {
    let server = BackendServer {
        url: request.url.clone(),
        weight: request.weight,
        max_conns: request.max_conns,
    };

    match manager.add_server(upstream_name, server).await {
        Ok(()) => json_response(
            StatusCode::CREATED,
            json!({
                "status": "ok",
                "message": format!("Server '{}' added to upstream '{}'", request.url, upstream_name),
            }),
        ),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": e
            }),
        ),
    }
}

/// Remove server from upstream
pub async fn remove_server_handler(
    manager: Arc<UpstreamManager>,
    upstream_name: &str,
    request: RemoveServerRequest,
) -> Response<Full<Bytes>> {
    match manager.remove_server(upstream_name, &request.url).await {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Server '{}' removed from upstream '{}'", request.url, upstream_name),
            }),
        ),
        Err(e) => json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": e
            }),
        ),
    }
}

/// Update load balancing configuration
pub async fn update_load_balancing_handler(
    manager: Arc<UpstreamManager>,
    upstream_name: &str,
    request: UpdateLoadBalancingRequest,
) -> Response<Full<Bytes>> {
    let lb_config = LoadBalancingConfig {
        algorithm: request.algorithm.clone(),
    };

    match manager
        .update_load_balancing(upstream_name, lb_config)
        .await
    {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Load balancing updated for upstream '{}'", upstream_name),
                "algorithm": request.algorithm,
            }),
        ),
        Err(e) => json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": e
            }),
        ),
    }
}

/// Helper function to create JSON response
fn json_response(status: StatusCode, body: serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .expect("response builder with valid status and content-type header")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn create_test_config() -> Arc<RwLock<Config>> {
        Arc::new(RwLock::new(Config {
            server: Default::default(),
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: Default::default(),
            websocket: Default::default(),
            grpc: Default::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }))
    }

    #[tokio::test]
    async fn test_create_upstream() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        let upstream = UpstreamDefinition {
            name: "test-upstream".to_string(),
            servers: vec![BackendServer {
                url: "http://localhost:8080".to_string(),
                weight: 1,
                max_conns: Some(100),
            }],
            load_balancing: LoadBalancingConfig {
                algorithm: "round_robin".to_string(),
            },
            health_checks: None,
        };

        let result = manager.create_upstream(upstream).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_duplicate_upstream() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        let upstream = UpstreamDefinition {
            name: "test-upstream".to_string(),
            servers: vec![],
            load_balancing: LoadBalancingConfig::default(),
            health_checks: None,
        };

        manager.create_upstream(upstream.clone()).await.unwrap();
        let result = manager.create_upstream(upstream).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_server() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        // Create upstream first
        let upstream = UpstreamDefinition {
            name: "test-upstream".to_string(),
            servers: vec![],
            load_balancing: LoadBalancingConfig::default(),
            health_checks: None,
        };
        manager.create_upstream(upstream).await.unwrap();

        // Add server
        let server = BackendServer {
            url: "http://localhost:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
        };

        let result = manager.add_server("test-upstream", server).await;
        assert!(result.is_ok());

        // Verify server was added
        let upstream = manager.get_upstream("test-upstream").await.unwrap();
        assert_eq!(upstream.servers.len(), 1);
    }

    #[tokio::test]
    async fn test_remove_server() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        // Create upstream with server
        let upstream = UpstreamDefinition {
            name: "test-upstream".to_string(),
            servers: vec![BackendServer {
                url: "http://localhost:8080".to_string(),
                weight: 1,
                max_conns: Some(100),
            }],
            load_balancing: LoadBalancingConfig::default(),
            health_checks: None,
        };
        manager.create_upstream(upstream).await.unwrap();

        // Remove server
        let result = manager
            .remove_server("test-upstream", "http://localhost:8080")
            .await;
        assert!(result.is_ok());

        // Verify server was removed
        let upstream = manager.get_upstream("test-upstream").await.unwrap();
        assert_eq!(upstream.servers.len(), 0);
    }

    #[tokio::test]
    async fn test_update_load_balancing() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        // Create upstream
        let upstream = UpstreamDefinition {
            name: "test-upstream".to_string(),
            servers: vec![],
            load_balancing: LoadBalancingConfig {
                algorithm: "round_robin".to_string(),
            },
            health_checks: None,
        };
        manager.create_upstream(upstream).await.unwrap();

        // Update load balancing
        let result = manager
            .update_load_balancing(
                "test-upstream",
                LoadBalancingConfig {
                    algorithm: "least_conn".to_string(),
                },
            )
            .await;
        assert!(result.is_ok());

        // Verify change
        let upstream = manager.get_upstream("test-upstream").await.unwrap();
        assert_eq!(upstream.load_balancing.algorithm, "least_conn");
    }

    #[tokio::test]
    async fn test_list_upstreams() {
        let config = create_test_config();
        let manager = UpstreamManager::new(config);

        // Create upstreams
        for i in 0..3 {
            let upstream = UpstreamDefinition {
                name: format!("upstream-{}", i),
                servers: vec![],
                load_balancing: LoadBalancingConfig::default(),
                health_checks: None,
            };
            manager.create_upstream(upstream).await.unwrap();
        }

        let upstreams = manager.list_upstreams().await;
        assert_eq!(upstreams.len(), 3);
    }
}
