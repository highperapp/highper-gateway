//! CORS middleware for handling Cross-Origin Resource Sharing

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, HeaderMap, Method, Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

/// CORS configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins (* for all)
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<Method>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Exposed headers
    pub exposed_headers: Vec<String>,
    /// Allow credentials
    pub allow_credentials: bool,
    /// Max age for preflight cache
    pub max_age: Option<u64>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::HEAD,
                Method::OPTIONS,
            ],
            allowed_headers: vec!["*".to_string()],
            exposed_headers: vec![],
            allow_credentials: false,
            max_age: Some(3600),
        }
    }
}

/// CORS middleware
pub struct CorsMiddleware {
    config: CorsConfig,
}

impl CorsMiddleware {
    /// Create a new CORS middleware
    pub fn new(config: CorsConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn permissive() -> Self {
        Self::new(CorsConfig::default())
    }

    /// Check if origin is allowed
    fn is_origin_allowed(&self, origin: &str) -> bool {
        if self.config.allowed_origins.contains(&"*".to_string()) {
            return true;
        }
        self.config.allowed_origins.iter().any(|o| o == origin)
    }

    /// Get allowed origin header value
    fn get_allowed_origin(&self, request_origin: Option<&str>) -> String {
        if self.config.allowed_origins.contains(&"*".to_string()) && !self.config.allow_credentials {
            return "*".to_string();
        }

        if let Some(origin) = request_origin {
            if self.is_origin_allowed(origin) {
                return origin.to_string();
            }
        }

        self.config
            .allowed_origins
            .first()
            .cloned()
            .unwrap_or_else(|| "*".to_string())
    }

    /// Add CORS headers to response
    fn add_cors_headers(&self, headers: &mut HeaderMap, request_headers: &HeaderMap) {
        // Get origin from request
        let origin = request_headers
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());

        // Add Access-Control-Allow-Origin
        let allowed_origin = self.get_allowed_origin(origin);
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            allowed_origin.parse().unwrap(),
        );

        // Add Access-Control-Allow-Methods
        let methods = self
            .config
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, methods.parse().unwrap());

        // Add Access-Control-Allow-Headers
        if !self.config.allowed_headers.is_empty() {
            let allowed_headers = self.config.allowed_headers.join(", ");
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                allowed_headers.parse().unwrap(),
            );
        }

        // Add Access-Control-Expose-Headers
        if !self.config.exposed_headers.is_empty() {
            let exposed_headers = self.config.exposed_headers.join(", ");
            headers.insert(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                exposed_headers.parse().unwrap(),
            );
        }

        // Add Access-Control-Allow-Credentials
        if self.config.allow_credentials {
            headers.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true".parse().unwrap());
        }

        // Add Access-Control-Max-Age
        if let Some(max_age) = self.config.max_age {
            headers.insert(
                header::ACCESS_CONTROL_MAX_AGE,
                max_age.to_string().parse().unwrap(),
            );
        }
    }
}

impl Middleware for CorsMiddleware {
    fn name(&self) -> &str {
        "cors"
    }

    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>>
    {
        let config = self.config.clone();
        let headers = req.headers().clone();

        Box::pin(async move {
            // Handle preflight OPTIONS request
            if req.method() == Method::OPTIONS {
                debug!("Handling CORS preflight request");

                let mut response = Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Full::new(Bytes::new()))
                    .unwrap();

                let middleware = CorsMiddleware::new(config);
                middleware.add_cors_headers(response.headers_mut(), &headers);

                return Err(response);
            }

            Ok(req)
        })
    }

    fn process_response(
        &self,
        mut response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            debug!("Adding CORS headers to response");

            // Add CORS headers (we don't have request headers here, so use empty)
            let middleware = CorsMiddleware::new(config);
            let empty_headers = HeaderMap::new();
            middleware.add_cors_headers(response.headers_mut(), &empty_headers);

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();
        assert_eq!(config.allowed_origins, vec!["*"]);
        assert_eq!(config.allowed_methods.len(), 6);
        assert!(!config.allow_credentials);
        assert_eq!(config.max_age, Some(3600));
    }

    #[test]
    fn test_origin_allowed() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            ..Default::default()
        };
        let middleware = CorsMiddleware::new(config);

        assert!(middleware.is_origin_allowed("https://example.com"));
        assert!(!middleware.is_origin_allowed("https://evil.com"));
    }

    #[test]
    fn test_wildcard_origin() {
        let middleware = CorsMiddleware::permissive();
        assert!(middleware.is_origin_allowed("https://any-domain.com"));
    }
}
