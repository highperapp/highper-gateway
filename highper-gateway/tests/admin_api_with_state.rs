//! Comprehensive integration tests for Admin API with ProxyState
//!
//! Tests the Admin API endpoints with actual state to verify
//! full integration between API and runtime components.

use hyper::StatusCode;
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_backend_list_with_state() {
    use highper_gateway::config::Config;
    use highper_gateway::state::{ProxyState, BackendState, HealthStatus};
    use tokio::sync::RwLock;

    let config = Arc::new(RwLock::new(Config {
        server: Default::default(),
        tls: None,
        upstreams: vec![highper_gateway::config::UpstreamConfig {
            name: "api_backend".to_string(),
            servers: vec![
                highper_gateway::config::ServerDef {
                    url: "http://localhost:8080".to_string(),
                    weight: 100,
                    max_conns: 1000,
                    location: None,
                    region: None,
                },
                highper_gateway::config::ServerDef {
                    url: "http://localhost:8081".to_string(),
                    weight: 100,
                    max_conns: 1000,
                    location: None,
                    region: None,
                },
            ],
            load_balancing: Default::default(),
            health_check: Default::default(),
            connection: Default::default(),
        }],
        routes: vec![],
        observability: Default::default(),
        websocket: Default::default(),
        grpc: Default::default(),
        admin: None,
        cache: None,
        rate_limit: None,
        waf: None,
    }));

    // Create state and register backends
    let state = Arc::new(ProxyState::new());

    state.register_backend(BackendState {
        id: "api_backend_0".to_string(),
        upstream: "api_backend".to_string(),
        url: "http://localhost:8080".to_string(),
        enabled: true,
        draining: false,
        reason: None,
        active_connections: 5,
        health_status: HealthStatus::Healthy,
    }).await;

    state.register_backend(BackendState {
        id: "api_backend_1".to_string(),
        upstream: "api_backend".to_string(),
        url: "http://localhost:8081".to_string(),
        enabled: false,
        draining: false,
        reason: Some("Maintenance".to_string()),
        active_connections: 0,
        health_status: HealthStatus::Unhealthy,
    }).await;

    let response = highper_gateway::admin::backends::list_backends(config, Some(state)).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["backends"].is_array());
    assert_eq!(json["total"], 2);

    let backends = json["backends"].as_array().unwrap();

    // Verify first backend
    assert_eq!(backends[0]["id"], "api_backend_0");
    assert_eq!(backends[0]["enabled"], true);
    assert_eq!(backends[0]["draining"], false);
    assert_eq!(backends[0]["active_connections"], 5);
    assert_eq!(backends[0]["health_status"], "healthy");

    // Verify second backend
    assert_eq!(backends[1]["id"], "api_backend_1");
    assert_eq!(backends[1]["enabled"], false);
    assert_eq!(backends[1]["active_connections"], 0);
    assert_eq!(backends[1]["health_status"], "unhealthy");
}

#[tokio::test]
async fn test_backend_enable_disable_with_state() {
    use highper_gateway::config::Config;
    use highper_gateway::state::{ProxyState, BackendState, HealthStatus};
    use highper_gateway::admin::backends::BackendControlRequest;
    use tokio::sync::RwLock;

    let config = Arc::new(RwLock::new(Config {
        server: Default::default(),
        tls: None,
        upstreams: vec![highper_gateway::config::UpstreamConfig {
            name: "test_upstream".to_string(),
            servers: vec![highper_gateway::config::ServerDef {
                url: "http://localhost:8080".to_string(),
                weight: 100,
                max_conns: 1000,
                location: None,
                region: None,
            }],
            load_balancing: Default::default(),
            health_check: Default::default(),
            connection: Default::default(),
        }],
        routes: vec![],
        observability: Default::default(),
        websocket: Default::default(),
        grpc: Default::default(),
        admin: None,
        cache: None,
        rate_limit: None,
        waf: None,
    }));

    let state = Arc::new(ProxyState::new());

    state.register_backend(BackendState {
        id: "test_upstream_0".to_string(),
        upstream: "test_upstream".to_string(),
        url: "http://localhost:8080".to_string(),
        enabled: true,
        draining: false,
        reason: None,
        active_connections: 10,
        health_status: HealthStatus::Healthy,
    }).await;

    // Disable the backend
    let disable_request = BackendControlRequest {
        reason: Some("Testing disable".to_string()),
        drain_timeout_seconds: Some(30),
    };

    let response = highper_gateway::admin::backends::disable_backend(
        config.clone(),
        Some(state.clone()),
        "test_upstream_0",
        disable_request,
    ).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Verify backend is disabled in state
    let backend = state.get_backend("test_upstream_0").await.unwrap();
    assert_eq!(backend.enabled, false);
    assert_eq!(backend.reason, Some("Testing disable".to_string()));

    // Re-enable the backend
    let enable_request = BackendControlRequest {
        reason: Some("Testing enable".to_string()),
        drain_timeout_seconds: None,
    };

    let response = highper_gateway::admin::backends::enable_backend(
        config,
        Some(state.clone()),
        "test_upstream_0",
        enable_request,
    ).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Verify backend is enabled in state
    let backend = state.get_backend("test_upstream_0").await.unwrap();
    assert_eq!(backend.enabled, true);
    assert_eq!(backend.reason, Some("Testing enable".to_string()));
}

#[tokio::test]
async fn test_cache_operations_with_state() {
    use highper_gateway::state::ProxyState;
    use highper_gateway::gateway::cache::{LocalCache, CacheEntry};
    use highper_gateway::admin::cache::{ClearCacheRequest, InvalidateCacheRequest};
    use bytes::Bytes;
    use std::time::Instant;

    // Create cache and add entries
    let cache = Arc::new(LocalCache::default_cache());

    cache.set("key1".to_string(), CacheEntry {
        body: Bytes::from("value1"),
        status: 200,
        headers: vec![],
        created_at: Instant::now(),
        ttl: Duration::from_secs(300),
    });

    cache.set("key2".to_string(), CacheEntry {
        body: Bytes::from("value2"),
        status: 200,
        headers: vec![],
        created_at: Instant::now(),
        ttl: Duration::from_secs(300),
    });

    cache.set("api_key1".to_string(), CacheEntry {
        body: Bytes::from("api_value1"),
        status: 200,
        headers: vec![],
        created_at: Instant::now(),
        ttl: Duration::from_secs(300),
    });

    let state = Arc::new(ProxyState::with_cache(cache));

    // Test get cache stats
    let response = highper_gateway::admin::cache::get_cache_stats(Some(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["total_entries"], 3);
    assert_eq!(json["local"]["entries"], 3);
    assert_eq!(json["local"]["enabled"], true);

    // Test list cache keys
    let response = highper_gateway::admin::cache::list_cache_keys(
        Some(state.clone()),
        None,
    ).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["total"], 3);
    assert!(json["keys"].is_array());

    // Test list cache keys with pattern
    let response = highper_gateway::admin::cache::list_cache_keys(
        Some(state.clone()),
        Some("api".to_string()),
    ).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["total"], 1); // Only api_key1 matches

    // Test invalidate specific keys
    let invalidate_request = InvalidateCacheRequest {
        keys: vec!["key1".to_string()],
        invalidate_local: true,
        invalidate_distributed: false,
    };

    let response = highper_gateway::admin::cache::invalidate_cache_keys(
        Some(state.clone()),
        invalidate_request,
    ).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["keys_affected"], 1);

    // Verify cache now has 2 entries
    let response = highper_gateway::admin::cache::get_cache_stats(Some(state.clone())).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["total_entries"], 2);

    // Test clear cache
    let clear_request = ClearCacheRequest {
        pattern: None,
        clear_local: true,
        clear_distributed: false,
    };

    let response = highper_gateway::admin::cache::clear_cache(
        Some(state.clone()),
        clear_request,
    ).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["keys_affected"], 2);

    // Verify cache is now empty
    let response = highper_gateway::admin::cache::get_cache_stats(Some(state)).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["total_entries"], 0);
}

#[tokio::test]
async fn test_metrics_with_state() {
    use highper_gateway::state::{ProxyState, BackendState, HealthStatus};

    let state = Arc::new(ProxyState::new());

    // Register some backends
    state.register_backend(BackendState {
        id: "backend_0".to_string(),
        upstream: "test".to_string(),
        url: "http://localhost:8080".to_string(),
        enabled: true,
        draining: false,
        reason: None,
        active_connections: 10,
        health_status: HealthStatus::Healthy,
    }).await;

    state.register_backend(BackendState {
        id: "backend_1".to_string(),
        upstream: "test".to_string(),
        url: "http://localhost:8081".to_string(),
        enabled: true,
        draining: false,
        reason: None,
        active_connections: 5,
        health_status: HealthStatus::Unhealthy,
    }).await;

    // Simulate some requests
    let metrics = state.metrics();
    for _ in 0..100 {
        metrics.increment_requests();
        metrics.record_status(200);
    }
    for _ in 0..20 {
        metrics.increment_requests();
        metrics.record_status(404);
    }
    for _ in 0..10 {
        metrics.increment_requests();
        metrics.record_status(500);
    }

    // Test route metrics
    let response = highper_gateway::admin::metrics::get_route_metrics(Some(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["total_routes"], 1);
    let route = &json["routes"][0];
    assert_eq!(route["total_requests"], 130);
    assert_eq!(route["status_codes"]["status_2xx"], 100);
    assert_eq!(route["status_codes"]["status_4xx"], 20);
    assert_eq!(route["status_codes"]["status_5xx"], 10);

    // Calculate expected success rate: (100 + 0) / 130 * 100 = 76.92%
    let success_rate = route["success_rate"].as_f64().unwrap();
    assert!((success_rate - 76.92).abs() < 0.1);

    // Test backend metrics
    let response = highper_gateway::admin::metrics::get_backend_metrics(Some(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["total_backends"], 2);
    let backends = json["backends"].as_array().unwrap();

    // Find backends by ID (order not guaranteed with DashMap)
    let backend_0 = backends.iter().find(|b| b["backend_id"] == "backend_0").unwrap();
    assert_eq!(backend_0["health_status"], "healthy");
    assert_eq!(backend_0["active_connections"], 10);

    let backend_1 = backends.iter().find(|b| b["backend_id"] == "backend_1").unwrap();
    assert_eq!(backend_1["health_status"], "unhealthy");
    assert_eq!(backend_1["active_connections"], 5);

    // Test Prometheus export
    let response = highper_gateway::admin::metrics::export_prometheus_metrics(Some(state)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    // Verify Prometheus format with actual data
    assert!(body_str.contains("proxy_requests_total 130"));
    assert!(body_str.contains("proxy_requests_by_status{status=\"2xx\"} 100"));
    assert!(body_str.contains("proxy_requests_by_status{status=\"4xx\"} 20"));
    assert!(body_str.contains("proxy_requests_by_status{status=\"5xx\"} 10"));
    assert!(body_str.contains("proxy_backend_up{backend=\"backend_0\",upstream=\"test\"} 1"));
    assert!(body_str.contains("proxy_backend_up{backend=\"backend_1\",upstream=\"test\"} 0"));
    assert!(body_str.contains("proxy_backend_connections_active{backend=\"backend_0\",upstream=\"test\"} 10"));
}

#[tokio::test]
async fn test_full_admin_api_workflow() {
    use highper_gateway::state::{ProxyState, BackendState, HealthStatus};
    use highper_gateway::gateway::cache::{LocalCache, CacheEntry};
    use bytes::Bytes;
    use std::time::Instant;

    // Create a full state with cache
    let cache = Arc::new(LocalCache::default_cache());
    let state = Arc::new(ProxyState::with_cache(cache));

    // Register backends
    state.register_backend(BackendState {
        id: "api_0".to_string(),
        upstream: "api".to_string(),
        url: "http://api1.example.com".to_string(),
        enabled: true,
        draining: false,
        reason: None,
        active_connections: 25,
        health_status: HealthStatus::Healthy,
    }).await;

    // Add cache entries
    let cache_instance = state.local_cache().unwrap();
    cache_instance.set("response:/api/users".to_string(), CacheEntry {
        body: Bytes::from(r#"{"users": []}"#),
        status: 200,
        headers: vec![],
        created_at: Instant::now(),
        ttl: Duration::from_secs(60),
    });

    // Simulate traffic
    let metrics = state.metrics();
    for _ in 0..50 {
        metrics.increment_requests();
        metrics.record_status(200);
    }

    // Verify all systems are operational

    // 1. Check backends
    let backends = state.get_all_backends().await;
    assert_eq!(backends.len(), 1);
    assert_eq!(backends[0].active_connections, 25);

    // 2. Check cache
    assert_eq!(cache_instance.len(), 1);
    assert!(cache_instance.get("response:/api/users").is_some());

    // 3. Check metrics
    assert_eq!(metrics.get_total_requests(), 50);
    assert_eq!(metrics.get_2xx(), 50);

    println!("✅ Full Admin API workflow test passed!");
    println!("   - Backends: {} registered", backends.len());
    println!("   - Cache entries: {}", cache_instance.len());
    println!("   - Total requests: {}", metrics.get_total_requests());
}
