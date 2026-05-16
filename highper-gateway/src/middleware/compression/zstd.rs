//! Zstandard compression adapter
//!
//! Provides Zstandard (zstd) compression using the zstd crate.
//! Zstd offers excellent compression ratio with fast compression/decompression.

use super::compressor::*;
use async_trait::async_trait;

/// Zstandard compressor implementation
pub struct ZstdCompressor {
    stats: StatsTracker,
}

impl ZstdCompressor {
    /// Create a new Zstd compressor
    pub fn new() -> Self {
        Self {
            stats: StatsTracker::new(),
        }
    }
}

impl Default for ZstdCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Compressor for ZstdCompressor {
    fn name(&self) -> &'static str {
        "zstd"
    }

    fn encoding(&self) -> &'static str {
        "zstd"
    }

    fn quality(&self) -> f32 {
        0.85 // Excellent balance of speed and compression
    }

    fn compress(
        &self,
        data: &[u8],
        config: &CompressorConfig,
    ) -> Result<CompressionResult, CompressionError> {
        let original_size = data.len();

        // Check minimum size
        if original_size < config.min_size {
            return Err(CompressionError::TooSmall(original_size, config.min_size));
        }

        // Compress with zstd
        let compressed = zstd::encode_all(data, config.level as i32).map_err(|e| {
            self.stats.record_error();
            e
        })?;

        let compressed_size = compressed.len();
        let ratio = compressed_size as f64 / original_size as f64;

        // Record statistics
        self.stats
            .record_compression(original_size as u64, compressed_size as u64);

        Ok(CompressionResult {
            data: compressed,
            original_size,
            compressed_size,
            algorithm: "zstd",
            compression_ratio: ratio,
        })
    }

    async fn compress_async(
        &self,
        data: &[u8],
        config: &CompressorConfig,
    ) -> Result<CompressionResult, CompressionError> {
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
    fn test_zstd_basic() {
        let compressor = ZstdCompressor::new();
        assert_eq!(compressor.name(), "zstd");
        assert_eq!(compressor.encoding(), "zstd");
        assert_eq!(compressor.quality(), 0.85);
    }

    #[test]
    fn test_zstd_compression() {
        let compressor = ZstdCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress(&data, &config).unwrap();

        assert!(result.is_beneficial());
        assert_eq!(result.algorithm, "zstd");
    }

    #[tokio::test]
    async fn test_zstd_async() {
        let compressor = ZstdCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress_async(&data, &config).await.unwrap();

        assert!(result.is_beneficial());
    }
}
