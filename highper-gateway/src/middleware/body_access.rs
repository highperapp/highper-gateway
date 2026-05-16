//! Middleware request body access and modification
//!
//! Provides utilities for middleware to access and modify request bodies.
//! The challenge with HTTP bodies is they're streams that can only be consumed once.
//! This module solves that by:
//! - Buffering the body into memory
//! - Providing immutable/mutable access to middleware
//! - Reconstructing the request with modified body

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

/// Maximum body size to buffer for middleware access (default: 10 MB)
pub const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Configuration for body access middleware
#[derive(Debug, Clone)]
pub struct BodyAccessConfig {
    /// Maximum body size to buffer in memory
    pub max_body_size: usize,

    /// Whether to enable body buffering
    pub enabled: bool,

    /// Content-Type patterns that should have bodies buffered
    /// Empty means all content types are buffered
    pub buffer_content_types: Vec<String>,
}

impl Default for BodyAccessConfig {
    fn default() -> Self {
        Self {
            max_body_size: DEFAULT_MAX_BODY_SIZE,
            enabled: true,
            buffer_content_types: vec![
                "application/json".to_string(),
                "application/x-www-form-urlencoded".to_string(),
                "text/*".to_string(),
            ],
        }
    }
}

/// Buffered request with accessible body
pub struct BufferedRequest {
    /// The original request (without body)
    request: Request<()>,

    /// Buffered body bytes
    body: Bytes,
}

impl BufferedRequest {
    /// Create a new buffered request by consuming the incoming request
    pub async fn from_request(
        req: Request<Incoming>,
        max_size: usize,
    ) -> Result<Self, BodyAccessError> {
        let (parts, body) = req.into_parts();

        // Collect body with size limit
        let collected = body
            .collect()
            .await
            .map_err(|e| BodyAccessError::BodyReadError(e.to_string()))?;

        let body_bytes = collected.to_bytes();

        // Check size limit
        if body_bytes.len() > max_size {
            return Err(BodyAccessError::BodyTooLarge {
                size: body_bytes.len(),
                limit: max_size,
            });
        }

        debug!("Buffered request body: {} bytes", body_bytes.len());

        Ok(Self {
            request: Request::from_parts(parts, ()),
            body: body_bytes,
        })
    }

    /// Get immutable reference to the body bytes
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// Get mutable reference to the body bytes
    pub fn body_mut(&mut self) -> &mut Bytes {
        &mut self.body
    }

    /// Get reference to the request (without body)
    pub fn request(&self) -> &Request<()> {
        &self.request
    }

    /// Get mutable reference to the request (without body)
    pub fn request_mut(&mut self) -> &mut Request<()> {
        &mut self.request
    }

    /// Get the body as a UTF-8 string (if valid)
    pub fn body_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.to_vec())
    }

    /// Parse the body as JSON
    pub fn body_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// Replace the entire body
    pub fn set_body(&mut self, new_body: impl Into<Bytes>) {
        self.body = new_body.into();
    }

    /// Append to the body
    pub fn append_body(&mut self, additional: impl Into<Bytes>) {
        let mut combined = self.body.to_vec();
        combined.extend_from_slice(&additional.into());
        self.body = Bytes::from(combined);
    }

    /// Convert back into a Request with Full<Bytes> body for forwarding
    ///
    /// Note: This returns Request<Full<Bytes>> instead of Request<Incoming>
    /// because we've buffered the body. The caller needs to handle this conversion
    /// if they need an Incoming body type.
    pub fn into_request_full(self) -> Request<Full<Bytes>> {
        let (parts, _) = self.request.into_parts();
        let body = Full::new(self.body);
        Request::from_parts(parts, body)
    }

    /// Convert back into parts for manual reconstruction
    pub fn into_parts(self) -> (hyper::http::request::Parts, Bytes) {
        let (parts, _) = self.request.into_parts();
        (parts, self.body)
    }

    /// Get the Content-Length of the buffered body
    pub fn content_length(&self) -> usize {
        self.body.len()
    }

    /// Update the Content-Length header to match the body size
    pub fn update_content_length(&mut self) {
        let len = self.body.len();
        self.request.headers_mut().insert(
            hyper::header::CONTENT_LENGTH,
            len.to_string()
                .parse()
                .expect("numeric content-length is valid"),
        );
    }
}

/// Errors that can occur during body access
#[derive(Debug, thiserror::Error)]
pub enum BodyAccessError {
    #[error("Failed to read body: {0}")]
    BodyReadError(String),

    #[error("Body too large: {size} bytes (limit: {limit} bytes)")]
    BodyTooLarge { size: usize, limit: usize },

    #[error("Invalid body format: {0}")]
    InvalidFormat(String),
}

impl BodyAccessError {
    /// Convert to HTTP response
    pub fn to_response(&self) -> Response<Full<Bytes>> {
        let (status, message) = match self {
            Self::BodyReadError(msg) => {
                (StatusCode::BAD_REQUEST, format!("Body read error: {}", msg))
            }
            Self::BodyTooLarge { size, limit } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Body too large: {} bytes (limit: {} bytes)", size, limit),
            ),
            Self::InvalidFormat(msg) => {
                (StatusCode::BAD_REQUEST, format!("Invalid format: {}", msg))
            }
        };

        Response::builder()
            .status(status)
            .header("Content-Type", "text/plain")
            .body(Full::new(Bytes::from(message)))
            .unwrap()
    }
}

/// Middleware that provides body access to downstream middleware
pub trait BodyAccessMiddleware: Send + Sync {
    /// Process a request with body access
    fn process_request_with_body(
        &self,
        req: BufferedRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BufferedRequest, Response<Full<Bytes>>>> + Send>>;

    /// Get middleware name
    fn name(&self) -> &str;
}

/// Helper to check if Content-Type matches patterns
pub fn content_type_matches(content_type: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return true; // No patterns means accept all
    }

    patterns.iter().any(|pattern| {
        if pattern.ends_with("/*") {
            // Wildcard matching
            let prefix = &pattern[..pattern.len() - 2];
            content_type.starts_with(prefix)
        } else {
            // Exact matching (or starts with for type/subtype+params)
            content_type.starts_with(pattern)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Bytes;

    #[test]
    fn test_content_type_matching() {
        let patterns = vec!["application/json".to_string(), "text/*".to_string()];

        assert!(content_type_matches("application/json", &patterns));
        assert!(content_type_matches(
            "application/json; charset=utf-8",
            &patterns
        ));
        assert!(content_type_matches("text/plain", &patterns));
        assert!(content_type_matches("text/html", &patterns));
        assert!(!content_type_matches("image/png", &patterns));
    }

    #[test]
    fn test_empty_patterns() {
        let patterns: Vec<String> = vec![];
        assert!(content_type_matches("anything/here", &patterns));
    }

    #[test]
    fn test_default_config() {
        let config = BodyAccessConfig::default();
        assert_eq!(config.max_body_size, DEFAULT_MAX_BODY_SIZE);
        assert!(config.enabled);
        assert!(!config.buffer_content_types.is_empty());
    }

    // Note: Tests for BufferedRequest require mocking Incoming body type
    // which is difficult without the full hyper server infrastructure.
    // The tests below verify the logic without requiring full integration.

    #[test]
    fn test_body_access_error_status_codes() {
        let err = BodyAccessError::BodyTooLarge {
            size: 1000,
            limit: 500,
        };
        let response = err.to_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let err = BodyAccessError::BodyReadError("test error".to_string());
        let response = err.to_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = BodyAccessError::InvalidFormat("bad format".to_string());
        let response = err.to_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
