//! Compression middleware with adapter pattern
//!
//! This module provides a flexible compression system using the adapter pattern.
//! It supports multiple compression algorithms (gzip, brotli, zstd, deflate) and
//! allows easy addition of new algorithms.
//!
//! # Architecture
//!
//! - **Compressor Trait**: Abstract interface for compression algorithms
//! - **Registry**: Global registry for discovering and selecting compressors
//! - **Negotiation**: HTTP content negotiation (Accept-Encoding parsing)
//! - **Middleware**: Integration with request/response pipeline
//!
//! # Example
//!
//! ```rust
//! use highper_gateway::middleware::compression::*;
//!
//! // Use default configuration
//! let middleware = CompressionMiddleware::with_defaults();
//!
//! // Or customize
//! let config = CompressionMiddlewareConfig {
//!     preferences: vec!["br".into(), "gzip".into()],
//!     compressor_config: CompressorConfig {
//!         level: 9,
//!         min_size: 2048,
//!         ..Default::default()
//!     },
//!     enabled: true,
//!     content_types: vec![],
//! };
//! let middleware = CompressionMiddleware::new(config);
//! ```
//!
//! # Adding Custom Compressors
//!
//! ```rust,ignore
//! use highper_gateway::middleware::compression::*;
//! use std::sync::Arc;
//!
//! struct CustomCompressor;
//!
//! #[async_trait::async_trait]
//! impl Compressor for CustomCompressor {
//!     fn name(&self) -> &'static str { "custom" }
//!     fn encoding(&self) -> &'static str { "x-custom" }
//!     fn quality(&self) -> f32 { 0.8 }
//!     fn compress(&self, data: &[u8], config: &CompressorConfig)
//!         -> Result<CompressionResult, CompressionError> {
//!         // Your compression logic here
//!         Ok(CompressionResult {
//!             data: data.to_vec(), // Replace with actual compression
//!             original_size: data.len(),
//!             compressed_size: data.len(),
//!             algorithm: self.name().to_string(),
//!         })
//!     }
//! }
//!
//! // Register globally
//! GLOBAL_COMPRESSOR_REGISTRY.register(Arc::new(CustomCompressor));
//! ```

mod compressor;
mod registry;
mod negotiation;
mod gzip;
mod brotli;
mod zstd;
mod deflate;

// Re-export public API
pub use compressor::{
    Compressor,
    CompressorStream,
    CompressorConfig,
    CompressionResult,
    CompressionError,
    CompressorStats,
    StatsTracker,
};
pub use registry::{
    CompressorRegistry,
    CompressorInfo,
    GLOBAL_COMPRESSOR_REGISTRY,
};
pub use negotiation::{
    parse_accept_encoding,
    select_compressor,
    is_compressible,
    is_already_compressed,
    default_server_preferences,
    EncodingPreference,
};
pub use gzip::GzipCompressor;
pub use brotli::BrotliCompressor;
pub use zstd::ZstdCompressor;
pub use deflate::DeflateCompressor;

use std::sync::Arc;
use tracing::info;

/// Initialize compression system with default compressors
///
/// This function registers all available compression algorithms
/// (gzip, brotli, zstd, deflate) with the global registry.
///
/// Call this during application initialization to ensure all
/// compressors are available.
pub fn init_compression() {
    info!("Initializing compression system");

    // Register all default compressors
    GLOBAL_COMPRESSOR_REGISTRY.register(Arc::new(GzipCompressor::new()));
    GLOBAL_COMPRESSOR_REGISTRY.register(Arc::new(BrotliCompressor::new()));
    GLOBAL_COMPRESSOR_REGISTRY.register(Arc::new(ZstdCompressor::new()));
    GLOBAL_COMPRESSOR_REGISTRY.register(Arc::new(DeflateCompressor::new()));

    let count = GLOBAL_COMPRESSOR_REGISTRY.count();
    let compressors = GLOBAL_COMPRESSOR_REGISTRY.list();

    info!(
        "Compression system initialized with {} compressors: {:?}",
        count, compressors
    );
}

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

/// Compression middleware using adapter pattern
///
/// This middleware automatically compresses responses based on client
/// Accept-Encoding headers and server configuration.
pub struct CompressionMiddleware {
    config: CompressionMiddlewareConfig,
}

impl CompressionMiddleware {
    /// Create new compression middleware with custom configuration
    pub fn new(config: CompressionMiddlewareConfig) -> Self {
        Self { config }
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

// Note: The actual Middleware trait implementation will be in a separate file
// or integrated with the existing middleware system. For now, this provides
// the core compression functionality.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_compression() {
        init_compression();
        assert!(GLOBAL_COMPRESSOR_REGISTRY.count() >= 4);
        assert!(GLOBAL_COMPRESSOR_REGISTRY.has("gzip"));
        assert!(GLOBAL_COMPRESSOR_REGISTRY.has("br"));
        assert!(GLOBAL_COMPRESSOR_REGISTRY.has("zstd"));
        assert!(GLOBAL_COMPRESSOR_REGISTRY.has("deflate"));
    }

    #[test]
    fn test_config_default() {
        let config = CompressionMiddlewareConfig::default();
        assert!(config.enabled);
        assert_eq!(config.preferences.len(), 4);
        assert_eq!(config.preferences[0], "br");
        assert_eq!(config.compressor_config.level, 6);
    }

    #[test]
    fn test_middleware_creation() {
        let middleware = CompressionMiddleware::with_defaults();
        assert!(middleware.config.enabled);
    }

    #[test]
    fn test_should_compress_content_type() {
        let middleware = CompressionMiddleware::with_defaults();
        assert!(middleware.should_compress_content_type("text/html"));
        assert!(middleware.should_compress_content_type("application/json"));
        assert!(!middleware.should_compress_content_type("image/png"));
    }

    #[test]
    fn test_custom_content_types() {
        let config = CompressionMiddlewareConfig {
            content_types: vec!["application/custom".to_string()],
            ..Default::default()
        };
        let middleware = CompressionMiddleware::new(config);

        assert!(middleware.should_compress_content_type("application/custom"));
        assert!(!middleware.should_compress_content_type("text/html"));
    }
}
