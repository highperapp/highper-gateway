//! Streaming request body validation
//!
//! Provides runtime validation of request bodies as they stream, enforcing:
//! - Maximum body size limits (even without Content-Length header)
//! - Content-Type validation
//! - Streaming rate limits
//! - Early rejection of invalid requests

use bytes::{Buf, Bytes};
use http_body::{Body, Frame};
use hyper::{HeaderMap, StatusCode};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;
use tracing::{debug, warn};

/// Validation error
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    #[error("Request body too large: {actual} bytes (limit: {limit} bytes)")]
    BodyTooLarge { actual: u64, limit: u64 },

    #[error("Invalid Content-Type: {0}")]
    InvalidContentType(String),

    #[error("Content-Type required but not provided")]
    MissingContentType,

    #[error("Body error: {0}")]
    BodyError(String),
}

impl ValidationError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BodyTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::InvalidContentType(_) | Self::MissingContentType => {
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            }
            Self::BodyError(_) => StatusCode::BAD_REQUEST,
        }
    }

    /// Get error message for HTTP response
    pub fn message(&self) -> String {
        self.to_string()
    }
}

/// Streaming validation configuration
#[derive(Debug, Clone)]
pub struct StreamingValidatorConfig {
    /// Maximum body size in bytes (enforced during streaming)
    pub max_body_size: Option<u64>,

    /// Allowed Content-Type patterns (e.g., "application/json", "multipart/*")
    pub allowed_content_types: Vec<String>,

    /// Require Content-Type header
    pub require_content_type: bool,

    /// Whether to enable streaming validation
    pub enabled: bool,
}

impl Default for StreamingValidatorConfig {
    fn default() -> Self {
        Self {
            max_body_size: Some(100 * 1024 * 1024), // 100 MB default
            allowed_content_types: vec![],          // Allow all by default
            require_content_type: false,
            enabled: true,
        }
    }
}

/// Streaming body validator that wraps a body and enforces limits
#[pin_project]
pub struct ValidatedBody<B> {
    #[pin]
    inner: B,
    bytes_read: u64,
    max_size: Option<u64>,
    error: Option<ValidationError>,
}

impl<B> ValidatedBody<B> {
    /// Create a new validated body wrapper
    pub fn new(body: B, max_size: Option<u64>) -> Self {
        Self {
            inner: body,
            bytes_read: 0,
            max_size,
            error: None,
        }
    }

    /// Get the number of bytes read so far
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl<B> Body for ValidatedBody<B>
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    type Data = Bytes;
    type Error = ValidationError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        // If we already have an error, return it
        if let Some(err) = this.error.take() {
            return Poll::Ready(Some(Err(err)));
        }

        match this.inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let chunk_size = data.remaining();

                    // Check if this chunk would exceed the limit (inline check)
                    if let Some(max) = this.max_size {
                        let new_total = *this.bytes_read + chunk_size as u64;
                        if new_total > *max {
                            let e = ValidationError::BodyTooLarge {
                                actual: new_total,
                                limit: *max,
                            };
                            warn!("Streaming body size limit exceeded: {}", e);
                            *this.error = Some(e.clone());
                            return Poll::Ready(Some(Err(e)));
                        }
                    }

                    *this.bytes_read += chunk_size as u64;
                    debug!(
                        "Validated chunk: {} bytes (total: {})",
                        chunk_size, this.bytes_read
                    );
                }

                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                let err = ValidationError::BodyError(e.to_string());
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                debug!("Body streaming complete: {} bytes total", this.bytes_read);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

/// Streaming validator
pub struct StreamingValidator {
    config: StreamingValidatorConfig,
}

impl StreamingValidator {
    /// Create a new streaming validator
    pub fn new(config: StreamingValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate request headers before accepting the body
    pub fn validate_headers(&self, headers: &HeaderMap) -> Result<(), ValidationError> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check Content-Type if required or if allowed types are specified
        if self.config.require_content_type || !self.config.allowed_content_types.is_empty() {
            let content_type = headers
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok());

            if let Some(ct) = content_type {
                // Check against allowed content types
                if !self.config.allowed_content_types.is_empty() {
                    let allowed = self.config.allowed_content_types.iter().any(|pattern| {
                        if pattern.ends_with("/*") {
                            // Wildcard matching (e.g., "application/*")
                            let prefix = &pattern[..pattern.len() - 2];
                            ct.starts_with(prefix)
                        } else {
                            // Exact matching
                            ct.starts_with(pattern)
                        }
                    });

                    if !allowed {
                        warn!("Invalid Content-Type: {}", ct);
                        return Err(ValidationError::InvalidContentType(ct.to_string()));
                    }
                }
            } else if self.config.require_content_type {
                warn!("Missing required Content-Type header");
                return Err(ValidationError::MissingContentType);
            }
        }

        Ok(())
    }

    /// Wrap a body with streaming validation
    pub fn wrap_body<B>(&self, body: B) -> ValidatedBody<B>
    where
        B: Body,
    {
        let max_size = if self.config.enabled {
            self.config.max_body_size
        } else {
            None
        };

        ValidatedBody::new(body, max_size)
    }

    /// Get the configuration
    pub fn config(&self) -> &StreamingValidatorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http_body_util::{BodyExt, Full};
    use hyper::HeaderMap;

    #[test]
    fn test_validation_error_status_codes() {
        let err = ValidationError::BodyTooLarge {
            actual: 1000,
            limit: 500,
        };
        assert_eq!(err.status_code(), StatusCode::PAYLOAD_TOO_LARGE);

        let err = ValidationError::InvalidContentType("text/html".to_string());
        assert_eq!(err.status_code(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let err = ValidationError::MissingContentType;
        assert_eq!(err.status_code(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let err = ValidationError::BodyError("test".to_string());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_default_config() {
        let config = StreamingValidatorConfig::default();
        assert_eq!(config.max_body_size, Some(100 * 1024 * 1024));
        assert!(config.allowed_content_types.is_empty());
        assert!(!config.require_content_type);
        assert!(config.enabled);
    }

    #[test]
    fn test_content_type_validation() {
        let mut config = StreamingValidatorConfig::default();
        config.allowed_content_types = vec!["application/json".to_string()];

        let validator = StreamingValidator::new(config);

        // Valid content type
        let mut headers = HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        assert!(validator.validate_headers(&headers).is_ok());

        // Invalid content type
        headers.insert(hyper::header::CONTENT_TYPE, "text/html".parse().unwrap());
        assert!(validator.validate_headers(&headers).is_err());
    }

    #[test]
    fn test_content_type_wildcard() {
        let mut config = StreamingValidatorConfig::default();
        config.allowed_content_types = vec!["application/*".to_string()];

        let validator = StreamingValidator::new(config);

        let mut headers = HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        assert!(validator.validate_headers(&headers).is_ok());

        headers.insert(
            hyper::header::CONTENT_TYPE,
            "application/xml".parse().unwrap(),
        );
        assert!(validator.validate_headers(&headers).is_ok());

        headers.insert(hyper::header::CONTENT_TYPE, "text/plain".parse().unwrap());
        assert!(validator.validate_headers(&headers).is_err());
    }

    #[tokio::test]
    async fn test_validated_body_within_limit() {
        let data = Bytes::from("Hello, World!");
        let body = Full::new(data.clone());

        let validated = ValidatedBody::new(body, Some(100));

        let collected = validated.collect().await.unwrap();
        let result = collected.to_bytes();

        assert_eq!(result, data);
    }

    #[tokio::test]
    async fn test_validated_body_exceeds_limit() {
        let data = Bytes::from("This is a test message that is too long");
        let body = Full::new(data);

        let validated = ValidatedBody::new(body, Some(10)); // Only allow 10 bytes

        let result = validated.collect().await;
        assert!(result.is_err());

        if let Err(e) = result {
            match e {
                ValidationError::BodyTooLarge { actual, limit } => {
                    assert!(actual > limit);
                    assert_eq!(limit, 10);
                }
                _ => panic!("Expected BodyTooLarge error"),
            }
        }
    }

    #[tokio::test]
    async fn test_validated_body_no_limit() {
        let data = Bytes::from("This can be any size!");
        let body = Full::new(data.clone());

        let validated = ValidatedBody::new(body, None);

        let collected = validated.collect().await.unwrap();
        let result = collected.to_bytes();

        assert_eq!(result, data);
    }
}
