//! Gzip compression adapter
//!
//! Provides gzip compression using the flate2 crate.
//! Gzip offers good compression with wide browser support.

use super::compressor::*;
use async_trait::async_trait;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

/// Gzip compressor implementation
pub struct GzipCompressor {
    stats: StatsTracker,
}

impl GzipCompressor {
    /// Create a new Gzip compressor
    pub fn new() -> Self {
        Self {
            stats: StatsTracker::new(),
        }
    }
}

impl Default for GzipCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Compressor for GzipCompressor {
    fn name(&self) -> &'static str {
        "gzip"
    }

    fn encoding(&self) -> &'static str {
        "gzip"
    }

    fn quality(&self) -> f32 {
        0.7 // Good compression, widely supported
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

        // Compress
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(config.level));
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
        self.stats
            .record_compression(original_size as u64, compressed_size as u64);

        Ok(CompressionResult {
            data: compressed,
            original_size,
            compressed_size,
            algorithm: "gzip",
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

        // Use spawn_blocking for CPU-intensive compression
        tokio::task::spawn_blocking(move || {
            let compressor = Self::new();
            compressor.compress(&data, &config)
        })
        .await
        .map_err(|e| CompressionError::Internal(format!("Task join error: {}", e)))?
    }

    fn create_stream(&self, config: &CompressorConfig) -> Option<Box<dyn CompressorStream>> {
        Some(Box::new(GzipStream::new(config.clone())))
    }

    fn stats(&self) -> CompressorStats {
        self.stats.get_stats()
    }

    fn reset_stats(&self) {
        self.stats.reset();
    }
}

/// Streaming Gzip compressor
pub struct GzipStream {
    encoder: GzEncoder<Vec<u8>>,
    level: u32,
}

impl GzipStream {
    fn new(config: CompressorConfig) -> Self {
        Self {
            encoder: GzEncoder::new(Vec::new(), Compression::new(config.level)),
            level: config.level,
        }
    }
}

impl CompressorStream for GzipStream {
    fn compress_chunk(&mut self, chunk: &[u8]) -> Result<Vec<u8>, CompressionError> {
        self.encoder.write_all(chunk)?;
        // Gzip buffers internally, so we return empty
        // Compressed data is retrieved on finalize()
        Ok(Vec::new())
    }

    fn finalize(&mut self) -> Result<Vec<u8>, CompressionError> {
        // Finish encoding and get the compressed data
        let compressed = std::mem::replace(
            &mut self.encoder,
            GzEncoder::new(Vec::new(), Compression::new(self.level)),
        );
        let result = compressed.finish()?;
        Ok(result)
    }

    fn reset(&mut self) -> Result<(), CompressionError> {
        // Create a new encoder
        self.encoder = GzEncoder::new(Vec::new(), Compression::new(self.level));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gzip_basic() {
        let compressor = GzipCompressor::new();
        assert_eq!(compressor.name(), "gzip");
        assert_eq!(compressor.encoding(), "gzip");
        assert!(compressor.quality() > 0.0 && compressor.quality() <= 1.0);
    }

    #[test]
    fn test_gzip_compression() {
        let compressor = GzipCompressor::new();
        let config = CompressorConfig::default();

        // Compressible data (repetitive)
        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress(&data, &config).unwrap();

        assert!(result.is_beneficial());
        assert_eq!(result.original_size, data.len());
        assert!(result.compressed_size < result.original_size);
        assert_eq!(result.algorithm, "gzip");
    }

    #[test]
    fn test_gzip_too_small() {
        let compressor = GzipCompressor::new();
        let config = CompressorConfig::default();

        // Data smaller than min_size
        let data = b"tiny";
        let result = compressor.compress(data, &config);

        assert!(result.is_err());
        match result {
            Err(CompressionError::TooSmall(_, _)) => {}
            _ => panic!("Expected TooSmall error"),
        }
    }

    #[test]
    fn test_gzip_stats() {
        let compressor = GzipCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let _ = compressor.compress(&data, &config).unwrap();

        let stats = compressor.stats();
        assert_eq!(stats.total_compressions, 1);
        assert_eq!(stats.total_bytes_in, data.len() as u64);
        assert!(stats.total_bytes_out > 0);
        assert!(stats.calculate_ratio() < 1.0);
    }

    #[tokio::test]
    async fn test_gzip_async() {
        let compressor = GzipCompressor::new();
        let config = CompressorConfig::default();

        let data = b"Hello, World! ".repeat(100);
        let result = compressor.compress_async(&data, &config).await.unwrap();

        assert!(result.is_beneficial());
        assert_eq!(result.algorithm, "gzip");
    }

    #[test]
    fn test_gzip_stream() {
        let compressor = GzipCompressor::new();
        let config = CompressorConfig::default();

        let mut stream = compressor.create_stream(&config).unwrap();

        // Compress in chunks
        let chunk1 = b"Hello, ";
        let chunk2 = b"World! ".repeat(100);

        stream.compress_chunk(chunk1).unwrap();
        stream.compress_chunk(&chunk2).unwrap();

        let compressed = stream.finalize().unwrap();
        assert!(!compressed.is_empty());
    }
}
