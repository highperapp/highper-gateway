//! Configuration for API aggregation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aggregation configuration for a route
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AggregationConfig {
    /// Backend calls to execute
    pub backends: Vec<BackendCall>,

    /// Response merge strategy
    #[serde(default)]
    pub merge_strategy: MergeStrategy,

    /// Error handling strategy
    #[serde(default)]
    pub error_strategy: ErrorStrategy,

    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Enable parallel execution
    #[serde(default = "default_true")]
    pub parallel: bool,
}

fn default_timeout() -> u64 {
    5000 // 5 seconds
}

fn default_true() -> bool {
    true
}

/// Backend call configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackendCall {
    /// Call identifier (used as key in merged response)
    pub name: String,

    /// Upstream service name
    pub upstream: String,

    /// Request path (supports templating)
    pub path: String,

    /// HTTP method
    #[serde(default = "default_method")]
    pub method: String,

    /// Additional headers to add
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// JSONPath for extracting data from response
    pub extract: Option<String>,

    /// Whether this call is required (affects error handling)
    #[serde(default = "default_true")]
    pub required: bool,

    /// Fallback value if call fails (JSON string)
    pub fallback: Option<serde_json::Value>,
}

fn default_method() -> String {
    "GET".to_string()
}

/// Response merge strategy
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// Merge all responses into a single JSON object with backend names as keys
    #[default]
    Object,

    /// Merge responses into a JSON array
    Array,

    /// Take the first successful response
    First,

    /// Custom JSONPath-based merge (advanced)
    Custom {
        /// JSONPath template for merge
        template: String,
    },
}

/// Error handling strategy
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStrategy {
    /// Fail if any required backend fails
    #[default]
    FailFast,

    /// Continue with partial results if non-required backends fail
    Partial,

    /// Return errors in response alongside successful results
    Include,

    /// Ignore all errors and return whatever succeeded
    Ignore,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            backends: vec![],
            merge_strategy: MergeStrategy::Object,
            error_strategy: ErrorStrategy::FailFast,
            timeout_ms: default_timeout(),
            parallel: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_config_default() {
        let config = AggregationConfig::default();
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.parallel, true);
        assert_eq!(config.merge_strategy, MergeStrategy::Object);
        assert_eq!(config.error_strategy, ErrorStrategy::FailFast);
    }

    #[test]
    fn test_backend_call_deserialization() {
        let yaml = r#"
name: "user"
upstream: "user_service"
path: "/api/users/{id}"
method: "GET"
required: true
"#;

        let call: BackendCall = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(call.name, "user");
        assert_eq!(call.upstream, "user_service");
        assert_eq!(call.method, "GET");
        assert_eq!(call.required, true);
    }

    #[test]
    fn test_merge_strategies() {
        let obj = MergeStrategy::Object;
        let arr = MergeStrategy::Array;
        let first = MergeStrategy::First;

        assert!(matches!(obj, MergeStrategy::Object));
        assert!(matches!(arr, MergeStrategy::Array));
        assert!(matches!(first, MergeStrategy::First));
    }

    #[test]
    fn test_error_strategies() {
        let fail = ErrorStrategy::FailFast;
        let partial = ErrorStrategy::Partial;
        let include = ErrorStrategy::Include;
        let ignore = ErrorStrategy::Ignore;

        assert!(matches!(fail, ErrorStrategy::FailFast));
        assert!(matches!(partial, ErrorStrategy::Partial));
        assert!(matches!(include, ErrorStrategy::Include));
        assert!(matches!(ignore, ErrorStrategy::Ignore));
    }

    #[test]
    fn test_backend_call_with_fallback() {
        let yaml = r#"
name: "profile"
upstream: "profile_service"
path: "/profile"
required: false
fallback: {"status": "unavailable"}
"#;

        let call: BackendCall = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(call.required, false);
        assert!(call.fallback.is_some());
    }
}
