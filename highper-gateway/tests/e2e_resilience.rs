//! End-to-End Resilience and Rate Limiting Tests
//!
//! These tests verify resilience features with actual HTTP requests:
//! 1. Rate limiting enforcement
//! 2. Circuit breaker behavior
//! 3. Health check and failover
//! 4. Connection pooling
//! 5. Timeout handling
//!
//! Run with: cargo test --test e2e_resilience -- --ignored --test-threads=1

use hyper::{Body, Client, StatusCode};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

/// Global port counter
static PORT_COUNTER: AtomicU32 = AtomicU32::new(21000);

fn get_next_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst) as u16
}

/// Test harness for resilience E2E tests
struct ResilienceTestHarness {
    proxy_process: Option<Child>,
    proxy_port: u16,
    backend_port: u16,
    config_path: String,
    backend_task: Option<tokio::task::JoinHandle<()>>,
}

impl ResilienceTestHarness {
    async fn new(config_template: &str) -> Result<Self, String> {
        let proxy_port = get_next_port();
        let backend_port = get_next_port();
        let config_path = format!("/tmp/e2e_resilience_test_{}.toml", proxy_port);

        let config = config_template
            .replace("{{PROXY_PORT}}", &proxy_port.to_string())
            .replace("{{BACKEND_PORT}}", &backend_port.to_string());

        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        let backend_task = Some(tokio::spawn(async move {
            run_simple_backend(backend_port).await;
        }));

        tokio::time::sleep(Duration::from_millis(200)).await;

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

        harness.wait_for_ready().await?;

        Ok(harness)
    }

    /// Create harness with custom backend
    async fn with_backend<F>(config_template: &str, backend_fn: F) -> Result<Self, String>
    where
        F: Fn(u16) -> tokio::task::JoinHandle<()> + Send + 'static,
    {
        let proxy_port = get_next_port();
        let backend_port = get_next_port();
        let config_path = format!("/tmp/e2e_resilience_test_{}.toml", proxy_port);

        let config = config_template
            .replace("{{PROXY_PORT}}", &proxy_port.to_string())
            .replace("{{BACKEND_PORT}}", &backend_port.to_string());

        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        let backend_task = Some(backend_fn(backend_port));

        tokio::time::sleep(Duration::from_millis(200)).await;

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

        harness.wait_for_ready().await?;

        Ok(harness)
    }

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

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.proxy_port, path)
    }
}

impl Drop for ResilienceTestHarness {
    fn drop(&mut self) {
        if let Some(mut process) = self.proxy_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }

        let _ = std::fs::remove_file(&self.config_path);

        if let Some(task) = self.backend_task.take() {
            task.abort();
        }
    }
}

/// Simple backend server
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

            if socket.read(&mut buffer).await.is_err() {
                return;
            }

            let response = "HTTP/1.1 200 OK\r\n\
                           Content-Type: application/json\r\n\
                           Content-Length: 18\r\n\
                           \r\n\
                           {\"status\":\"ok\"}";

            let _ = socket.write_all(response.as_bytes()).await;
        });
    }
}

/// Backend that fails intermittently
fn run_failing_backend(port: u16, should_fail: Arc<AtomicBool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
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

            let should_fail = Arc::clone(&should_fail);

            tokio::spawn(async move {
                let mut buffer = vec![0u8; 8192];

                if socket.read(&mut buffer).await.is_err() {
                    return;
                }

                if should_fail.load(Ordering::Relaxed) {
                    // Send 500 error
                    let response = "HTTP/1.1 500 Internal Server Error\r\n\
                                   Content-Length: 0\r\n\
                                   \r\n";
                    let _ = socket.write_all(response.as_bytes()).await;
                } else {
                    // Send 200 OK
                    let response = "HTTP/1.1 200 OK\r\n\
                                   Content-Type: application/json\r\n\
                                   Content-Length: 18\r\n\
                                   \r\n\
                                   {\"status\":\"ok\"}";
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            });
        }
    })
}

/// Slow backend for timeout testing
fn run_slow_backend(port: u16, delay_ms: u64) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
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

                if socket.read(&mut buffer).await.is_err() {
                    return;
                }

                // Delay before responding
                sleep(Duration::from_millis(delay_ms)).await;

                let response = "HTTP/1.1 200 OK\r\n\
                               Content-Type: application/json\r\n\
                               Content-Length: 18\r\n\
                               \r\n\
                               {\"status\":\"ok\"}";

                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    })
}

/// Test rate limiting enforcement
#[tokio::test]
#[ignore]
async fn test_e2e_rate_limiting() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.rate_limit]
enabled = true
requests_per_second = 10
burst = 5

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = ResilienceTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Send requests rapidly
    let mut success_count = 0;
    let mut rate_limited_count = 0;

    for _ in 0..20 {
        let response = client
            .get(harness.url("/test").parse().unwrap())
            .await
            .expect("Request failed");

        if response.status() == StatusCode::OK {
            success_count += 1;
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
            rate_limited_count += 1;
        }
    }

    // With limit of 10 RPS + burst of 5, we should see some rate limiting
    assert!(
        rate_limited_count > 0,
        "Should have rate limited at least some requests (limited: {})",
        rate_limited_count
    );

    assert!(
        success_count > 0,
        "Should have allowed some requests (success: {})",
        success_count
    );

    println!("✅ Rate limiting test passed: {} success, {} rate-limited",
             success_count, rate_limited_count);
}

/// Test circuit breaker opens after failures
#[tokio::test]
#[ignore]
async fn test_e2e_circuit_breaker() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[middleware.circuit_breaker]
enabled = true
failure_threshold = 3
timeout_seconds = 5
half_open_requests = 1

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"
health_check = false

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let should_fail = Arc::new(AtomicBool::new(true));

    let harness = ResilienceTestHarness::with_backend(config, {
        let should_fail = Arc::clone(&should_fail);
        move |port| run_failing_backend(port, should_fail)
    }).await.expect("Failed to start test harness");

    let client = Client::new();

    // Phase 1: Send failing requests to trip circuit breaker
    for i in 0..5 {
        let response = client
            .get(harness.url("/test").parse().unwrap())
            .await
            .expect("Request failed");

        println!("Request {}: status = {}", i + 1, response.status());
        sleep(Duration::from_millis(100)).await;
    }

    // Phase 2: Circuit should be open - requests should fail immediately
    let start = Instant::now();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");
    let duration = start.elapsed();

    // Circuit breaker should reject quickly (not wait for backend timeout)
    assert!(
        duration < Duration::from_secs(1),
        "Circuit breaker should reject quickly"
    );

    // Status should be 503 Service Unavailable
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Circuit breaker should return 503"
    );

    println!("✅ Circuit breaker test passed");
}

/// Test health check and failover
#[tokio::test]
#[ignore]
async fn test_e2e_health_check_failover() {
    let backend1_port = get_next_port();
    let backend2_port = get_next_port();

    let config = format!(r#"
[server]
host = "127.0.0.1"
port = {{{{PROXY_PORT}}}}

[[upstreams]]
name = "test-backend"
servers = [
    {{ url = "http://127.0.0.1:{}" }},
    {{ url = "http://127.0.0.1:{}" }}
]
load_balancing = "round_robin"
health_check = true

[upstreams.health_check]
interval_seconds = 2
timeout_seconds = 1
unhealthy_threshold = 2
healthy_threshold = 1

[[routes]]
path = "/"
upstream = "test-backend"
"#, backend1_port, backend2_port);

    // Start only second backend (first is down)
    let backend2_task = tokio::spawn(async move {
        run_simple_backend(backend2_port).await;
    });

    sleep(Duration::from_millis(200)).await;

    let proxy_port = get_next_port();
    let config_path = format!("/tmp/e2e_health_test_{}.toml", proxy_port);
    let final_config = config.replace("{{PROXY_PORT}}", &proxy_port.to_string());

    std::fs::write(&config_path, final_config)
        .expect("Failed to write config");

    let proxy_process = Command::new("cargo")
        .args([
            "run", "--release", "--",
            "--config", &config_path,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start proxy");

    // Wait for proxy and health checks
    sleep(Duration::from_secs(3)).await;

    let client = Client::new();
    let url = format!("http://127.0.0.1:{}/test", proxy_port);

    // Requests should succeed via healthy backend
    let response = client
        .get(url.parse().unwrap())
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Requests should succeed via healthy backend"
    );

    // Cleanup
    backend2_task.abort();
    std::fs::remove_file(&config_path).ok();

    println!("✅ Health check failover test passed");
}

/// Test timeout handling
#[tokio::test]
#[ignore]
async fn test_e2e_timeout_handling() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"
timeout_seconds = 2  # 2 second timeout

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    // Backend that takes 5 seconds to respond (exceeds 2 second timeout)
    let harness = ResilienceTestHarness::with_backend(config, |port| {
        run_slow_backend(port, 5000)
    }).await.expect("Failed to start test harness");

    let client = Client::new();

    let start = Instant::now();
    let response = client
        .get(harness.url("/test").parse().unwrap())
        .await
        .expect("Request failed");
    let duration = start.elapsed();

    // Should timeout around 2 seconds (not wait full 5 seconds)
    assert!(
        duration < Duration::from_millis(3000),
        "Should timeout within ~2 seconds (got {:?})",
        duration
    );

    // Should get gateway timeout
    assert_eq!(
        response.status(),
        StatusCode::GATEWAY_TIMEOUT,
        "Should return 504 Gateway Timeout"
    );

    println!("✅ Timeout handling test passed");
}

/// Test connection pooling efficiency
#[tokio::test]
#[ignore]
async fn test_e2e_connection_pooling() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

[connection_pool]
max_idle_per_host = 10
idle_timeout_seconds = 30

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = ResilienceTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Make multiple sequential requests
    let start = Instant::now();
    for _ in 0..10 {
        let response = client
            .get(harness.url("/test").parse().unwrap())
            .await
            .expect("Request failed");

        assert_eq!(response.status(), StatusCode::OK);
    }
    let duration = start.elapsed();

    // With connection pooling, subsequent requests should be faster
    // 10 requests should complete quickly (< 1 second)
    assert!(
        duration < Duration::from_secs(1),
        "Connection pooling should make requests fast (got {:?})",
        duration
    );

    println!("✅ Connection pooling test passed: 10 requests in {:?}", duration);
}

/// Test per-route rate limiting
#[tokio::test]
#[ignore]
async fn test_e2e_per_route_rate_limiting() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}

# Global rate limit (permissive)
[middleware.rate_limit]
enabled = true
requests_per_second = 100
burst = 50

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

# API route with permissive global limit
[[routes]]
path = "/api"
upstream = "test-backend"

# Admin route with strict limit (override)
[[routes]]
path = "/admin"
upstream = "test-backend"

[routes.middleware.rate_limit]
enabled = true
requests_per_second = 2
burst = 1
"#;

    let harness = ResilienceTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();

    // Test 1: API route should allow many requests
    let mut api_success = 0;
    for _ in 0..10 {
        let response = client
            .get(harness.url("/api/test").parse().unwrap())
            .await
            .expect("Request failed");

        if response.status() == StatusCode::OK {
            api_success += 1;
        }
    }

    assert!(
        api_success >= 8,
        "API route should allow most requests (got {})",
        api_success
    );

    // Test 2: Admin route should rate limit quickly
    let mut admin_success = 0;
    let mut admin_limited = 0;
    for _ in 0..10 {
        let response = client
            .get(harness.url("/admin/test").parse().unwrap())
            .await
            .expect("Request failed");

        if response.status() == StatusCode::OK {
            admin_success += 1;
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
            admin_limited += 1;
        }
    }

    assert!(
        admin_limited > 0,
        "Admin route should rate limit requests (limited: {})",
        admin_limited
    );

    println!("✅ Per-route rate limiting test passed: API {} success, Admin {} success / {} limited",
             api_success, admin_success, admin_limited);
}

/// Test concurrent requests handling
#[tokio::test]
#[ignore]
async fn test_e2e_concurrent_requests() {
    let config = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
worker_threads = 4

[[upstreams]]
name = "test-backend"
servers = [{ url = "http://127.0.0.1:{{BACKEND_PORT}}" }]
load_balancing = "round_robin"

[[routes]]
path = "/"
upstream = "test-backend"
"#;

    let harness = ResilienceTestHarness::new(config).await
        .expect("Failed to start test harness");

    let client = Client::new();
    let url = harness.url("/test");

    // Launch 50 concurrent requests
    let mut tasks = vec![];
    let start = Instant::now();

    for _ in 0..50 {
        let client = client.clone();
        let url = url.clone();

        let task = tokio::spawn(async move {
            client
                .get(url.parse().unwrap())
                .await
                .map(|r| r.status())
        });

        tasks.push(task);
    }

    // Wait for all requests
    let mut success_count = 0;
    for task in tasks {
        if let Ok(Ok(status)) = task.await {
            if status == StatusCode::OK {
                success_count += 1;
            }
        }
    }

    let duration = start.elapsed();

    assert_eq!(success_count, 50, "All requests should succeed");
    assert!(
        duration < Duration::from_secs(5),
        "50 concurrent requests should complete quickly"
    );

    println!("✅ Concurrent requests test passed: 50 requests in {:?}", duration);
}
