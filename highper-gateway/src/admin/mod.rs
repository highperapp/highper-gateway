//! Admin API module
//!
//! Provides REST API for managing the reverse proxy:
//! - Configuration management (hot reload)
//! - Route management (CRUD operations)
//! - Backend control (enable/disable)
//! - Cache control
//! - Metrics retrieval
//! - Health status
//! - Real-time statistics

pub mod auth;
pub mod backends;
pub mod cache;
pub mod config_persistence;
pub mod dashboard;
pub mod metrics;
pub mod pool;
pub mod request_metrics;
pub mod routes;
pub mod server;
pub mod stats;
pub mod upstreams;

pub use auth::*;
pub use backends::*;
pub use cache::*;
pub use config_persistence::*;
pub use metrics::*;
pub use pool::*;
pub use request_metrics as req_metrics;
pub use routes::*;
pub use server::*;
pub use stats::*;
pub use upstreams::*;

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Route definition in JSON format (for API management)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDefinition {
    /// Route name
    pub name: String,

    /// Route matching criteria
    pub match_criteria: RouteMatcher,

    /// Target upstream name
    pub upstream: String,

    /// Route priority (higher = checked first)
    #[serde(default)]
    pub priority: i32,

    /// Timeout override for this route
    pub timeout: Option<u64>,

    /// Middleware to apply
    #[serde(default)]
    pub middleware: Vec<String>,

    /// Enable/disable route
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Route matching criteria
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteMatcher {
    /// Path patterns
    #[serde(default)]
    pub paths: Vec<String>,

    /// Host patterns
    #[serde(default)]
    pub hosts: Vec<String>,

    /// HTTP methods
    #[serde(default)]
    pub methods: Vec<String>,

    /// Header matches
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// Query parameter matches
    #[serde(default)]
    pub query_params: std::collections::HashMap<String, String>,
}

/// Upstream definition in JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamDefinition {
    /// Upstream name
    pub name: String,

    /// Backend servers
    pub servers: Vec<BackendServer>,

    /// Load balancing algorithm
    #[serde(default)]
    pub load_balancing: LoadBalancingConfig,

    /// Health check configuration
    pub health_checks: Option<HealthCheckConfig>,
}

/// Backend server definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendServer {
    /// Server URL
    pub url: String,

    /// Server weight (for weighted load balancing)
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Maximum connections
    pub max_conns: Option<u32>,
}

fn default_weight() -> u32 {
    1
}

/// Load balancing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// Algorithm name
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
}

fn default_algorithm() -> String {
    "round_robin".to_string()
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health checks
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Check interval in seconds
    #[serde(default = "default_interval")]
    pub interval: u64,

    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Healthy threshold
    #[serde(default = "default_threshold")]
    pub healthy_threshold: u32,

    /// Unhealthy threshold
    #[serde(default = "default_threshold")]
    pub unhealthy_threshold: u32,
}

fn default_interval() -> u64 {
    10
}

fn default_timeout() -> u64 {
    5
}

fn default_threshold() -> u32 {
    2
}
