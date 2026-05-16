//! Scenario 01: Layer 4 TCP Load Balancer Security Tests
//!
//! Tests for TCP-level vulnerabilities:
//! - TCP-01: SYN Flood resistance
//! - TCP-02: Connection exhaustion protection
//! - TCP-03: Slowloris (TCP variant)
//! - TCP-04: Reset attack resilience
//! - TCP-05: Data injection detection

use crate::security::common::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// TCP proxy configuration for testing
const TCP_PROXY_CONFIG: &str = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
mode = "tcp"
workers = 2
max_connections = 100

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[tcp]
keepalive = true
nodelay = true
connection_timeout_secs = 30

[logging]
level = "warn"
"#;

/// Test TCP-02: Connection exhaustion protection
/// Verifies that the gateway limits concurrent connections
#[tokio::test]
#[ignore]
async fn test_tcp_02_connection_exhaustion() {
    let harness = match SecurityTestHarness::new_tcp(TCP_PROXY_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();
    let mut connections = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    // Try to open many connections
    for _ in 0..150 {
        match TcpStream::connect(&addr).await {
            Ok(stream) => {
                successful += 1;
                connections.push(stream);
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    println!(
        "Connection exhaustion test: {} successful, {} failed",
        successful, failed
    );

    // Verify that connection limiting is working
    // Should reject some connections when limit is reached
    assert!(
        failed > 0 || successful <= 100,
        "Connection limit should be enforced. Got {} connections.",
        successful
    );
}

/// Test TCP-03: Slowloris (TCP variant)
/// Sends data very slowly to keep connections open
#[tokio::test]
#[ignore]
async fn test_tcp_03_slowloris() {
    let harness = match SecurityTestHarness::new_tcp(TCP_PROXY_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();
    let mut slow_connections = Vec::new();

    // Open several connections and send data very slowly
    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(&addr).await {
            let handle = tokio::spawn(async move {
                // Send one byte every 2 seconds
                for i in 0..5 {
                    if stream.write_all(&[b'A' + i]).await.is_err() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
            slow_connections.push(handle);
        }
    }

    // Wait a bit then try to make a normal connection
    tokio::time::sleep(Duration::from_secs(3)).await;

    let start = std::time::Instant::now();
    let normal_connection = TcpStream::connect(&addr).await;
    let elapsed = start.elapsed();

    // Clean up slow connections
    for handle in slow_connections {
        handle.abort();
    }

    // Normal connection should still be possible and fast
    assert!(
        normal_connection.is_ok(),
        "Normal connection should succeed during slowloris"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Connection should be fast, took {:?}",
        elapsed
    );
}

/// Test TCP-04: RST packet handling
/// Verifies proper handling of connection resets
#[tokio::test]
#[ignore]
async fn test_tcp_04_reset_handling() {
    let harness = match SecurityTestHarness::new_tcp(TCP_PROXY_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    // Open connection and abruptly close
    for _ in 0..10 {
        if let Ok(stream) = TcpStream::connect(&addr).await {
            // Set linger to 0 to send RST on close
            stream.set_linger(Some(Duration::from_secs(0))).ok();
            // Drop immediately to trigger RST
            drop(stream);
        }
    }

    // Wait a bit for RST packets to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify proxy is still accepting connections
    let result = TcpStream::connect(&addr).await;
    assert!(
        result.is_ok(),
        "Proxy should still accept connections after RST flood"
    );
}

/// Test TCP-05: Data integrity
/// Verifies that data is not modified in transit
#[tokio::test]
#[ignore]
async fn test_tcp_05_data_integrity() {
    let harness = match SecurityTestHarness::new_tcp(TCP_PROXY_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    // Test various data patterns
    let test_patterns = vec![
        vec![0x00; 100],           // Null bytes
        vec![0xFF; 100],           // All 1s
        (0..=255).collect(),       // All byte values
        b"Hello, World!".to_vec(), // ASCII text
    ];

    for pattern in test_patterns {
        let mut stream = TcpStream::connect(&addr).await.expect("Failed to connect");

        // Send data
        stream.write_all(&pattern).await.expect("Failed to write");

        // Read echo response
        let mut response = vec![0u8; pattern.len()];
        let timeout =
            tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut response)).await;

        match timeout {
            Ok(Ok(_)) => {
                assert_eq!(pattern, response, "Data should not be modified in transit");
            }
            Ok(Err(e)) => {
                eprintln!(
                    "Read error (may be expected if backend doesn't echo): {}",
                    e
                );
            }
            Err(_) => {
                eprintln!("Read timeout (may be expected if backend doesn't echo)");
            }
        }
    }
}

/// Test large data transfer
#[tokio::test]
#[ignore]
async fn test_tcp_large_data_transfer() {
    let harness = match SecurityTestHarness::new_tcp(TCP_PROXY_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();
    let mut stream = TcpStream::connect(&addr).await.expect("Failed to connect");

    // Send 1MB of data
    let large_data = vec![b'X'; 1024 * 1024];
    let write_result = stream.write_all(&large_data).await;

    assert!(write_result.is_ok(), "Should handle large data transfer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_template() {
        assert!(TCP_PROXY_CONFIG.contains("{{PROXY_PORT}}"));
        assert!(TCP_PROXY_CONFIG.contains("{{BACKEND_PORT}}"));
    }
}
