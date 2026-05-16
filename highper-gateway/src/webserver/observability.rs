use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
/// Observability and monitoring hooks for webserver operations
///
/// Provides comprehensive metrics, logging, and health checks for production monitoring.
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Webserver metrics aggregator
#[derive(Default)]
pub struct WebserverMetrics {
    // Request counters
    total_requests: AtomicU64,
    static_file_requests: AtomicU64,
    php_requests: AtomicU64,
    directory_listing_requests: AtomicU64,

    // Success/Error counters
    successful_responses: AtomicU64,
    error_responses: AtomicU64,
    not_found_errors: AtomicU64,
    forbidden_errors: AtomicU64,
    server_errors: AtomicU64,

    // Security metrics
    path_traversal_attempts: AtomicU64,
    sensitive_file_access_attempts: AtomicU64,
    hidden_file_access_attempts: AtomicU64,
    oversized_file_requests: AtomicU64,
    oversized_request_bodies: AtomicU64,
    invalid_php_scripts: AtomicU64,
    fastcgi_injection_attempts: AtomicU64,

    // Resource limit metrics
    connection_limit_rejections: AtomicU64,
    rate_limit_rejections: AtomicU64,
    file_descriptor_limit_rejections: AtomicU64,
    memory_limit_rejections: AtomicU64,

    // Performance metrics
    total_bytes_served: AtomicU64,
    total_request_duration_ms: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,

    // Per-path metrics
    path_metrics: Arc<DashMap<String, PathMetrics>>,
}

/// Metrics for a specific path
#[derive(Default)]
struct PathMetrics {
    request_count: AtomicU64,
    total_duration_ms: AtomicU64,
    error_count: AtomicU64,
    bytes_served: AtomicU64,
}

impl WebserverMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    // Request tracking
    pub fn record_request(&self, path: &str) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.path_metrics
            .entry(path.to_string())
            .or_default()
            .request_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_static_file_request(&self) {
        self.static_file_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_php_request(&self) {
        self.php_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_directory_listing_request(&self) {
        self.directory_listing_requests
            .fetch_add(1, Ordering::Relaxed);
    }

    // Response tracking
    pub fn record_success(&self) {
        self.successful_responses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self, status_code: u16, path: &str) {
        self.error_responses.fetch_add(1, Ordering::Relaxed);

        match status_code {
            404 => self.not_found_errors.fetch_add(1, Ordering::Relaxed),
            403 => self.forbidden_errors.fetch_add(1, Ordering::Relaxed),
            500..=599 => self.server_errors.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };

        self.path_metrics
            .entry(path.to_string())
            .or_default()
            .error_count
            .fetch_add(1, Ordering::Relaxed);
    }

    // Security event tracking
    pub fn record_path_traversal_attempt(&self, path: &str) {
        self.path_traversal_attempts.fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Path traversal attempt detected: {}", path);
    }

    pub fn record_sensitive_file_access(&self, file: &str) {
        self.sensitive_file_access_attempts
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Sensitive file access attempt: {}", file);
    }

    pub fn record_hidden_file_access(&self, file: &str) {
        self.hidden_file_access_attempts
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Hidden file access attempt: {}", file);
    }

    pub fn record_oversized_file(&self, size: u64, limit: u64) {
        self.oversized_file_requests.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            "Oversized file request: {} bytes (limit: {} bytes)",
            size,
            limit
        );
    }

    pub fn record_oversized_request_body(&self, size: usize, limit: usize) {
        self.oversized_request_bodies
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            "Oversized request body: {} bytes (limit: {} bytes)",
            size,
            limit
        );
    }

    pub fn record_invalid_php_script(&self, path: &str) {
        self.invalid_php_scripts.fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Invalid PHP script access attempt: {}", path);
    }

    pub fn record_fastcgi_injection_attempt(&self, param: &str) {
        self.fastcgi_injection_attempts
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            "Potential FastCGI injection attempt in parameter: {}",
            param
        );
    }

    // Resource limit tracking
    pub fn record_connection_limit_rejection(&self, client_ip: &str) {
        self.connection_limit_rejections
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Connection limit rejection for IP: {}", client_ip);
    }

    pub fn record_rate_limit_rejection(&self, client_ip: &str) {
        self.rate_limit_rejections.fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Rate limit rejection for IP: {}", client_ip);
    }

    pub fn record_file_descriptor_limit_rejection(&self) {
        self.file_descriptor_limit_rejections
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!("File descriptor limit reached");
    }

    pub fn record_memory_limit_rejection(&self, size: usize) {
        self.memory_limit_rejections.fetch_add(1, Ordering::Relaxed);
        tracing::warn!("Memory limit rejection: requested {} bytes", size);
    }

    // Performance tracking
    pub fn record_bytes_served(&self, bytes: u64, path: &str) {
        self.total_bytes_served.fetch_add(bytes, Ordering::Relaxed);
        self.path_metrics
            .entry(path.to_string())
            .or_insert_with(PathMetrics::default)
            .bytes_served
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_request_duration(&self, duration: Duration, path: &str) {
        let duration_ms = duration.as_millis() as u64;
        self.total_request_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.path_metrics
            .entry(path.to_string())
            .or_insert_with(PathMetrics::default)
            .total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    // Getters for metrics
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            static_file_requests: self.static_file_requests.load(Ordering::Relaxed),
            php_requests: self.php_requests.load(Ordering::Relaxed),
            directory_listing_requests: self.directory_listing_requests.load(Ordering::Relaxed),
            successful_responses: self.successful_responses.load(Ordering::Relaxed),
            error_responses: self.error_responses.load(Ordering::Relaxed),
            not_found_errors: self.not_found_errors.load(Ordering::Relaxed),
            forbidden_errors: self.forbidden_errors.load(Ordering::Relaxed),
            server_errors: self.server_errors.load(Ordering::Relaxed),
            path_traversal_attempts: self.path_traversal_attempts.load(Ordering::Relaxed),
            sensitive_file_access_attempts: self
                .sensitive_file_access_attempts
                .load(Ordering::Relaxed),
            hidden_file_access_attempts: self.hidden_file_access_attempts.load(Ordering::Relaxed),
            oversized_file_requests: self.oversized_file_requests.load(Ordering::Relaxed),
            oversized_request_bodies: self.oversized_request_bodies.load(Ordering::Relaxed),
            invalid_php_scripts: self.invalid_php_scripts.load(Ordering::Relaxed),
            fastcgi_injection_attempts: self.fastcgi_injection_attempts.load(Ordering::Relaxed),
            connection_limit_rejections: self.connection_limit_rejections.load(Ordering::Relaxed),
            rate_limit_rejections: self.rate_limit_rejections.load(Ordering::Relaxed),
            file_descriptor_limit_rejections: self
                .file_descriptor_limit_rejections
                .load(Ordering::Relaxed),
            memory_limit_rejections: self.memory_limit_rejections.load(Ordering::Relaxed),
            total_bytes_served: self.total_bytes_served.load(Ordering::Relaxed),
            total_request_duration_ms: self.total_request_duration_ms.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            timestamp: Instant::now(),
        }
    }

    pub fn get_path_metrics(&self, path: &str) -> Option<PathMetricsSnapshot> {
        self.path_metrics
            .get(path)
            .map(|metrics| PathMetricsSnapshot {
                path: path.to_string(),
                request_count: metrics.request_count.load(Ordering::Relaxed),
                total_duration_ms: metrics.total_duration_ms.load(Ordering::Relaxed),
                error_count: metrics.error_count.load(Ordering::Relaxed),
                bytes_served: metrics.bytes_served.load(Ordering::Relaxed),
            })
    }

    pub fn get_top_paths(&self, limit: usize) -> Vec<PathMetricsSnapshot> {
        let mut paths: Vec<_> = self
            .path_metrics
            .iter()
            .map(|entry| PathMetricsSnapshot {
                path: entry.key().clone(),
                request_count: entry.value().request_count.load(Ordering::Relaxed),
                total_duration_ms: entry.value().total_duration_ms.load(Ordering::Relaxed),
                error_count: entry.value().error_count.load(Ordering::Relaxed),
                bytes_served: entry.value().bytes_served.load(Ordering::Relaxed),
            })
            .collect();

        paths.sort_by(|a, b| b.request_count.cmp(&a.request_count));
        paths.truncate(limit);
        paths
    }

    /// Reset all metrics (useful for testing or periodic resets)
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.static_file_requests.store(0, Ordering::Relaxed);
        self.php_requests.store(0, Ordering::Relaxed);
        self.directory_listing_requests.store(0, Ordering::Relaxed);
        self.successful_responses.store(0, Ordering::Relaxed);
        self.error_responses.store(0, Ordering::Relaxed);
        self.not_found_errors.store(0, Ordering::Relaxed);
        self.forbidden_errors.store(0, Ordering::Relaxed);
        self.server_errors.store(0, Ordering::Relaxed);
        self.path_traversal_attempts.store(0, Ordering::Relaxed);
        self.sensitive_file_access_attempts
            .store(0, Ordering::Relaxed);
        self.hidden_file_access_attempts.store(0, Ordering::Relaxed);
        self.oversized_file_requests.store(0, Ordering::Relaxed);
        self.oversized_request_bodies.store(0, Ordering::Relaxed);
        self.invalid_php_scripts.store(0, Ordering::Relaxed);
        self.fastcgi_injection_attempts.store(0, Ordering::Relaxed);
        self.connection_limit_rejections.store(0, Ordering::Relaxed);
        self.rate_limit_rejections.store(0, Ordering::Relaxed);
        self.file_descriptor_limit_rejections
            .store(0, Ordering::Relaxed);
        self.memory_limit_rejections.store(0, Ordering::Relaxed);
        self.total_bytes_served.store(0, Ordering::Relaxed);
        self.total_request_duration_ms.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.path_metrics.clear();
    }
}

/// Snapshot of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub static_file_requests: u64,
    pub php_requests: u64,
    pub directory_listing_requests: u64,
    pub successful_responses: u64,
    pub error_responses: u64,
    pub not_found_errors: u64,
    pub forbidden_errors: u64,
    pub server_errors: u64,
    pub path_traversal_attempts: u64,
    pub sensitive_file_access_attempts: u64,
    pub hidden_file_access_attempts: u64,
    pub oversized_file_requests: u64,
    pub oversized_request_bodies: u64,
    pub invalid_php_scripts: u64,
    pub fastcgi_injection_attempts: u64,
    pub connection_limit_rejections: u64,
    pub rate_limit_rejections: u64,
    pub file_descriptor_limit_rejections: u64,
    pub memory_limit_rejections: u64,
    pub total_bytes_served: u64,
    pub total_request_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub timestamp: Instant,
}

impl MetricsSnapshot {
    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.error_responses as f64 / self.total_requests as f64
        }
    }

    /// Calculate average request duration in milliseconds
    pub fn avg_request_duration_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_request_duration_ms as f64 / self.total_requests as f64
        }
    }

    /// Calculate cache hit rate (0.0 to 1.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total_cache_requests = self.cache_hits + self.cache_misses;
        if total_cache_requests == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total_cache_requests as f64
        }
    }

    /// Calculate total security violations
    pub fn total_security_violations(&self) -> u64 {
        self.path_traversal_attempts
            + self.sensitive_file_access_attempts
            + self.hidden_file_access_attempts
            + self.oversized_file_requests
            + self.oversized_request_bodies
            + self.invalid_php_scripts
            + self.fastcgi_injection_attempts
    }

    /// Calculate total resource limit rejections
    pub fn total_resource_limit_rejections(&self) -> u64 {
        self.connection_limit_rejections
            + self.rate_limit_rejections
            + self.file_descriptor_limit_rejections
            + self.memory_limit_rejections
    }
}

/// Snapshot of metrics for a specific path
#[derive(Debug, Clone)]
pub struct PathMetricsSnapshot {
    pub path: String,
    pub request_count: u64,
    pub total_duration_ms: u64,
    pub error_count: u64,
    pub bytes_served: u64,
}

impl PathMetricsSnapshot {
    pub fn avg_duration_ms(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.request_count as f64
        }
    }

    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.request_count as f64
        }
    }
}

/// Health check status
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub checks: Vec<HealthCheck>,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self {
            healthy: true,
            checks: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn add_check(&mut self, name: String, status: String, message: Option<String>) {
        if status != "ok" {
            self.healthy = false;
        }
        self.checks.push(HealthCheck {
            name,
            status,
            message,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = WebserverMetrics::new();

        metrics.record_request("/test");
        metrics.record_static_file_request();
        metrics.record_success();

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.static_file_requests, 1);
        assert_eq!(snapshot.successful_responses, 1);
    }

    #[test]
    fn test_security_metrics() {
        let metrics = WebserverMetrics::new();

        metrics.record_path_traversal_attempt("../etc/passwd");
        metrics.record_sensitive_file_access(".env");
        metrics.record_hidden_file_access(".htaccess");

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.path_traversal_attempts, 1);
        assert_eq!(snapshot.sensitive_file_access_attempts, 1);
        assert_eq!(snapshot.hidden_file_access_attempts, 1);
        assert_eq!(snapshot.total_security_violations(), 3);
    }

    #[test]
    fn test_resource_limit_metrics() {
        let metrics = WebserverMetrics::new();

        metrics.record_connection_limit_rejection("192.168.1.1");
        metrics.record_rate_limit_rejection("192.168.1.2");
        metrics.record_file_descriptor_limit_rejection();
        metrics.record_memory_limit_rejection(1024);

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.connection_limit_rejections, 1);
        assert_eq!(snapshot.rate_limit_rejections, 1);
        assert_eq!(snapshot.file_descriptor_limit_rejections, 1);
        assert_eq!(snapshot.memory_limit_rejections, 1);
        assert_eq!(snapshot.total_resource_limit_rejections(), 4);
    }

    #[test]
    fn test_path_metrics() {
        let metrics = WebserverMetrics::new();

        metrics.record_request("/test");
        metrics.record_bytes_served(1024, "/test");
        metrics.record_request_duration(Duration::from_millis(50), "/test");

        let path_metrics = metrics.get_path_metrics("/test").unwrap();
        assert_eq!(path_metrics.request_count, 1);
        assert_eq!(path_metrics.bytes_served, 1024);
        assert_eq!(path_metrics.total_duration_ms, 50);
    }

    #[test]
    fn test_error_rate_calculation() {
        let metrics = WebserverMetrics::new();

        for _ in 0..7 {
            metrics.record_request("/test");
            metrics.record_success();
        }

        for _ in 0..3 {
            metrics.record_request("/test");
            metrics.record_error(404, "/test");
        }

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_requests, 10);
        assert_eq!(snapshot.successful_responses, 7);
        assert_eq!(snapshot.error_responses, 3);
        assert!((snapshot.error_rate() - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = WebserverMetrics::new();

        metrics.record_request("/test");
        metrics.record_success();

        let snapshot1 = metrics.get_snapshot();
        assert_eq!(snapshot1.total_requests, 1);

        metrics.reset();

        let snapshot2 = metrics.get_snapshot();
        assert_eq!(snapshot2.total_requests, 0);
    }
}
