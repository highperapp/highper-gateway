//! Compression middleware using the adapter pattern
//!
//! This middleware integrates the compression adapter system into the middleware chain.
//! It automatically compresses responses based on Accept-Encoding headers and content type.

use super::{Middleware, MiddlewareResult};
use super::compression::{
    select_compressor, is_compressible, is_already_compressed,
    CompressorConfig,
};
use crate::http::ResponseBody;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, header};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, warn};

/// Compression middleware configuration
#[derive(Debug, Clone)]
pub struct CompressionMiddlewareConfig {
    /// Server compression preferences (in order of preference)
    pub preferences: Vec<String>,
    /// Compressor configuration (level, min_size, etc.)
    pub compressor_config: CompressorConfig,
    /// Enable compression
    pub enabled: bool,
    /// Custom content types to compress (if empty, use default list)
    pub content_types: Vec<String>,
}

impl Default for CompressionMiddlewareConfig {
    fn default() -> Self {
        Self {
            preferences: vec![
                "br".to_string(),      // Brotli (best compression)
                "zstd".to_string(),    // Zstandard (fast + good compression)
                "gzip".to_string(),    // Gzip (widely supported)
                "deflate".to_string(), // Deflate (legacy)
            ],
            compressor_config: CompressorConfig::default(),
            enabled: true,
            content_types: Vec::new(),
        }
    }
}

/// Compression middleware
pub struct CompressionMiddleware {
    config: CompressionMiddlewareConfig,
    // Store Accept-Encoding from request for use in response processing
    accept_encoding: std::sync::Arc<tokio::sync::RwLock<Option<String>>>,
}

impl CompressionMiddleware {
    /// Create new compression middleware with custom configuration
    pub fn new(config: CompressionMiddlewareConfig) -> Self {
        Self {
            config,
            accept_encoding: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Create compression middleware with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CompressionMiddlewareConfig::default())
    }

    /// Check if content type should be compressed
    fn should_compress_content_type(&self, content_type: &str) -> bool {
        if self.config.content_types.is_empty() {
            // Use default compressible types
            is_compressible(content_type)
        } else {
            // Use custom list
            let content_type_lower = content_type.to_lowercase();
            self.config
                .content_types
                .iter()
                .any(|t| content_type_lower.starts_with(&t.to_lowercase()))
        }
    }
}

impl Middleware for CompressionMiddleware {
    fn name(&self) -> &str {
        "compression"
    }

    fn process_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<hyper::Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>> {
        let accept_encoding = self.accept_encoding.clone();

        Box::pin(async move {
            // Extract Accept-Encoding header from request
            if let Some(encoding) = req.headers().get(header::ACCEPT_ENCODING) {
                if let Ok(encoding_str) = encoding.to_str() {
                    *accept_encoding.write().await = Some(encoding_str.to_string());
                    debug!("Captured Accept-Encoding: {}", encoding_str);
                }
            } else {
                // Clear any previous value
                *accept_encoding.write().await = None;
            }

            Ok(req)
        })
    }

    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();
        let should_compress = self.config.enabled;
        let content_types = self.config.content_types.clone();
        let accept_encoding = self.accept_encoding.clone();

        Box::pin(async move {
            if !should_compress {
                debug!("Compression disabled, skipping");
                return Ok(response);
            }

            // Extract headers
            let (parts, body) = response.into_parts();

            // Check if already compressed
            if let Some(encoding) = parts.headers.get(header::CONTENT_ENCODING) {
                if is_already_compressed(encoding.to_str().ok()) {
                    debug!("Response already compressed, skipping");
                    return Ok(Response::from_parts(parts, body));
                }
            }

            // Check content type
            let content_type = parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let should_compress_ct = if content_types.is_empty() {
                is_compressible(content_type)
            } else {
                let content_type_lower = content_type.to_lowercase();
                content_types
                    .iter()
                    .any(|t| content_type_lower.starts_with(&t.to_lowercase()))
            };

            if !should_compress_ct {
                debug!("Content-Type '{}' not compressible, skipping", content_type);
                return Ok(Response::from_parts(parts, body));
            }

            // Get Accept-Encoding from the stored request header
            let accept_encoding_value = accept_encoding.read().await.clone();
            let accept_encoding_str = accept_encoding_value.as_deref().unwrap_or("*");

            // Convert preferences Vec<String> to Vec<&str>
            let server_prefs: Vec<&str> = config.preferences.iter().map(|s| s.as_str()).collect();

            // Select compressor
            let compressor = match select_compressor(accept_encoding_str, &server_prefs) {
                Some(c) => c,
                None => {
                    debug!("No suitable compressor found, sending uncompressed");
                    return Ok(Response::from_parts(parts, body));
                }
            };

            debug!("Selected compressor: {}", compressor.name());

            // Get body bytes
            // Full<Bytes> wraps Bytes, we need to extract it by collecting
            use http_body_util::BodyExt as _;
            let body_bytes = match body.collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(_) => {
                    // If collection fails, return uncompressed
                    warn!("Failed to collect body for compression");
                    // We can't reconstruct the original body easily, so skip compression
                    return Ok(Response::builder()
                        .status(parts.status)
                        .version(parts.version)
                        .body(ResponseBody::empty())
                        .unwrap());
                }
            };

            // Check minimum size
            if body_bytes.len() < config.compressor_config.min_size {
                debug!(
                    "Body size {} < min_size {}, skipping compression",
                    body_bytes.len(),
                    config.compressor_config.min_size
                );
                return Ok(Response::from_parts(parts, ResponseBody::buffered(body_bytes)));
            }

            // Compress
            match compressor.compress(&body_bytes, &config.compressor_config) {
                Ok(result) => {
                    if result.is_beneficial() {
                        debug!(
                            "Compressed {} -> {} bytes ({:.1}% saved) using {}",
                            result.original_size,
                            result.compressed_size,
                            result.percentage_saved(),
                            compressor.name()
                        );

                        // Build new response with compressed body
                        let mut new_response = Response::from_parts(parts, ResponseBody::buffered(Bytes::from(result.data)));

                        // Add Content-Encoding header
                        new_response.headers_mut().insert(
                            header::CONTENT_ENCODING,
                            compressor.encoding().parse().expect("encoding name is valid header value"),
                        );

                        // Update Content-Length
                        new_response.headers_mut().insert(
                            header::CONTENT_LENGTH,
                            result.compressed_size.to_string().parse().expect("numeric content-length is valid"),
                        );

                        Ok(new_response)
                    } else {
                        debug!("Compression not beneficial, sending uncompressed");
                        Ok(Response::from_parts(parts, ResponseBody::buffered(body_bytes)))
                    }
                }
                Err(e) => {
                    warn!("Compression failed: {:?}, sending uncompressed", e);
                    Ok(Response::from_parts(parts, ResponseBody::buffered(body_bytes)))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::StatusCode;

    #[tokio::test]
    async fn test_compression_middleware_disabled() {
        let config = CompressionMiddlewareConfig {
            enabled: false,
            ..Default::default()
        };
        let middleware = CompressionMiddleware::new(config);

        let body = "Hello, World!".repeat(100);
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(ResponseBody::buffered(Bytes::from(body.clone())))
            .unwrap();

        let result = middleware.process_response(response).await.unwrap();

        // Collect body bytes
        use http_body_util::BodyExt as _;
        let result_body = result.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(result_body, Bytes::from(body));
    }

    #[tokio::test]
    async fn test_compression_middleware_already_compressed() {
        let middleware = CompressionMiddleware::with_defaults();

        let body = "Hello, World!".repeat(100);
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::CONTENT_ENCODING, "gzip")
            .body(ResponseBody::buffered(Bytes::from(body.clone())))
            .unwrap();

        let result = middleware.process_response(response).await.unwrap();

        // Should not double-compress
        assert_eq!(
            result.headers().get(header::CONTENT_ENCODING).unwrap(),
            "gzip"
        );
    }

    #[tokio::test]
    async fn test_compression_middleware_non_compressible() {
        let middleware = CompressionMiddleware::with_defaults();

        let body = vec![0u8; 1000]; // Binary data
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .body(ResponseBody::buffered(Bytes::from(body.clone())))
            .unwrap();

        let result = middleware.process_response(response).await.unwrap();

        // Should not compress
        assert!(result.headers().get(header::CONTENT_ENCODING).is_none());
    }
}
