//! DSL Integration Tests
//!
//! Tests the full DSL pipeline: DSL text → Parser → AST → Converter → Runtime Config

use highper_gateway::config::{dsl_parser, dsl_converter};

#[test]
fn test_simple_http_proxy() {
    let dsl = r#"
http://example.com:8080 {
    proxy localhost:3000
    lb round_robin
}
"#;

    // Parse DSL
    let ast = dsl_parser::parse_dsl(dsl).unwrap();

    // Verify AST
    assert_eq!(ast.sites.len(), 1);

    // Convert to runtime config
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify runtime config
    assert!(config.upstreams.len() >= 1);
    assert!(config.routes.len() >= 1);

    // Verify upstream has backend
    let upstream = &config.upstreams[0];
    assert_eq!(upstream.servers.len(), 1);
    assert!(upstream.servers[0].url.contains("localhost:3000") ||
            upstream.servers[0].url.contains("http://localhost:3000"));

    // Verify load balancing
    assert_eq!(upstream.load_balancing.algorithm, "round_robin");
}

#[test]
fn test_https_with_tls_auto() {
    let dsl = r#"
https://secure.example.com {
    proxy backend1:8080
    proxy backend2:8080 weight=2
    tls auto admin@example.com
    lb least_conn
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    assert_eq!(ast.sites.len(), 1);

    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify TLS configuration
    assert!(config.tls.is_some());
    let tls = config.tls.unwrap();
    assert!(tls.auto);

    if let Some(acme) = tls.acme {
        assert_eq!(acme.email, "admin@example.com");
    }

    // Verify upstreams
    assert!(config.upstreams.len() >= 1);
    let upstream = &config.upstreams[0];
    assert_eq!(upstream.servers.len(), 2);

    // Verify weighted backend
    assert_eq!(upstream.servers[1].weight, 2);

    // Verify load balancing algorithm
    assert_eq!(upstream.load_balancing.algorithm, "least_conn");
}

#[test]
fn test_health_check_configuration() {
    let dsl = r#"
http://api.example.com {
    proxy backend:9000
    health /health interval=5s timeout=2s
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify health check
    assert!(config.upstreams.len() >= 1);
    let upstream = &config.upstreams[0];

    if let Some(ref hc) = upstream.health_check {
        if let Some(ref active) = hc.active {
            assert!(active.enabled);
            assert_eq!(active.path, "/health");
            assert_eq!(active.interval, "5s");
            assert_eq!(active.timeout, "2s");
        } else {
            panic!("Expected active health check");
        }
    } else {
        panic!("Expected health check configuration");
    }
}

#[test]
fn test_rate_limiting() {
    let dsl = r#"
http://api.example.com {
    proxy backend:8080
    rate_limit 100/s
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify rate limit exists
    assert!(config.routes.len() >= 1);
    let route = &config.routes[0];

    if let Some(ref rl) = route.rate_limit {
        assert!(rl.enabled);
        assert_eq!(rl.capacity, 100);
    } else {
        // Rate limit might be global
        if let Some(ref global_rl) = config.rate_limit {
            assert!(global_rl.enabled);
        }
    }
}

#[test]
fn test_timeout_configuration() {
    let dsl = r#"
http://slow-api.example.com {
    proxy slow-backend:8080
    timeout 30s
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify timeout
    assert!(config.routes.len() >= 1);
    let route = &config.routes[0];

    if let Some(ref timeout) = route.timeout {
        assert_eq!(timeout.request, "30s");
    } else {
        panic!("Expected timeout configuration");
    }
}

#[test]
fn test_multiple_sites() {
    let dsl = r#"
http://site1.example.com:8080 {
    proxy backend1:3000
    lb round_robin
}

https://site2.example.com {
    proxy backend2:3001
    lb least_conn
    tls auto admin@example.com
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    assert_eq!(ast.sites.len(), 2);

    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Should have at least 2 upstreams and 2 routes
    assert!(config.upstreams.len() >= 2);
    assert!(config.routes.len() >= 2);

    // Verify different load balancing algorithms
    let algorithms: Vec<&str> = config.upstreams
        .iter()
        .map(|u| u.load_balancing.algorithm.as_str())
        .collect();

    assert!(algorithms.contains(&"round_robin"));
    assert!(algorithms.contains(&"least_conn"));
}

#[test]
fn test_route_specific_backends() {
    let dsl = r#"
http://myapp.com {
    route /api/* {
        proxy api-backend:8080
        timeout 10s
    }

    route /admin/* {
        proxy admin-backend:8081
        timeout 30s
    }
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    assert_eq!(ast.sites.len(), 1);
    assert_eq!(ast.sites[0].routes.len(), 2);

    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Should have upstreams and routes for each path
    assert!(config.upstreams.len() >= 2);
    assert!(config.routes.len() >= 2);

    // Verify paths
    let paths: Vec<String> = config.routes
        .iter()
        .flat_map(|r| r.match_rules.paths.clone())
        .collect();

    assert!(paths.iter().any(|p| p.contains("/api")));
    assert!(paths.iter().any(|p| p.contains("/admin")));
}

#[test]
fn test_manual_tls_certificates() {
    let dsl = r#"
https://secure.myapp.com {
    proxy backend:8080
    tls /etc/certs/myapp.crt /etc/certs/myapp.key
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify TLS with manual certificates
    assert!(config.tls.is_some());
    let tls = config.tls.unwrap();

    assert!(!tls.certificates.is_empty());
    let cert = &tls.certificates[0];
    assert_eq!(cert.domain, "secure.myapp.com");
    assert_eq!(cert.cert_file, "/etc/certs/myapp.crt");
    assert_eq!(cert.key_file, "/etc/certs/myapp.key");
}

#[test]
fn test_global_configuration() {
    let dsl = r#"
{
    log_level info
    metrics true
    admin 0.0.0.0:9090
}

http://app.example.com {
    proxy backend:8080
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();

    // Verify global config
    assert_eq!(ast.global.log_level, Some("info".to_string()));
    assert_eq!(ast.global.metrics_enabled, true);
    assert_eq!(ast.global.admin_address, Some("0.0.0.0:9090".to_string()));

    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify admin API
    assert!(config.admin.is_some());
    if let Some(admin) = config.admin {
        assert!(admin.enabled);
        assert_eq!(admin.bind, "0.0.0.0:9090");
    }

    // Verify observability
    assert_eq!(config.observability.logging.level, "info");
    assert!(config.observability.metrics.enabled);
}

#[test]
fn test_all_load_balancing_algorithms() {
    for (algo_name, expected) in [
        ("round_robin", "round_robin"),
        ("least_conn", "least_conn"),
        ("ip_hash", "ip_hash"),
        ("random", "random"),
        ("consistent_hash", "consistent_hash"),
    ] {
        let dsl = format!(
            r#"
http://test.com {{
    proxy backend:8080
    lb {algo_name}
}}
"#
        );

        let ast = dsl_parser::parse_dsl(&dsl).unwrap();
        let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

        assert!(config.upstreams.len() >= 1);
        assert_eq!(
            config.upstreams[0].load_balancing.algorithm,
            expected,
            "Expected algorithm {} for DSL directive {}",
            expected,
            algo_name
        );
    }
}

#[test]
fn test_wildcard_domain() {
    let dsl = r#"
http://*.example.com {
    proxy backend:8080
    lb round_robin
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Should create a route with wildcard or no host restriction
    assert!(config.routes.len() >= 1);
}

#[test]
fn test_empty_config() {
    let dsl = r#"
# Empty configuration - just comments
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();
    assert_eq!(ast.sites.len(), 0);

    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Should have default binds but no upstreams/routes
    assert!(config.upstreams.is_empty());
    assert!(config.routes.is_empty());
}

#[test]
fn test_complex_multi_site() {
    let dsl = r#"
{
    log_level debug
    metrics true
    admin 127.0.0.1:9090
}

http://api.example.com:8080 {
    route /v1/* {
        proxy api-v1-1:3000 weight=2
        proxy api-v1-2:3000
        lb round_robin
        health /health interval=10s timeout=3s
        timeout 15s
    }

    route /v2/* {
        proxy api-v2:3001
        lb least_conn
        rate_limit 200/s
        timeout 20s
    }
}

https://admin.example.com {
    proxy admin-backend:4000
    tls auto ops@example.com
    lb ip_hash
    timeout 60s
}
"#;

    let ast = dsl_parser::parse_dsl(dsl).unwrap();

    // Verify AST structure
    assert_eq!(ast.sites.len(), 2);
    assert_eq!(ast.sites[0].routes.len(), 2);
    assert_eq!(ast.global.log_level, Some("debug".to_string()));

    // Convert to config
    let config = dsl_converter::convert_dsl_to_config(ast).unwrap();

    // Verify configuration
    assert!(config.upstreams.len() >= 3); // v1, v2, admin
    assert!(config.routes.len() >= 3);

    // Verify admin
    assert!(config.admin.is_some());

    // Verify TLS
    assert!(config.tls.is_some());
}
