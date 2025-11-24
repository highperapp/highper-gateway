//! Request and response transformation middleware
//!
//! Supports header manipulation, path rewriting, and query parameter modification.

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, Request, Response, Uri};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

/// Transformation rules
#[derive(Debug, Clone)]
pub struct TransformConfig {
    /// Headers to add/set on requests
    pub request_headers_add: HashMap<String, String>,
    /// Headers to remove from requests
    pub request_headers_remove: Vec<String>,
    /// Headers to add/set on responses
    pub response_headers_add: HashMap<String, String>,
    /// Headers to remove from responses
    pub response_headers_remove: Vec<String>,
    /// Path prefix to add
    pub path_prefix: Option<String>,
    /// Path prefix to remove
    pub strip_path_prefix: Option<String>,
    /// Path rewrite rules (from -> to)
    pub path_rewrites: Vec<(String, String)>,
    /// Query parameters to add
    pub query_params_add: HashMap<String, String>,
}

impl Default for TransformConfig {
    fn default() -> Self {
        Self {
            request_headers_add: HashMap::new(),
            request_headers_remove: Vec::new(),
            response_headers_add: HashMap::new(),
            response_headers_remove: Vec::new(),
            path_prefix: None,
            strip_path_prefix: None,
            path_rewrites: Vec::new(),
            query_params_add: HashMap::new(),
        }
    }
}

/// Transformation middleware
pub struct TransformMiddleware {
    config: TransformConfig,
}

impl TransformMiddleware {
    /// Create a new transformation middleware
    pub fn new(config: TransformConfig) -> Self {
        Self { config }
    }

    /// Transform request path
    fn transform_path(&self, original_uri: &Uri) -> Result<Uri, hyper::http::uri::InvalidUri> {
        let mut path = original_uri.path().to_string();
        let query = original_uri.query();

        // Strip prefix if configured
        if let Some(prefix) = &self.config.strip_path_prefix {
            if path.starts_with(prefix) {
                path = path[prefix.len()..].to_string();
                if !path.starts_with('/') {
                    path = format!("/{}", path);
                }
                debug!("Stripped prefix '{}' from path: {}", prefix, path);
            }
        }

        // Apply path rewrites
        for (from, to) in &self.config.path_rewrites {
            if path.starts_with(from) {
                path = path.replacen(from, to, 1);
                debug!("Rewrote path from '{}' to '{}'", from, to);
                break;
            }
        }

        // Add prefix if configured
        if let Some(prefix) = &self.config.path_prefix {
            if !path.starts_with(prefix) {
                path = format!("{}{}", prefix, path);
                debug!("Added prefix '{}' to path: {}", prefix, path);
            }
        }

        // Handle query parameters
        let mut query_string = query.map(|q| q.to_string()).unwrap_or_default();

        // Add query parameters
        if !self.config.query_params_add.is_empty() {
            for (key, value) in &self.config.query_params_add {
                let param = format!("{}={}", key, value);
                if query_string.is_empty() {
                    query_string = param;
                } else {
                    query_string = format!("{}&{}", query_string, param);
                }
            }
        }

        // Build new URI
        let uri_string = if query_string.is_empty() {
            path
        } else {
            format!("{}?{}", path, query_string)
        };

        uri_string.parse()
    }
}

impl Middleware for TransformMiddleware {
    fn name(&self) -> &str {
        "transform"
    }

    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            let (mut parts, body) = req.into_parts();

            // Transform URI
            match TransformMiddleware::transform_path_static(&config, &parts.uri) {
                Ok(new_uri) => {
                    if new_uri != parts.uri {
                        debug!("Transformed URI: {} -> {}", parts.uri, new_uri);
                        parts.uri = new_uri;
                    }
                }
                Err(e) => {
                    debug!("Failed to transform URI: {}", e);
                }
            }

            // Add/set request headers
            for (key, value) in &config.request_headers_add {
                if let (Ok(header_name), Ok(header_value)) = (
                    header::HeaderName::from_bytes(key.as_bytes()),
                    header::HeaderValue::from_str(value),
                ) {
                    parts.headers.insert(header_name, header_value);
                    debug!("Added request header: {} = {}", key, value);
                }
            }

            // Remove request headers
            for key in &config.request_headers_remove {
                if let Ok(header_name) = header::HeaderName::from_bytes(key.as_bytes()) {
                    parts.headers.remove(&header_name);
                    debug!("Removed request header: {}", key);
                }
            }

            Ok(Request::from_parts(parts, body))
        })
    }

    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            let (mut parts, body) = response.into_parts();

            // Add/set response headers
            for (key, value) in &config.response_headers_add {
                if let (Ok(header_name), Ok(header_value)) = (
                    header::HeaderName::from_bytes(key.as_bytes()),
                    header::HeaderValue::from_str(value),
                ) {
                    parts.headers.insert(header_name, header_value);
                    debug!("Added response header: {} = {}", key, value);
                }
            }

            // Remove response headers
            for key in &config.response_headers_remove {
                if let Ok(header_name) = header::HeaderName::from_bytes(key.as_bytes()) {
                    parts.headers.remove(&header_name);
                    debug!("Removed response header: {}", key);
                }
            }

            Ok(Response::from_parts(parts, body))
        })
    }
}

impl TransformMiddleware {
    /// Static method for path transformation (for use in async context)
    fn transform_path_static(config: &TransformConfig, original_uri: &Uri) -> Result<Uri, hyper::http::uri::InvalidUri> {
        let mut path = original_uri.path().to_string();
        let query = original_uri.query();

        // Strip prefix if configured
        if let Some(prefix) = &config.strip_path_prefix {
            if path.starts_with(prefix) {
                path = path[prefix.len()..].to_string();
                if !path.starts_with('/') {
                    path = format!("/{}", path);
                }
            }
        }

        // Apply path rewrites
        for (from, to) in &config.path_rewrites {
            if path.starts_with(from) {
                path = path.replacen(from, to, 1);
                break;
            }
        }

        // Add prefix if configured
        if let Some(prefix) = &config.path_prefix {
            if !path.starts_with(prefix) {
                path = format!("{}{}", prefix, path);
            }
        }

        // Handle query parameters
        let mut query_string = query.map(|q| q.to_string()).unwrap_or_default();

        // Add query parameters
        if !config.query_params_add.is_empty() {
            for (key, value) in &config.query_params_add {
                let param = format!("{}={}", key, value);
                if query_string.is_empty() {
                    query_string = param;
                } else {
                    query_string = format!("{}&{}", query_string, param);
                }
            }
        }

        // Build new URI
        let uri_string = if query_string.is_empty() {
            path
        } else {
            format!("{}?{}", path, query_string)
        };

        uri_string.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_path_prefix() {
        let mut config = TransformConfig::default();
        config.strip_path_prefix = Some("/api/v1".to_string());

        let uri: Uri = "/api/v1/users".parse().unwrap();
        let middleware = TransformMiddleware::new(config);
        let result = middleware.transform_path(&uri).unwrap();

        assert_eq!(result.path(), "/users");
    }

    #[test]
    fn test_add_path_prefix() {
        let mut config = TransformConfig::default();
        config.path_prefix = Some("/api".to_string());

        let uri: Uri = "/users".parse().unwrap();
        let middleware = TransformMiddleware::new(config);
        let result = middleware.transform_path(&uri).unwrap();

        assert_eq!(result.path(), "/api/users");
    }

    #[test]
    fn test_path_rewrite() {
        let mut config = TransformConfig::default();
        config.path_rewrites = vec![
            ("/old".to_string(), "/new".to_string()),
        ];

        let uri: Uri = "/old/path".parse().unwrap();
        let middleware = TransformMiddleware::new(config);
        let result = middleware.transform_path(&uri).unwrap();

        assert_eq!(result.path(), "/new/path");
    }

    #[test]
    fn test_query_params() {
        let mut config = TransformConfig::default();
        config.query_params_add.insert("version".to_string(), "v1".to_string());
        config.query_params_add.insert("api_key".to_string(), "test123".to_string());

        let uri: Uri = "/users?page=1".parse().unwrap();
        let middleware = TransformMiddleware::new(config);
        let result = middleware.transform_path(&uri).unwrap();

        let query = result.query().unwrap();
        assert!(query.contains("page=1"));
        assert!(query.contains("version=v1"));
        assert!(query.contains("api_key=test123"));
    }

    #[test]
    fn test_combined_transformations() {
        let mut config = TransformConfig::default();
        config.strip_path_prefix = Some("/api".to_string());
        config.path_prefix = Some("/v2".to_string());
        config.query_params_add.insert("client".to_string(), "proxy".to_string());

        let uri: Uri = "/api/users?page=1".parse().unwrap();
        let middleware = TransformMiddleware::new(config);
        let result = middleware.transform_path(&uri).unwrap();

        assert_eq!(result.path(), "/v2/users");
        let query = result.query().unwrap();
        assert!(query.contains("page=1"));
        assert!(query.contains("client=proxy"));
    }
}
