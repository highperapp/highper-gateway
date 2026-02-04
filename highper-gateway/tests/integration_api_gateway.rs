//! Integration tests for API Gateway functionality
//! Tests hostname-based routing, path matching, and hot reload

use highper_gateway::gateway::routing::{
    HostConfig, HostnameRouter, HostnameRoutesConfig, MatchedRoute, PathMatch, RouteConfig,
    UpstreamConfig,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_hostname_routing_exact_match() {
    let router = HostnameRouter::new();

    // Create test configuration
    let mut upstreams = HashMap::new();
    upstreams.insert(
        "users-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8081".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "users".to_string(),
                path_match: PathMatch::Exact {
                    path: "/api/users".to_string(),
                },
                upstream: "users-service".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams,
    };

    // Load configuration
    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test exact match
    let matched = router
        .match_request("api.example.com", "/api/users", "GET")
        .await;

    assert!(matched.is_some());
    let matched_route = matched.unwrap();
    assert_eq!(matched_route.route.name, "users");
    assert_eq!(matched_route.route.upstream, "users-service");

    // Test non-matching path
    let not_matched = router
        .match_request("api.example.com", "/api/products", "GET")
        .await;
    assert!(not_matched.is_none());
}

#[tokio::test]
async fn test_hostname_routing_prefix_match() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "products-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8082".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "products".to_string(),
                path_match: PathMatch::Prefix {
                    prefix: "/api/products/".to_string(),
                },
                upstream: "products-service".to_string(),
                methods: vec![],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test prefix match - should match any path starting with prefix
    let matched1 = router
        .match_request("api.example.com", "/api/products/123", "GET")
        .await;
    assert!(matched1.is_some());

    let matched2 = router
        .match_request("api.example.com", "/api/products/123/details", "GET")
        .await;
    assert!(matched2.is_some());

    // Should not match if prefix doesn't match
    let not_matched = router
        .match_request("api.example.com", "/api/users", "GET")
        .await;
    assert!(not_matched.is_none());
}

#[tokio::test]
async fn test_hostname_routing_pattern_match() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "orders-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8083".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "orders".to_string(),
                path_match: PathMatch::Pattern {
                    pattern: r"^/api/orders/\d+$".to_string(),
                },
                upstream: "orders-service".to_string(),
                methods: vec![],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test pattern match with valid order ID
    let matched = router
        .match_request("api.example.com", "/api/orders/12345", "GET")
        .await;
    assert!(matched.is_some());

    // Should not match if pattern doesn't match
    let not_matched = router
        .match_request("api.example.com", "/api/orders/abc", "GET")
        .await;
    assert!(not_matched.is_none());
}

#[tokio::test]
async fn test_wildcard_hostname_matching() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "tenant-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8084".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "*.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "wildcard-route".to_string(),
                path_match: PathMatch::Exact {
                    path: "/api/health".to_string(),
                },
                upstream: "tenant-service".to_string(),
                methods: vec![],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test wildcard matching with different subdomains
    let matched1 = router
        .match_request_wildcard("tenant1.example.com", "/api/health", "GET")
        .await;
    assert!(matched1.is_some());

    let matched2 = router
        .match_request_wildcard("tenant2.example.com", "/api/health", "GET")
        .await;
    assert!(matched2.is_some());

    // Should not match different domain
    let not_matched = router
        .match_request_wildcard("other.com", "/api/health", "GET")
        .await;
    assert!(not_matched.is_none());
}

#[tokio::test]
async fn test_method_filtering() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "write-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8085".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "create-user".to_string(),
                path_match: PathMatch::Exact {
                    path: "/api/users".to_string(),
                },
                upstream: "write-service".to_string(),
                methods: vec!["POST".to_string(), "PUT".to_string()],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test allowed methods
    let matched_post = router
        .match_request("api.example.com", "/api/users", "POST")
        .await;
    assert!(matched_post.is_some());

    let matched_put = router
        .match_request("api.example.com", "/api/users", "PUT")
        .await;
    assert!(matched_put.is_some());

    // Test disallowed method
    let not_matched_get = router
        .match_request("api.example.com", "/api/users", "GET")
        .await;
    assert!(not_matched_get.is_none());
}

#[tokio::test]
async fn test_route_priority_exact_over_prefix() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "exact-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8086".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );
    upstreams.insert(
        "prefix-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8087".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![
                RouteConfig {
                    name: "exact-route".to_string(),
                    path_match: PathMatch::Exact {
                        path: "/api/special".to_string(),
                    },
                    upstream: "exact-service".to_string(),
                    methods: vec![],
                    timeout_ms: None,
                    middleware: vec![],
                    metadata: HashMap::new(),
                },
                RouteConfig {
                    name: "prefix-route".to_string(),
                    path_match: PathMatch::Prefix {
                        prefix: "/api/".to_string(),
                    },
                    upstream: "prefix-service".to_string(),
                    methods: vec![],
                    timeout_ms: None,
                    middleware: vec![],
                    metadata: HashMap::new(),
                },
            ],
        }],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Exact match should take priority
    let matched = router
        .match_request("api.example.com", "/api/special", "GET")
        .await;
    assert!(matched.is_some());
    let matched_route = matched.unwrap();
    assert_eq!(matched_route.route.name, "exact-route");
    assert_eq!(matched_route.route.upstream, "exact-service");

    // Other paths should match prefix
    let matched_prefix = router
        .match_request("api.example.com", "/api/other", "GET")
        .await;
    assert!(matched_prefix.is_some());
    let matched_route = matched_prefix.unwrap();
    assert_eq!(matched_route.route.name, "prefix-route");
    assert_eq!(matched_route.route.upstream, "prefix-service");
}

#[tokio::test]
async fn test_multiple_hosts() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "api-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8088".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );
    upstreams.insert(
        "web-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8089".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![
            HostConfig {
                hostname: "api.example.com".to_string(),
                routes: vec![RouteConfig {
                    name: "api-route".to_string(),
                    path_match: PathMatch::Prefix {
                        prefix: "/api/".to_string(),
                    },
                    upstream: "api-service".to_string(),
                    methods: vec![],
                    timeout_ms: None,
                    middleware: vec![],
                    metadata: HashMap::new(),
                }],
            },
            HostConfig {
                hostname: "www.example.com".to_string(),
                routes: vec![RouteConfig {
                    name: "web-route".to_string(),
                    path_match: PathMatch::Prefix {
                        prefix: "/".to_string(),
                    },
                    upstream: "web-service".to_string(),
                    methods: vec![],
                    timeout_ms: None,
                    middleware: vec![],
                    metadata: HashMap::new(),
                }],
            },
        ],
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Test first hostname
    let matched_api = router
        .match_request("api.example.com", "/api/users", "GET")
        .await;
    assert!(matched_api.is_some());
    assert_eq!(matched_api.unwrap().route.upstream, "api-service");

    // Test second hostname
    let matched_web = router
        .match_request("www.example.com", "/index.html", "GET")
        .await;
    assert!(matched_web.is_some());
    assert_eq!(matched_web.unwrap().route.upstream, "web-service");
}

#[tokio::test]
async fn test_load_performance_10k_routes() {
    let router = HostnameRouter::new();

    // Create 10,000 routes across 100 hosts
    let mut upstreams = HashMap::new();
    for i in 0..1000 {
        upstreams.insert(
            format!("service-{}", i),
            UpstreamConfig {
                servers: vec![format!("http://localhost:{}", 9000 + i)],
                algorithm: "round_robin".to_string(),
                health_check: None,
            },
        );
    }

    let mut hosts = Vec::new();
    for host_idx in 0..100 {
        let mut routes = Vec::new();
        for route_idx in 0..100 {
            routes.push(RouteConfig {
                name: format!("route-{}-{}", host_idx, route_idx),
                path_match: PathMatch::Exact {
                    path: format!("/api/resource{}", route_idx),
                },
                upstream: format!("service-{}", (host_idx * 10 + route_idx % 10)),
                methods: vec![],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            });
        }
        hosts.push(HostConfig {
            hostname: format!("api{}.example.com", host_idx),
            routes,
        });
    }

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts,
        upstreams,
    };

    let json = serde_json::to_string(&config).unwrap();

    // Measure load time
    let start = std::time::Instant::now();
    router.load_from_json(&json).await.unwrap();
    let load_duration = start.elapsed();

    println!(
        "✓ Loaded 10,000 routes across 100 hosts in {:?}",
        load_duration
    );
    assert!(
        load_duration.as_secs() < 5,
        "Load should complete in under 5 seconds"
    );

    // Measure lookup time (should be O(1) for hostname)
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let host = format!("api{}.example.com", i % 100);
        let path = format!("/api/resource{}", i % 100);
        let _ = router.match_request(&host, &path, "GET").await;
    }
    let lookup_duration = start.elapsed();

    println!("✓ Performed 1,000 lookups in {:?}", lookup_duration);
    println!("  Average: {:?} per lookup", lookup_duration / 1000);
    assert!(
        lookup_duration.as_millis() < 100,
        "1000 lookups should complete in under 100ms"
    );
}

#[tokio::test]
async fn test_json_export() {
    let router = HostnameRouter::new();

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "test-service".to_string(),
        UpstreamConfig {
            servers: vec!["http://localhost:8090".to_string()],
            algorithm: "round_robin".to_string(),
            health_check: None,
        },
    );

    let config = HostnameRoutesConfig {
        version: "1.0".to_string(),
        hosts: vec![HostConfig {
            hostname: "api.example.com".to_string(),
            routes: vec![RouteConfig {
                name: "test-route".to_string(),
                path_match: PathMatch::Exact {
                    path: "/api/test".to_string(),
                },
                upstream: "test-service".to_string(),
                methods: vec![],
                timeout_ms: None,
                middleware: vec![],
                metadata: HashMap::new(),
            }],
        }],
        upstreams: upstreams.clone(),
    };

    let json = serde_json::to_string(&config).unwrap();
    router.load_from_json(&json).await.unwrap();

    // Export and verify
    let exported = router.export_to_json().await.unwrap();
    let exported_config: HostnameRoutesConfig = serde_json::from_str(&exported).unwrap();

    assert_eq!(exported_config.version, "1.0");
    assert_eq!(exported_config.hosts.len(), 1);
    assert_eq!(exported_config.hosts[0].hostname, "api.example.com");
    assert_eq!(exported_config.hosts[0].routes.len(), 1);
    assert_eq!(exported_config.hosts[0].routes[0].name, "test-route");
    assert_eq!(exported_config.hosts[0].routes[0].upstream, "test-service");

    // Verify upstreams are now tracked in HostnameRouter
    assert_eq!(exported_config.upstreams.len(), 1);
    assert!(exported_config.upstreams.contains_key("test-service"));
    assert_eq!(
        exported_config
            .upstreams
            .get("test-service")
            .unwrap()
            .servers
            .len(),
        1
    );
}
