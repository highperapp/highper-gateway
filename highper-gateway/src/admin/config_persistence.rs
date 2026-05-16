//! Configuration persistence for runtime changes
//!
//! Allows saving runtime configuration changes to disk so they
//! persist across restarts.

use crate::admin::{RouteDefinition, UpstreamDefinition};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing::info;

/// Configuration persistence manager
pub struct ConfigPersistence {
    /// Path to save runtime configuration
    config_path: PathBuf,
    /// Whether persistence is enabled
    enabled: bool,
}

impl ConfigPersistence {
    /// Create a new persistence manager
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            enabled: true,
        }
    }

    /// Create a disabled persistence manager
    pub fn disabled() -> Self {
        Self {
            config_path: PathBuf::new(),
            enabled: false,
        }
    }

    /// Check if persistence is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Save runtime configuration to disk
    pub async fn save(&self, runtime_config: &RuntimeConfig) -> Result<(), String> {
        if !self.enabled {
            return Err("Configuration persistence is disabled".to_string());
        }

        // Serialize to YAML
        let yaml = serde_yaml::to_string(runtime_config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Write to file atomically (write to temp, then rename)
        let temp_path = self.config_path.with_extension("tmp");
        fs::write(&temp_path, yaml)
            .await
            .map_err(|e| format!("Failed to write config: {}", e))?;

        fs::rename(&temp_path, &self.config_path)
            .await
            .map_err(|e| format!("Failed to save config: {}", e))?;

        info!("Runtime configuration saved to {:?}", self.config_path);
        Ok(())
    }

    /// Load runtime configuration from disk
    pub async fn load(&self) -> Result<RuntimeConfig, String> {
        if !self.enabled {
            return Err("Configuration persistence is disabled".to_string());
        }

        if !self.config_path.exists() {
            return Ok(RuntimeConfig::default());
        }

        let yaml = fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let config: RuntimeConfig =
            serde_yaml::from_str(&yaml).map_err(|e| format!("Failed to parse config: {}", e))?;

        info!("Runtime configuration loaded from {:?}", self.config_path);
        Ok(config)
    }

    /// Get the configuration file path
    pub fn path(&self) -> &Path {
        &self.config_path
    }
}

/// Runtime configuration that can be persisted
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Routes added at runtime
    #[serde(default)]
    pub routes: Vec<RouteDefinition>,

    /// Upstreams added/modified at runtime
    #[serde(default)]
    pub upstreams: Vec<UpstreamDefinition>,

    /// Timestamp when config was last saved
    #[serde(default)]
    pub last_saved: Option<String>,

    /// Version of the runtime config format
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl RuntimeConfig {
    /// Create a new runtime config
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            upstreams: Vec::new(),
            last_saved: None,
            version: default_version(),
        }
    }

    /// Add a route
    pub fn add_route(&mut self, route: RouteDefinition) {
        // Remove existing route with same name
        self.routes.retain(|r| r.name != route.name);
        self.routes.push(route);
    }

    /// Remove a route
    pub fn remove_route(&mut self, name: &str) {
        self.routes.retain(|r| r.name != name);
    }

    /// Add an upstream
    pub fn add_upstream(&mut self, upstream: UpstreamDefinition) {
        // Remove existing upstream with same name
        self.upstreams.retain(|u| u.name != upstream.name);
        self.upstreams.push(upstream);
    }

    /// Remove an upstream
    pub fn remove_upstream(&mut self, name: &str) {
        self.upstreams.retain(|u| u.name != name);
    }

    /// Update timestamp
    pub fn update_timestamp(&mut self) {
        self.last_saved = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Configuration API manager that combines routes, upstreams, and persistence
pub struct ConfigManager {
    /// Route manager
    route_manager: Arc<crate::admin::RouteManager>,
    /// Upstream manager
    upstream_manager: Arc<crate::admin::UpstreamManager>,
    /// Persistence manager
    persistence: Arc<ConfigPersistence>,
    /// Auto-save on changes
    auto_save: bool,
}

impl ConfigManager {
    /// Create a new config manager
    pub fn new(
        route_manager: Arc<crate::admin::RouteManager>,
        upstream_manager: Arc<crate::admin::UpstreamManager>,
        persistence: Arc<ConfigPersistence>,
    ) -> Self {
        Self {
            route_manager,
            upstream_manager,
            persistence,
            auto_save: true,
        }
    }

    /// Set auto-save behavior
    pub fn with_auto_save(mut self, auto_save: bool) -> Self {
        self.auto_save = auto_save;
        self
    }

    /// Get the route manager
    pub fn routes(&self) -> &Arc<crate::admin::RouteManager> {
        &self.route_manager
    }

    /// Get the upstream manager
    pub fn upstreams(&self) -> &Arc<crate::admin::UpstreamManager> {
        &self.upstream_manager
    }

    /// Save current configuration
    pub async fn save(&self) -> Result<(), String> {
        let routes = self.route_manager.list_routes().await;
        let upstreams = self.upstream_manager.export_config().await;

        let mut runtime_config = RuntimeConfig {
            routes,
            upstreams,
            last_saved: None,
            version: default_version(),
        };
        runtime_config.update_timestamp();

        self.persistence.save(&runtime_config).await
    }

    /// Load configuration
    pub async fn load(&self) -> Result<RuntimeConfig, String> {
        self.persistence.load().await
    }

    /// Export current configuration as JSON
    pub async fn export_json(&self) -> serde_json::Value {
        let routes = self.route_manager.list_routes().await;
        let upstreams = self.upstream_manager.list_upstreams().await;

        json!({
            "routes": routes,
            "upstreams": upstreams,
            "persistence": {
                "enabled": self.persistence.is_enabled(),
                "path": self.persistence.path().to_string_lossy(),
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })
    }
}

// HTTP handlers

/// Save configuration to disk
pub async fn save_config_handler(
    persistence: Arc<ConfigPersistence>,
    route_manager: Arc<crate::admin::RouteManager>,
    upstream_manager: Arc<crate::admin::UpstreamManager>,
) -> Response<Full<Bytes>> {
    if !persistence.is_enabled() {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "error": "Configuration persistence is disabled"
            }),
        );
    }

    let routes = route_manager.list_routes().await;
    let upstreams = upstream_manager.export_config().await;

    let mut runtime_config = RuntimeConfig {
        routes: routes.clone(),
        upstreams: upstreams.clone(),
        last_saved: None,
        version: default_version(),
    };
    runtime_config.update_timestamp();

    match persistence.save(&runtime_config).await {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": "Configuration saved successfully",
                "path": persistence.path().to_string_lossy(),
                "routes_count": routes.len(),
                "upstreams_count": upstreams.len(),
                "timestamp": runtime_config.last_saved,
            }),
        ),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "error": format!("Failed to save configuration: {}", e)
            }),
        ),
    }
}

/// Load configuration from disk
pub async fn load_config_handler(persistence: Arc<ConfigPersistence>) -> Response<Full<Bytes>> {
    if !persistence.is_enabled() {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "error": "Configuration persistence is disabled"
            }),
        );
    }

    match persistence.load().await {
        Ok(config) => json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "config": config,
            }),
        ),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "error": format!("Failed to load configuration: {}", e)
            }),
        ),
    }
}

/// Export current configuration
pub async fn export_config_handler(
    route_manager: Arc<crate::admin::RouteManager>,
    upstream_manager: Arc<crate::admin::UpstreamManager>,
    persistence: Arc<ConfigPersistence>,
) -> Response<Full<Bytes>> {
    let routes = route_manager.list_routes().await;
    let upstreams = upstream_manager.list_upstreams().await;

    json_response(
        StatusCode::OK,
        json!({
            "routes": routes,
            "upstreams": upstreams,
            "persistence": {
                "enabled": persistence.is_enabled(),
                "path": persistence.path().to_string_lossy(),
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    )
}

/// Helper function to create JSON response
fn json_response(status: StatusCode, body: serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::{BackendServer, LoadBalancingConfig, RouteMatcher};
    use std::env::temp_dir;

    #[tokio::test]
    async fn test_save_and_load_config() {
        let temp_path = temp_dir().join("highper-gateway-config-test.yaml");
        let persistence = ConfigPersistence::new(&temp_path);

        let mut config = RuntimeConfig::new();
        config.add_route(RouteDefinition {
            name: "test-route".to_string(),
            match_criteria: RouteMatcher {
                paths: vec!["/api/*".to_string()],
                hosts: vec![],
                methods: vec![],
                headers: Default::default(),
                query_params: Default::default(),
            },
            upstream: "test-upstream".to_string(),
            priority: 0,
            timeout: None,
            middleware: vec![],
            enabled: true,
        });
        config.update_timestamp();

        // Save
        persistence.save(&config).await.unwrap();

        // Load
        let loaded = persistence.load().await.unwrap();
        assert_eq!(loaded.routes.len(), 1);
        assert_eq!(loaded.routes[0].name, "test-route");

        // Cleanup
        fs::remove_file(&temp_path).await.ok();
    }

    #[tokio::test]
    async fn test_runtime_config_operations() {
        let mut config = RuntimeConfig::new();

        // Add routes
        config.add_route(RouteDefinition {
            name: "route1".to_string(),
            match_criteria: RouteMatcher::default(),
            upstream: "upstream1".to_string(),
            priority: 0,
            timeout: None,
            middleware: vec![],
            enabled: true,
        });
        assert_eq!(config.routes.len(), 1);

        // Add another route
        config.add_route(RouteDefinition {
            name: "route2".to_string(),
            match_criteria: RouteMatcher::default(),
            upstream: "upstream2".to_string(),
            priority: 0,
            timeout: None,
            middleware: vec![],
            enabled: true,
        });
        assert_eq!(config.routes.len(), 2);

        // Update route (same name)
        config.add_route(RouteDefinition {
            name: "route1".to_string(),
            match_criteria: RouteMatcher::default(),
            upstream: "upstream3".to_string(),
            priority: 0,
            timeout: None,
            middleware: vec![],
            enabled: true,
        });
        assert_eq!(config.routes.len(), 2);
        assert_eq!(
            config
                .routes
                .iter()
                .find(|r| r.name == "route1")
                .unwrap()
                .upstream,
            "upstream3"
        );

        // Remove route
        config.remove_route("route1");
        assert_eq!(config.routes.len(), 1);
        assert!(config.routes.iter().find(|r| r.name == "route1").is_none());
    }

    #[tokio::test]
    async fn test_disabled_persistence() {
        let persistence = ConfigPersistence::disabled();
        assert!(!persistence.is_enabled());

        let result = persistence.save(&RuntimeConfig::new()).await;
        assert!(result.is_err());

        let result = persistence.load().await;
        assert!(result.is_err());
    }
}
