//! Security Headers Middleware Integration Tests
//!
//! These tests verify that security headers are properly applied to HTTP responses
//! across different configurations and scenarios.

use hyper::{Response, StatusCode};
use highper_gateway::http::ResponseBody;
use highper_gateway::middleware::{
    headers::{SecurityHeadersConfig, SecurityHeadersMiddleware},
    Middleware,
};

/// Helper to create a basic response for testing
fn create_test_response() -> Response<ResponseBody> {
    Response::builder()
        .status(StatusCode::OK)
        .body(ResponseBody::empty())
        .unwrap()
}

/// Test that default security headers are applied
#[tokio::test]
async fn test_default_security_headers() {
    let middleware = SecurityHeadersMiddleware::default_security();
    let response = create_test_response();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");

    let response = result.unwrap();
    let headers = response.headers();

    // Verify X-Content-Type-Options
    assert_eq!(
        headers.get("x-content-type-options").and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "X-Content-Type-Options should be nosniff"
    );

    // Verify X-Frame-Options
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY"),
        "X-Frame-Options should be DENY by default"
    );

    // Verify X-XSS-Protection
    assert_eq!(
        headers.get("x-xss-protection").and_then(|v| v.to_str().ok()),
        Some("1; mode=block"),
        "X-XSS-Protection should be enabled"
    );

    // Verify HSTS
    let hsts = headers.get("strict-transport-security")
        .and_then(|v| v.to_str().ok());
    assert!(hsts.is_some(), "HSTS header should be present");
    assert!(hsts.unwrap().contains("max-age="), "HSTS should have max-age");

    // Verify Referrer-Policy
    assert_eq!(
        headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
        Some("strict-origin-when-cross-origin"),
        "Referrer-Policy should be set"
    );

    // Verify X-Powered-By (branding)
    let powered_by = headers.get("x-powered-by")
        .and_then(|v| v.to_str().ok());
    assert!(powered_by.is_some(), "X-Powered-By header should be present");
    assert!(powered_by.unwrap().contains("highper-gateway"), "Should contain project name");
}

/// Test strict security headers configuration
#[tokio::test]
async fn test_strict_security_headers() {
    let middleware = SecurityHeadersMiddleware::strict();
    let response = create_test_response();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");

    let response = result.unwrap();
    let headers = response.headers();

    // Verify strict HSTS (2 years with preload)
    let hsts = headers.get("strict-transport-security")
        .and_then(|v| v.to_str().ok())
        .expect("HSTS should be present");
    assert!(hsts.contains("max-age=63072000"), "HSTS should be 2 years");
    assert!(hsts.contains("includeSubDomains"), "HSTS should include subdomains");
    assert!(hsts.contains("preload"), "HSTS should have preload");

    // Verify CSP is present in strict mode
    let csp = headers.get("content-security-policy")
        .and_then(|v| v.to_str().ok());
    assert!(csp.is_some(), "CSP should be present in strict mode");
    assert!(csp.unwrap().contains("default-src"), "CSP should have default-src");

    // Verify strict referrer policy
    assert_eq!(
        headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
        Some("no-referrer"),
        "Strict mode should use no-referrer"
    );

    // Verify Permissions-Policy is present
    let perms = headers.get("permissions-policy")
        .and_then(|v| v.to_str().ok());
    assert!(perms.is_some(), "Permissions-Policy should be present in strict mode");
}

/// Test relaxed security headers configuration
#[tokio::test]
async fn test_relaxed_security_headers() {
    let middleware = SecurityHeadersMiddleware::relaxed();
    let response = create_test_response();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");

    let response = result.unwrap();
    let headers = response.headers();

    // Verify X-Frame-Options is SAMEORIGIN (more permissive)
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("SAMEORIGIN"),
        "Relaxed mode should use SAMEORIGIN"
    );

    // Verify HSTS is NOT present in relaxed mode
    let hsts = headers.get("strict-transport-security");
    assert!(hsts.is_none(), "HSTS should not be present in relaxed mode");

    // Verify CSP is NOT present in relaxed mode
    let csp = headers.get("content-security-policy");
    assert!(csp.is_none(), "CSP should not be present in relaxed mode");

    // Basic headers should still be present
    assert_eq!(
        headers.get("x-content-type-options").and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "X-Content-Type-Options should always be set"
    );
}

/// Test custom security headers configuration
#[tokio::test]
async fn test_custom_security_headers() {
    let config = SecurityHeadersConfig {
        x_content_type_options: true,
        x_frame_options: Some("SAMEORIGIN".to_string()),
        x_xss_protection: None, // Disabled
        hsts: Some("max-age=15552000".to_string()), // 6 months
        csp: Some("default-src 'self'; script-src 'self' https://cdn.example.com".to_string()),
        referrer_policy: Some("same-origin".to_string()),
        permissions_policy: Some("geolocation=(), camera=()".to_string()),
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");

    let response = result.unwrap();
    let headers = response.headers();

    // Verify custom X-Frame-Options
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("SAMEORIGIN"),
        "Custom X-Frame-Options should be applied"
    );

    // Verify X-XSS-Protection is absent (disabled)
    assert!(
        headers.get("x-xss-protection").is_none(),
        "X-XSS-Protection should be disabled"
    );

    // Verify custom HSTS
    assert_eq!(
        headers.get("strict-transport-security").and_then(|v| v.to_str().ok()),
        Some("max-age=15552000"),
        "Custom HSTS should be applied"
    );

    // Verify custom CSP
    let csp = headers.get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .expect("CSP should be present");
    assert!(csp.contains("cdn.example.com"), "Custom CSP should be applied");

    // Verify custom Referrer-Policy
    assert_eq!(
        headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
        Some("same-origin"),
        "Custom Referrer-Policy should be applied"
    );

    // Verify custom Permissions-Policy
    let perms = headers.get("permissions-policy")
        .and_then(|v| v.to_str().ok())
        .expect("Permissions-Policy should be present");
    assert!(perms.contains("geolocation=()"), "Custom Permissions-Policy should be applied");
}

/// Test that security headers don't interfere with existing headers
#[tokio::test]
async fn test_security_headers_preserve_existing() {
    let middleware = SecurityHeadersMiddleware::default_security();

    // Create response with existing headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-cache")
        .header("X-Custom-Header", "custom-value")
        .body(ResponseBody::empty())
        .unwrap();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");

    let response = result.unwrap();
    let headers = response.headers();

    // Verify existing headers are preserved
    assert_eq!(
        headers.get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "Content-Type should be preserved"
    );

    assert_eq!(
        headers.get("cache-control").and_then(|v| v.to_str().ok()),
        Some("no-cache"),
        "Cache-Control should be preserved"
    );

    assert_eq!(
        headers.get("x-custom-header").and_then(|v| v.to_str().ok()),
        Some("custom-value"),
        "Custom headers should be preserved"
    );

    // Verify security headers are added
    assert!(
        headers.get("x-content-type-options").is_some(),
        "Security headers should be added"
    );
}

/// Test security headers with different status codes
#[tokio::test]
async fn test_security_headers_with_error_responses() {
    let middleware = SecurityHeadersMiddleware::default_security();

    // Test with 404 Not Found
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(ResponseBody::empty())
        .unwrap();

    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");
    let response = result.unwrap();
    assert!(response.headers().get("x-content-type-options").is_some(),
            "Security headers should be added to 404 responses");

    // Test with 500 Internal Server Error
    let response = Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(ResponseBody::empty())
        .unwrap();

    let middleware = SecurityHeadersMiddleware::default_security();
    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");
    let response = result.unwrap();
    assert!(response.headers().get("x-frame-options").is_some(),
            "Security headers should be added to 500 responses");

    // Test with 204 No Content
    let response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(ResponseBody::empty())
        .unwrap();

    let middleware = SecurityHeadersMiddleware::default_security();
    let result = middleware.process_response(response).await;
    assert!(result.is_ok(), "Processing should succeed");
    let response = result.unwrap();
    assert!(response.headers().get("strict-transport-security").is_some(),
            "Security headers should be added to 204 responses");
}

/// Test HSTS header variations
#[tokio::test]
async fn test_hsts_configurations() {
    // Test basic HSTS
    let config = SecurityHeadersConfig {
        x_content_type_options: false,
        x_frame_options: None,
        x_xss_protection: None,
        hsts: Some("max-age=31536000".to_string()), // 1 year
        csp: None,
        referrer_policy: None,
        permissions_policy: None,
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();
    let result = middleware.process_response(response).await.unwrap();

    let hsts = result.headers().get("strict-transport-security")
        .and_then(|v| v.to_str().ok())
        .expect("HSTS should be present");
    assert_eq!(hsts, "max-age=31536000");

    // Test HSTS with includeSubDomains
    let config = SecurityHeadersConfig {
        x_content_type_options: false,
        x_frame_options: None,
        x_xss_protection: None,
        hsts: Some("max-age=31536000; includeSubDomains".to_string()),
        csp: None,
        referrer_policy: None,
        permissions_policy: None,
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();
    let result = middleware.process_response(response).await.unwrap();

    let hsts = result.headers().get("strict-transport-security")
        .and_then(|v| v.to_str().ok())
        .expect("HSTS should be present");
    assert!(hsts.contains("includeSubDomains"));

    // Test HSTS with preload
    let config = SecurityHeadersConfig {
        x_content_type_options: false,
        x_frame_options: None,
        x_xss_protection: None,
        hsts: Some("max-age=31536000; includeSubDomains; preload".to_string()),
        csp: None,
        referrer_policy: None,
        permissions_policy: None,
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();
    let result = middleware.process_response(response).await.unwrap();

    let hsts = result.headers().get("strict-transport-security")
        .and_then(|v| v.to_str().ok())
        .expect("HSTS should be present");
    assert!(hsts.contains("preload"));
}

/// Test CSP configurations
#[tokio::test]
async fn test_csp_configurations() {
    // Test basic CSP
    let config = SecurityHeadersConfig {
        x_content_type_options: false,
        x_frame_options: None,
        x_xss_protection: None,
        hsts: None,
        csp: Some("default-src 'self'".to_string()),
        referrer_policy: None,
        permissions_policy: None,
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();
    let result = middleware.process_response(response).await.unwrap();

    let csp = result.headers().get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .expect("CSP should be present");
    assert_eq!(csp, "default-src 'self'");

    // Test complex CSP
    let csp_value = "default-src 'self'; script-src 'self' 'unsafe-inline' https://cdn.example.com; style-src 'self' 'unsafe-inline'";
    let config = SecurityHeadersConfig {
        x_content_type_options: false,
        x_frame_options: None,
        x_xss_protection: None,
        hsts: None,
        csp: Some(csp_value.to_string()),
        referrer_policy: None,
        permissions_policy: None,
    };

    let middleware = SecurityHeadersMiddleware::new(config);
    let response = create_test_response();
    let result = middleware.process_response(response).await.unwrap();

    let csp = result.headers().get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .expect("CSP should be present");
    assert!(csp.contains("script-src"));
    assert!(csp.contains("style-src"));
    assert!(csp.contains("cdn.example.com"));
}

/// Test middleware name
#[test]
fn test_middleware_name() {
    let middleware = SecurityHeadersMiddleware::default_security();
    assert_eq!(middleware.name(), "security-headers");
}

/// Performance test: Ensure minimal overhead
#[tokio::test]
async fn test_security_headers_performance() {
    use std::time::Instant;

    let middleware = SecurityHeadersMiddleware::default_security();
    let iterations = 10000;

    let start = Instant::now();
    for _ in 0..iterations {
        let response = create_test_response();
        let _ = middleware.process_response(response).await;
    }
    let duration = start.elapsed();

    let per_request = duration / iterations;
    println!("Average time per request: {:?}", per_request);

    // Should be very fast (< 1ms per request)
    assert!(per_request.as_micros() < 1000,
            "Security headers should add minimal overhead");
}
