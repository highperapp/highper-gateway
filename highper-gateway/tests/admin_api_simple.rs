//! Simple integration tests for Admin API endpoints
//!
//! Tests the endpoint functions directly to verify correct behavior.

use hyper::StatusCode;
use http_body_util::BodyExt;
use serde_json::Value;

#[tokio::test]
async fn test_backend_list_empty() {
    use highper_gateway::config::Config;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = Arc::new(RwLock::new(Config {
        server: Default::default(),
        tls: None,
        upstreams: vec![],
        routes: vec![],
        observability: Default::default(),
        websocket: Default::default(),
        grpc: Default::default(),
        admin: None,
        cache: None,
        rate_limit: None,
        waf: None,
    }));

    let response = highper_gateway::admin::backends::list_backends(config, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["backends"].is_array());
    assert_eq!(json["total"], 0);
}

#[tokio::test]
async fn test_cache_stats() {
    let response = highper_gateway::admin::cache::get_cache_stats(None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json.get("total_entries").is_some());
}

#[tokio::test]
async fn test_cache_clear() {
    let request = highper_gateway::admin::cache::ClearCacheRequest {
        pattern: Some("/api/*".to_string()),
        clear_local: true,
        clear_distributed: false,
    };

    let response = highper_gateway::admin::cache::clear_cache(None, request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["success"], true);
    assert!(json["message"].as_str().unwrap().contains("/api/*"));
}

#[tokio::test]
async fn test_cache_invalidate() {
    let request = highper_gateway::admin::cache::InvalidateCacheRequest {
        keys: vec!["key1".to_string(), "key2".to_string()],
        invalidate_local: true,
        invalidate_distributed: true,
    };

    let response = highper_gateway::admin::cache::invalidate_cache_keys(None, request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["keys_affected"], 0); // No cache, so 0 keys affected
}

#[tokio::test]
async fn test_list_cache_keys() {
    let response = highper_gateway::admin::cache::list_cache_keys(None, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["keys"].is_array());
    assert_eq!(json["total"], 0);
}

#[tokio::test]
async fn test_route_metrics() {
    let response = highper_gateway::admin::metrics::get_route_metrics(None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["routes"].is_array());
    assert_eq!(json["total_routes"], 0);
}

#[tokio::test]
async fn test_backend_metrics() {
    let response = highper_gateway::admin::metrics::get_backend_metrics(None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["backends"].is_array());
    assert_eq!(json["total_backends"], 0);
}

#[tokio::test]
async fn test_health_history() {
    let response = highper_gateway::admin::metrics::get_health_history(Some(50)).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["history"].is_array());
    assert_eq!(json["total"], 0);
    assert_eq!(json["limit"], 50);
}

#[tokio::test]
async fn test_prometheus_export() {
    let response = highper_gateway::admin::metrics::export_prometheus_metrics(None).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Check content type
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/plain"));

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    // Verify Prometheus format
    assert!(body_str.contains("# HELP"));
    assert!(body_str.contains("# TYPE"));
    assert!(body_str.contains("proxy_requests_total"));
    assert!(body_str.contains("proxy_backend_up"));
}

#[tokio::test]
async fn test_backend_get_invalid_id() {
    use highper_gateway::config::Config;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = Arc::new(RwLock::new(Config {
        server: Default::default(),
        tls: None,
        upstreams: vec![],
        routes: vec![],
        observability: Default::default(),
        websocket: Default::default(),
        grpc: Default::default(),
        admin: None,
        cache: None,
        rate_limit: None,
        waf: None,
    }));

    // Test invalid format (no underscore)
    let response = highper_gateway::admin::backends::get_backend(config.clone(), "invalid").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(json["error"].as_str().unwrap().contains("Invalid backend ID"));
}

#[tokio::test]
async fn test_backend_get_not_found() {
    use highper_gateway::config::Config;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = Arc::new(RwLock::new(Config {
        server: Default::default(),
        tls: None,
        upstreams: vec![],
        routes: vec![],
        observability: Default::default(),
        websocket: Default::default(),
        grpc: Default::default(),
        admin: None,
        cache: None,
        rate_limit: None,
        waf: None,
    }));

    // Test non-existent upstream
    let response = highper_gateway::admin::backends::get_backend(config, "nonexistent_0").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(json["error"].as_str().unwrap().contains("not found"));
}
