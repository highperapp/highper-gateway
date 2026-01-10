# Phase 2.1: HTTP/3 (QUIC) Support - Implementation Specification

**Duration:** 4 weeks
**Priority:** High (Major Protocol)
**Difficulty:** High
**Impact:** +10% core protocols score

---

## Executive Summary

Implement HTTP/3 over QUIC protocol to provide next-generation HTTP transport. HTTP/3 is the latest HTTP version using QUIC (UDP-based) instead of TCP, offering improved performance, reduced latency, and better handling of packet loss.

---

## What is HTTP/3 / QUIC?

### Protocol Evolution

```
HTTP/1.1 ──→ HTTP/2 ──→ HTTP/3
   │            │           │
  TCP          TCP        QUIC (UDP)
   │            │           │
Single stream  Multiplexed  Multiplexed + 0-RTT
Head-of-line   Head-of-line No head-of-line
blocking       blocking     blocking
```

### Key Benefits

1. **0-RTT Connection Establishment** - Resume connections instantly
2. **No Head-of-Line Blocking** - Independent streams at transport layer
3. **Connection Migration** - Survive IP changes (mobile networks)
4. **Improved Congestion Control** - Built into QUIC
5. **Mandatory Encryption** - TLS 1.3 integrated into QUIC

### Use Cases
- Mobile applications (connection migration)
- High-latency networks (0-RTT)
- Video streaming (no HOL blocking)
- Real-time applications
- Modern web applications

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                 HTTP/3 + QUIC System                     │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │         QUIC Listener (UDP Socket)               │  │
│  │         Port: 443 (standard HTTP/3)              │  │
│  └──────────────────────────────────────────────────┘  │
│              │                                           │
│              ↓                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │         QUIC Connection Handler                  │  │
│  │  - TLS 1.3 handshake                            │  │
│  │  - 0-RTT support                                │  │
│  │  - Connection migration                         │  │
│  └──────────────────────────────────────────────────┘  │
│              │                                           │
│              ↓                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │         HTTP/3 Frame Handler                     │  │
│  │  - HEADERS frame                                │  │
│  │  - DATA frame                                   │  │
│  │  - SETTINGS frame                               │  │
│  └──────────────────────────────────────────────────┘  │
│              │                                           │
│              ↓                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Request Router                           │  │
│  │  (shared with HTTP/1.1 and HTTP/2)              │  │
│  └──────────────────────────────────────────────────┘  │
│              │                                           │
│              ↓                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Backend Proxy                            │  │
│  └──────────────────────────────────────────────────┘  │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

### Protocol Discovery (Alt-Svc)

```
Client ──HTTP/1.1──→ Server
                      │
Client ←─Alt-Svc─────┘
       "h3=\":443\""
       │
Client ──HTTP/3──→ Server (UDP 443)
```

---

## Implementation

### 1. Configuration Schema

**File:** `highper-gateway/src/config/schema.rs` (enhance)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,

    /// HTTP/3 configuration
    #[serde(default)]
    pub http3: Http3Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http3Config {
    /// Enable HTTP/3
    #[serde(default)]
    pub enabled: bool,

    /// HTTP/3 port (usually same as HTTPS port 443)
    #[serde(default = "default_http3_port")]
    pub port: u16,

    /// Enable 0-RTT (early data)
    #[serde(default)]
    pub enable_0rtt: bool,

    /// Maximum number of concurrent streams per connection
    #[serde(default = "default_max_streams")]
    pub max_concurrent_streams: u64,

    /// Initial flow control window size (bytes)
    #[serde(default = "default_initial_window")]
    pub initial_window_size: u64,

    /// Maximum datagram size (bytes)
    #[serde(default = "default_max_datagram_size")]
    pub max_datagram_size: u64,

    /// Connection idle timeout (seconds)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

fn default_http3_port() -> u16 {
    443
}

fn default_max_streams() -> u64 {
    100
}

fn default_initial_window() -> u64 {
    10_485_760 // 10 MB
}

fn default_max_datagram_size() -> u64 {
    1350
}

fn default_idle_timeout() -> u64 {
    30
}
```

**Configuration Example:**

```yaml
server:
  host: "0.0.0.0"
  port: 8080

  # HTTP/3 configuration
  http3:
    enabled: true
    port: 443
    enable_0rtt: true
    max_concurrent_streams: 100
    initial_window_size: 10485760  # 10 MB
    max_datagram_size: 1350
    idle_timeout: 30

tls:
  enabled: true
  port: 443  # Same port for HTTP/2 (TCP) and HTTP/3 (UDP)
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"
```

---

### 2. QUIC Server Implementation

**File:** `highper-gateway/src/http/http3.rs` (new)

```rust
use anyhow::Result;
use bytes::Bytes;
use h3::server::Connection;
use h3_quinn::quinn;
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

use crate::config::schema::{Config, Http3Config};
use crate::proxy::handler::ProxyHandler;

/// HTTP/3 server using quinn (QUIC implementation)
pub struct Http3Server {
    config: Arc<Config>,
    handler: Arc<ProxyHandler>,
}

impl Http3Server {
    pub fn new(config: Arc<Config>, handler: Arc<ProxyHandler>) -> Self {
        Self { config, handler }
    }

    /// Start HTTP/3 server
    pub async fn run(self) -> Result<()> {
        let http3_config = &self.config.server.http3;

        if !http3_config.enabled {
            return Ok(());
        }

        let addr = format!("{}:{}", self.config.server.host, http3_config.port);
        let addr: SocketAddr = addr.parse()?;

        info!("Starting HTTP/3 server on {}", addr);

        // Create QUIC configuration
        let server_config = self.build_quic_server_config(http3_config)?;

        // Bind UDP socket
        let socket = UdpSocket::bind(addr).await?;
        let endpoint = quinn::Endpoint::new_with_abstract_socket(
            quinn::EndpointConfig::default(),
            Some(server_config),
            socket,
            Arc::new(quinn::TokioRuntime),
        )?;

        info!("HTTP/3 server listening on {}", addr);

        // Accept connections
        while let Some(connecting) = endpoint.accept().await {
            let handler = Arc::clone(&self.handler);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(connecting, handler).await {
                    error!("HTTP/3 connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Build QUIC server configuration
    fn build_quic_server_config(&self, http3_config: &Http3Config) -> Result<quinn::ServerConfig> {
        // Load TLS certificates
        let tls_config = self.config.tls.as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS config required for HTTP/3"))?;

        let (certs, key) = crate::tls::acceptor::load_certificates(
            &tls_config.cert_path,
            &tls_config.key_path,
        )?;

        // Build rustls config for QUIC
        let mut rustls_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        // Set ALPN protocols
        rustls_config.alpn_protocols = vec![
            b"h3".to_vec(),      // HTTP/3
            b"h3-29".to_vec(),   // HTTP/3 draft 29
        ];

        // Enable 0-RTT if configured
        if http3_config.enable_0rtt {
            rustls_config.max_early_data_size = 16384; // 16 KB
        }

        // Build QUIC config
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(rustls_config));

        // Configure transport parameters
        let mut transport_config = quinn::TransportConfig::default();

        transport_config.max_concurrent_bidi_streams(
            http3_config.max_concurrent_streams.try_into().unwrap_or(100)
        );

        transport_config.initial_max_stream_data_bidi_local(
            http3_config.initial_window_size
        );

        transport_config.initial_max_stream_data_bidi_remote(
            http3_config.initial_window_size
        );

        transport_config.max_idle_timeout(
            Some(std::time::Duration::from_secs(http3_config.idle_timeout).try_into()?)
        );

        server_config.transport_config(Arc::new(transport_config));

        Ok(server_config)
    }

    /// Handle QUIC connection
    async fn handle_connection(
        connecting: quinn::Connecting,
        handler: Arc<ProxyHandler>,
    ) -> Result<()> {
        let connection = connecting.await?;

        debug!(
            "New HTTP/3 connection from {}",
            connection.remote_address()
        );

        // Create HTTP/3 connection
        let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(connection))
            .await?;

        loop {
            match h3_conn.accept().await {
                Ok(Some((req, stream))) => {
                    let handler = Arc::clone(&handler);

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_request(req, stream, handler).await {
                            error!("HTTP/3 request error: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    // Connection closed
                    break;
                }
                Err(e) => {
                    error!("HTTP/3 accept error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle HTTP/3 request
    async fn handle_request(
        req: Request<()>,
        mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
        handler: Arc<ProxyHandler>,
    ) -> Result<()> {
        debug!("HTTP/3 request: {} {}", req.method(), req.uri());

        // Read request body if present
        let mut body_bytes = Vec::new();
        while let Some(data) = stream.recv_data().await? {
            body_bytes.extend_from_slice(&data);
        }

        // Convert to hyper request format
        let (parts, _) = req.into_parts();
        let body = Full::new(Bytes::from(body_bytes));
        let hyper_req = Request::from_parts(parts, body);

        // Process request through proxy handler
        let response = handler.handle_request(hyper_req).await?;

        // Convert response
        let (parts, body) = response.into_parts();
        let response = Response::from_parts(parts, ());

        // Send response
        stream.send_response(response).await?;

        // Send body
        let body_bytes = body.into_inner();
        stream.send_data(body_bytes).await?;

        stream.finish().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http3_config_defaults() {
        let config = Http3Config::default();
        assert_eq!(config.port, 443);
        assert_eq!(config.max_concurrent_streams, 100);
    }

    #[tokio::test]
    async fn test_http3_server_creation() {
        // Test server can be created with valid config
    }
}
```

---

### 3. Alt-Svc Header Support

**File:** `highper-gateway/src/http/alt_svc.rs` (new)

```rust
use hyper::{header::HeaderValue, Response};
use http_body_util::Full;
use bytes::Bytes;

/// Add Alt-Svc header to advertise HTTP/3 support
pub fn add_alt_svc_header(
    response: &mut Response<Full<Bytes>>,
    http3_port: u16,
) {
    // Alt-Svc format: h3=":port"; ma=maxage
    let alt_svc_value = format!("h3=\":{}\"; ma=2592000", http3_port);

    if let Ok(header_value) = HeaderValue::from_str(&alt_svc_value) {
        response.headers_mut().insert("alt-svc", header_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alt_svc_header() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header(&mut response, 443);

        let alt_svc = response.headers().get("alt-svc").unwrap();
        assert_eq!(alt_svc.to_str().unwrap(), "h3=\":443\"; ma=2592000");
    }
}
```

**Integration in HTTP/2 handler:**

```rust
// In highper-gateway/src/proxy/handler.rs

impl ProxyHandler {
    pub async fn handle_request(&self, req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        // ... existing request handling ...

        let mut response = self.forward_to_backend(req).await?;

        // Advertise HTTP/3 support via Alt-Svc header
        if self.config.server.http3.enabled {
            crate::http::alt_svc::add_alt_svc_header(
                &mut response,
                self.config.server.http3.port,
            );
        }

        Ok(response)
    }
}
```

---

### 4. Runtime Integration

**File:** `highper-gateway/src/runtime/mod.rs` (enhance)

```rust
pub async fn run_all_servers(config: Arc<Config>) -> Result<()> {
    let mut handles = vec![];

    // Start HTTP server
    let http_handle = tokio::spawn({
        let config = Arc::clone(&config);
        async move {
            crate::proxy::server::run_http_server(config).await
        }
    });
    handles.push(http_handle);

    // Start HTTPS server
    if config.tls.is_some() {
        let https_handle = tokio::spawn({
            let config = Arc::clone(&config);
            async move {
                crate::proxy::server::run_tls_server(config).await
            }
        });
        handles.push(https_handle);
    }

    // Start HTTP/3 server
    if config.server.http3.enabled {
        let http3_handle = tokio::spawn({
            let config = Arc::clone(&config);
            let handler = Arc::new(ProxyHandler::new(&config));
            async move {
                let http3_server = Http3Server::new(config, handler);
                http3_server.run().await
            }
        });
        handles.push(http3_handle);
    }

    // Start TLS passthrough server
    if let Some(tls) = &config.tls {
        if tls.passthrough_port.is_some() {
            let passthrough_handle = tokio::spawn({
                let config = Arc::clone(&config);
                async move {
                    crate::proxy::server::run_tls_passthrough(config).await
                }
            });
            handles.push(passthrough_handle);
        }
    }

    // Wait for all servers
    for handle in handles {
        handle.await??;
    }

    Ok(())
}
```

---

## Dependencies

### New Dependencies Required

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# HTTP/3 and QUIC
h3 = "0.0.4"
h3-quinn = "0.0.5"
quinn = "0.11"

# Additional crypto for QUIC
rustls = { version = "0.23", features = ["quic"] }
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_http3_config_parsing()

#[test]
fn test_alt_svc_header_generation()

#[test]
fn test_quic_transport_config()
```

### Integration Tests

**File:** `highper-gateway/tests/http3_test.rs`

```rust
use h3::client::SendRequest;
use h3_quinn::quinn;

#[tokio::test]
async fn test_http3_request() {
    // Start proxy with HTTP/3 enabled
    let proxy = start_proxy_with_http3().await;

    // Create HTTP/3 client
    let client = create_http3_client().await;

    // Make request
    let response = client
        .get("https://localhost:443/api/test")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_http3_0rtt() {
    // Test 0-RTT connection resumption
    let proxy = start_proxy_with_http3().await;
    let client = create_http3_client().await;

    // First request (full handshake)
    let response1 = client.get("https://localhost:443/").send().await.unwrap();
    assert_eq!(response1.status(), 200);

    // Second request (should use 0-RTT)
    let response2 = client.get("https://localhost:443/").send().await.unwrap();
    assert_eq!(response2.status(), 200);

    // Verify 0-RTT was used (check connection stats)
}

#[tokio::test]
async fn test_connection_migration() {
    // Test QUIC connection migration (IP change)
    // This requires network simulation
}

#[tokio::test]
async fn test_alt_svc_discovery() {
    let proxy = start_proxy_with_http3().await;

    // Make HTTP/2 request
    let response = reqwest::get("https://localhost:443/")
        .await
        .unwrap();

    // Check for Alt-Svc header
    let alt_svc = response.headers().get("alt-svc").unwrap();
    assert!(alt_svc.to_str().unwrap().contains("h3"));
}
```

### Manual Testing

```bash
# Test with curl (HTTP/3 support)
curl --http3 https://localhost:443/api/test

# Test with Chrome
# chrome://flags -> enable-quic
# Visit: https://localhost:443

# Check protocol used in DevTools -> Network -> Protocol column

# Test with h3 client
cargo install h3-client
h3-client https://localhost:443/api/test

# Verify Alt-Svc header
curl -I https://localhost:443/ | grep -i alt-svc
```

---

## Performance Considerations

### Benchmarks

Expected performance improvements with HTTP/3:

| Metric | HTTP/2 | HTTP/3 | Improvement |
|--------|--------|--------|-------------|
| Connection time | 100ms | 0ms (0-RTT) | **100% faster** |
| Head-of-line blocking | Yes | No | **Better parallelism** |
| Packet loss impact | High | Low | **Better on lossy networks** |
| Mobile performance | Good | Excellent | **Connection migration** |

### Resource Usage

- **CPU:** +10-15% (QUIC crypto overhead)
- **Memory:** +2-5 MB per connection (QUIC state)
- **Network:** Similar to HTTP/2
- **UDP buffering:** May need OS tuning

### OS Tuning for UDP

```bash
# Increase UDP buffer sizes
sysctl -w net.core.rmem_max=2500000
sysctl -w net.core.wmem_max=2500000

# Increase number of UDP connections
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
```

---

## Browser Support

### Current Browser Support (2025)

| Browser | HTTP/3 Support | Notes |
|---------|---------------|-------|
| Chrome | ✅ Yes | Enabled by default |
| Firefox | ✅ Yes | Enabled by default |
| Safari | ✅ Yes | iOS 14+, macOS 11+ |
| Edge | ✅ Yes | Chromium-based |
| Opera | ✅ Yes | Chromium-based |

### Client Detection

Browsers automatically upgrade to HTTP/3 via:
1. Alt-Svc header (from HTTP/2 response)
2. DNS HTTPS records
3. Connection pool reuse

---

## Debugging

### Enable QUIC Logging

```rust
// In main.rs
std::env::set_var("RUST_LOG", "quinn=debug,h3=debug");
```

### Wireshark Capture

```bash
# Capture QUIC traffic
sudo tcpdump -i any -w quic.pcap 'udp port 443'

# Analyze in Wireshark with QUIC dissector
wireshark quic.pcap
```

### Common Issues

**1. UDP Port Blocked**
```
Error: Connection timeout
Solution: Ensure UDP port 443 is open in firewall
```

**2. Certificate Issues**
```
Error: TLS handshake failed
Solution: Verify TLS 1.3 support, check certificate validity
```

**3. 0-RTT Not Working**
```
Issue: Every connection does full handshake
Solution: Enable session tickets in rustls config
```

---

## Acceptance Criteria

- [ ] HTTP/3 server starts on UDP port
- [ ] QUIC connections accepted
- [ ] HTTP/3 requests processed correctly
- [ ] Alt-Svc header advertised
- [ ] 0-RTT works for resumed connections
- [ ] Integration with existing routing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] curl --http3 works
- [ ] Browser auto-upgrades to HTTP/3

---

## Migration Strategy

### Gradual Rollout

```yaml
# Step 1: Enable HTTP/3 alongside HTTP/2
server:
  http3:
    enabled: true
    port: 443  # Same port, different protocol (UDP vs TCP)

# Step 2: Monitor adoption
# Check metrics: http3_connections_total

# Step 3: Optimize based on usage
# Tune max_concurrent_streams, window sizes, etc.
```

### Fallback Behavior

HTTP/3 is optional:
- Clients try HTTP/3 if advertised
- Fall back to HTTP/2 if QUIC fails
- Fall back to HTTP/1.1 if HTTP/2 fails
- Proxy continues working if HTTP/3 disabled

---

## Security Considerations

- **Mandatory TLS 1.3** - QUIC requires TLS 1.3
- **Certificate validation** - Same as HTTPS
- **0-RTT replay attacks** - Mitigate with anti-replay tokens
- **Amplification attacks** - QUIC has built-in protection
- **Connection migration** - Validate connection ID

---

## Future Enhancements

- **WebTransport** - Bidirectional streaming over HTTP/3
- **MASQUE** - Proxying UDP over HTTP/3
- **HTTP/3 prioritization** - Request prioritization
- **QUIC multipath** - Use multiple network paths

---

## Documentation

Create `docs/HTTP3.md`:
- What is HTTP/3 and why use it
- Configuration guide
- Performance tuning
- Browser compatibility
- Troubleshooting

---

## Next Steps

After implementation:
1. Benchmark performance vs HTTP/2
2. Monitor adoption metrics
3. Tune QUIC parameters
4. Proceed to Phase 2.2: API Aggregation/Composition

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
