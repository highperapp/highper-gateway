//! Type definitions for per-hostname routing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Route name (unique identifier)
    pub name: String,

    /// Target upstream name
    pub upstream: String,

    /// Allowed HTTP methods (empty = all methods)
    #[serde(default)]
    pub methods: Vec<String>,

    /// Timeout override in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Middleware to apply (in order)
    #[serde(default)]
    pub middleware: Vec<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Route {
    /// Check if route matches the given HTTP method
    pub fn matches_method(&self, method: &str) -> bool {
        self.methods.is_empty() || self.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
    }
}

/// Prefix-based route (e.g., /api/*, /users/*)
#[derive(Debug, Clone)]
pub struct PrefixRoute {
    pub prefix: String,
    pub route: Route,
}

/// Pattern-based route (regex)
#[derive(Debug, Clone)]
pub struct PatternRoute {
    pub pattern: String,
    pub regex: regex::Regex,
    pub route: Route,
}

/// Matched route with extracted parameters
#[derive(Debug, Clone)]
pub struct MatchedRoute {
    /// The matched route
    pub route: Route,

    /// Extracted path parameters (from regex captures)
    pub path_params: HashMap<String, String>,

    /// Type of match that succeeded
    pub match_type: MatchType,
}

/// Type of route match
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    /// Exact path match (fastest)
    Exact,
    /// Prefix match
    Prefix,
    /// Regex pattern match
    Pattern,
}

/// JSON configuration for per-hostname routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostnameRoutesConfig {
    /// Configuration version (for migrations)
    #[serde(default)]
    pub version: String,

    /// All hostname configurations
    pub hosts: Vec<HostConfig>,

    /// Global upstreams (referenced by routes)
    #[serde(default)]
    pub upstreams: HashMap<String, UpstreamConfig>,
}

/// Configuration for a single hostname
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Hostname (exact or wildcard: *.example.com)
    pub hostname: String,

    /// Routes for this hostname
    pub routes: Vec<RouteConfig>,
}

/// Route configuration in JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Route name
    pub name: String,

    /// Path match type and pattern
    #[serde(flatten)]
    pub path_match: PathMatch,

    /// Target upstream
    pub upstream: String,

    /// HTTP methods (empty = all)
    #[serde(default)]
    pub methods: Vec<String>,

    /// Timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Middleware chain
    #[serde(default)]
    pub middleware: Vec<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Path matching strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "match_type")]
pub enum PathMatch {
    /// Exact path match
    #[serde(rename = "exact")]
    Exact {
        /// Exact path
        path: String,
    },

    /// Prefix match
    #[serde(rename = "prefix")]
    Prefix {
        /// Path prefix
        prefix: String,
    },

    /// Regex pattern match
    #[serde(rename = "pattern")]
    Pattern {
        /// Regex pattern (use named groups for path params: (?P<id>\\d+))
        pattern: String,
    },
}

/// Upstream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    /// Backend servers
    pub servers: Vec<String>,

    /// Load balancing algorithm
    #[serde(default = "default_lb_algorithm")]
    pub algorithm: String,

    /// Health check configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,
}

fn default_lb_algorithm() -> String {
    "round_robin".to_string()
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Check interval in seconds
    #[serde(default = "default_interval")]
    pub interval_secs: u64,

    /// Check timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// HTTP path to check
    #[serde(default = "default_health_path")]
    pub path: String,

    /// Expected status code
    #[serde(default = "default_status_code")]
    pub expected_status: u16,
}

fn default_interval() -> u64 {
    10
}

fn default_timeout() -> u64 {
    5
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_status_code() -> u16 {
    200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_method_matching() {
        let route = Route {
            name: "test".to_string(),
            upstream: "backend".to_string(),
            methods: vec!["GET".to_string(), "POST".to_string()],
            timeout_ms: None,
            middleware: vec![],
            metadata: HashMap::new(),
        };

        assert!(route.matches_method("GET"));
        assert!(route.matches_method("get"));
        assert!(route.matches_method("POST"));
        assert!(!route.matches_method("DELETE"));
    }

    #[test]
    fn test_route_all_methods() {
        let route = Route {
            name: "test".to_string(),
            upstream: "backend".to_string(),
            methods: vec![],
            timeout_ms: None,
            middleware: vec![],
            metadata: HashMap::new(),
        };

        assert!(route.matches_method("GET"));
        assert!(route.matches_method("POST"));
        assert!(route.matches_method("DELETE"));
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "api.example.com",
                    "routes": [
                        {
                            "name": "users",
                            "match_type": "prefix",
                            "prefix": "/api/users/",
                            "upstream": "user-service",
                            "methods": ["GET", "POST"]
                        }
                    ]
                }
            ],
            "upstreams": {
                "user-service": {
                    "servers": ["http://localhost:3001", "http://localhost:3002"],
                    "algorithm": "round_robin"
                }
            }
        }
        "#;

        let config: HostnameRoutesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hosts.len(), 1);
        assert_eq!(config.hosts[0].hostname, "api.example.com");
        assert_eq!(config.hosts[0].routes.len(), 1);
    }
}
