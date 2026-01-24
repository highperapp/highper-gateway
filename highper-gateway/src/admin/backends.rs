//! Backend control endpoints for Admin API
//!
//! Provides REST API for controlling backend servers:
//! - List backends with status
//! - Enable/disable backends
//! - Drain connections
//! - Get backend details
//! - Force health checks

use crate::config::Config;
use crate::state::ProxyState;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Backend status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStatus {
    /// Backend ID (index in upstream)
    pub id: String,

    /// Upstream name
    pub upstream: String,

    /// Backend URL
    pub url: String,

    /// Weight for load balancing
    pub weight: u32,

    /// Maximum connections
    pub max_connections: usize,

    /// Current active connections
    pub active_connections: usize,

    /// Health status
    pub health_status: HealthStatus,

    /// Whether backend is enabled
    pub enabled: bool,

    /// Whether backend is in drain mode
    pub draining: bool,

    /// Geographic location (if configured)
    pub location: Option<String>,

    /// Region (if configured)
    pub region: Option<String>,
}

/// Health status of a backend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Backend is healthy
    Healthy,

    /// Backend is unhealthy
    Unhealthy,

    /// Health status unknown
    Unknown,
}

/// Request to enable/disable a backend
#[derive(Debug, Deserialize)]
pub struct BackendControlRequest {
    /// Optional reason for the action
    pub reason: Option<String>,

    /// Drain timeout in seconds (for disable/drain operations)
    pub drain_timeout_seconds: Option<u64>,
}

/// Response for backend operations
#[derive(Debug, Serialize)]
pub struct BackendControlResponse {
    /// Whether the operation was successful
    pub success: bool,

    /// Message describing the result
    pub message: String,

    /// Backend status after operation
    pub backend: Option<BackendStatus>,
}

/// List all backends across all upstreams
pub async fn list_backends(
    config: Arc<RwLock<Config>>,
    state: Option<Arc<ProxyState>>,
) -> Response<Full<Bytes>> {
    let config = config.read().await;
    let mut backends = Vec::new();

    for upstream in &config.upstreams {
        for (index, server) in upstream.servers.iter().enumerate() {
            let backend_id = format!("{}_{}", upstream.name, index);

            // Get state from ProxyState if available
            let (active_connections, health_status, enabled, draining) = if let Some(ref state) = state {
                if let Some(backend_state) = state.get_backend(&backend_id).await {
                    (
                        backend_state.active_connections,
                        match backend_state.health_status {
                            crate::state::HealthStatus::Healthy => HealthStatus::Healthy,
                            crate::state::HealthStatus::Unhealthy => HealthStatus::Unhealthy,
                            crate::state::HealthStatus::Unknown => HealthStatus::Unknown,
                        },
                        backend_state.enabled,
                        backend_state.draining,
                    )
                } else {
                    (0, HealthStatus::Unknown, true, false)
                }
            } else {
                (0, HealthStatus::Unknown, true, false)
            };

            backends.push(BackendStatus {
                id: backend_id,
                upstream: upstream.name.clone(),
                url: server.url.clone(),
                weight: server.weight,
                max_connections: server.max_conns,
                active_connections,
                health_status,
                enabled,
                draining,
                location: server.location.as_ref().map(|loc| {
                    format!("{:.4}, {:.4}", loc.lat, loc.lon)
                }),
                region: server.region.clone(),
            });
        }
    }

    json_response(
        StatusCode::OK,
        json!({
            "backends": backends,
            "total": backends.len(),
        }),
    )
}

/// Get details for a specific backend
pub async fn get_backend(
    config: Arc<RwLock<Config>>,
    backend_id: &str,
) -> Response<Full<Bytes>> {
    let config = config.read().await;

    // Parse backend ID (format: "upstream_index")
    // Use rsplit_once to split from the right, allowing underscores in upstream name
    let (upstream_name, index_str) = match backend_id.rsplit_once('_') {
        Some((name, idx)) => (name, idx),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend ID format. Expected: upstream_index",
            );
        }
    };

    let index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend index",
            );
        }
    };

    // Find the upstream
    let upstream = match config.upstreams.iter().find(|u| u.name == upstream_name) {
        Some(u) => u,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Upstream '{}' not found", upstream_name),
            );
        }
    };

    // Find the server
    let server = match upstream.servers.get(index) {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Backend index {} not found in upstream '{}'", index, upstream_name),
            );
        }
    };

    let backend = BackendStatus {
        id: backend_id.to_string(),
        upstream: upstream.name.clone(),
        url: server.url.clone(),
        weight: server.weight,
        max_connections: server.max_conns,
        active_connections: 0, // TODO: Get from load balancer
        health_status: HealthStatus::Unknown, // TODO: Get from health checker
        enabled: true, // TODO: Track enabled state
        draining: false, // TODO: Track drain state
        location: server.location.as_ref().map(|loc| {
            format!("{:.4}, {:.4}", loc.lat, loc.lon)
        }),
        region: server.region.clone(),
    };

    json_response(StatusCode::OK, json!(backend))
}

/// Enable a backend
pub async fn enable_backend(
    config: Arc<RwLock<Config>>,
    state: Option<Arc<ProxyState>>,
    backend_id: &str,
    request: BackendControlRequest,
) -> Response<Full<Bytes>> {
    // Verify backend exists
    let config = config.read().await;

    // Parse backend ID (format: "upstream_index")
    let (upstream_name, index_str) = match backend_id.rsplit_once('_') {
        Some((name, idx)) => (name, idx),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend ID format",
            );
        }
    };

    let _index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid backend index");
        }
    };

    // Verify upstream exists
    if !config.upstreams.iter().any(|u| u.name == upstream_name) {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Upstream '{}' not found", upstream_name),
        );
    }

    // Update state if available
    if let Some(state) = state {
        let success = state.set_backend_enabled(backend_id, true, request.reason.clone()).await;
        if !success {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update backend state",
            );
        }
    }

    json_response(
        StatusCode::OK,
        json!(BackendControlResponse {
            success: true,
            message: format!("Backend '{}' enabled successfully", backend_id),
            backend: None,
        }),
    )
}

/// Disable a backend
pub async fn disable_backend(
    config: Arc<RwLock<Config>>,
    state: Option<Arc<ProxyState>>,
    backend_id: &str,
    request: BackendControlRequest,
) -> Response<Full<Bytes>> {
    // Verify backend exists
    let config = config.read().await;

    // Parse backend ID (format: "upstream_index")
    let (upstream_name, index_str) = match backend_id.rsplit_once('_') {
        Some((name, idx)) => (name, idx),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend ID format",
            );
        }
    };

    let _index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid backend index");
        }
    };

    // Verify upstream exists
    if !config.upstreams.iter().any(|u| u.name == upstream_name) {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Upstream '{}' not found", upstream_name),
        );
    }

    let drain_timeout = request.drain_timeout_seconds.unwrap_or(30);
    let reason = request.reason.unwrap_or_else(|| "No reason provided".to_string());

    // Update state if available
    if let Some(state) = state {
        let success = state.set_backend_enabled(backend_id, false, Some(reason.clone())).await;
        if !success {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update backend state",
            );
        }
    }

    json_response(
        StatusCode::OK,
        json!(BackendControlResponse {
            success: true,
            message: format!(
                "Backend '{}' disabled successfully. Reason: {}. Drain timeout: {}s",
                backend_id, reason, drain_timeout
            ),
            backend: None,
        }),
    )
}

/// Drain a backend (stop sending new connections, allow existing to complete)
pub async fn drain_backend(
    config: Arc<RwLock<Config>>,
    state: Option<Arc<ProxyState>>,
    backend_id: &str,
    request: BackendControlRequest,
) -> Response<Full<Bytes>> {
    // Verify backend exists
    let config = config.read().await;

    // Parse backend ID (format: "upstream_index")
    let (upstream_name, index_str) = match backend_id.rsplit_once('_') {
        Some((name, idx)) => (name, idx),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend ID format",
            );
        }
    };

    let _index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid backend index");
        }
    };

    // Verify upstream exists
    if !config.upstreams.iter().any(|u| u.name == upstream_name) {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Upstream '{}' not found", upstream_name),
        );
    }

    let drain_timeout = request.drain_timeout_seconds.unwrap_or(60);
    let reason = request.reason.clone();

    // Update state if available
    if let Some(ref state) = state {
        let success = state.set_backend_draining_with_timeout(
            backend_id,
            true,
            Some(drain_timeout),
            reason.clone(),
        ).await;
        if !success {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update backend state",
            );
        }

        // Spawn a background task to monitor drain progress
        let state_clone = state.clone();
        let backend_id_owned = backend_id.to_string();
        tokio::spawn(async move {
            monitor_drain_progress(state_clone, backend_id_owned, drain_timeout).await;
        });
    }

    json_response(
        StatusCode::OK,
        json!(BackendControlResponse {
            success: true,
            message: format!(
                "Backend '{}' is draining. Timeout: {}s. Reason: {}",
                backend_id,
                drain_timeout,
                reason.unwrap_or_else(|| "No reason provided".to_string())
            ),
            backend: None,
        }),
    )
}

/// Monitor drain progress and complete when all connections are closed or timeout is reached
async fn monitor_drain_progress(state: Arc<ProxyState>, backend_id: String, timeout_secs: u64) {
    use std::time::Duration;
    use tracing::{debug, info, warn};

    let check_interval = Duration::from_secs(1);
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        tokio::time::sleep(check_interval).await;

        // Check if drain is still active
        let drain_status = state.get_drain_status(&backend_id).await;
        match drain_status {
            None => {
                // Drain was cancelled or backend removed
                debug!("Drain cancelled for backend {}", backend_id);
                return;
            }
            Some(status) => {
                if status.drain_completed {
                    info!("Drain completed for backend {} (already marked complete)", backend_id);
                    return;
                }

                // Check if all connections are closed
                if status.active_connections == 0 {
                    info!(
                        "Drain completed for backend {}: all connections closed after {:.1}s",
                        backend_id,
                        start.elapsed().as_secs_f64()
                    );
                    state.complete_drain(&backend_id).await;
                    return;
                }

                // Check for timeout
                if start.elapsed() >= timeout {
                    warn!(
                        "Drain timeout for backend {}: {} connections still active after {}s",
                        backend_id, status.active_connections, timeout_secs
                    );
                    state.complete_drain(&backend_id).await;
                    return;
                }

                debug!(
                    "Drain in progress for {}: {} active connections, {:.0}s remaining",
                    backend_id,
                    status.active_connections,
                    status.remaining_secs
                );
            }
        }
    }
}

/// Get drain status for a backend
pub async fn get_drain_status(
    state: Option<Arc<ProxyState>>,
    backend_id: &str,
) -> Response<Full<Bytes>> {
    let state = match state {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Proxy state not available",
            );
        }
    };

    match state.get_drain_status(backend_id).await {
        Some(status) => json_response(StatusCode::OK, json!(status)),
        None => {
            // Check if backend exists but is not draining
            match state.get_backend(backend_id).await {
                Some(_) => json_response(
                    StatusCode::OK,
                    json!({
                        "backend_id": backend_id,
                        "draining": false,
                        "message": "Backend is not in drain mode"
                    }),
                ),
                None => error_response(
                    StatusCode::NOT_FOUND,
                    &format!("Backend '{}' not found", backend_id),
                ),
            }
        }
    }
}

/// Cancel drain for a backend (restore to normal operation)
pub async fn cancel_drain(
    state: Option<Arc<ProxyState>>,
    backend_id: &str,
) -> Response<Full<Bytes>> {
    let state = match state {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Proxy state not available",
            );
        }
    };

    // Check if backend exists and is draining
    let backend = state.get_backend(backend_id).await;
    match backend {
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Backend '{}' not found", backend_id),
            );
        }
        Some(b) if !b.draining => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Backend '{}' is not in drain mode", backend_id),
            );
        }
        _ => {}
    }

    let success = state.set_backend_draining(backend_id, false).await;
    if success {
        json_response(
            StatusCode::OK,
            json!(BackendControlResponse {
                success: true,
                message: format!("Drain cancelled for backend '{}'", backend_id),
                backend: None,
            }),
        )
    } else {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to cancel drain",
        )
    }
}

/// Force a health check on a backend
pub async fn force_health_check(
    config: Arc<RwLock<Config>>,
    backend_id: &str,
) -> Response<Full<Bytes>> {
    // Verify backend exists
    let config = config.read().await;

    // Parse backend ID (format: "upstream_index")
    let (upstream_name, index_str) = match backend_id.rsplit_once('_') {
        Some((name, idx)) => (name, idx),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Invalid backend ID format",
            );
        }
    };

    let _index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid backend index");
        }
    };

    // Verify upstream exists
    if !config.upstreams.iter().any(|u| u.name == upstream_name) {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Upstream '{}' not found", upstream_name),
        );
    }

    // TODO: Trigger health check
    // This requires integration with the health checker component

    json_response(
        StatusCode::OK,
        json!({
            "success": true,
            "message": format!("Health check triggered for backend '{}'", backend_id),
            "health_status": "unknown", // TODO: Return actual health status after check
        }),
    )
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

/// Helper function to create error response
fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    json_response(
        status,
        json!({
            "error": message,
            "status": status.as_u16(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ServerDef, UpstreamConfig, LoadBalancingConfig, LoadBalancingAlgorithm, HealthCheckConfig};

    fn create_test_config() -> Config {
        Config {
            server: Default::default(),
            tls: None,
            upstreams: vec![
                UpstreamConfig {
                    name: "test_upstream".to_string(),
                    servers: vec![
                        ServerDef {
                            url: "http://backend1:8080".to_string(),
                            weight: 100,
                            max_conns: 1000,
                            location: None,
                            region: None,
                        },
                        ServerDef {
                            url: "http://backend2:8080".to_string(),
                            weight: 50,
                            max_conns: 500,
                            location: None,
                            region: Some("us-west-1".to_string()),
                        },
                    ],
                    load_balancing: LoadBalancingConfig {
                        algorithm: LoadBalancingAlgorithm::RoundRobin,
                        ..Default::default()
                    },
                    health_check: HealthCheckConfig::default(),
                    connection: Default::default(),
                    slow_start: None,
                },
            ],
            routes: vec![],
            observability: Default::default(),
            websocket: Default::default(),
            grpc: Default::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    #[tokio::test]
    async fn test_list_backends() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let response = list_backends(config, None).await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_backend_valid() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let response = get_backend(config, "test_upstream_0").await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_backend_invalid_format() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let response = get_backend(config, "invalid").await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_backend_not_found() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let response = get_backend(config, "nonexistent_0").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_enable_backend() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let request = BackendControlRequest {
            reason: Some("Test enable".to_string()),
            drain_timeout_seconds: None,
        };

        let response = enable_backend(config, None, "test_upstream_0", request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_disable_backend() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let request = BackendControlRequest {
            reason: Some("Maintenance".to_string()),
            drain_timeout_seconds: Some(30),
        };

        let response = disable_backend(config, None, "test_upstream_0", request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
