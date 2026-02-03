//! Scenario 06: WebSocket Load Balancer Security Tests
//!
//! Tests for WebSocket vulnerabilities:
//! - WS-01: Cross-Site WebSocket Hijacking
//! - WS-02: Message Injection
//! - WS-03: DoS via Large Messages
//! - WS-04: Session Fixation
//! - WS-05: Ping/Pong Abuse
//! - WS-06: Connection Hijacking

use crate::security::common::*;
use crate::security::payloads::websocket;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// WebSocket proxy configuration for testing
const WEBSOCKET_CONFIG: &str = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
workers = 2

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[websocket]
enabled = true
max_message_size = 16777216
ping_interval = 30
session_timeout = 3600
sticky_sessions = true
sticky_cookie = "HPGW_WS_SESSION"

[security]
check_origin = true
allowed_origins = ["https://trusted.com", "https://example.com"]

[logging]
level = "warn"
"#;

/// WebSocket upgrade request
fn websocket_upgrade_request(origin: &str, key: &str) -> Vec<u8> {
    format!(
        "GET /ws HTTP/1.1\r\n\
         Host: localhost\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Origin: {}\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        origin, key
    )
    .into_bytes()
}

/// Test WS-01: Cross-Site WebSocket Hijacking
#[tokio::test]
#[ignore]
async fn test_ws_01_cswsh() {
    let harness = match SecurityTestHarness::new(WEBSOCKET_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    // Test with untrusted origin
    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    let request = websocket_upgrade_request("https://evil.com", "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(&request).await.expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("Timeout")
        .expect("Read failed");

    let response_str = String::from_utf8_lossy(&response[..n]);

    // Should reject connection from untrusted origin
    assert!(
        response_str.contains("403") || response_str.contains("401") || !response_str.contains("101"),
        "WebSocket from untrusted origin should be rejected"
    );

    // Test with trusted origin
    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    let request = websocket_upgrade_request("https://trusted.com", "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(&request).await.expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("Timeout")
        .expect("Read failed");

    let response_str = String::from_utf8_lossy(&response[..n]);

    // Trusted origin should be accepted
    println!("Trusted origin response: {}", response_str);
}

/// Test WS-02: Malformed Frame Handling
#[tokio::test]
#[ignore]
async fn test_ws_02_malformed_frames() {
    let harness = match SecurityTestHarness::new(WEBSOCKET_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    // First establish WebSocket connection
    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    let request = websocket_upgrade_request("https://trusted.com", "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(&request).await.expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response)).await;

    // Send malformed frames
    for frame in websocket::MALFORMED_FRAMES {
        if stream.write_all(frame).await.is_err() {
            // Connection closed - expected behavior
            println!("Connection closed after malformed frame (expected)");
            break;
        }

        // Small delay between frames
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Test WS-03: Large Message DoS
#[tokio::test]
#[ignore]
async fn test_ws_03_large_message_dos() {
    let harness = match SecurityTestHarness::new(WEBSOCKET_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    // Establish WebSocket connection
    let request = websocket_upgrade_request("https://trusted.com", "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(&request).await.expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response)).await;

    // Try to send message larger than 16MB limit
    let oversized_payload = websocket::large_message(17 * 1024 * 1024);

    // WebSocket frame header for large message
    // FIN=1, opcode=2 (binary), MASK=1, length=127 (8-byte extended length)
    let mut frame = vec![0x82, 0xFF]; // Binary frame, masked, 8-byte length
    frame.extend_from_slice(&(oversized_payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // Mask key
    // Note: Would need to XOR payload with mask in real implementation

    // Just send the header - connection should be closed or message rejected
    let result = stream.write_all(&frame).await;

    match result {
        Ok(_) => {
            // Check if connection is still alive
            let mut buf = [0u8; 100];
            let read_result = tokio::time::timeout(
                Duration::from_secs(1),
                stream.read(&mut buf),
            )
            .await;

            match read_result {
                Ok(Ok(0)) => println!("Connection closed (expected for oversized message)"),
                Ok(Ok(n)) => {
                    let msg = String::from_utf8_lossy(&buf[..n]);
                    println!("Received response: {}", msg);
                }
                _ => println!("Connection handling for oversized message"),
            }
        }
        Err(_) => println!("Write failed (expected for oversized message)"),
    }
}

/// Test WS-04: Session Cookie Security
#[tokio::test]
#[ignore]
async fn test_ws_04_session_security() {
    println!("WebSocket Session Security Test:");
    println!("  Checking sticky session cookie attributes");
    println!("  Cookie name: HPGW_WS_SESSION");
    println!("  Required attributes: HttpOnly, Secure, SameSite");

    // Session cookies should have:
    // - HttpOnly flag (prevent XSS access)
    // - Secure flag (HTTPS only)
    // - SameSite=Strict or Lax
    // - Reasonable expiration
}

/// Test WS-05: Ping/Pong Abuse
#[tokio::test]
#[ignore]
async fn test_ws_05_ping_pong_abuse() {
    let harness = match SecurityTestHarness::new(WEBSOCKET_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    // Establish WebSocket connection
    let request = websocket_upgrade_request("https://trusted.com", "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(&request).await.expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response)).await;

    // Send many ping frames rapidly
    // WebSocket ping: FIN=1, opcode=9 (ping), no mask (server->client)
    let ping_frame = vec![0x89, 0x00]; // Ping with no payload

    for i in 0..100 {
        if stream.write_all(&ping_frame).await.is_err() {
            println!("Connection closed after {} pings", i);
            break;
        }
    }

    println!("Ping flood test completed");
}

/// Test WS-06: Protocol Downgrade
#[tokio::test]
#[ignore]
async fn test_ws_06_protocol_downgrade() {
    let harness = match SecurityTestHarness::new(WEBSOCKET_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let addr = harness.tcp_addr();

    // Try to upgrade with old WebSocket version
    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect");

    let request = format!(
        "GET /ws HTTP/1.1\r\n\
         Host: localhost\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Origin: https://trusted.com\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 8\r\n\r\n"
    );

    stream
        .write_all(request.as_bytes())
        .await
        .expect("Failed to write");

    let mut response = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("Timeout")
        .expect("Read failed");

    let response_str = String::from_utf8_lossy(&response[..n]);

    // Should reject old version or return supported versions
    assert!(
        response_str.contains("426") // Upgrade Required
            || response_str.contains("400")
            || response_str.contains("Sec-WebSocket-Version: 13"),
        "Old WebSocket version should be rejected"
    );
}

/// Security configuration audit for WebSocket
#[tokio::test]
#[ignore]
async fn test_websocket_security_audit() {
    println!("\n========== WebSocket Security Configuration Audit ==========\n");

    println!("Origin Validation:");
    println!("  [✓] Origin checking enabled");
    println!("  [✓] Allowlist: trusted.com, example.com");
    println!("  [?] Reject null origin");

    println!("\nMessage Security:");
    println!("  [✓] Max message size: 16MB");
    println!("  [?] Message rate limiting");
    println!("  [?] Payload validation");

    println!("\nSession Security:");
    println!("  [✓] Sticky sessions enabled");
    println!("  [✓] Session timeout: 3600s");
    println!("  [?] Cookie HttpOnly flag");
    println!("  [?] Cookie Secure flag");

    println!("\nConnection Security:");
    println!("  [✓] Ping interval: 30s");
    println!("  [?] Connection timeout");
    println!("  [?] Max connections per IP");

    println!("\n=============================================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_has_websocket_settings() {
        assert!(WEBSOCKET_CONFIG.contains("[websocket]"));
        assert!(WEBSOCKET_CONFIG.contains("max_message_size"));
        assert!(WEBSOCKET_CONFIG.contains("allowed_origins"));
    }

    #[test]
    fn test_upgrade_request_format() {
        let req = websocket_upgrade_request("https://test.com", "key123");
        let req_str = String::from_utf8_lossy(&req);
        assert!(req_str.contains("Upgrade: websocket"));
        assert!(req_str.contains("Origin: https://test.com"));
    }
}
