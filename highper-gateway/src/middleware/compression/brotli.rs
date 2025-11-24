//! Brotli compression adapter
//!
//! Provides Brotli compression using the brotli crate.
//! Brotli offers the best compression ratio among common algorithms.

use super::compressor::*;
use async_trait::async_trait;
use std::io::Cursor;

/// Brotli compressor implementation
pub struct BrotliCompressor {
    stats: StatsTracker,
}

impl BrotliCompressor {
    /// Create a new Brotli compressor
    pub fn new() -> Self {
        Self {
            stats: StatsTracker::new(),
        }
    }
}

impl Default for BrotliCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Compressor for BrotliCompressor {
    fn name(&self) -> &'static str {
        "brotli"
    }

    fn encoding(&self) -> &'static str {
        "br"
    }

    fn quality(&self) -> f32 {
        0.9  // Best compression ratio
    }

    fn compress(&self, data: &[u8], config: &CompressorConfig)
        -> Result<CompressionResult, CompressionError>
    {
        let original_size = data.len();

        // Check minimum size
        if original_size < config.min_size {
            return Err(CompressionError::TooSmall(original_size, config.min_size));
        }

        let mut output = Vec::new();
        let mut reader = Cursor::new(data);

        // Compress with brotli
        let params = brotli::enc::BrotliEncoderParams {
            quality: config.level as i32,
            ..Default::default()
        };

        brotli::BrotliCompress(&mut reader, &mut output, &params).map_err(|e| {
            self.stats.record_error();
            CompressionError::Failed(format!("Brotli compression failed: {}", e))
        })?;

        let compressed_size = output.len();
        let ratio = compressed_size as f64 / original_size as f64;

        // Record statistics
        self.stats.record_compression(original_size as u64, compressed_size as u64);

        Ok(CompressionResult {
            data: output,
            original_size,
            compressed_size,
            algorithm: "brotli",
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
    fn test_brotli_basic() {
        let compressor = BrotliCompressor::new();
        assert_eq!(compressor.name(), "brotli");
        assert_eq!(compressor.encoding(), "br");
        assert_eq!(compressor.quality(), 0.9);
    }

    #[test]
    fn test_brotli_compression() {
        let compressor = BrotliCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress(&data, &config).unwrap();

        assert!(result.is_beneficial());
        assert_eq!(result.algorithm, "brotli");
    }

    #[tokio::test]
    async fn test_brotli_async() {
        let compressor = BrotliCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress_async(&data, &config).await.unwrap();

        assert!(result.is_beneficial());
    }
}
