//! End-to-End Security Tests
//!
//! These tests verify security features with actual HTTP requests and responses:
//! 1. Security headers in HTTP responses
//! 2. Request size limit enforcement
//! 3. HSTS enforcement
//! 4. CSP enforcement
//! 5. Large payload rejection
//!
//! Run with: cargo test --test e2e_security -- --ignored --test-threads=1

use hyper::{body::Buf, Body, Client, Method, Request, StatusCode};
use hyper::header::{HeaderValue, CONTENT_LENGTH};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Global port counter to avoid port conflicts
static PORT_COUNTER: AtomicU32 = AtomicU32::new(20000);

fn get_next_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst) as u16
}

/// Test harness for security E2E tests
struct SecurityTestHarness {
    proxy_process: Option<Child>,
    proxy_port: u16,
    backend_port: u16,
    config_path: String,
    backend_task: Option<tokio::task::JoinHandle<()>>,
}

impl SecurityTestHarness {
    /// Create and start proxy with backend server
    async fn new(config_template: &str) -> Result<Self, String> {
        let proxy_port = get_next_port();
        let backend_port = get_next_port();
        let config_path = format!("/tmp/e2e_security_test_{}.toml", proxy_port);

        // Replace port placeholders in config
        let config = config_template
            .replace("{{PROXY_PORT}}", &proxy_port.to_string())
            .replace("{{BACKEND_PORT}}", &backend_port.to_string());

        // Write config to temp file
        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        // Start simple backend server
        let backend_task = Some(tokio::spawn(async move {
            run_simple_backend(backend_port).await;
        }));

        // Wait for backend to be ready
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Start proxy process
        let proxy_process = Command::new("cargo")
            .args([
                "run", "--release", "--",
                "--config", &config_path,
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start proxy: {}", e))?;

        let mut harness = Self {
            proxy_process: Some(proxy_process),
            proxy_port,
            backend_port,
            config_path,
            backend_task,
        };

        // Wait for proxy to be ready
        harness.wait_for_ready().await?;

        Ok(harness)
    }

    /// Wait for proxy to be ready
    async fn wait_for_ready(&self) -> Result<(), String> {
        let addr = format!("127.0.0.1:{}", self.proxy_port);

        for _ in 0..50 {
            if tokio::net::TcpStream::connect(&addr).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(format!("Proxy failed to start on port {}", self.proxy_port))
    }

    /// Get proxy URL
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.proxy_port, path)
    }
}

impl Drop for SecurityTestHarness {
    fn drop(&mut self) {
        // Kill proxy process
        if let Some(mut process) = self.proxy_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }

        // Clean up config file
        let _ = std::fs::remove_file(&self.config_path);

        // Cancel backend task
        if let Some(task) = self.backend_task.take() {
            task.abort();
        }
    }
}

/// Simple backend server for testing
async fn run_simple_backend(port: u16) {
    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(_) => return,
    };

    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];

            // Read request
            if socket.read(&mut buffer).await.is_err() {
                return;
            }

            // Simple HTTP response
            let response = "HTTP/1.1 200 OK\r\n\
                           Content-Type: application/json\r\n\
                           Content-Length: 18\r\n\
                           \r\n\
                           {\"status\":\"ok\"}";

            let _ = socket.write_all(response.as_bytes()).await;
        });
    }
}

/// Test that default security headers are present in responses
#[tokio::test]
#[ignore] // E2E test - run explicitly
async fn test_e2e_default_security_headers() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.security_headers]
enabled = true
preset = "default"

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    // Make HTTP request
    let client = Client::new();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");

    // Verify security headers are present
    let headers = response.headers();

    assert_eq!(
        headers.get("x-content-type-options").and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "X-Content-Type-Options should be present"
    );

    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY"),
        "X-Frame-Options should be DENY"
    );

    assert_eq!(
        headers.get("x-xss-protection").and_then(|v| v.to_str().ok()),
        Some("1; mode=block"),
        "X-XSS-Protection should be present"
    );

    assert!(
        headers.get("strict-transport-security").is_some(),
        "HSTS header should be present"
    );

    assert_eq!(
        headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
        Some("strict-origin-when-cross-origin"),
        "Referrer-Policy should be present"
    );

    println!("✅ Default security headers test passed");
}

/// Test that strict security headers include CSP and strict HSTS
#[tokio::test]
#[ignore]
async fn test_e2e_strict_security_headers() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.security_headers]
enabled = true
preset = "strict"

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");

    let headers = response.headers();

    // Verify strict HSTS (2 years with preload)
    let hsts = headers.get("strict-transport-security")
        .and_then(|v| v.to_str().ok())
        .expect("HSTS should be present");

    assert!(hsts.contains("max-age=63072000"), "HSTS should have 2-year max-age");
    assert!(hsts.contains("includeSubDomains"), "HSTS should include subdomains");
    assert!(hsts.contains("preload"), "HSTS should have preload");

    // Verify CSP is present
    assert!(
        headers.get("content-security-policy").is_some(),
        "CSP should be present in strict mode"
    );

    // Verify strict referrer policy
    assert_eq!(
        headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
        Some("no-referrer"),
        "Referrer-Policy should be no-referrer in strict mode"
    );

    // Verify Permissions-Policy is present
    assert!(
        headers.get("permissions-policy").is_some(),
        "Permissions-Policy should be present in strict mode"
    );

    println!("✅ Strict security headers test passed");
}

/// Test custom CSP configuration
#[tokio::test]
#[ignore]
async fn test_e2e_custom_csp() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.security_headers]
enabled = true
x_content_type_options = true
csp = "default-src 'self'; script-src 'self' https://cdn.example.com"

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");

    let headers = response.headers();

    // Verify custom CSP
    let csp = headers.get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .expect("CSP should be present");

    assert!(csp.contains("cdn.example.com"), "Custom CSP should contain trusted CDN");
    assert!(csp.contains("script-src"), "CSP should have script-src directive");

    println!("✅ Custom CSP test passed");
}

/// Test that security headers are applied to error responses
#[tokio::test]
#[ignore]
async fn test_e2e_security_headers_on_errors() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.security_headers]
enabled = true
preset = "default"

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:19999" }]  # Non-existent backend
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");

    // Should get error response (502 or 503)
    let status = response.status();
    assert!(
        status == StatusCode::BAD_GATEWAY || status == StatusCode::SERVICE_UNAVAILABLE,
        "Should get error response"
    );

    // Verify security headers are still present on error
    let headers = response.headers();
    assert!(
        headers.get("x-content-type-options").is_some(),
        "Security headers should be present on error responses"
    );

    println!("✅ Security headers on errors test passed");
}

/// Test request size limit enforcement
#[tokio::test]
#[ignore]
async fn test_e2e_request_size_limit() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.request_size_limit]
enabled = true
max_body_size = 1024  # 1 KB limit

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Test 1: Request within limit should succeed
    let small_body = vec![b'x'; 512]; // 512 bytes
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/test"))
        .header(CONTENT_LENGTH, small_body.len())
        .body(Body::from(small_body))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Small request should succeed"
    );

    // Test 2: Request exceeding limit should be rejected with 413
    let large_body = vec![b'x'; 2048]; // 2 KB (exceeds 1 KB limit)
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/test"))
        .header(CONTENT_LENGTH, large_body.len())
        .body(Body::from(large_body))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Large request should be rejected with 413"
    );

    println!("✅ Request size limit test passed");
}

/// Test custom request size limit error message
#[tokio::test]
#[ignore]
async fn test_e2e_request_size_limit_custom_error() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.request_size_limit]
enabled = true
max_body_size = 512  # 512 bytes
error_message = "File too large. Maximum size is 512 bytes."

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Send large request
    let large_body = vec![b'x'; 1024]; // 1 KB (exceeds 512 byte limit)
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/test"))
        .header(CONTENT_LENGTH, large_body.len())
        .body(Body::from(large_body))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // Read response body to verify custom error message
    let body_bytes = hyper::body::to_bytes(response.into_body())
        .await
        .expect("Failed to read body");
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert!(
        body_text.contains("512 bytes"),
        "Custom error message should be present"
    );

    println!("✅ Custom error message test passed");
}

/// Test per-route request size limits
#[tokio::test]
#[ignore]
async fn test_e2e_per_route_size_limits() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

# Global strict limit
[middleware.request_size_limit]
enabled = true
max_body_size = 512

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

# API route with strict limit (inherits global 512 bytes)
[[routes]]
path = "/api"
upstream = "test-backend"

# Upload route with permissive limit (override to 10 KB)
[[routes]]
path = "/upload"
upstream = "test-backend"

[routes.middleware.request_size_limit]
enabled = true
max_body_size = 10240  # 10 KB
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Test 1: Large request to /api should be rejected
    let large_body = vec![b'x'; 1024]; // 1 KB (exceeds 512 byte limit)
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/api/test"))
        .header(CONTENT_LENGTH, large_body.len())
        .body(Body::from(large_body.clone()))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Large request to /api should be rejected"
    );

    // Test 2: Same large request to /upload should succeed
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/upload/file"))
        .header(CONTENT_LENGTH, large_body.len())
        .body(Body::from(large_body))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Large request to /upload should succeed"
    );

    println!("✅ Per-route size limits test passed");
}

/// Test that security headers preserve backend headers
#[tokio::test]
#[ignore]
async fn test_e2e_headers_preservation() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.security_headers]
enabled = true
preset = "default"

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");

    let headers = response.headers();

    // Verify backend headers are preserved
    assert_eq!(
        headers.get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "Backend Content-Type should be preserved"
    );

    // Verify security headers are added
    assert!(
        headers.get("x-content-type-options").is_some(),
        "Security headers should be added"
    );

    println!("✅ Header preservation test passed");
}

/// Test disabled request size limit
#[tokio::test]
#[ignore]
async fn test_e2e_disabled_size_limit() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.request_size_limit]
enabled = false  # Disabled

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = SecurityTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Send very large request (should not be rejected when disabled)
    let large_body = vec![b'x'; 100_000]; // 100 KB
    let request = Request::builder()
        .method(Method::POST)
        .uri(harness.url("/test"))
        .header(CONTENT_LENGTH, large_body.len())
        .body(Body::from(large_body))
        .unwrap();

    let response = client.request(request).await.expect("Request failed");

    // Should succeed since limit is disabled
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Large request should succeed when limit is disabled"
    );

    println!("✅ Disabled size limit test passed");
}

/// Integration test runner helper
#[cfg(test)]
mod test_helpers {
    use super::*;

    /// Run all E2E security tests
    pub async fn run_all_security_tests() {
        println!("\n🔒 Running E2E Security Tests...\n");

        test_e2e_default_security_headers().await;
        test_e2e_strict_security_headers().await;
        test_e2e_custom_csp().await;
        test_e2e_security_headers_on_errors().await;
        test_e2e_request_size_limit().await;
        test_e2e_request_size_limit_custom_error().await;
        test_e2e_per_route_size_limits().await;
        test_e2e_headers_preservation().await;
        test_e2e_disabled_size_limit().await;

        println!("\n✅ All E2E Security Tests Passed!\n");
    }
}
