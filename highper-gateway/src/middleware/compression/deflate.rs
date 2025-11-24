//! Deflate compression adapter
//!
//! Provides Deflate compression using the flate2 crate.
//! Deflate is the raw compression format (without gzip wrapper).

use super::compressor::*;
use async_trait::async_trait;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;

/// Deflate compressor implementation
pub struct DeflateCompressor {
    stats: StatsTracker,
}

impl DeflateCompressor {
    /// Create a new Deflate compressor
    pub fn new() -> Self {
        Self {
            stats: StatsTracker::new(),
        }
    }
}

impl Default for DeflateCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Compressor for DeflateCompressor {
    fn name(&self) -> &'static str {
        "deflate"
    }

    fn encoding(&self) -> &'static str {
        "deflate"
    }

    fn quality(&self) -> f32 {
        0.6  // Legacy support, lower quality preference
    }

    fn compress(&self, data: &[u8], config: &CompressorConfig)
        -> Result<CompressionResult, CompressionError>
    {
        let original_size = data.len();

        // Check minimum size
        if original_size < config.min_size {
            return Err(CompressionError::TooSmall(original_size, config.min_size));
        }

        // Compress
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(config.level));
        encoder.write_all(data).map_err(|e| {
            self.stats.record_error();
            e
        })?;

        let compressed = encoder.finish().map_err(|e| {
            self.stats.record_error();
            e
        })?;

        let compressed_size = compressed.len();
        let ratio = compressed_size as f64 / original_size as f64;

        // Record statistics
        self.stats.record_compression(original_size as u64, compressed_size as u64);

        Ok(CompressionResult {
            data: compressed,
            original_size,
            compressed_size,
            algorithm: "deflate",
            compression_ratio: ratio,
        })
    }

    async fn compress_async(&self, data: &[u8], config: &CompressorConfig)
        -> Result<CompressionResult, CompressionError>
    {
        let data = data.to_vec();
        let config = config.clone();

        tokio::task::spawn_blocking(move || {
            let compressor = Self::new();
            compressor.compress(&data, &config)
        })
        .await
        .map_err(|e| CompressionError::Internal(format!("Task join error: {}", e)))?
    }

    fn stats(&self) -> CompressorStats {
        self.stats.get_stats()
    }

    fn reset_stats(&self) {
        self.stats.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflate_basic() {
        let compressor = DeflateCompressor::new();
        assert_eq!(compressor.name(), "deflate");
        assert_eq!(compressor.encoding(), "deflate");
        assert_eq!(compressor.quality(), 0.6);
    }

    #[test]
    fn test_deflate_compression() {
        let compressor = DeflateCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress(&data, &config).unwrap();

        assert!(result.is_beneficial());
        assert_eq!(result.algorithm, "deflate");
    }

    #[tokio::test]
    async fn test_deflate_async() {
        let compressor = DeflateCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress_async(&data, &config).await.unwrap();

        assert!(result.is_beneficial());
    }
}
