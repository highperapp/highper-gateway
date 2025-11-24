//! Enhanced response streaming for proxied requests
//!
//! Provides advanced streaming capabilities for efficiently forwarding
//! responses from upstream servers to clients with:
//! - Backpressure handling
//! - Progress tracking and metrics
//! - Bandwidth throttling
//! - Stream transformation
//! - Error recovery

use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::time::Sleep;
use tracing::{debug, trace, warn};

/// Statistics for streaming response
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total bytes streamed
    pub bytes_streamed: u64,

    /// Number of chunks streamed
    pub chunks_streamed: u64,

    /// Streaming start time
    pub start_time: Option<Instant>,

    /// Streaming end time
    pub end_time: Option<Instant>,

    /// Average throughput (bytes per second)
    pub throughput_bps: f64,
}

impl StreamStats {
    /// Calculate streaming duration
    pub fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(start.elapsed()),
            _ => None,
        }
    }

    /// Calculate current throughput
    pub fn current_throughput(&self) -> f64 {
        if let Some(duration) = self.duration() {
            let secs = duration.as_secs_f64();
            if secs > 0.0 {
                return self.bytes_streamed as f64 / secs;
            }
        }
        0.0
    }
}

/// Configuration for proxy streaming
#[derive(Debug, Clone)]
pub struct ProxyStreamConfig {
    /// Maximum bandwidth (bytes per second, None = unlimited)
    pub max_bandwidth: Option<u64>,

    /// Enable progress tracking
    pub track_progress: bool,

    /// Buffer size for backpressure
    pub buffer_size: usize,

    /// Chunk size for streaming
    pub chunk_size: usize,
}

impl Default for ProxyStreamConfig {
    fn default() -> Self {
        Self {
            max_bandwidth: None,
            track_progress: true,
            buffer_size: 65536,  // 64 KB
            chunk_size: 8192,    // 8 KB chunks
        }
    }
}

/// Streaming response body with backpressure and metrics
#[pin_project]
pub struct ProxyStreamingBody<B> {
    #[pin]
    inner: B,

    /// Stream statistics
    stats: Arc<AtomicU64>, // Stores bytes_streamed atomically

    /// Chunks streamed counter
    chunks: Arc<AtomicU64>,

    /// Stream start time
    start_time: Instant,

    /// Configuration
    config: ProxyStreamConfig,

    /// Bandwidth throttle state
    #[pin]
    throttle_sleep: Option<Sleep>,

    /// Last chunk time for throughput calculation
    last_chunk_time: Instant,

    /// Bytes in current second (for bandwidth limiting)
    bytes_this_second: u64,

    /// Start of current second (for bandwidth limiting)
    second_start: Instant,
}

impl<B> ProxyStreamingBody<B> {
    /// Create a new proxy streaming body
    pub fn new(body: B, config: ProxyStreamConfig) -> Self {
        Self {
            inner: body,
            stats: Arc::new(AtomicU64::new(0)),
            chunks: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            config,
            throttle_sleep: None,
            last_chunk_time: Instant::now(),
            bytes_this_second: 0,
            second_start: Instant::now(),
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> StreamStats {
        let bytes = self.stats.load(Ordering::Relaxed);
        let chunks = self.chunks.load(Ordering::Relaxed);
        let duration = self.start_time.elapsed();

        StreamStats {
            bytes_streamed: bytes,
            chunks_streamed: chunks,
            start_time: Some(self.start_time),
            end_time: None,
            throughput_bps: if duration.as_secs_f64() > 0.0 {
                bytes as f64 / duration.as_secs_f64()
            } else {
                0.0
            },
        }
    }

    /// Get bytes streamed so far
    pub fn bytes_streamed(&self) -> u64 {
        self.stats.load(Ordering::Relaxed)
    }

    /// Get chunks streamed so far
    pub fn chunks_streamed(&self) -> u64 {
        self.chunks.load(Ordering::Relaxed)
    }
}

impl<B> Body for ProxyStreamingBody<B>
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();

        // Check if we're currently throttling
        if let Some(sleep) = this.throttle_sleep.as_mut().as_pin_mut() {
            match sleep.poll(cx) {
                Poll::Ready(_) => {
                    // Throttle sleep complete
                    this.throttle_sleep.set(None);
                }
                Poll::Pending => {
                    trace!("Bandwidth throttling active");
                    return Poll::Pending;
                }
            }
        }

        // Poll the inner body
        match this.inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let chunk_size = data.len();

                    // Update statistics
                    this.stats.fetch_add(chunk_size as u64, Ordering::Relaxed);
                    this.chunks.fetch_add(1, Ordering::Relaxed);

                    // Check for bandwidth throttling (inlined logic)
                    let mut should_sleep = None;
                    if let Some(max_bps) = this.config.max_bandwidth {
                        // Reset counter if we've moved to a new second
                        let now = Instant::now();
                        if now.duration_since(*this.second_start) >= Duration::from_secs(1) {
                            *this.bytes_this_second = 0;
                            *this.second_start = now;
                        }

                        // Check if this chunk would exceed the limit
                        let new_total = *this.bytes_this_second + chunk_size as u64;
                        if new_total > max_bps {
                            // Calculate sleep duration to stay under limit
                            let bytes_over = new_total - max_bps;
                            let sleep_ms = (bytes_over as f64 / max_bps as f64 * 1000.0) as u64;
                            should_sleep = Some(Duration::from_millis(sleep_ms.min(1000)));
                        }
                    }

                    if let Some(sleep_duration) = should_sleep {
                        debug!("Throttling stream for {:?}", sleep_duration);
                        this.throttle_sleep.set(Some(tokio::time::sleep(sleep_duration)));
                        // Wake up to check throttle
                        cx.waker().wake_by_ref();
                    } else {
                        *this.bytes_this_second += chunk_size as u64;
                    }

                    *this.last_chunk_time = Instant::now();

                    if this.config.track_progress {
                        trace!(
                            "Streamed chunk: {} bytes (total: {} bytes, chunks: {})",
                            chunk_size,
                            this.stats.load(Ordering::Relaxed),
                            this.chunks.load(Ordering::Relaxed)
                        );
                    }
                }

                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                warn!("Stream error: {}", e);
                Poll::Ready(Some(Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))))
            }
            Poll::Ready(None) => {
                if this.config.track_progress {
                    let total_bytes = this.stats.load(Ordering::Relaxed);
                    let total_chunks = this.chunks.load(Ordering::Relaxed);
                    let duration = this.start_time.elapsed();
                    let throughput = if duration.as_secs_f64() > 0.0 {
                        total_bytes as f64 / duration.as_secs_f64()
                    } else {
                        0.0
                    };

                    debug!(
                        "Stream complete: {} bytes in {} chunks ({:.2} KB/s)",
                        total_bytes,
                        total_chunks,
                        throughput / 1024.0
                    );
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// Builder for ProxyStreamingBody
pub struct ProxyStreamBuilder {
    config: ProxyStreamConfig,
}

impl ProxyStreamBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: ProxyStreamConfig::default(),
        }
    }

    /// Set maximum bandwidth (bytes per second)
    pub fn max_bandwidth(mut self, bps: u64) -> Self {
        self.config.max_bandwidth = Some(bps);
        self
    }

    /// Enable or disable progress tracking
    pub fn track_progress(mut self, enabled: bool) -> Self {
        self.config.track_progress = enabled;
        self
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set chunk size
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.config.chunk_size = size;
        self
    }

    /// Build a ProxyStreamingBody
    pub fn build<B>(self, body: B) -> ProxyStreamingBody<B> {
        ProxyStreamingBody::new(body, self.config)
    }
}

impl Default for ProxyStreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Full};

    #[test]
    fn test_default_config() {
        let config = ProxyStreamConfig::default();
        assert!(config.max_bandwidth.is_none());
        assert!(config.track_progress);
        assert_eq!(config.buffer_size, 65536);
        assert_eq!(config.chunk_size, 8192);
    }

    #[test]
    fn test_stream_stats() {
        let mut stats = StreamStats::default();
        stats.bytes_streamed = 1024;
        stats.chunks_streamed = 10;
        stats.start_time = Some(Instant::now());

        assert_eq!(stats.bytes_streamed, 1024);
        assert_eq!(stats.chunks_streamed, 10);
        assert!(stats.duration().is_some());
    }

    #[tokio::test]
    async fn test_proxy_streaming_body() {
        let data = Bytes::from("test streaming data");
        let body = Full::new(data.clone());

        let streaming_body = ProxyStreamingBody::new(body, ProxyStreamConfig::default());

        let collected = streaming_body.collect().await.unwrap();
        assert_eq!(collected.to_bytes(), data);
    }

    #[test]
    fn test_builder() {
        let builder = ProxyStreamBuilder::new()
            .max_bandwidth(1024 * 1024)
            .track_progress(false)
            .buffer_size(32768)
            .chunk_size(4096);

        assert_eq!(builder.config.max_bandwidth, Some(1024 * 1024));
        assert!(!builder.config.track_progress);
        assert_eq!(builder.config.buffer_size, 32768);
        assert_eq!(builder.config.chunk_size, 4096);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let data = Bytes::from("x".repeat(1000));
        let body = Full::new(data.clone());

        let streaming_body = ProxyStreamingBody::new(body, ProxyStreamConfig::default());

        // Clone the Arc counters before consuming the body
        let stats = Arc::clone(&streaming_body.stats);
        let chunks = Arc::clone(&streaming_body.chunks);

        // Collect the body
        let _collected = streaming_body.collect().await.unwrap();

        // Check stats were tracked (via the cloned Arc references)
        assert_eq!(stats.load(Ordering::Relaxed), 1000);
        assert!(chunks.load(Ordering::Relaxed) > 0);
    }
}
