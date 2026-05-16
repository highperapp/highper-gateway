//! Compressor trait and core types for the compression adapter pattern
//!
//! This module defines the abstract interface for compression algorithms,
//! enabling a flexible, extensible compression system.

use async_trait::async_trait;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Compression result with metadata
#[derive(Debug, Clone)]
pub struct CompressionResult {
    /// Compressed data
    pub data: Vec<u8>,
    /// Original uncompressed size
    pub original_size: usize,
    /// Compressed size
    pub compressed_size: usize,
    /// Algorithm name used
    pub algorithm: &'static str,
    /// Compression ratio (compressed / original)
    pub compression_ratio: f64,
}

impl CompressionResult {
    /// Check if compression was beneficial (reduced size)
    pub fn is_beneficial(&self) -> bool {
        self.compressed_size < self.original_size
    }

    /// Calculate space saved in bytes
    pub fn space_saved(&self) -> i64 {
        self.original_size as i64 - self.compressed_size as i64
    }

    /// Calculate percentage saved
    pub fn percentage_saved(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        ((self.original_size - self.compressed_size) as f64 / self.original_size as f64) * 100.0
    }
}

/// Compression configuration per algorithm
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    /// Compression level (1-9, algorithm-specific interpretation)
    /// 1 = fastest, 9 = best compression
    pub level: u32,
    /// Minimum size to compress (bytes)
    pub min_size: usize,
    /// Buffer size for streaming
    pub buffer_size: usize,
    /// Enable streaming mode (if supported by algorithm)
    pub streaming: bool,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            level: 6,
            min_size: 1024,    // 1 KB
            buffer_size: 8192, // 8 KB
            streaming: false,
        }
    }
}

/// Compressor trait - adapter pattern for compression algorithms
///
/// This trait defines the interface that all compression algorithms must implement.
/// It supports both synchronous and asynchronous compression, as well as streaming.
#[async_trait]
pub trait Compressor: Send + Sync {
    /// Algorithm name (e.g., "gzip", "brotli", "zstd", "deflate")
    fn name(&self) -> &'static str;

    /// Algorithm encoding name for Content-Encoding header
    /// (e.g., "gzip", "br", "zstd", "deflate")
    fn encoding(&self) -> &'static str;

    /// Quality preference (0.0-1.0, higher = prefer this algorithm)
    /// Used for algorithm selection when multiple options are available
    fn quality(&self) -> f32;

    /// Check if algorithm is available on this platform
    fn is_available(&self) -> bool {
        true
    }

    /// Compress data (blocking, for small payloads)
    fn compress(
        &self,
        data: &[u8],
        config: &CompressorConfig,
    ) -> Result<CompressionResult, CompressionError>;

    /// Compress async (for large payloads)
    /// Default implementation uses spawn_blocking
    async fn compress_async(
        &self,
        data: &[u8],
        config: &CompressorConfig,
    ) -> Result<CompressionResult, CompressionError> {
        let data = data.to_vec();
        let config = config.clone();
        let name = self.name();

        // Create a temporary compressor for the blocking task
        // In actual implementation, this would clone self or use Arc
        tokio::task::spawn_blocking(move || {
            Err(CompressionError::Unsupported(format!(
                "{} async compression not implemented",
                name
            )))
        })
        .await
        .map_err(|e| CompressionError::Internal(e.to_string()))?
    }

    /// Create streaming compressor (optional)
    fn create_stream(&self, _config: &CompressorConfig) -> Option<Box<dyn CompressorStream>> {
        None
    }

    /// Get compression statistics (for monitoring)
    fn stats(&self) -> CompressorStats {
        CompressorStats::default()
    }

    /// Reset statistics
    fn reset_stats(&self) {}
}

/// Streaming compression interface
pub trait CompressorStream: Send {
    /// Compress a chunk of data
    /// Returns compressed data (may be empty if buffering)
    fn compress_chunk(&mut self, chunk: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Finalize compression and get remaining data
    fn finalize(&mut self) -> Result<Vec<u8>, CompressionError>;

    /// Reset the stream for reuse
    fn reset(&mut self) -> Result<(), CompressionError> {
        Err(CompressionError::Unsupported(
            "Stream reset not supported".to_string(),
        ))
    }
}

/// Compression error types
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Compression failed: {0}")]
    Failed(String),

    #[error("Algorithm not supported: {0}")]
    Unsupported(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Data too small to compress (size: {0}, minimum: {1})")]
    TooSmall(usize, usize),
}

/// Compression statistics for monitoring
#[derive(Debug, Clone)]
pub struct CompressorStats {
    /// Total number of compression operations
    pub total_compressions: u64,
    /// Total bytes input (uncompressed)
    pub total_bytes_in: u64,
    /// Total bytes output (compressed)
    pub total_bytes_out: u64,
    /// Average compression ratio
    pub average_ratio: f64,
    /// Number of compression errors
    pub errors: u64,
    /// Number of times compression was skipped (increased size)
    pub skipped: u64,
}

impl Default for CompressorStats {
    fn default() -> Self {
        Self {
            total_compressions: 0,
            total_bytes_in: 0,
            total_bytes_out: 0,
            average_ratio: 0.0,
            errors: 0,
            skipped: 0,
        }
    }
}

impl CompressorStats {
    /// Calculate current average compression ratio
    pub fn calculate_ratio(&self) -> f64 {
        if self.total_bytes_in == 0 {
            0.0
        } else {
            self.total_bytes_out as f64 / self.total_bytes_in as f64
        }
    }

    /// Calculate percentage saved
    pub fn percentage_saved(&self) -> f64 {
        if self.total_bytes_in == 0 {
            return 0.0;
        }
        let saved = self.total_bytes_in.saturating_sub(self.total_bytes_out);
        (saved as f64 / self.total_bytes_in as f64) * 100.0
    }

    /// Calculate total space saved in bytes
    pub fn total_saved(&self) -> i64 {
        self.total_bytes_in as i64 - self.total_bytes_out as i64
    }
}

impl fmt::Display for CompressorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Compressions: {}, Bytes: {}->{} ({:.1}% saved), Errors: {}, Skipped: {}",
            self.total_compressions,
            self.total_bytes_in,
            self.total_bytes_out,
            self.percentage_saved(),
            self.errors,
            self.skipped
        )
    }
}

/// Thread-safe statistics tracker
pub struct StatsTracker {
    total_compressions: AtomicU64,
    total_bytes_in: AtomicU64,
    total_bytes_out: AtomicU64,
    errors: AtomicU64,
    skipped: AtomicU64,
}

impl StatsTracker {
    pub fn new() -> Self {
        Self {
            total_compressions: AtomicU64::new(0),
            total_bytes_in: AtomicU64::new(0),
            total_bytes_out: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
        }
    }

    pub fn record_compression(&self, bytes_in: u64, bytes_out: u64) {
        self.total_compressions.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_in.fetch_add(bytes_in, Ordering::Relaxed);
        self.total_bytes_out.fetch_add(bytes_out, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> CompressorStats {
        let total_in = self.total_bytes_in.load(Ordering::Relaxed);
        let total_out = self.total_bytes_out.load(Ordering::Relaxed);

        CompressorStats {
            total_compressions: self.total_compressions.load(Ordering::Relaxed),
            total_bytes_in: total_in,
            total_bytes_out: total_out,
            average_ratio: if total_in > 0 {
                total_out as f64 / total_in as f64
            } else {
                0.0
            },
            errors: self.errors.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.total_compressions.store(0, Ordering::Relaxed);
        self.total_bytes_in.store(0, Ordering::Relaxed);
        self.total_bytes_out.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.skipped.store(0, Ordering::Relaxed);
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_result() {
        let result = CompressionResult {
            data: vec![0u8; 100],
            original_size: 200,
            compressed_size: 100,
            algorithm: "test",
            compression_ratio: 0.5,
        };

        assert!(result.is_beneficial());
        assert_eq!(result.space_saved(), 100);
        assert_eq!(result.percentage_saved(), 50.0);
    }

    #[test]
    fn test_compressor_config() {
        let config = CompressorConfig::default();
        assert_eq!(config.level, 6);
        assert_eq!(config.min_size, 1024);
        assert_eq!(config.buffer_size, 8192);
        assert!(!config.streaming);
    }

    #[test]
    fn test_stats_tracker() {
        let tracker = StatsTracker::new();
        tracker.record_compression(1000, 500);
        tracker.record_compression(2000, 1000);
        tracker.record_error();

        let stats = tracker.get_stats();
        assert_eq!(stats.total_compressions, 2);
        assert_eq!(stats.total_bytes_in, 3000);
        assert_eq!(stats.total_bytes_out, 1500);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.calculate_ratio(), 0.5);
        assert_eq!(stats.percentage_saved(), 50.0);
    }

    #[test]
    fn test_compression_error() {
        let err = CompressionError::TooSmall(500, 1024);
        assert!(err.to_string().contains("too small"));

        let err = CompressionError::Unsupported("custom".to_string());
        assert!(err.to_string().contains("not supported"));
    }
}
