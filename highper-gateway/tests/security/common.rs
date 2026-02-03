//! Common utilities for security testing
//!
//! Provides test harnesses, helpers, and shared functionality
//! for all security tests.

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Global port counter to avoid port conflicts across tests
static PORT_COUNTER: AtomicU32 = AtomicU32::new(21000);

/// Get next available port for testing
pub fn get_next_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst) as u16
}

/// Security test result with detailed information
#[derive(Debug, Clone)]
pub struct SecurityTestResult {
    pub test_id: String,
    pub vulnerability: String,
    pub severity: Severity,
    pub status: TestStatus,
    pub details: String,
    pub response_code: Option<u16>,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::High => write!(f, "HIGH"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::Low => write!(f, "LOW"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Pass,
    Fail,
    Blocked,
    Error,
    Skipped,
}

impl std::fmt::Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestStatus::Pass => write!(f, "PASS"),
            TestStatus::Fail => write!(f, "FAIL"),
            TestStatus::Blocked => write!(f, "BLOCKED"),
            TestStatus::Error => write!(f, "ERROR"),
            TestStatus::Skipped => write!(f, "SKIPPED"),
        }
    }
}

/// Security test harness for E2E penetration testing
pub struct SecurityTestHarness {
    pub proxy_process: Option<Child>,
    pub proxy_port: u16,
    pub backend_port: u16,
    pub config_path: String,
    pub backend_task: Option<tokio::task::JoinHandle<()>>,
    pub results: Vec<SecurityTestResult>,
}

impl SecurityTestHarness {
    /// Create new test harness with config template
    pub async fn new(config_template: &str) -> Result<Self, String> {
        let proxy_port = get_next_port();
        let backend_port = get_next_port();
        let config_path = format!("/tmp/security_test_{}.toml", proxy_port);

        // Replace port placeholders in config
        let config = config_template
            .replace("{{PROXY_PORT}}", &proxy_port.to_string())
            .replace("{{BACKEND_PORT}}", &backend_port.to_string());

        // Write config to temp file
        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        // Start simple backend server
        let backend_task = Some(tokio::spawn(async move {
            run_test_backend(backend_port).await;
        }));

        // Wait for backend to be ready
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Start proxy process
        let proxy_process = Command::new("cargo")
            .args(["run", "--release", "--", "--config", &config_path])
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
            results: Vec::new(),
        };

        // Wait for proxy to be ready
        harness.wait_for_ready().await?;

        Ok(harness)
    }

    /// Create harness for TCP testing (no HTTP)
    pub async fn new_tcp(config_template: &str) -> Result<Self, String> {
        let proxy_port = get_next_port();
        let backend_port = get_next_port();
        let config_path = format!("/tmp/security_tcp_test_{}.toml", proxy_port);

        let config = config_template
            .replace("{{PROXY_PORT}}", &proxy_port.to_string())
            .replace("{{BACKEND_PORT}}", &backend_port.to_string());

        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        // Start TCP echo backend
        let backend_task = Some(tokio::spawn(async move {
            run_tcp_echo_backend(backend_port).await;
        }));

        tokio::time::sleep(Duration::from_millis(200)).await;

        let proxy_process = Command::new("cargo")
            .args(["run", "--release", "--", "--config", &config_path])
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
            results: Vec::new(),
        };

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

    /// Get HTTP URL for proxy
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.proxy_port, path)
    }

    /// Get HTTPS URL for proxy
    pub fn https_url(&self, path: &str) -> String {
        format!("https://127.0.0.1:{}{}", self.proxy_port, path)
    }

    /// Get TCP address for proxy
    pub fn tcp_addr(&self) -> String {
        format!("127.0.0.1:{}", self.proxy_port)
    }

    /// Record test result
    pub fn record_result(&mut self, result: SecurityTestResult) {
        self.results.push(result);
    }

    /// Get all results
    pub fn get_results(&self) -> &[SecurityTestResult] {
        &self.results
    }

    /// Print test results summary
    pub fn print_summary(&self) {
        println!("\n========== Security Test Results ==========\n");

        let mut passed = 0;
        let mut failed = 0;
        let mut blocked = 0;
        let mut errors = 0;

        for result in &self.results {
            match result.status {
                TestStatus::Pass => passed += 1,
                TestStatus::Fail => failed += 1,
                TestStatus::Blocked => blocked += 1,
                TestStatus::Error => errors += 1,
                TestStatus::Skipped => {}
            }

            let status_icon = match result.status {
                TestStatus::Pass => "✓",
                TestStatus::Fail => "✗",
                TestStatus::Blocked => "◌",
                TestStatus::Error => "⚠",
                TestStatus::Skipped => "○",
            };

            println!(
                "[{}] {} - {} ({})",
                status_icon, result.test_id, result.vulnerability, result.severity
            );

            if result.status == TestStatus::Fail || result.status == TestStatus::Error {
                println!("    Details: {}", result.details);
            }
        }

        println!("\n--------------------------------------------");
        println!(
            "Total: {} | Passed: {} | Failed: {} | Blocked: {} | Errors: {}",
            self.results.len(),
            passed,
            failed,
            blocked,
            errors
        );
        println!("============================================\n");
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

/// Simple HTTP backend server for testing
pub async fn run_test_backend(port: u16) {
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
            let mut buffer = vec![0u8; 65536];

            // Read request
            if socket.read(&mut buffer).await.is_err() {
                return;
            }

            // Simple HTTP response
            let response = "HTTP/1.1 200 OK\r\n\
                           Content-Type: application/json\r\n\
                           Content-Length: 25\r\n\
                           Connection: close\r\n\r\n\
                           {\"status\":\"ok\",\"test\":1}";

            let _ = socket.write_all(response.as_bytes()).await;
        });
    }
}

/// TCP echo backend for Layer 4 testing
pub async fn run_tcp_echo_backend(port: u16) {
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
            let mut buffer = vec![0u8; 4096];

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if socket.write_all(&buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

/// HTTP client for security testing
pub struct SecurityHttpClient {
    client: Client<
        hyper_util::client::legacy::connect::HttpConnector,
        Full<Bytes>,
    >,
    base_url: String,
}

impl SecurityHttpClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder(TokioExecutor::new()).build_http();
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    /// Send GET request with custom headers
    pub async fn get_with_headers(
        &self,
        path: &str,
        headers: HashMap<String, String>,
    ) -> Result<(StatusCode, String, u64), String> {
        let url = format!("{}{}", self.base_url, path);
        let start = std::time::Instant::now();

        let mut builder = Request::builder()
            .method(Method::GET)
            .uri(&url);

        for (key, value) in headers {
            builder = builder.header(&key, &value);
        }

        let request = builder
            .body(Full::new(Bytes::new()))
            .map_err(|e| e.to_string())?;

        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();

        let elapsed = start.elapsed().as_millis() as u64;

        Ok((status, String::from_utf8_lossy(&body).to_string(), elapsed))
    }

    /// Send POST request with body
    pub async fn post_with_body(
        &self,
        path: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(StatusCode, String, u64), String> {
        let url = format!("{}{}", self.base_url, path);
        let start = std::time::Instant::now();

        let mut builder = Request::builder()
            .method(Method::POST)
            .uri(&url);

        for (key, value) in headers {
            builder = builder.header(&key, &value);
        }

        let request = builder
            .body(Full::new(Bytes::from(body)))
            .map_err(|e| e.to_string())?;

        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();

        let elapsed = start.elapsed().as_millis() as u64;

        Ok((status, String::from_utf8_lossy(&body).to_string(), elapsed))
    }

    /// Send raw HTTP request (for smuggling tests)
    pub async fn send_raw(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        // Parse base URL to get host:port
        let url = self.base_url.trim_start_matches("http://");
        let mut stream = tokio::net::TcpStream::connect(url)
            .await
            .map_err(|e| e.to_string())?;

        stream.write_all(data).await.map_err(|e| e.to_string())?;

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            match tokio::time::timeout(
                Duration::from_millis(500),
                stream.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
                Ok(Err(e)) => return Err(e.to_string()),
                Err(_) => break, // Timeout
            }
        }

        Ok(response)
    }
}

/// Test configuration builder
pub struct TestConfigBuilder {
    config: String,
}

impl TestConfigBuilder {
    /// Create new HTTP proxy config
    pub fn http_proxy() -> Self {
        Self {
            config: r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
workers = 2

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[security]
enabled = true

[logging]
level = "warn"
"#
            .to_string(),
        }
    }

    /// Create TCP proxy config
    pub fn tcp_proxy() -> Self {
        Self {
            config: r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
mode = "tcp"
workers = 2

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[logging]
level = "warn"
"#
            .to_string(),
        }
    }

    /// Enable rate limiting
    pub fn with_rate_limit(mut self, requests: u32, window_secs: u32) -> Self {
        self.config.push_str(&format!(
            r#"
[rate_limit]
enabled = true
requests = {}
window_secs = {}
"#,
            requests, window_secs
        ));
        self
    }

    /// Enable WAF
    pub fn with_waf(mut self) -> Self {
        self.config.push_str(
            r#"
[waf]
enabled = true
mode = "custom"
sql_injection_protection = true
xss_protection = true
path_traversal_protection = true
"#,
        );
        self
    }

    /// Enable security headers
    pub fn with_security_headers(mut self) -> Self {
        self.config.push_str(
            r#"
[security_headers]
enabled = true
x_content_type_options = "nosniff"
x_frame_options = "DENY"
x_xss_protection = "1; mode=block"
strict_transport_security = "max-age=31536000; includeSubDomains"
content_security_policy = "default-src 'self'"
"#,
        );
        self
    }

    /// Enable request size limit
    pub fn with_size_limit(mut self, max_bytes: usize) -> Self {
        self.config.push_str(&format!(
            r#"
[request_size_limit]
enabled = true
max_body_size = {}
"#,
            max_bytes
        ));
        self
    }

    /// Build the config string
    pub fn build(self) -> String {
        self.config
    }
}

/// Assertion helpers for security tests
pub mod assertions {
    use super::*;

    /// Assert that a response indicates the request was blocked
    pub fn assert_blocked(status: StatusCode, body: &str, expected_reason: &str) {
        assert!(
            status == StatusCode::FORBIDDEN
                || status == StatusCode::BAD_REQUEST
                || status == StatusCode::TOO_MANY_REQUESTS,
            "Expected blocked status (403/400/429), got {}. Body: {}",
            status,
            body
        );
    }

    /// Assert that a response indicates success (attack got through - test failure)
    pub fn assert_not_blocked(status: StatusCode) {
        assert!(
            status.is_success(),
            "Expected request to succeed, got {}",
            status
        );
    }

    /// Assert rate limiting is active
    pub fn assert_rate_limited(status: StatusCode) {
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Expected 429 Too Many Requests"
        );
    }

    /// Assert request was rejected due to size
    pub fn assert_payload_too_large(status: StatusCode) {
        assert_eq!(
            status,
            StatusCode::PAYLOAD_TOO_LARGE,
            "Expected 413 Payload Too Large"
        );
    }
}

/// CVSS score calculator (simplified)
pub fn calculate_cvss_score(
    attack_vector: &str,
    attack_complexity: &str,
    privileges_required: &str,
    user_interaction: &str,
    scope: &str,
    confidentiality_impact: &str,
    integrity_impact: &str,
    availability_impact: &str,
) -> f64 {
    // Simplified CVSS 3.1 calculation
    let av = match attack_vector {
        "N" => 0.85, // Network
        "A" => 0.62, // Adjacent
        "L" => 0.55, // Local
        "P" => 0.20, // Physical
        _ => 0.0,
    };

    let ac = match attack_complexity {
        "L" => 0.77, // Low
        "H" => 0.44, // High
        _ => 0.0,
    };

    let pr = match (privileges_required, scope) {
        ("N", _) => 0.85,        // None
        ("L", "U") => 0.62,      // Low, Unchanged
        ("L", "C") => 0.68,      // Low, Changed
        ("H", "U") => 0.27,      // High, Unchanged
        ("H", "C") => 0.50,      // High, Changed
        _ => 0.0,
    };

    let ui = match user_interaction {
        "N" => 0.85, // None
        "R" => 0.62, // Required
        _ => 0.0,
    };

    let c = match confidentiality_impact {
        "H" => 0.56, // High
        "L" => 0.22, // Low
        "N" => 0.0,  // None
        _ => 0.0,
    };

    let i = match integrity_impact {
        "H" => 0.56, // High
        "L" => 0.22, // Low
        "N" => 0.0,  // None
        _ => 0.0,
    };

    let a = match availability_impact {
        "H" => 0.56, // High
        "L" => 0.22, // Low
        "N" => 0.0,  // None
        _ => 0.0,
    };

    // Calculate exploitability and impact
    let exploitability = 8.22 * av * ac * pr * ui;
    let impact_base = 1.0 - ((1.0 - c) * (1.0 - i) * (1.0 - a));

    let impact: f64 = if scope == "U" {
        6.42 * impact_base
    } else {
        7.52 * (impact_base - 0.029) - 3.25 * (impact_base - 0.02_f64).powf(15.0)
    };

    if impact <= 0.0 {
        return 0.0;
    }

    let base_score: f64 = if scope == "U" {
        (impact + exploitability).min(10.0)
    } else {
        (1.08 * (impact + exploitability)).min(10.0)
    };

    // Round up to 1 decimal
    (base_score * 10.0).ceil() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvss_critical() {
        // Remote code execution
        let score = calculate_cvss_score("N", "L", "N", "N", "C", "H", "H", "H");
        assert!(score >= 9.0, "RCE should be critical: {}", score);
    }

    #[test]
    fn test_cvss_high() {
        // SQL injection with limited impact (requires privileges)
        let score = calculate_cvss_score("N", "L", "L", "N", "U", "H", "H", "N");
        assert!(score >= 7.0 && score < 9.0, "SQLi with privs should be high: {}", score);
    }

    #[test]
    fn test_cvss_medium() {
        // XSS
        let score = calculate_cvss_score("N", "L", "N", "R", "C", "L", "L", "N");
        assert!(
            score >= 4.0 && score < 7.0,
            "XSS should be medium: {}",
            score
        );
    }
}
