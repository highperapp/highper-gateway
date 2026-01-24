//! BFF (Backend for Frontend) Pattern Configuration
//!
//! Defines client profiles and their specific API configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::super::aggregation::AggregationConfig;

/// BFF configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BffConfig {
    /// Client profiles (mobile, web, desktop, etc.)
    pub profiles: HashMap<String, ClientProfile>,

    /// Default profile if client type cannot be detected
    #[serde(default)]
    pub default_profile: Option<String>,

    /// Client detection rules
    #[serde(default)]
    pub detection: ClientDetectionConfig,
}

/// Client profile configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientProfile {
    /// Profile name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Routes for this client
    #[serde(default)]
    pub routes: Vec<BffRoute>,

    /// Response transformations
    #[serde(default)]
    pub transformations: ResponseTransformConfig,

    /// Cache settings for this client
    #[serde(default)]
    pub cache: ClientCacheConfig,

    /// Rate limiting for this client
    #[serde(default)]
    pub rate_limit: Option<ClientRateLimitConfig>,

    /// Headers to add to responses
    #[serde(default)]
    pub response_headers: HashMap<String, String>,
}

/// BFF route configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BffRoute {
    /// Route path (e.g., "/api/v1/dashboard")
    pub path: String,

    /// HTTP methods (GET, POST, etc.)
    #[serde(default = "default_methods")]
    pub methods: Vec<String>,

    /// Backend aggregation configuration
    pub aggregation: AggregationConfig,

    /// Response schema (for validation/documentation)
    #[serde(default)]
    pub response_schema: Option<serde_json::Value>,

    /// Cache TTL override (seconds)
    #[serde(default)]
    pub cache_ttl: Option<u64>,

    /// Authorization requirements
    #[serde(default)]
    pub auth_required: bool,

    /// Required scopes
    #[serde(default)]
    pub required_scopes: Vec<String>,
}

fn default_methods() -> Vec<String> {
    vec!["GET".to_string()]
}

/// Client detection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientDetectionConfig {
    /// Detection method
    #[serde(default)]
    pub method: DetectionMethod,

    /// Custom header for client type
    #[serde(default = "default_client_header")]
    pub header_name: String,

    /// User-Agent patterns for detection
    #[serde(default)]
    pub user_agent_patterns: HashMap<String, String>,
}

fn default_client_header() -> String {
    "X-Client-Type".to_string()
}

impl Default for ClientDetectionConfig {
    fn default() -> Self {
        Self {
            method: DetectionMethod::default(),
            header_name: default_client_header(),
            user_agent_patterns: HashMap::new(),
        }
    }
}

/// Client detection method
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    /// Use custom header (X-Client-Type)
    #[default]
    Header,

    /// Parse User-Agent header
    UserAgent,

    /// Use query parameter (?client=mobile)
    QueryParam,

    /// Use path prefix (/mobile/api/...)
    PathPrefix,
}

/// Response transformation config
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ResponseTransformConfig {
    /// Remove fields from response
    #[serde(default)]
    pub remove_fields: Vec<String>,

    /// Rename fields in response
    #[serde(default)]
    pub rename_fields: HashMap<String, String>,

    /// Flatten nested objects
    #[serde(default)]
    pub flatten: Vec<String>,

    /// Add computed fields
    #[serde(default)]
    pub computed_fields: Vec<ComputedField>,

    /// Compact mode (minimize JSON)
    #[serde(default)]
    pub compact: bool,
}

/// Computed field definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComputedField {
    /// Field name in output
    pub name: String,

    /// JSONPath expression to compute value
    pub expression: String,
}

/// Client-specific cache configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCacheConfig {
    /// Enable caching for this client
    #[serde(default)]
    pub enabled: bool,

    /// Default TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub default_ttl: u64,

    /// Vary cache by these headers
    #[serde(default)]
    pub vary_by: Vec<String>,
}

fn default_cache_ttl() -> u64 {
    60
}

impl Default for ClientCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_ttl: default_cache_ttl(),
            vary_by: Vec::new(),
        }
    }
}

/// Client-specific rate limiting
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientRateLimitConfig {
    /// Requests per second
    pub rps: u32,

    /// Burst size
    #[serde(default = "default_burst")]
    pub burst: u32,
}

fn default_burst() -> u32 {
    10
}

impl Default for BffConfig {
    fn default() -> Self {
        Self {
            profiles: HashMap::new(),
            default_profile: Some("web".to_string()),
            detection: ClientDetectionConfig::default(),
        }
    }
}

impl Default for ClientProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            routes: Vec::new(),
            transformations: ResponseTransformConfig::default(),
            cache: ClientCacheConfig::default(),
            rate_limit: None,
            response_headers: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bff_config_default() {
        let config = BffConfig::default();
        assert_eq!(config.default_profile, Some("web".to_string()));
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_client_profile_deserialization() {
        let yaml = r#"
name: "mobile"
description: "Mobile app profile"
routes:
  - path: "/api/dashboard"
    methods: ["GET"]
    aggregation:
      backends:
        - name: "user"
          upstream: "user_service"
          path: "/user"
transformations:
  compact: true
"#;
        let profile: ClientProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(profile.name, "mobile");
        assert_eq!(profile.routes.len(), 1);
        assert!(profile.transformations.compact);
    }

    #[test]
    fn test_detection_methods() {
        assert_eq!(DetectionMethod::default(), DetectionMethod::Header);

        let header = DetectionMethod::Header;
        let user_agent = DetectionMethod::UserAgent;
        let query = DetectionMethod::QueryParam;
        let path = DetectionMethod::PathPrefix;

        assert!(matches!(header, DetectionMethod::Header));
        assert!(matches!(user_agent, DetectionMethod::UserAgent));
        assert!(matches!(query, DetectionMethod::QueryParam));
        assert!(matches!(path, DetectionMethod::PathPrefix));
    }

    #[test]
    fn test_client_cache_config() {
        let cache = ClientCacheConfig::default();
        assert_eq!(cache.enabled, false);
        assert_eq!(cache.default_ttl, 60);
    }

    #[test]
    fn test_bff_route_with_auth() {
        let yaml = r#"
path: "/api/admin"
methods: ["GET", "POST"]
auth_required: true
required_scopes: ["admin:read", "admin:write"]
aggregation:
  backends: []
"#;
        let route: BffRoute = serde_yaml::from_str(yaml).unwrap();
        assert!(route.auth_required);
        assert_eq!(route.required_scopes.len(), 2);
    }
}
