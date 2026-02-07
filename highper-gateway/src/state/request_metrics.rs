//! Enhanced request metrics with per-route and per-backend tracking
//!
//! Provides detailed metrics including:
//! - Request counts per route and backend
//! - Response time histograms (p50, p95, p99)
//! - Status code distribution
//! - Bytes transferred

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Simple histogram for response time tracking
/// Uses fixed buckets for efficiency
#[derive(Debug)]
pub struct Histogram {
    // Buckets: <1ms, <5ms, <10ms, <50ms, <100ms, <500ms, <1s, <5s, >=5s
    buckets: [AtomicU64; 9],
    total_samples: AtomicU64,
    sum_ms: AtomicU64,
}

impl Histogram {
    /// Create a new histogram
    pub fn new() -> Self {
        Self {
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            total_samples: AtomicU64::new(0),
            sum_ms: AtomicU64::new(0),
        }
    }

    /// Record a response time
    pub fn record(&self, duration: Duration) {
        let ms = duration.as_millis() as u64;

        // Determine bucket
        let bucket_index = match ms {
            0..=1 => 0,           // <1ms
            2..=5 => 1,           // <5ms
            6..=10 => 2,          // <10ms
            11..=50 => 3,         // <50ms
            51..=100 => 4,        // <100ms
            101..=500 => 5,       // <500ms
            501..=1000 => 6,      // <1s
            1001..=5000 => 7,     // <5s
            _ => 8,               // >=5s
        };

        self.buckets[bucket_index].fetch_add(1, Ordering::Relaxed);
        self.total_samples.fetch_add(1, Ordering::Relaxed);
        self.sum_ms.fetch_add(ms, Ordering::Relaxed);
    }

    /// Get snapshot of histogram
    pub fn snapshot(&self) -> HistogramSnapshot {
        let buckets: Vec<u64> = self.buckets
            .iter()
            .map(|b| b.load(Ordering::Relaxed))
            .collect();

        let total = self.total_samples.load(Ordering::Relaxed);
        let sum = self.sum_ms.load(Ordering::Relaxed);

        let p50 = Self::calculate_percentile(&buckets, total, 0.50);
        let p95 = Self::calculate_percentile(&buckets, total, 0.95);
        let p99 = Self::calculate_percentile(&buckets, total, 0.99);

        HistogramSnapshot {
            buckets,
            total_samples: total,
            avg_ms: if total > 0 { sum as f64 / total as f64 } else { 0.0 },
            p50,
            p95,
            p99,
        }
    }

    /// Calculate percentile from bucket data
    fn calculate_percentile(buckets: &[u64], total: u64, percentile: f64) -> f64 {
        if total == 0 {
            return 0.0;
        }

        let target = (total as f64 * percentile) as u64;
        let mut count = 0u64;

        // Bucket boundaries in milliseconds
        let boundaries = [1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, f64::INFINITY];

        for (i, &bucket_count) in buckets.iter().enumerate() {
            count += bucket_count;
            if count >= target {
                // Linear interpolation within bucket
                let bucket_start = if i == 0 { 0.0 } else { boundaries[i - 1] };
                let bucket_end = boundaries[i];
                let position_in_bucket = if bucket_count > 0 {
                    (target as f64 - (count - bucket_count) as f64) / bucket_count as f64
                } else {
                    0.0
                };
                return bucket_start + (bucket_end - bucket_start) * position_in_bucket;
            }
        }

        boundaries[boundaries.len() - 2] // Return last finite boundary if we somehow don't find it
    }

    /// Reset histogram
    pub fn reset(&self) {
        for bucket in &self.buckets {
            bucket.store(0, Ordering::Relaxed);
        }
        self.total_samples.store(0, Ordering::Relaxed);
        self.sum_ms.store(0, Ordering::Relaxed);
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of histogram data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramSnapshot {
    /// Bucket counts
    pub buckets: Vec<u64>,

    /// Total samples
    pub total_samples: u64,

    /// Average response time in milliseconds
    pub avg_ms: f64,

    /// 50th percentile (median)
    pub p50: f64,

    /// 95th percentile
    pub p95: f64,

    /// 99th percentile
    pub p99: f64,
}

/// Per-route metrics
#[derive(Debug)]
pub struct RouteMetrics {
    /// Total requests for this route
    pub request_count: AtomicU64,

    /// Status code distribution
    pub status_2xx: AtomicU64,
    pub status_3xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,

    /// Response time histogram
    pub response_time: Histogram,

    /// Bytes sent (response body)
    pub bytes_sent: AtomicU64,

    /// Bytes received (request body)
    pub bytes_received: AtomicU64,
}

impl RouteMetrics {
    /// Create new route metrics
    pub fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            response_time: Histogram::new(),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
        }
    }

    /// Record a request
    pub fn record_request(
        &self,
        status_code: u16,
        response_time: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        self.request_count.fetch_add(1, Ordering::Relaxed);

        // Record status code
        match status_code {
            200..=299 => self.status_2xx.fetch_add(1, Ordering::Relaxed),
            300..=399 => self.status_3xx.fetch_add(1, Ordering::Relaxed),
            400..=499 => self.status_4xx.fetch_add(1, Ordering::Relaxed),
            500..=599 => self.status_5xx.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };

        // Record response time
        self.response_time.record(response_time);

        // Record bytes
        self.bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes_received, Ordering::Relaxed);
    }

    /// Get snapshot of metrics
    pub fn snapshot(&self) -> RouteMetricsSnapshot {
        RouteMetricsSnapshot {
            request_count: self.request_count.load(Ordering::Relaxed),
            status_2xx: self.status_2xx.load(Ordering::Relaxed),
            status_3xx: self.status_3xx.load(Ordering::Relaxed),
            status_4xx: self.status_4xx.load(Ordering::Relaxed),
            status_5xx: self.status_5xx.load(Ordering::Relaxed),
            response_time: self.response_time.snapshot(),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }

    /// Reset metrics
    pub fn reset(&self) {
        self.request_count.store(0, Ordering::Relaxed);
        self.status_2xx.store(0, Ordering::Relaxed);
        self.status_3xx.store(0, Ordering::Relaxed);
        self.status_4xx.store(0, Ordering::Relaxed);
        self.status_5xx.store(0, Ordering::Relaxed);
        self.response_time.reset();
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
    }
}

impl Default for RouteMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of route metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetricsSnapshot {
    pub request_count: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub response_time: HistogramSnapshot,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Per-backend metrics
#[derive(Debug)]
pub struct BackendMetrics {
    /// Total requests to this backend
    pub request_count: AtomicU64,

    /// Error count (5xx responses + connection errors)
    pub error_count: AtomicU64,

    /// Response time histogram
    pub response_time: Histogram,

    /// Bytes sent to backend
    pub bytes_sent: AtomicU64,

    /// Bytes received from backend
    pub bytes_received: AtomicU64,
}

impl BackendMetrics {
    /// Create new backend metrics
    pub fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            response_time: Histogram::new(),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
        }
    }

    /// Record a request
    pub fn record_request(
        &self,
        is_error: bool,
        response_time: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        self.request_count.fetch_add(1, Ordering::Relaxed);

        if is_error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }

        self.response_time.record(response_time);
        self.bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes_received, Ordering::Relaxed);
    }

    /// Get snapshot of metrics
    pub fn snapshot(&self) -> BackendMetricsSnapshot {
        let request_count = self.request_count.load(Ordering::Relaxed);
        let error_count = self.error_count.load(Ordering::Relaxed);

        BackendMetricsSnapshot {
            request_count,
            error_count,
            error_rate: if request_count > 0 {
                error_count as f64 / request_count as f64
            } else {
                0.0
            },
            response_time: self.response_time.snapshot(),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }

    /// Reset metrics
    pub fn reset(&self) {
        self.request_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        self.response_time.reset();
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
    }
}

impl Default for BackendMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of backend metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendMetricsSnapshot {
    pub request_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub response_time: HistogramSnapshot,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Enhanced request metrics tracker
pub struct RequestMetrics {
    /// Per-route metrics
    route_metrics: Arc<DashMap<String, RouteMetrics>>,

    /// Per-backend metrics
    backend_metrics: Arc<DashMap<String, BackendMetrics>>,

    /// Global response time histogram
    global_response_time: Histogram,
}

impl RequestMetrics {
    /// Create new request metrics tracker
    pub fn new() -> Self {
        Self {
            route_metrics: Arc::new(DashMap::new()),
            backend_metrics: Arc::new(DashMap::new()),
            global_response_time: Histogram::new(),
        }
    }

    /// Record a route request
    pub fn record_route_request(
        &self,
        route: &str,
        status_code: u16,
        response_time: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        // Get or create route metrics
        let metrics = self.route_metrics
            .entry(route.to_string())
            .or_default();

        metrics.record_request(status_code, response_time, bytes_sent, bytes_received);

        // Also record in global histogram
        self.global_response_time.record(response_time);
    }

    /// Record a backend request
    pub fn record_backend_request(
        &self,
        backend_id: &str,
        is_error: bool,
        response_time: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        // Get or create backend metrics
        let metrics = self.backend_metrics
            .entry(backend_id.to_string())
            .or_default();

        metrics.record_request(is_error, response_time, bytes_sent, bytes_received);
    }

    /// Get route metrics snapshot
    pub fn get_route_metrics(&self, route: &str) -> Option<RouteMetricsSnapshot> {
        self.route_metrics.get(route).map(|m| m.snapshot())
    }

    /// Get all route metrics
    pub fn get_all_route_metrics(&self) -> Vec<(String, RouteMetricsSnapshot)> {
        self.route_metrics
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().snapshot()))
            .collect()
    }

    /// Get backend metrics snapshot
    pub fn get_backend_metrics(&self, backend_id: &str) -> Option<BackendMetricsSnapshot> {
        self.backend_metrics.get(backend_id).map(|m| m.snapshot())
    }

    /// Get all backend metrics
    pub fn get_all_backend_metrics(&self) -> Vec<(String, BackendMetricsSnapshot)> {
        self.backend_metrics
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().snapshot()))
            .collect()
    }

    /// Get global response time histogram
    pub fn get_global_response_time(&self) -> HistogramSnapshot {
        self.global_response_time.snapshot()
    }

    /// Reset all metrics
    pub fn reset_all(&self) {
        for entry in self.route_metrics.iter() {
            entry.value().reset();
        }
        for entry in self.backend_metrics.iter() {
            entry.value().reset();
        }
        self.global_response_time.reset();
    }

    /// Reset route metrics
    pub fn reset_route(&self, route: &str) {
        if let Some(metrics) = self.route_metrics.get(route) {
            metrics.reset();
        }
    }

    /// Reset backend metrics
    pub fn reset_backend(&self, backend_id: &str) {
        if let Some(metrics) = self.backend_metrics.get(backend_id) {
            metrics.reset();
        }
    }
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_recording() {
        let hist = Histogram::new();

        // Record some samples
        hist.record(Duration::from_millis(1));
        hist.record(Duration::from_millis(5));
        hist.record(Duration::from_millis(10));
        hist.record(Duration::from_millis(50));
        hist.record(Duration::from_millis(100));

        let snapshot = hist.snapshot();
        assert_eq!(snapshot.total_samples, 5);
        assert!(snapshot.avg_ms > 0.0);
        assert!(snapshot.p50 > 0.0);
        assert!(snapshot.p95 > 0.0);
        assert!(snapshot.p99 > 0.0);
    }

    #[test]
    fn test_route_metrics() {
        let metrics = RouteMetrics::new();

        // Record a request
        metrics.record_request(
            200,
            Duration::from_millis(10),
            1024,
            512,
        );

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.request_count, 1);
        assert_eq!(snapshot.status_2xx, 1);
        assert_eq!(snapshot.bytes_sent, 1024);
        assert_eq!(snapshot.bytes_received, 512);
    }

    #[test]
    fn test_backend_metrics() {
        let metrics = BackendMetrics::new();

        // Record successful request
        metrics.record_request(
            false,
            Duration::from_millis(5),
            100,
            200,
        );

        // Record failed request
        metrics.record_request(
            true,
            Duration::from_millis(10),
            100,
            0,
        );

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.request_count, 2);
        assert_eq!(snapshot.error_count, 1);
        assert_eq!(snapshot.error_rate, 0.5);
    }

    #[test]
    fn test_request_metrics_tracker() {
        let tracker = RequestMetrics::new();

        // Record route request
        tracker.record_route_request(
            "/api/users",
            200,
            Duration::from_millis(10),
            1024,
            512,
        );

        // Record backend request
        tracker.record_backend_request(
            "api_0",
            false,
            Duration::from_millis(5),
            512,
            1024,
        );

        // Verify route metrics
        let route_metrics = tracker.get_route_metrics("/api/users");
        assert!(route_metrics.is_some());
        assert_eq!(route_metrics.unwrap().request_count, 1);

        // Verify backend metrics
        let backend_metrics = tracker.get_backend_metrics("api_0");
        assert!(backend_metrics.is_some());
        assert_eq!(backend_metrics.unwrap().request_count, 1);
    }

    #[test]
    fn test_histogram_percentiles() {
        let hist = Histogram::new();

        // Record 100 samples with varying times
        for i in 1..=100 {
            hist.record(Duration::from_millis(i));
        }

        let snapshot = hist.snapshot();
        assert_eq!(snapshot.total_samples, 100);

        // p50 should be around 50ms
        assert!(snapshot.p50 >= 40.0 && snapshot.p50 <= 60.0);

        // p95 should be around 95ms
        assert!(snapshot.p95 >= 85.0 && snapshot.p95 <= 105.0);

        // p99 should be around 99ms
        assert!(snapshot.p99 >= 90.0 && snapshot.p99 <= 110.0);
    }

    #[test]
    fn test_reset_metrics() {
        let tracker = RequestMetrics::new();

        tracker.record_route_request(
            "/test",
            200,
            Duration::from_millis(10),
            100,
            100,
        );

        tracker.reset_route("/test");

        let metrics = tracker.get_route_metrics("/test");
        assert!(metrics.is_some());
        assert_eq!(metrics.unwrap().request_count, 0);
    }
}
