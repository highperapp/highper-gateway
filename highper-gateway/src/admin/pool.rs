//! Admin API endpoints for connection pool metrics

use crate::proxy::ConnectionPoolMetrics;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use serde_json;
use std::sync::Arc;
use tracing::{debug, error};

/// Get global connection pool metrics
///
/// Returns aggregated metrics across all upstream hosts including:
/// - Total active/idle connections
/// - Connection reuse ratio
/// - Pool exhaustion events
/// - Per-host breakdown
pub async fn get_pool_metrics(
    pool_metrics: Arc<ConnectionPoolMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    debug!("Admin API: Getting connection pool metrics");

    let metrics = pool_metrics.get_global_metrics();

    match serde_json::to_string_pretty(&metrics) {
        Ok(json) => {
            debug!(
                "Pool metrics: {} active, {} idle, {:.2}% reuse ratio across {} hosts",
                metrics.total_active,
                metrics.total_idle,
                metrics.global_reuse_ratio * 100.0,
                metrics.tracked_hosts
            );

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json)))
                .unwrap())
        }
        Err(e) => {
            error!("Failed to serialize pool metrics: {}", e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(format!(
                    "Failed to serialize metrics: {}",
                    e
                ))))
                .unwrap())
        }
    }
}

/// Get connection pool metrics for a specific host
///
/// Query parameter: `host` - The upstream host URL
///
/// Returns detailed metrics for the specified host:
/// - Active/idle connections
/// - Reuse ratio
/// - Pool utilization
/// - Average connection lifetime
pub async fn get_host_pool_metrics<B>(
    pool_metrics: Arc<ConnectionPoolMetrics>,
    req: Request<B>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // Extract host from query parameters
    let query = req.uri().query().unwrap_or("");
    let host = query
        .split('&')
        .find(|param| param.starts_with("host="))
        .and_then(|param| param.strip_prefix("host="));

    let host = match host {
        Some(h) => h,
        None => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Missing 'host' query parameter")))
                .unwrap());
        }
    };

    debug!("Admin API: Getting pool metrics for host: {}", host);

    match pool_metrics.get_host_metrics(host) {
        Some(metrics) => match serde_json::to_string_pretty(&metrics) {
            Ok(json) => {
                debug!(
                    "Host {} metrics: {} active, {} idle, {:.2}% reuse",
                    host,
                    metrics.active_connections,
                    metrics.idle_connections,
                    metrics.reuse_ratio * 100.0
                );

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Full::new(Bytes::from(json)))
                    .unwrap())
            }
            Err(e) => {
                error!("Failed to serialize host metrics: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from(format!(
                        "Failed to serialize metrics: {}",
                        e
                    ))))
                    .unwrap())
            }
        },
        None => {
            debug!("No metrics found for host: {}", host);
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from(format!(
                    "No metrics found for host: {}",
                    host
                ))))
                .unwrap())
        }
    }
}

/// Reset connection pool metrics (for testing/debugging)
///
/// Clears all tracked metrics and resets counters to zero.
/// This is useful for testing or when you want to start fresh metrics collection.
pub async fn reset_pool_metrics(
    pool_metrics: Arc<ConnectionPoolMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    debug!("Admin API: Resetting connection pool metrics");

    pool_metrics.reset();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(
            "Connection pool metrics reset successfully",
        )))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Empty;

    #[tokio::test]
    async fn test_get_pool_metrics() {
        let metrics = Arc::new(ConnectionPoolMetrics::new());

        // Record some test data
        metrics.record_connection_created("http://backend1:8080");
        metrics.record_connection_created("http://backend2:8080");

        let response = get_pool_metrics(metrics).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_host_pool_metrics() {
        let metrics = Arc::new(ConnectionPoolMetrics::new());
        let host = "http://backend1:8080";

        // Record some test data
        metrics.record_connection_created(host);
        metrics.record_connection_reused(host);

        let req = Request::builder()
            .uri(format!("/api/pool/host?host={}", host))
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = get_host_pool_metrics(metrics, req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_host_pool_metrics_missing_param() {
        let metrics = Arc::new(ConnectionPoolMetrics::new());

        let req = Request::builder()
            .uri("/api/pool/host")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = get_host_pool_metrics(metrics, req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_reset_pool_metrics() {
        let metrics = Arc::new(ConnectionPoolMetrics::new());

        // Add some data
        metrics.record_connection_created("http://backend:8080");

        let response = reset_pool_metrics(Arc::clone(&metrics)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify metrics are reset
        let global = metrics.get_global_metrics();
        assert_eq!(global.total_active, 0);
        assert_eq!(global.total_created, 0);
    }
}
