//! Prometheus metrics implementation

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

/// Metrics collector for the proxy
pub struct Metrics {
    handle: PrometheusHandle,
}

impl Metrics {
    /// Initialize metrics with Prometheus exporter
    pub fn new() -> Self {
        let builder = PrometheusBuilder::new();

        // Configure histogram buckets for latency (in seconds)
        let builder = builder.set_buckets_for_metric(
            Matcher::Full("http_request_duration_seconds".to_string()),
            &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        ).unwrap();

        let handle = builder.install_recorder().unwrap();

        // Describe all metrics
        Self::describe_metrics();

        Self { handle }
    }

    /// Describe all metrics for Prometheus
    fn describe_metrics() {
        // Request metrics
        describe_counter!("http_requests_total", "Total number of HTTP requests");
        describe_counter!("http_requests_errors_total", "Total number of HTTP request errors");
        describe_histogram!("http_request_duration_seconds", "HTTP request latency in seconds");
        describe_counter!("http_requests_bytes_total", "Total bytes received");
        describe_counter!("http_responses_bytes_total", "Total bytes sent");

        // Connection metrics
        describe_gauge!("http_connections_active", "Number of active HTTP connections");
        describe_counter!("http_connections_total", "Total number of HTTP connections");

        // Upstream metrics
        describe_counter!("upstream_requests_total", "Total requests to upstream servers");
        describe_counter!("upstream_requests_errors_total", "Total upstream errors");
        describe_histogram!("upstream_request_duration_seconds", "Upstream request latency");

        // Load balancer metrics
        describe_counter!("load_balancer_selections_total", "Total load balancer selections");

        // TLS metrics
        describe_counter!("tls_handshakes_total", "Total TLS handshakes");
        describe_counter!("tls_handshakes_errors_total", "Total TLS handshake errors");

        // Certificate metrics
        describe_gauge!("certificates_count", "Number of loaded certificates");
        describe_counter!("acme_requests_total", "Total ACME certificate requests");
        describe_counter!("acme_renewals_total", "Total certificate renewals");

        // Per-route metrics
        describe_counter!("route_requests_total", "Total requests per route");
        describe_histogram!("route_request_duration_seconds", "Request latency per route");
        describe_counter!("route_requests_bytes_total", "Total request bytes per route");
        describe_counter!("route_responses_bytes_total", "Total response bytes per route");

        // Protocol-specific metrics
        crate::observability::tcp_metrics::describe_tcp_metrics();
        crate::observability::tls_metrics::describe_tls_metrics();
        crate::observability::quic_metrics::describe_quic_metrics();
        crate::observability::grpc_metrics::describe_grpc_metrics();
        crate::observability::graphql_metrics::describe_graphql_metrics();
        crate::observability::cache_metrics::describe_cache_metrics();
    }

    /// Get the Prometheus metrics handle for rendering
    pub fn handle(&self) -> &PrometheusHandle {
        &self.handle
    }

    /// Render metrics in Prometheus format
    pub fn render(&self) -> String {
        self.handle.render()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Record an HTTP request
pub fn record_request(method: &str, status: u16, duration: f64) {
    counter!("http_requests_total", "method" => method.to_string(), "status" => status.to_string()).increment(1);
    histogram!("http_request_duration_seconds", "method" => method.to_string(), "status" => status.to_string()).record(duration);
}

/// Record an HTTP error
pub fn record_error(method: &str, error_type: &str) {
    counter!("http_requests_errors_total", "method" => method.to_string(), "error" => error_type.to_string()).increment(1);
}

/// Record request/response bytes
pub fn record_bytes(request_bytes: u64, response_bytes: u64) {
    counter!("http_requests_bytes_total").increment(request_bytes);
    counter!("http_responses_bytes_total").increment(response_bytes);
}

/// Record active connections
pub fn record_active_connections(count: i64) {
    gauge!("http_connections_active").set(count as f64);
}

/// Record a new connection
pub fn record_connection() {
    counter!("http_connections_total").increment(1);
}

/// Record upstream request
pub fn record_upstream_request(upstream: &str, status: u16, duration: f64) {
    counter!("upstream_requests_total", "upstream" => upstream.to_string(), "status" => status.to_string()).increment(1);
    histogram!("upstream_request_duration_seconds", "upstream" => upstream.to_string()).record(duration);
}

/// Record upstream error
pub fn record_upstream_error(upstream: &str, error_type: &str) {
    counter!("upstream_requests_errors_total", "upstream" => upstream.to_string(), "error" => error_type.to_string()).increment(1);
}

/// Record load balancer selection
pub fn record_lb_selection(upstream: &str, backend: &str) {
    counter!("load_balancer_selections_total", "upstream" => upstream.to_string(), "backend" => backend.to_string()).increment(1);
}

/// Record TLS handshake
pub fn record_tls_handshake(success: bool) {
    counter!("tls_handshakes_total").increment(1);
    if !success {
        counter!("tls_handshakes_errors_total").increment(1);
    }
}

/// Record certificate count
pub fn record_certificates_count(count: usize) {
    gauge!("certificates_count").set(count as f64);
}

/// Record ACME request
pub fn record_acme_request(domain: &str, success: bool) {
    counter!("acme_requests_total", "domain" => domain.to_string(), "success" => success.to_string()).increment(1);
}

/// Record per-route request metrics
pub fn record_route_request(route: &str, method: &str, status: u16, duration: f64, request_bytes: u64, response_bytes: u64) {
    // Count total requests per route
    counter!("route_requests_total", "route" => route.to_string(), "method" => method.to_string(), "status" => status.to_string()).increment(1);

    // Record request duration
    histogram!("route_request_duration_seconds", "route" => route.to_string(), "method" => method.to_string()).record(duration);

    // Record bytes transferred
    counter!("route_requests_bytes_total", "route" => route.to_string()).increment(request_bytes);
    counter!("route_responses_bytes_total", "route" => route.to_string()).increment(response_bytes);
}

/// Record certificate renewal
pub fn record_acme_renewal(domain: &str, success: bool) {
    counter!("acme_renewals_total", "domain" => domain.to_string(), "success" => success.to_string()).increment(1);
}

/// Request timer for automatic duration tracking
pub struct RequestTimer {
    start: Instant,
    method: String,
}

impl RequestTimer {
    /// Start a new request timer
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            method: method.into(),
        }
    }

    /// Complete the timer and record metrics
    pub fn complete(self, status: u16) {
        let duration = self.start.elapsed().as_secs_f64();
        record_request(&self.method, status, duration);
    }

    /// Complete with error
    pub fn error(self, error_type: &str) {
        record_error(&self.method, error_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    // Ensure metrics are initialized only once across all tests
    static METRICS: OnceLock<Metrics> = OnceLock::new();

    fn get_test_metrics() -> &'static Metrics {
        METRICS.get_or_init(|| Metrics::new())
    }

    #[test]
    fn test_metrics_initialization() {
        let metrics = get_test_metrics();

        // Record a test metric to ensure something is in the output
        record_request("TEST", 200, 0.001);

        let output = metrics.render();
        // Metrics output should contain metric data
        assert!(!output.is_empty(), "Metrics output should not be empty");
        assert!(output.contains("http_requests_total"), "Should contain http_requests_total metric");
    }

    #[test]
    fn test_record_request() {
        let _metrics = get_test_metrics();
        record_request("GET", 200, 0.123);
        record_request("POST", 201, 0.456);
        // Test passes if no panic occurs
    }

    #[test]
    fn test_request_timer() {
        let _metrics = get_test_metrics();
        let timer = RequestTimer::new("GET");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.complete(200);
        // Test passes if no panic occurs
    }
}
