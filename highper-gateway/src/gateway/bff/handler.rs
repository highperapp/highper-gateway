//! BFF (Backend for Frontend) Handler
//!
//! Handles client detection, route resolution, and request processing.

use super::super::aggregation::AggregationConfig;
use super::config::*;
use hyper::{header::USER_AGENT, HeaderMap, Method, Request};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

/// BFF Handler for processing client-specific requests
pub struct BffHandler {
    config: Arc<BffConfig>,
    user_agent_patterns: Vec<(String, Regex)>,
}

/// Client type detected from request
#[derive(Debug, Clone, PartialEq)]
pub enum ClientType {
    /// Mobile app (iOS/Android)
    Mobile,
    /// Web browser
    Web,
    /// Desktop application
    Desktop,
    /// IoT device
    IoT,
    /// Custom client type
    Custom(String),
    /// Unknown client
    Unknown,
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Mobile => write!(f, "mobile"),
            ClientType::Web => write!(f, "web"),
            ClientType::Desktop => write!(f, "desktop"),
            ClientType::IoT => write!(f, "iot"),
            ClientType::Custom(name) => write!(f, "{}", name),
            ClientType::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<&str> for ClientType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mobile" | "ios" | "android" => ClientType::Mobile,
            "web" | "browser" => ClientType::Web,
            "desktop" | "electron" | "tauri" => ClientType::Desktop,
            "iot" | "device" => ClientType::IoT,
            "" | "unknown" => ClientType::Unknown,
            other => ClientType::Custom(other.to_string()),
        }
    }
}

/// Route match result
#[derive(Debug)]
pub struct RouteMatch<'a> {
    /// Matched route configuration
    pub route: &'a BffRoute,

    /// Client profile
    pub profile: &'a ClientProfile,

    /// Path parameters extracted
    pub params: HashMap<String, String>,
}

impl BffHandler {
    /// Create a new BFF handler
    pub fn new(config: BffConfig) -> Self {
        let mut user_agent_patterns = Vec::new();

        // Compile user agent patterns
        for (client_type, pattern) in &config.detection.user_agent_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                user_agent_patterns.push((client_type.clone(), regex));
            }
        }

        // Add default patterns if none specified
        if user_agent_patterns.is_empty() {
            let defaults = vec![
                ("mobile", r"(?i)(iphone|ipad|android|mobile|okhttp|dart)"),
                ("web", r"(?i)(mozilla|chrome|safari|firefox|edge)"),
                ("desktop", r"(?i)(electron|tauri|nw\.js)"),
                ("iot", r"(?i)(esp32|arduino|raspberry)"),
            ];

            for (name, pattern) in defaults {
                if let Ok(regex) = Regex::new(pattern) {
                    user_agent_patterns.push((name.to_string(), regex));
                }
            }
        }

        Self {
            config: Arc::new(config),
            user_agent_patterns,
        }
    }

    /// Detect client type from request
    pub fn detect_client<B>(&self, request: &Request<B>) -> ClientType {
        match self.config.detection.method {
            DetectionMethod::Header => self.detect_from_header(request.headers()),
            DetectionMethod::UserAgent => self.detect_from_user_agent(request.headers()),
            DetectionMethod::QueryParam => self.detect_from_query(request.uri().query()),
            DetectionMethod::PathPrefix => self.detect_from_path(request.uri().path()),
        }
    }

    /// Detect client from custom header
    fn detect_from_header(&self, headers: &HeaderMap) -> ClientType {
        headers
            .get(self.config.detection.header_name.as_str())
            .and_then(|v| v.to_str().ok())
            .map(ClientType::from)
            .unwrap_or(ClientType::Unknown)
    }

    /// Detect client from User-Agent header
    fn detect_from_user_agent(&self, headers: &HeaderMap) -> ClientType {
        let user_agent = headers
            .get(USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        for (client_type, pattern) in &self.user_agent_patterns {
            if pattern.is_match(user_agent) {
                return ClientType::from(client_type.as_str());
            }
        }

        ClientType::Unknown
    }

    /// Detect client from query parameter
    fn detect_from_query(&self, query: Option<&str>) -> ClientType {
        query
            .and_then(|q| {
                q.split('&')
                    .find(|p| p.starts_with("client="))
                    .map(|p| &p[7..])
            })
            .map(ClientType::from)
            .unwrap_or(ClientType::Unknown)
    }

    /// Detect client from path prefix
    fn detect_from_path(&self, path: &str) -> ClientType {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if let Some(first) = segments.first() {
            match *first {
                "mobile" | "m" => return ClientType::Mobile,
                "web" | "w" => return ClientType::Web,
                "desktop" | "d" => return ClientType::Desktop,
                "iot" => return ClientType::IoT,
                _ => {}
            }
        }

        ClientType::Unknown
    }

    /// Get profile for client type
    pub fn get_profile(&self, client_type: &ClientType) -> Option<&ClientProfile> {
        let profile_name = client_type.to_string();

        self.config.profiles.get(&profile_name).or_else(|| {
            self.config
                .default_profile
                .as_ref()
                .and_then(|default| self.config.profiles.get(default))
        })
    }

    /// Resolve route for request
    pub fn resolve_route<'a, B>(
        &'a self,
        request: &Request<B>,
        profile: &'a ClientProfile,
    ) -> Option<RouteMatch<'a>> {
        let path = request.uri().path();
        let method = request.method().as_str();

        for route in &profile.routes {
            if route.methods.iter().any(|m| m.eq_ignore_ascii_case(method)) {
                if let Some(params) = self.match_path(&route.path, path) {
                    return Some(RouteMatch {
                        route,
                        profile,
                        params,
                    });
                }
            }
        }

        None
    }

    /// Match path with pattern and extract parameters
    fn match_path(&self, pattern: &str, path: &str) -> Option<HashMap<String, String>> {
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if pattern_parts.len() != path_parts.len() {
            // Check for wildcard
            if !pattern.ends_with("*") {
                return None;
            }
        }

        let mut params = HashMap::new();

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                let param_name = &pattern_part[1..pattern_part.len() - 1];
                params.insert(param_name.to_string(), path_part.to_string());
            } else if *pattern_part == "*" {
                // Wildcard matches anything
                continue;
            } else if *pattern_part != *path_part {
                return None;
            }
        }

        Some(params)
    }

    /// Transform response based on profile configuration
    pub fn transform_response(
        &self,
        profile: &ClientProfile,
        mut response: serde_json::Value,
    ) -> serde_json::Value {
        let transform = &profile.transformations;

        // Remove fields
        if let Some(obj) = response.as_object_mut() {
            for field in &transform.remove_fields {
                obj.remove(field);
            }
        }

        // Rename fields
        if let Some(obj) = response.as_object_mut() {
            for (old_name, new_name) in &transform.rename_fields {
                if let Some(value) = obj.remove(old_name) {
                    obj.insert(new_name.clone(), value);
                }
            }
        }

        // Add computed fields
        for computed in &transform.computed_fields {
            // Simple JSONPath extraction (basic implementation)
            if let Some(value) = self.extract_jsonpath(&response, &computed.expression) {
                if let Some(obj) = response.as_object_mut() {
                    obj.insert(computed.name.clone(), value);
                }
            }
        }

        response
    }

    /// Extract value using JSONPath (simplified)
    fn extract_jsonpath(&self, value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
        let parts: Vec<&str> = path
            .trim_start_matches('$')
            .split('.')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    }

    /// Get response headers for profile
    pub fn get_response_headers(&self, profile: &ClientProfile) -> HashMap<String, String> {
        let mut headers = profile.response_headers.clone();

        // Add standard BFF headers
        headers.insert("X-BFF-Profile".to_string(), profile.name.clone());

        headers
    }

    /// Check if request is authorized for route
    pub fn check_authorization(&self, route: &BffRoute, scopes: &[String]) -> bool {
        if !route.auth_required {
            return true;
        }

        if route.required_scopes.is_empty() {
            return true;
        }

        route
            .required_scopes
            .iter()
            .all(|required| scopes.iter().any(|s| s == required))
    }

    /// Get aggregation config for route with path parameters substituted
    pub fn prepare_aggregation(
        &self,
        route: &BffRoute,
        params: &HashMap<String, String>,
    ) -> AggregationConfig {
        let mut config = route.aggregation.clone();

        // Substitute path parameters in backend paths
        for backend in &mut config.backends {
            for (key, value) in params {
                backend.path = backend.path.replace(&format!("{{{}}}", key), value);
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Request;

    fn test_config() -> BffConfig {
        let mut profiles = HashMap::new();

        profiles.insert(
            "mobile".to_string(),
            ClientProfile {
                name: "mobile".to_string(),
                description: "Mobile profile".to_string(),
                routes: vec![BffRoute {
                    path: "/api/users/{id}".to_string(),
                    methods: vec!["GET".to_string()],
                    aggregation: AggregationConfig::default(),
                    response_schema: None,
                    cache_ttl: Some(60),
                    auth_required: false,
                    required_scopes: vec![],
                }],
                transformations: ResponseTransformConfig::default(),
                cache: ClientCacheConfig::default(),
                rate_limit: None,
                response_headers: HashMap::new(),
            },
        );

        profiles.insert(
            "web".to_string(),
            ClientProfile {
                name: "web".to_string(),
                description: "Web profile".to_string(),
                routes: vec![],
                transformations: ResponseTransformConfig::default(),
                cache: ClientCacheConfig::default(),
                rate_limit: None,
                response_headers: HashMap::new(),
            },
        );

        BffConfig {
            profiles,
            default_profile: Some("web".to_string()),
            detection: ClientDetectionConfig::default(),
        }
    }

    #[test]
    fn test_client_type_from_str() {
        assert_eq!(ClientType::from("mobile"), ClientType::Mobile);
        assert_eq!(ClientType::from("ios"), ClientType::Mobile);
        assert_eq!(ClientType::from("web"), ClientType::Web);
        assert_eq!(ClientType::from("desktop"), ClientType::Desktop);
        assert_eq!(
            ClientType::from("custom_app"),
            ClientType::Custom("custom_app".to_string())
        );
    }

    #[test]
    fn test_detect_from_header() {
        let config = test_config();
        let handler = BffHandler::new(config);

        let request = Request::builder()
            .header("X-Client-Type", "mobile")
            .body(())
            .unwrap();

        let client_type = handler.detect_client(&request);
        assert_eq!(client_type, ClientType::Mobile);
    }

    #[test]
    fn test_detect_from_user_agent() {
        let mut config = test_config();
        config.detection.method = DetectionMethod::UserAgent;
        let handler = BffHandler::new(config);

        let request = Request::builder()
            .header("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS)")
            .body(())
            .unwrap();

        let client_type = handler.detect_client(&request);
        assert_eq!(client_type, ClientType::Mobile);
    }

    #[test]
    fn test_detect_from_query() {
        let mut config = test_config();
        config.detection.method = DetectionMethod::QueryParam;
        let handler = BffHandler::new(config);

        let request = Request::builder()
            .uri("/api/data?client=mobile&foo=bar")
            .body(())
            .unwrap();

        let client_type = handler.detect_client(&request);
        assert_eq!(client_type, ClientType::Mobile);
    }

    #[test]
    fn test_detect_from_path() {
        let mut config = test_config();
        config.detection.method = DetectionMethod::PathPrefix;
        let handler = BffHandler::new(config);

        let request = Request::builder()
            .uri("/mobile/api/users")
            .body(())
            .unwrap();

        let client_type = handler.detect_client(&request);
        assert_eq!(client_type, ClientType::Mobile);
    }

    #[test]
    fn test_path_matching() {
        let config = test_config();
        let handler = BffHandler::new(config);

        let params = handler.match_path("/api/users/{id}", "/api/users/123");
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_route_resolution() {
        let config = test_config();
        let handler = BffHandler::new(config);

        let profile = handler.get_profile(&ClientType::Mobile).unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/users/123")
            .body(())
            .unwrap();

        let route_match = handler.resolve_route(&request, profile);
        assert!(route_match.is_some());

        let route_match = route_match.unwrap();
        assert_eq!(route_match.params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_response_transformation() {
        let mut config = test_config();
        if let Some(profile) = config.profiles.get_mut("mobile") {
            profile.transformations.remove_fields = vec!["internal_id".to_string()];
            profile
                .transformations
                .rename_fields
                .insert("userName".to_string(), "name".to_string());
        }

        let handler = BffHandler::new(config);

        let profile = handler.get_profile(&ClientType::Mobile).unwrap();

        let response = serde_json::json!({
            "internal_id": "abc123",
            "userName": "John Doe",
            "email": "john@example.com"
        });

        let transformed = handler.transform_response(profile, response);

        assert!(transformed.get("internal_id").is_none());
        assert!(transformed.get("userName").is_none());
        assert_eq!(transformed.get("name").unwrap(), "John Doe");
        assert_eq!(transformed.get("email").unwrap(), "john@example.com");
    }

    #[test]
    fn test_authorization_check() {
        let config = test_config();
        let handler = BffHandler::new(config);

        let route_no_auth = BffRoute {
            path: "/public".to_string(),
            methods: vec!["GET".to_string()],
            aggregation: AggregationConfig::default(),
            response_schema: None,
            cache_ttl: None,
            auth_required: false,
            required_scopes: vec![],
        };

        assert!(handler.check_authorization(&route_no_auth, &[]));

        let route_with_auth = BffRoute {
            path: "/admin".to_string(),
            methods: vec!["GET".to_string()],
            aggregation: AggregationConfig::default(),
            response_schema: None,
            cache_ttl: None,
            auth_required: true,
            required_scopes: vec!["admin:read".to_string()],
        };

        assert!(!handler.check_authorization(&route_with_auth, &[]));
        assert!(handler.check_authorization(&route_with_auth, &["admin:read".to_string()]));
    }
}
