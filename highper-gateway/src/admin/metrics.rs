//! Enhanced metrics endpoints for Admin API
//!
//! Provides detailed metrics by route and backend:
//! - Per-route metrics
//! - Per-backend metrics
//! - Health check history
//! - Prometheus export format

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::state::ProxyState;
use std::sync::Arc;

/// Per-route metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetrics {
    /// Route identifier
    pub route_id: String,

    /// Route path pattern
    pub path_pattern: String,

    /// HTTP methods
    pub methods: Vec<String>,

    /// Total requests
    pub total_requests: u64,

    /// Requests per second (current)
    pub requests_per_second: f64,

    /// Average response time (ms)
    pub avg_response_time_ms: f64,

    /// P50 latency (ms)
    pub p50_latency_ms: f64,

    /// P95 latency (ms)
    pub p95_latency_ms: f64,

    /// P99 latency (ms)
    pub p99_latency_ms: f64,

    /// Success rate (%)
    pub success_rate: f64,

    /// Error rate (%)
    pub error_rate: f64,

    /// Status code breakdown
    pub status_codes: StatusCodeBreakdown,

    /// Upstream name
    pub upstream: String,
}

/// Status code breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCodeBreakdown {
    /// 2xx responses
    pub status_2xx: u64,

    /// 3xx responses
    pub status_3xx: u64,

    /// 4xx responses
    pub status_4xx: u64,

    /// 5xx responses
    pub status_5xx: u64,
}

/// Per-backend metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendMetrics {
    /// Backend ID
    pub backend_id: String,

    /// Upstream name
    pub upstream: String,

    /// Backend URL
    pub url: String,

    /// Health status
    pub health_status: String,

    /// Active connections
    pub active_connections: u64,

    /// Total requests
    pub total_requests: u64,

    /// Requests per second (current)
    pub requests_per_second: f64,

    /// Average response time (ms)
    pub avg_response_time_ms: f64,

    /// P50 latency (ms)
    pub p50_latency_ms: f64,

    /// P95 latency (ms)
    pub p95_latency_ms: f64,

    /// P99 latency (ms)
    pub p99_latency_ms: f64,

    /// Success rate (%)
    pub success_rate: f64,

    /// Error rate (%)
    pub error_rate: f64,

    /// Status code breakdown
    pub status_codes: StatusCodeBreakdown,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,
}

/// Health check history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckEntry {
    /// Backend ID
    pub backend_id: String,

    /// Upstream name
    pub upstream: String,

    /// Backend URL
    pub url: String,

    /// Timestamp
    pub timestamp: String,

    /// Health status
    pub status: String,

    /// Response time (ms)
    pub response_time_ms: f64,

    /// Status code (if HTTP check)
    pub status_code: Option<u16>,

    /// Error message (if failed)
    pub error: Option<String>,
}

/// Get detailed per-route metrics
pub async fn get_route_metrics(state: Option<Arc<ProxyState>>) -> Response<Full<Bytes>> {
    // TODO: Full integration with routing engine requires:
    // 1. Per-route metrics tracking
    // 2. Latency histograms for percentiles
    // 3. Aggregation of request/response data per route

    // For now, return basic global metrics as a single "global" route
    let metrics: Vec<RouteMetrics> = if let Some(state) = state {
        let m = state.metrics();
        let total = m.get_total_requests();
        let success = m.get_2xx() + m.get_3xx();
        let errors = m.get_4xx() + m.get_5xx();
        let success_rate = if total > 0 {
            (success as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let error_rate = if total > 0 {
            (errors as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        vec![RouteMetrics {
            route_id: "global".to_string(),
            path_pattern: "/*".to_string(),
            methods: vec!["*".to_string()],
            total_requests: total,
            requests_per_second: 0.0, // TODO: Track over time window
            avg_response_time_ms: 0.0, // TODO: Track response times
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            success_rate,
            error_rate,
            status_codes: StatusCodeBreakdown {
                status_2xx: m.get_2xx(),
                status_3xx: m.get_3xx(),
                status_4xx: m.get_4xx(),
                status_5xx: m.get_5xx(),
            },
            upstream: "all".to_string(),
        }]
    } else {
        Vec::new()
    };

    json_response(
        StatusCode::OK,
        json!({
            "routes": metrics,
            "total_routes": metrics.len(),
        }),
    )
}

/// Get detailed per-backend metrics
pub async fn get_backend_metrics(state: Option<Arc<ProxyState>>) -> Response<Full<Bytes>> {
    // Get basic metrics from backend state
    let metrics: Vec<BackendMetrics> = if let Some(state) = state {
        let backends = state.get_all_backends().await;
        backends
            .into_iter()
            .map(|backend| BackendMetrics {
                backend_id: backend.id.clone(),
                upstream: backend.upstream.clone(),
                url: backend.url.clone(),
                health_status: match backend.health_status {
                    crate::state::HealthStatus::Healthy => "healthy".to_string(),
                    crate::state::HealthStatus::Unhealthy => "unhealthy".to_string(),
                    crate::state::HealthStatus::Unknown => "unknown".to_string(),
                },
                active_connections: backend.active_connections as u64,
                total_requests: 0, // TODO: Track per-backend requests
                requests_per_second: 0.0,
                avg_response_time_ms: 0.0,
                p50_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                p99_latency_ms: 0.0,
                success_rate: 0.0,
                error_rate: 0.0,
                status_codes: StatusCodeBreakdown {
                    status_2xx: 0,
                    status_3xx: 0,
                    status_4xx: 0,
                    status_5xx: 0,
                },
                bytes_sent: 0,
                bytes_received: 0,
            })
            .collect()
    } else {
        Vec::new()
    };

    json_response(
        StatusCode::OK,
        json!({
            "backends": metrics,
            "total_backends": metrics.len(),
        }),
    )
}

/// Get health check history
pub async fn get_health_history(limit: Option<usize>) -> Response<Full<Bytes>> {
    // TODO: Integrate with health checker
    // This requires:
    // 1. Access to health check history storage
    // 2. Filtering and pagination support

    let limit = limit.unwrap_or(100);

    // For now, return stub data
    let history: Vec<HealthCheckEntry> = Vec::new();

    json_response(
        StatusCode::OK,
        json!({
            "history": history,
            "total": 0,
            "limit": limit,
        }),
    )
}

/// Export metrics in Prometheus format
pub async fn export_prometheus_metrics(state: Option<Arc<ProxyState>>) -> Response<Full<Bytes>> {
    let mut prometheus_output = String::new();

    // Global request metrics
    if let Some(state) = &state {
        let m = state.metrics();
        let total = m.get_total_requests();
        let status_2xx = m.get_2xx();
        let status_3xx = m.get_3xx();
        let status_4xx = m.get_4xx();
        let status_5xx = m.get_5xx();

        prometheus_output.push_str(&format!(
            r#"# HELP proxy_requests_total Total number of requests
# TYPE proxy_requests_total counter
proxy_requests_total {}

# HELP proxy_requests_by_status Requests by status code range
# TYPE proxy_requests_by_status counter
proxy_requests_by_status{{status="2xx"}} {}
proxy_requests_by_status{{status="3xx"}} {}
proxy_requests_by_status{{status="4xx"}} {}
proxy_requests_by_status{{status="5xx"}} {}

"#,
            total, status_2xx, status_3xx, status_4xx, status_5xx
        ));

        // Backend metrics
        let backends = state.get_all_backends().await;
        if !backends.is_empty() {
            prometheus_output.push_str("# HELP proxy_backend_up Backend health status (1=up, 0=down)\n");
            prometheus_output.push_str("# TYPE proxy_backend_up gauge\n");

            for backend in &backends {
                let up = match backend.health_status {
                    crate::state::HealthStatus::Healthy => 1,
                    _ => 0,
                };
                prometheus_output.push_str(&format!(
                    "proxy_backend_up{{backend=\"{}\",upstream=\"{}\"}} {}\n",
                    backend.id, backend.upstream, up
                ));
            }

            prometheus_output.push_str("\n# HELP proxy_backend_connections_active Active backend connections\n");
            prometheus_output.push_str("# TYPE proxy_backend_connections_active gauge\n");

            for backend in &backends {
                prometheus_output.push_str(&format!(
                    "proxy_backend_connections_active{{backend=\"{}\",upstream=\"{}\"}} {}\n",
                    backend.id, backend.upstream, backend.active_connections
                ));
            }
            prometheus_output.push('\n');
        }

        // Connection pool metrics
        if let Some(pool_metrics) = state.pool_metrics() {
            let global = pool_metrics.get_global_metrics();

            prometheus_output.push_str(&format!(
                r#"# HELP proxy_pool_connections_active Total active connections in pool
# TYPE proxy_pool_connections_active gauge
proxy_pool_connections_active {}

# HELP proxy_pool_connections_idle Total idle connections in pool
# TYPE proxy_pool_connections_idle gauge
proxy_pool_connections_idle {}

# HELP proxy_pool_connections_created_total Total connections created (lifetime)
# TYPE proxy_pool_connections_created_total counter
proxy_pool_connections_created_total {}

# HELP proxy_pool_connections_reused_total Total connections reused from pool
# TYPE proxy_pool_connections_reused_total counter
proxy_pool_connections_reused_total {}

# HELP proxy_pool_reuse_ratio Connection reuse ratio (0.0-1.0)
# TYPE proxy_pool_reuse_ratio gauge
proxy_pool_reuse_ratio {}

# HELP proxy_pool_errors_total Total connection errors
# TYPE proxy_pool_errors_total counter
proxy_pool_errors_total {}

# HELP proxy_pool_exhausted_total Pool exhaustion events
# TYPE proxy_pool_exhausted_total counter
proxy_pool_exhausted_total {}

# HELP proxy_pool_tracked_hosts Number of tracked upstream hosts
# TYPE proxy_pool_tracked_hosts gauge
proxy_pool_tracked_hosts {}

"#,
                global.total_active,
                global.total_idle,
                global.total_created,
                global.total_reused,
                global.global_reuse_ratio,
                global.total_errors,
                global.total_exhausted,
                global.tracked_hosts
            ));

            // Per-host pool metrics
            if !global.per_host.is_empty() {
                prometheus_output.push_str("# HELP proxy_pool_host_connections_active Active connections per host\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_connections_active gauge\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_connections_active{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.active_connections
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_connections_idle Idle connections per host\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_connections_idle gauge\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_connections_idle{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.idle_connections
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_reuse_ratio Connection reuse ratio per host\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_reuse_ratio gauge\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_reuse_ratio{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.reuse_ratio
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_utilization Pool utilization per host (0.0-1.0)\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_utilization gauge\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_utilization{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.pool_utilization
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_errors_total Connection errors per host\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_errors_total counter\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_errors_total{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.connection_errors
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_exhausted_total Pool exhaustion events per host\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_exhausted_total counter\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_exhausted_total{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.pool_exhausted_count
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_pool_host_avg_lifetime_ms Average connection lifetime per host (milliseconds)\n");
                prometheus_output.push_str("# TYPE proxy_pool_host_avg_lifetime_ms gauge\n");
                for host_metrics in &global.per_host {
                    prometheus_output.push_str(&format!(
                        "proxy_pool_host_avg_lifetime_ms{{host=\"{}\"}} {}\n",
                        host_metrics.host, host_metrics.avg_connection_lifetime_ms
                    ));
                }
                prometheus_output.push('\n');
            }
        }

        // Request metrics (enhanced per-route and per-backend tracking)
        if let Some(request_metrics) = state.request_metrics() {
            // Global response time histogram
            let global_histogram = request_metrics.get_global_response_time();

            prometheus_output.push_str(&format!(
                r#"# HELP proxy_request_response_time_seconds_avg Average response time (seconds)
# TYPE proxy_request_response_time_seconds_avg gauge
proxy_request_response_time_seconds_avg {}

# HELP proxy_request_response_time_seconds Request response time percentiles (seconds)
# TYPE proxy_request_response_time_seconds gauge
proxy_request_response_time_seconds{{quantile="0.5"}} {}
proxy_request_response_time_seconds{{quantile="0.95"}} {}
proxy_request_response_time_seconds{{quantile="0.99"}} {}

# HELP proxy_request_response_time_samples_total Total response time samples
# TYPE proxy_request_response_time_samples_total counter
proxy_request_response_time_samples_total {}

"#,
                global_histogram.avg_ms / 1000.0,
                global_histogram.p50 / 1000.0,
                global_histogram.p95 / 1000.0,
                global_histogram.p99 / 1000.0,
                global_histogram.total_samples
            ));

            // Per-route metrics
            let route_metrics = request_metrics.get_all_route_metrics();
            if !route_metrics.is_empty() {
                prometheus_output.push_str("# HELP proxy_route_requests_total Total requests per route\n");
                prometheus_output.push_str("# TYPE proxy_route_requests_total counter\n");
                for (route, metrics) in &route_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_route_requests_total{{route=\"{}\"}} {}\n",
                        route, metrics.request_count
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_route_requests_by_status Requests per route by status\n");
                prometheus_output.push_str("# TYPE proxy_route_requests_by_status counter\n");
                for (route, metrics) in &route_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_route_requests_by_status{{route=\"{}\",status=\"2xx\"}} {}\n",
                        route, metrics.status_2xx
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_route_requests_by_status{{route=\"{}\",status=\"3xx\"}} {}\n",
                        route, metrics.status_3xx
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_route_requests_by_status{{route=\"{}\",status=\"4xx\"}} {}\n",
                        route, metrics.status_4xx
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_route_requests_by_status{{route=\"{}\",status=\"5xx\"}} {}\n",
                        route, metrics.status_5xx
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_route_response_time_seconds Route response time percentiles\n");
                prometheus_output.push_str("# TYPE proxy_route_response_time_seconds gauge\n");
                for (route, metrics) in &route_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_route_response_time_seconds{{route=\"{}\",quantile=\"0.5\"}} {}\n",
                        route, metrics.response_time.p50 / 1000.0
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_route_response_time_seconds{{route=\"{}\",quantile=\"0.95\"}} {}\n",
                        route, metrics.response_time.p95 / 1000.0
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_route_response_time_seconds{{route=\"{}\",quantile=\"0.99\"}} {}\n",
                        route, metrics.response_time.p99 / 1000.0
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_route_bytes_sent_total Total bytes sent per route\n");
                prometheus_output.push_str("# TYPE proxy_route_bytes_sent_total counter\n");
                for (route, metrics) in &route_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_route_bytes_sent_total{{route=\"{}\"}} {}\n",
                        route, metrics.bytes_sent
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_route_bytes_received_total Total bytes received per route\n");
                prometheus_output.push_str("# TYPE proxy_route_bytes_received_total counter\n");
                for (route, metrics) in &route_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_route_bytes_received_total{{route=\"{}\"}} {}\n",
                        route, metrics.bytes_received
                    ));
                }
                prometheus_output.push('\n');
            }

            // Per-backend metrics
            let backend_metrics = request_metrics.get_all_backend_metrics();
            if !backend_metrics.is_empty() {
                prometheus_output.push_str("# HELP proxy_backend_requests_total Total requests per backend\n");
                prometheus_output.push_str("# TYPE proxy_backend_requests_total counter\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_requests_total{{backend=\"{}\"}} {}\n",
                        backend_id, metrics.request_count
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_backend_errors_total Total errors per backend\n");
                prometheus_output.push_str("# TYPE proxy_backend_errors_total counter\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_errors_total{{backend=\"{}\"}} {}\n",
                        backend_id, metrics.error_count
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_backend_error_rate Backend error rate (0.0-1.0)\n");
                prometheus_output.push_str("# TYPE proxy_backend_error_rate gauge\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_error_rate{{backend=\"{}\"}} {}\n",
                        backend_id, metrics.error_rate
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_backend_response_time_seconds Backend response time percentiles\n");
                prometheus_output.push_str("# TYPE proxy_backend_response_time_seconds gauge\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_response_time_seconds{{backend=\"{}\",quantile=\"0.5\"}} {}\n",
                        backend_id, metrics.response_time.p50 / 1000.0
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_backend_response_time_seconds{{backend=\"{}\",quantile=\"0.95\"}} {}\n",
                        backend_id, metrics.response_time.p95 / 1000.0
                    ));
                    prometheus_output.push_str(&format!(
                        "proxy_backend_response_time_seconds{{backend=\"{}\",quantile=\"0.99\"}} {}\n",
                        backend_id, metrics.response_time.p99 / 1000.0
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_backend_bytes_sent_total Total bytes sent per backend\n");
                prometheus_output.push_str("# TYPE proxy_backend_bytes_sent_total counter\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_bytes_sent_total{{backend=\"{}\"}} {}\n",
                        backend_id, metrics.bytes_sent
                    ));
                }
                prometheus_output.push('\n');

                prometheus_output.push_str("# HELP proxy_backend_bytes_received_total Total bytes received per backend\n");
                prometheus_output.push_str("# TYPE proxy_backend_bytes_received_total counter\n");
                for (backend_id, metrics) in &backend_metrics {
                    prometheus_output.push_str(&format!(
                        "proxy_backend_bytes_received_total{{backend=\"{}\"}} {}\n",
                        backend_id, metrics.bytes_received
                    ));
                }
                prometheus_output.push('\n');
            }
        }
    } else {
        // Return stub data when no state available
        prometheus_output.push_str(
            r#"# HELP proxy_requests_total Total number of requests
# TYPE proxy_requests_total counter
proxy_requests_total 0

# HELP proxy_backend_up Backend health status (1=up, 0=down)
# TYPE proxy_backend_up gauge
proxy_backend_up 0

# HELP proxy_backend_connections_active Active backend connections
# TYPE proxy_backend_connections_active gauge
proxy_backend_connections_active 0
"#,
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(hyper::header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(Full::new(Bytes::from(prometheus_output)))
        .unwrap()
}

/// Helper function to create JSON response
fn json_response<T: Serialize>(status: StatusCode, body: T) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_string(&body).unwrap(),
        )))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_route_metrics() {
        let response = get_route_metrics(None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_backend_metrics() {
        let response = get_backend_metrics(None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_health_history() {
        let response = get_health_history(None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_health_history_with_limit() {
        let response = get_health_history(Some(50)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_export_prometheus_metrics() {
        let response = export_prometheus_metrics(None).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Check content type is Prometheus format
        let content_type = response.headers().get(hyper::header::CONTENT_TYPE).unwrap();
        assert!(content_type.to_str().unwrap().contains("text/plain"));
    }
}
