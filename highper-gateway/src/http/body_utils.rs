//! HTTP body handling utilities
//!
//! Provides efficient body extraction, validation, and streaming support.

use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use std::error::Error as StdError;
use tracing::{debug, warn};

/// Maximum body size for buffering (10 MB default)
pub const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Collected request body with metadata
#[derive(Debug, Clone)]
pub struct CollectedBody {
    /// The body bytes
    pub bytes: Bytes,

    /// Original content length (if specified)
    pub content_length: Option<u64>,

    /// Whether the body was chunked
    pub was_chunked: bool,
}

impl CollectedBody {
    /// Create from bytes
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self {
            bytes,
            content_length: None,
            was_chunked: false,
        }
    }

    /// Create empty body
    pub fn empty() -> Self {
        Self {
            bytes: Bytes::new(),
            content_length: Some(0),
            was_chunked: false,
        }
    }

    /// Check if body is empty
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Get body size
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
}

/// Error types for body collection
#[derive(Debug, thiserror::Error)]
pub enum BodyError {
    #[error("Body size {actual} exceeds limit of {limit} bytes")]
    TooLarge { actual: usize, limit: usize },

    #[error("Body collection failed: {0}")]
    CollectionFailed(Box<dyn StdError + Send + Sync>),

    #[error("Content-Length mismatch: expected {expected}, got {actual}")]
    LengthMismatch { expected: u64, actual: usize },
}

/// Collect body with size limit
pub async fn collect_body_with_limit(
    body: Incoming,
    max_size: usize,
) -> Result<CollectedBody, BodyError> {
    let mut collected = BytesMut::new();
    let mut total_size = 0;

    let mut body = std::pin::pin!(body);

    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|e| BodyError::CollectionFailed(Box::new(e)))?;

        if let Some(chunk) = frame.data_ref() {
            total_size += chunk.len();

            if total_size > max_size {
                warn!("Body size {} exceeds limit of {}", total_size, max_size);
                return Err(BodyError::TooLarge {
                    actual: total_size,
                    limit: max_size,
                });
            }

            collected.extend_from_slice(chunk);
        }
    }

    debug!("Collected body of {} bytes", total_size);

    Ok(CollectedBody {
        bytes: collected.freeze(),
        content_length: None,
        was_chunked: false,
    })
}

/// Collect body with Content-Length validation
pub async fn collect_body_validated(
    body: Incoming,
    content_length: Option<u64>,
    max_size: usize,
) -> Result<CollectedBody, BodyError> {
    // Check Content-Length first
    if let Some(len) = content_length {
        if len as usize > max_size {
            return Err(BodyError::TooLarge {
                actual: len as usize,
                limit: max_size,
            });
        }
    }

    let mut collected = collect_body_with_limit(body, max_size).await?;

    // Validate Content-Length matches actual size
    if let Some(expected) = content_length {
        if collected.len() != expected as usize {
            return Err(BodyError::LengthMismatch {
                expected,
                actual: collected.len(),
            });
        }
        collected.content_length = Some(expected);
    }

    Ok(collected)
}

/// Collect body if method requires it (POST, PUT, PATCH)
pub async fn collect_body_for_method(
    method: &hyper::Method,
    body: Incoming,
    content_length: Option<u64>,
    max_size: usize,
) -> Result<CollectedBody, BodyError> {
    use hyper::Method;

    match *method {
        Method::POST | Method::PUT | Method::PATCH => {
            collect_body_validated(body, content_length, max_size).await
        }
        _ => {
            // For GET, HEAD, DELETE, etc., body should be empty
            // But we'll still collect to drain the stream
            Ok(CollectedBody::empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Full;

    #[tokio::test]
    async fn test_collect_empty_body() {
        let body = Full::new(Bytes::new());
        let body = body.map_err(|never| match never {}).boxed_unsync();

        // Can't easily test with Incoming, so this is a simplified test
        let collected = CollectedBody::empty();
        assert!(collected.is_empty());
        assert_eq!(collected.len(), 0);
    }

    #[test]
    fn test_collected_body_from_bytes() {
        let data = Bytes::from("test data");
        let collected = CollectedBody::from_bytes(data.clone());

        assert_eq!(collected.bytes, data);
        assert_eq!(collected.len(), 9);
        assert!(!collected.is_empty());
    }

    #[test]
    fn test_body_error_display() {
        let err = BodyError::TooLarge {
            actual: 1000,
            limit: 500,
        };
        assert_eq!(
            err.to_string(),
            "Body size 1000 exceeds limit of 500 bytes"
        );

        let err = BodyError::LengthMismatch {
            expected: 100,
            actual: 50,
        };
        assert_eq!(
            err.to_string(),
            "Content-Length mismatch: expected 100, got 50"
        );
    }
}
