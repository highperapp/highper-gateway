//! Request size limit middleware
//!
//! Prevents excessively large requests that could lead to resource exhaustion.
//! Checks Content-Length header and rejects requests exceeding the limit.

use hyper::{Request, Response, StatusCode, body::Incoming};
use http_body_util::Full;
use bytes::Bytes;
use tracing::{debug, warn};

/// Request size limit configuration
#[derive(Debug, Clone)]
pub struct RequestSizeLimitConfig {
    /// Maximum request body size in bytes
    pub max_body_size: u64,

    /// Whether to enable size limiting
    pub enabled: bool,

    /// Custom error message
    pub error_message: Option<String>,
}

impl Default for RequestSizeLimitConfig {
    fn default() -> Self {
        Self {
            max_body_size: 10 * 1024 * 1024, // 10 MB default
            enabled: true,
            error_message: None,
        }
    }
}

/// Request size limiter
pub struct RequestSizeLimiter {
    config: RequestSizeLimitConfig,
}

impl RequestSizeLimiter {
    /// Create a new request size limiter
    pub fn new(config: RequestSizeLimitConfig) -> Self {
        Self { config }
    }

    /// Check if request size is within limits
    pub fn check_request_size(&self, req: &Request<Incoming>) -> Result<(), Response<Full<Bytes>>> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check Content-Length header
        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(length_str) = content_length.to_str() {
                if let Ok(length) = length_str.parse::<u64>() {
                    if length > self.config.max_body_size {
                        warn!(
                            "Request body size {} exceeds limit of {} bytes",
                            length, self.config.max_body_size
                        );
                        return Err(self.payload_too_large_response(length));
                    }
                    debug!("Request body size {} is within limit", length);
                }
            }
        }

        Ok(())
    }

    /// Create payload too large response
    fn payload_too_large_response(&self, actual_size: u64) -> Response<Full<Bytes>> {
        let message = self.config.error_message.clone().unwrap_or_else(|| {
            format!(
                "Request body too large: {} bytes (limit: {} bytes)",
                actual_size, self.config.max_body_size
            )
        });

        Response::builder()
            .status(StatusCode::PAYLOAD_TOO_LARGE)
            .header("Content-Type", "text/plain")
            .body(Full::new(Bytes::from(message)))
            .unwrap()
    }

    /// Get maximum allowed size
    pub fn max_size(&self) -> u64 {
        self.config.max_body_size
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

/// Helper to format byte sizes in human-readable format
pub fn format_byte_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_byte_size() {
        assert_eq!(format_byte_size(512), "512.00 B");
        assert_eq!(format_byte_size(1024), "1.00 KB");
        assert_eq!(format_byte_size(1536), "1.50 KB");
        assert_eq!(format_byte_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_byte_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_request_size_limiter_config() {
        let config = RequestSizeLimitConfig {
            max_body_size: 1024,
            enabled: true,
            error_message: None,
        };

        let limiter = RequestSizeLimiter::new(config);

        assert_eq!(limiter.max_size(), 1024);
        assert!(limiter.is_enabled());
    }

    #[test]
    fn test_request_size_limiter_disabled() {
        let config = RequestSizeLimitConfig {
            max_body_size: 1024,
            enabled: false,
            error_message: None,
        };

        let limiter = RequestSizeLimiter::new(config);

        assert_eq!(limiter.max_size(), 1024);
        assert!(!limiter.is_enabled());
    }

    #[test]
    fn test_default_config() {
        let config = RequestSizeLimitConfig::default();

        assert_eq!(config.max_body_size, 10 * 1024 * 1024); // 10 MB
        assert!(config.enabled);
        assert!(config.error_message.is_none());
    }

    #[test]
    fn test_custom_error_message() {
        let config = RequestSizeLimitConfig {
            max_body_size: 1024,
            enabled: true,
            error_message: Some("Custom error message".to_string()),
        };

        let limiter = RequestSizeLimiter::new(config);
        let response = limiter.payload_too_large_response(2048);

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
