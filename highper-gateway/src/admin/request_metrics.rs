//! Admin API endpoints for request metrics

use crate::state::RequestMetrics;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use serde_json::json;
use std::sync::Arc;

/// Get all route metrics
pub async fn get_route_metrics(
    request_metrics: Arc<RequestMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let all_metrics = request_metrics.get_all_route_metrics();

    let response = json!({
        "routes": all_metrics.iter().map(|(route, metrics)| {
            json!({
                "route": route,
                "request_count": metrics.request_count,
                "status_2xx": metrics.status_2xx,
                "status_3xx": metrics.status_3xx,
                "status_4xx": metrics.status_4xx,
                "status_5xx": metrics.status_5xx,
                "response_time": {
                    "avg_ms": metrics.response_time.avg_ms,
                    "p50_ms": metrics.response_time.p50,
                    "p95_ms": metrics.response_time.p95,
                    "p99_ms": metrics.response_time.p99,
                    "total_samples": metrics.response_time.total_samples,
                },
                "bytes_sent": metrics.bytes_sent,
                "bytes_received": metrics.bytes_received,
            })
        }).collect::<Vec<_>>(),
        "total_routes": all_metrics.len(),
    });

    let json_str = serde_json::to_string_pretty(&response).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json_str)))
        .unwrap())
}

/// Get metrics for a specific route
pub async fn get_route_metric<B>(
    request_metrics: Arc<RequestMetrics>,
    req: Request<B>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // Parse query parameter
    let uri = req.uri();
    let query = uri.query().unwrap_or("");

    let route_param = query.split('&').find_map(|pair| {
        let mut parts = pair.split('=');
        if parts.next() == Some("route") {
            parts.next()
        } else {
            None
        }
    });

    let route = match route_param {
        Some(r) => urlencoding::decode(r).unwrap_or_default().to_string(),
        None => {
            let error_response = json!({
                "error": "Missing 'route' query parameter",
                "example": "/api/metrics/route?route=/api/users"
            });
            let json_str = serde_json::to_string_pretty(&error_response).unwrap();
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap());
        }
    };

    match request_metrics.get_route_metrics(&route) {
        Some(metrics) => {
            let response = json!({
                "route": route,
                "request_count": metrics.request_count,
                "status_2xx": metrics.status_2xx,
                "status_3xx": metrics.status_3xx,
                "status_4xx": metrics.status_4xx,
                "status_5xx": metrics.status_5xx,
                "response_time": {
                    "avg_ms": metrics.response_time.avg_ms,
                    "p50_ms": metrics.response_time.p50,
                    "p95_ms": metrics.response_time.p95,
                    "p99_ms": metrics.response_time.p99,
                    "total_samples": metrics.response_time.total_samples,
                },
                "bytes_sent": metrics.bytes_sent,
                "bytes_received": metrics.bytes_received,
            });

            let json_str = serde_json::to_string_pretty(&response).unwrap();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap())
        }
        None => {
            let error_response = json!({
                "error": "Route not found",
                "route": route
            });
            let json_str = serde_json::to_string_pretty(&error_response).unwrap();
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap())
        }
    }
}

/// Get all backend metrics
pub async fn get_backend_metrics(
    request_metrics: Arc<RequestMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let all_metrics = request_metrics.get_all_backend_metrics();

    let response = json!({
        "backends": all_metrics.iter().map(|(backend_id, metrics)| {
            json!({
                "backend_id": backend_id,
                "request_count": metrics.request_count,
                "error_count": metrics.error_count,
                "error_rate": metrics.error_rate,
                "response_time": {
                    "avg_ms": metrics.response_time.avg_ms,
                    "p50_ms": metrics.response_time.p50,
                    "p95_ms": metrics.response_time.p95,
                    "p99_ms": metrics.response_time.p99,
                    "total_samples": metrics.response_time.total_samples,
                },
                "bytes_sent": metrics.bytes_sent,
                "bytes_received": metrics.bytes_received,
            })
        }).collect::<Vec<_>>(),
        "total_backends": all_metrics.len(),
    });

    let json_str = serde_json::to_string_pretty(&response).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json_str)))
        .unwrap())
}

/// Get metrics for a specific backend
pub async fn get_backend_metric<B>(
    request_metrics: Arc<RequestMetrics>,
    req: Request<B>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // Parse query parameter
    let uri = req.uri();
    let query = uri.query().unwrap_or("");

    let backend_param = query.split('&').find_map(|pair| {
        let mut parts = pair.split('=');
        if parts.next() == Some("backend") {
            parts.next()
        } else {
            None
        }
    });

    let backend_id = match backend_param {
        Some(b) => urlencoding::decode(b).unwrap_or_default().to_string(),
        None => {
            let error_response = json!({
                "error": "Missing 'backend' query parameter",
                "example": "/api/metrics/backend?backend=api_0"
            });
            let json_str = serde_json::to_string_pretty(&error_response).unwrap();
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap());
        }
    };

    match request_metrics.get_backend_metrics(&backend_id) {
        Some(metrics) => {
            let response = json!({
                "backend_id": backend_id,
                "request_count": metrics.request_count,
                "error_count": metrics.error_count,
                "error_rate": metrics.error_rate,
                "response_time": {
                    "avg_ms": metrics.response_time.avg_ms,
                    "p50_ms": metrics.response_time.p50,
                    "p95_ms": metrics.response_time.p95,
                    "p99_ms": metrics.response_time.p99,
                    "total_samples": metrics.response_time.total_samples,
                },
                "bytes_sent": metrics.bytes_sent,
                "bytes_received": metrics.bytes_received,
            });

            let json_str = serde_json::to_string_pretty(&response).unwrap();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap())
        }
        None => {
            let error_response = json!({
                "error": "Backend not found",
                "backend_id": backend_id
            });
            let json_str = serde_json::to_string_pretty(&error_response).unwrap();
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json_str)))
                .unwrap())
        }
    }
}

/// Get global response time histogram
pub async fn get_global_response_time(
    request_metrics: Arc<RequestMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let histogram = request_metrics.get_global_response_time();

    let response = json!({
        "global_response_time": {
            "avg_ms": histogram.avg_ms,
            "p50_ms": histogram.p50,
            "p95_ms": histogram.p95,
            "p99_ms": histogram.p99,
            "total_samples": histogram.total_samples,
            "buckets": {
                "lt_1ms": histogram.buckets.get(0).unwrap_or(&0),
                "lt_5ms": histogram.buckets.get(1).unwrap_or(&0),
                "lt_10ms": histogram.buckets.get(2).unwrap_or(&0),
                "lt_50ms": histogram.buckets.get(3).unwrap_or(&0),
                "lt_100ms": histogram.buckets.get(4).unwrap_or(&0),
                "lt_500ms": histogram.buckets.get(5).unwrap_or(&0),
                "lt_1s": histogram.buckets.get(6).unwrap_or(&0),
                "lt_5s": histogram.buckets.get(7).unwrap_or(&0),
                "gte_5s": histogram.buckets.get(8).unwrap_or(&0),
            }
        }
    });

    let json_str = serde_json::to_string_pretty(&response).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json_str)))
        .unwrap())
}

/// Reset all request metrics
pub async fn reset_request_metrics(
    request_metrics: Arc<RequestMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    request_metrics.reset_all();

    let response = json!({
        "status": "success",
        "message": "All request metrics have been reset"
    });

    let json_str = serde_json::to_string_pretty(&response).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json_str)))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Empty;
    use std::time::Duration;

    #[tokio::test]
    async fn test_get_route_metrics() {
        let metrics = Arc::new(RequestMetrics::new());

        // Record some test data
        metrics.record_route_request("/api/users", 200, Duration::from_millis(10), 1024, 512);

        let response = get_route_metrics(metrics).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_route_metric_with_query() {
        let metrics = Arc::new(RequestMetrics::new());

        metrics.record_route_request("/api/users", 200, Duration::from_millis(10), 1024, 512);

        let req = Request::builder()
            .uri("/api/metrics/route?route=%2Fapi%2Fusers")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = get_route_metric(metrics, req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_backend_metrics() {
        let metrics = Arc::new(RequestMetrics::new());

        metrics.record_backend_request("api_0", false, Duration::from_millis(5), 512, 1024);

        let response = get_backend_metrics(metrics).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_global_response_time() {
        let metrics = Arc::new(RequestMetrics::new());

        metrics.record_route_request("/test", 200, Duration::from_millis(10), 100, 100);

        let response = get_global_response_time(metrics).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_reset_request_metrics() {
        let metrics = Arc::new(RequestMetrics::new());

        metrics.record_route_request("/test", 200, Duration::from_millis(10), 100, 100);

        let response = reset_request_metrics(metrics.clone()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify reset
        let route_metrics = metrics.get_route_metrics("/test");
        assert!(route_metrics.is_some());
        assert_eq!(route_metrics.unwrap().request_count, 0);
    }
}
