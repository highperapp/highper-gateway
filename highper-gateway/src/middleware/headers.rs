//! Security headers middleware

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use hyper::{header, Response};
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    /// Add X-Content-Type-Options: nosniff
    pub x_content_type_options: bool,
    /// Add X-Frame-Options
    pub x_frame_options: Option<String>,
    /// Add X-XSS-Protection
    pub x_xss_protection: Option<String>,
    /// Add Strict-Transport-Security
    pub hsts: Option<String>,
    /// Add Content-Security-Policy
    pub csp: Option<String>,
    /// Add Referrer-Policy
    pub referrer_policy: Option<String>,
    /// Add Permissions-Policy
    pub permissions_policy: Option<String>,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            x_content_type_options: true,
            x_frame_options: Some("DENY".to_string()),
            x_xss_protection: Some("1; mode=block".to_string()),
            hsts: Some("max-age=31536000; includeSubDomains".to_string()),
            csp: None,
            referrer_policy: Some("strict-origin-when-cross-origin".to_string()),
            permissions_policy: None,
        }
    }
}

impl SecurityHeadersConfig {
    /// Create a strict security headers configuration
    pub fn strict() -> Self {
        Self {
            x_content_type_options: true,
            x_frame_options: Some("DENY".to_string()),
            x_xss_protection: Some("1; mode=block".to_string()),
            hsts: Some("max-age=63072000; includeSubDomains; preload".to_string()),
            csp: Some("default-src 'self'".to_string()),
            referrer_policy: Some("no-referrer".to_string()),
            permissions_policy: Some("geolocation=(), microphone=(), camera=()".to_string()),
        }
    }

    /// Create a relaxed security headers configuration
    pub fn relaxed() -> Self {
        Self {
            x_content_type_options: true,
            x_frame_options: Some("SAMEORIGIN".to_string()),
            x_xss_protection: Some("1; mode=block".to_string()),
            hsts: None,
            csp: None,
            referrer_policy: Some("origin-when-cross-origin".to_string()),
            permissions_policy: None,
        }
    }
}

/// Security headers middleware
pub struct SecurityHeadersMiddleware {
    config: SecurityHeadersConfig,
}

impl SecurityHeadersMiddleware {
    /// Create a new security headers middleware
    pub fn new(config: SecurityHeadersConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn default_security() -> Self {
        Self::new(SecurityHeadersConfig::default())
    }

    /// Create with strict config
    pub fn strict() -> Self {
        Self::new(SecurityHeadersConfig::strict())
    }

    /// Create with relaxed config
    pub fn relaxed() -> Self {
        Self::new(SecurityHeadersConfig::relaxed())
    }
}

impl Middleware for SecurityHeadersMiddleware {
    fn name(&self) -> &str {
        "security-headers"
    }

    fn process_response(
        &self,
        mut response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            debug!("Adding security headers to response");

            let headers = response.headers_mut();

            // X-Content-Type-Options
            if config.x_content_type_options {
                headers.insert("x-content-type-options", "nosniff".parse().unwrap());
            }

            // X-Frame-Options
            if let Some(value) = &config.x_frame_options {
                headers.insert("x-frame-options", value.parse().unwrap());
            }

            // X-XSS-Protection
            if let Some(value) = &config.x_xss_protection {
                headers.insert("x-xss-protection", value.parse().unwrap());
            }

            // Strict-Transport-Security (HSTS)
            if let Some(value) = &config.hsts {
                headers.insert(header::STRICT_TRANSPORT_SECURITY, value.parse().unwrap());
            }

            // Content-Security-Policy
            if let Some(value) = &config.csp {
                headers.insert("content-security-policy", value.parse().unwrap());
            }

            // Referrer-Policy
            if let Some(value) = &config.referrer_policy {
                headers.insert(header::REFERRER_POLICY, value.parse().unwrap());
            }

            // Permissions-Policy
            if let Some(value) = &config.permissions_policy {
                headers.insert("permissions-policy", value.parse().unwrap());
            }

            // Add X-Powered-By header (optional branding)
            headers.insert("x-powered-by", "highper-gateway/0.1.0".parse().unwrap());

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SecurityHeadersConfig::default();
        assert!(config.x_content_type_options);
        assert_eq!(config.x_frame_options, Some("DENY".to_string()));
        assert!(config.hsts.is_some());
    }

    #[test]
    fn test_strict_config() {
        let config = SecurityHeadersConfig::strict();
        assert!(config.x_content_type_options);
        assert!(config.csp.is_some());
        assert_eq!(config.referrer_policy, Some("no-referrer".to_string()));
    }

    #[test]
    fn test_relaxed_config() {
        let config = SecurityHeadersConfig::relaxed();
        assert_eq!(config.x_frame_options, Some("SAMEORIGIN".to_string()));
        assert!(config.hsts.is_none());
        assert!(config.csp.is_none());
    }
}
