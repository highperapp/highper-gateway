# Phase 2.2: HTTP/3 Support - Implementation Plan

**Date**: December 13, 2025
**Phase**: 2.2 - HTTP/3 (QUIC) Support
**Status**: Ready to Begin
**Estimated Effort**: 50-60 hours
**Dependencies**: Phase 2.1 (WebSocket) ✅ Complete

---

## Executive Summary

Phase 2.2 will implement HTTP/3 support using the QUIC protocol, enabling:
- Ultra-low latency connections
- 0-RTT reconnection
- Connection migration
- Improved multiplexing
- Better loss recovery

**Current State**: ~60% code exists, needs completion and integration
**Target**: Full HTTP/3 support with alt-svc negotiation and connection migration

---

## Existing Code Assessment

### What Exists (~60% Complete)

#### 1. HTTP/3 Server Structure (`http3.rs`, 150 lines)
✅ **Implemented**:
- Basic Http3Server struct
- Configuration loading
- Placeholder server methods

❌ **Missing**:
- Actual server startup logic
- Integration with main server
- Connection handling
- Request processing

#### 2. Quiche Integration (`http3_quiche.rs`, 840 lines)
✅ **Implemented**:
- Quiche wrapper structures
- Connection management scaffolding
- Stream handling framework
- Error types and conversions

❌ **Missing**:
- Address validation with tokens
- Connection migration
- 0-RTT support
- Alt-svc header generation
- Integration with routing

#### 3. Configuration Support
✅ **Implemented**:
- HTTP/3 config schema
- DSL configuration support
- QUIC parameters

❌ **Missing**:
- Connection migration settings
- Address validation settings
- 0-RTT configuration

---

## Implementation Tasks

### Task 1: Wire Http3Server into Main Server (5 hours) 🔴 CRITICAL

**File**: `src/proxy/server.rs`

**Current State**:
```rust
// HTTP/1.1 and HTTP/2 servers running
// HTTP/3 server NOT started
```

**Required Changes**:
1. Import Http3Server from http module
2. Add HTTP/3 server initialization in `Server::start()`
3. Share TLS configuration with HTTP/2
4. Bind to UDP port for QUIC
5. Spawn HTTP/3 listener task
6. Handle HTTP/3 connections alongside HTTP/1.1/2

**Implementation**:
```rust
// In src/proxy/server.rs

pub async fn start(config: Arc<Config>) -> Result<()> {
    // ... existing HTTP/1.1/HTTP/2 setup ...

    // Start HTTP/3 server if enabled
    if config.server.http3.enabled {
        let http3_server = crate::http::Http3Server::new(config.clone());
        let http3_addr = config.server.bind_http3
            .unwrap_or_else(|| config.server.bind.clone());

        info!("Starting HTTP/3 server on {}", http3_addr);
        tokio::spawn(async move {
            if let Err(e) = http3_server.serve(http3_addr).await {
                error!("HTTP/3 server error: {}", e);
            }
        });
    }

    // ... rest of server logic ...
}
```

**Testing**:
- Verify HTTP/3 server starts without errors
- Check UDP socket binding
- Validate port allocation

---

### Task 2: Alt-Svc Header Generation (3 hours) 🔴 CRITICAL

**File**: `src/http/alt_svc.rs` (new file)

**Purpose**: Advertise HTTP/3 availability to HTTP/1.1 and HTTP/2 clients

**Implementation**:
```rust
// src/http/alt_svc.rs

/// Generate Alt-Svc header for HTTP/3 advertisement
pub fn generate_alt_svc_header(
    http3_port: u16,
    max_age: u32,
) -> hyper::header::HeaderValue {
    let value = format!(r#"h3=":{}"#, http3_port);
    if max_age > 0 {
        format!("{};ma={}", value, max_age)
            .parse()
            .unwrap()
    } else {
        value.parse().unwrap()
    }
}

/// Middleware to inject Alt-Svc header
pub struct AltSvcMiddleware {
    http3_port: u16,
    max_age: u32,
}

impl AltSvcMiddleware {
    pub fn new(http3_port: u16, max_age: u32) -> Self {
        Self { http3_port, max_age }
    }

    pub fn inject_header(&self, response: &mut hyper::Response<Body>) {
        if !response.headers().contains_key("alt-svc") {
            response.headers_mut().insert(
                "alt-svc",
                generate_alt_svc_header(self.http3_port, self.max_age),
            );
        }
    }
}
```

**Integration Point**: Add to response middleware chain in Handler

**Testing**:
- Verify alt-svc header in HTTP/1.1 responses
- Verify alt-svc header in HTTP/2 responses
- Test with various ports and max-age values

---

### Task 3: Address Validation with Tokens (8 hours) 🔴 CRITICAL

**File**: `src/http/http3_quiche.rs` (enhance existing)

**Purpose**: Prevent DDoS amplification attacks, required by QUIC spec

**Implementation**:
```rust
// Add to http3_quiche.rs

use ring::hmac;
use std::net::SocketAddr;

/// Address validation token manager
pub struct AddressValidator {
    key: hmac::Key,
    token_lifetime: Duration,
}

impl AddressValidator {
    pub fn new(secret: &[u8], lifetime: Duration) -> Self {
        Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, secret),
            token_lifetime: lifetime,
        }
    }

    /// Generate address validation token
    pub fn generate_token(&self, addr: &SocketAddr) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut data = Vec::new();
        data.extend_from_slice(&now.to_be_bytes());
        data.extend_from_slice(&addr.ip().to_string().as_bytes());
        data.extend_from_slice(&addr.port().to_be_bytes());

        let tag = hmac::sign(&self.key, &data);
        let mut token = data;
        token.extend_from_slice(tag.as_ref());
        token
    }

    /// Validate address token
    pub fn validate_token(&self, addr: &SocketAddr, token: &[u8]) -> bool {
        if token.len() < 40 {
            return false;
        }

        let (data, tag) = token.split_at(token.len() - 32);

        // Verify HMAC
        if let Err(_) = hmac::verify(&self.key, data, tag) {
            return false;
        }

        // Check timestamp
        let timestamp = u64::from_be_bytes(data[0..8].try_into().unwrap());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - timestamp > self.token_lifetime.as_secs() {
            return false;
        }

        // Verify address matches
        let token_addr_str = String::from_utf8_lossy(&data[8..data.len()-2]);
        let token_port = u16::from_be_bytes(data[data.len()-2..].try_into().unwrap());

        token_addr_str == addr.ip().to_string() && token_port == addr.port()
    }
}

/// Integrate into connection acceptance
impl Http3Server {
    async fn handle_initial_packet(&self, addr: SocketAddr, packet: &[u8]) -> Result<()> {
        // Extract token from packet
        let token = extract_token_from_packet(packet)?;

        if token.is_empty() {
            // First contact - send RETRY with token
            let token = self.validator.generate_token(&addr);
            self.send_retry(addr, token).await?;
            return Ok(());
        }

        // Validate token
        if !self.validator.validate_token(&addr, &token) {
            warn!("Invalid token from {}", addr);
            return Err(Error::InvalidToken);
        }

        // Token valid - proceed with connection
        self.accept_connection(addr).await
    }
}
```

**Testing**:
- Test token generation and validation
- Test token expiration
- Test DDoS mitigation
- Test legitimate client flow

---

### Task 4: Connection Migration Support (10 hours) 🟡 HIGH PRIORITY

**File**: `src/http/http3_quiche.rs` (enhance existing)

**Purpose**: Allow connections to survive network changes (WiFi → 4G, IP changes)

**Implementation**:
```rust
// Add to http3_quiche.rs

use dashmap::DashMap;

/// Connection ID to connection mapping
pub struct ConnectionMigrator {
    connections: Arc<DashMap<ConnectionId, Arc<QuicConnection>>>,
    path_challenges: Arc<DashMap<PathChallengeToken, SocketAddr>>,
}

impl ConnectionMigrator {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            path_challenges: Arc::new(DashMap::new()),
        }
    }

    /// Handle connection migration
    pub async fn handle_migration(
        &self,
        conn_id: &ConnectionId,
        old_addr: SocketAddr,
        new_addr: SocketAddr,
    ) -> Result<()> {
        info!("Connection {} migrating from {} to {}",
              conn_id, old_addr, new_addr);

        // Verify connection exists
        let conn = self.connections.get(conn_id)
            .ok_or(Error::ConnectionNotFound)?;

        // Send PATH_CHALLENGE to new address
        let challenge_token = self.generate_path_challenge();
        self.path_challenges.insert(challenge_token, new_addr);

        conn.send_path_challenge(new_addr, challenge_token).await?;

        Ok(())
    }

    /// Handle PATH_RESPONSE
    pub async fn handle_path_response(
        &self,
        conn_id: &ConnectionId,
        addr: SocketAddr,
        token: PathChallengeToken,
    ) -> Result<()> {
        // Verify token matches
        let expected_addr = self.path_challenges.remove(&token)
            .ok_or(Error::InvalidPathChallenge)?
            .1;

        if expected_addr != addr {
            return Err(Error::AddressMismatch);
        }

        // Update connection's peer address
        if let Some(conn) = self.connections.get(conn_id) {
            conn.update_peer_addr(addr);
            info!("Connection {} migrated to {}", conn_id, addr);
        }

        Ok(())
    }

    fn generate_path_challenge(&self) -> PathChallengeToken {
        let mut token = [0u8; 8];
        ring::rand::SystemRandom::new().fill(&mut token).unwrap();
        token
    }
}

/// Add to QuicConnection
impl QuicConnection {
    pub fn update_peer_addr(&self, new_addr: SocketAddr) {
        *self.peer_addr.write().unwrap() = new_addr;
        self.connection.set_peer_addr(new_addr);
    }
}
```

**Testing**:
- Test IP address change mid-connection
- Test port change mid-connection
- Test PATH_CHALLENGE/RESPONSE exchange
- Test connection recovery after migration

---

### Task 5: Integration with Routing and Load Balancing (8 hours) 🟡 HIGH PRIORITY

**File**: `src/proxy/handler.rs` (enhance existing)

**Purpose**: Route HTTP/3 requests through same logic as HTTP/1.1/2

**Implementation**:
```rust
// In src/proxy/handler.rs

impl Handler {
    /// Handle HTTP/3 request
    pub async fn handle_http3_request(
        &self,
        req: http::Request<()>,
        stream: &mut quiche::h3::Stream,
    ) -> Result<http::Response<Vec<u8>>> {
        // Extract request info
        let method = req.method().clone();
        let uri = req.uri().clone();
        let headers = req.headers().clone();

        // Use same routing logic as HTTP/1.1/2
        let path = uri.path();
        let host = headers.get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        // Find route
        let upstream_name = self.find_route_async(&method, host, path).await
            .ok_or(Error::NoRoute)?;

        // Get upstream
        let upstream = self.upstreams.get(&upstream_name)
            .ok_or(Error::UpstreamNotFound)?;

        // Select backend (same load balancing)
        let client_ip = None; // Extract from QUIC connection if needed
        let backend = upstream.load_balancer.select(client_ip, None)
            .ok_or(Error::NoBackendAvailable)?;

        // Forward request to backend
        self.forward_http3_request(req, backend, stream).await
    }

    async fn forward_http3_request(
        &self,
        req: http::Request<()>,
        backend: Arc<BackendServer>,
        stream: &mut quiche::h3::Stream,
    ) -> Result<http::Response<Vec<u8>>> {
        // Create HTTP client for backend
        // Send request
        // Read response
        // Apply response middleware
        // Return response

        todo!("Implement HTTP/3 backend forwarding")
    }
}
```

**Testing**:
- Test routing with various path patterns
- Test load balancing across backends
- Test middleware application
- Test error handling

---

### Task 6: 0-RTT Support (6 hours) 🟢 MEDIUM PRIORITY

**File**: `src/http/http3_quiche.rs` (enhance existing)

**Purpose**: Enable resumption without round-trip for returning clients

**Implementation**:
```rust
// Add to http3_quiche.rs

use std::collections::HashMap;

/// Session ticket manager for 0-RTT
pub struct SessionTicketManager {
    tickets: Arc<DashMap<Vec<u8>, SessionTicket>>,
    max_ticket_age: Duration,
}

pub struct SessionTicket {
    params: quiche::ConnectionParams,
    created_at: Instant,
}

impl SessionTicketManager {
    pub fn new(max_age: Duration) -> Self {
        Self {
            tickets: Arc::new(DashMap::new()),
            max_ticket_age: max_age,
        }
    }

    /// Store session ticket
    pub fn store_ticket(&self, id: Vec<u8>, params: quiche::ConnectionParams) {
        self.tickets.insert(id, SessionTicket {
            params,
            created_at: Instant::now(),
        });
    }

    /// Retrieve session ticket
    pub fn get_ticket(&self, id: &[u8]) -> Option<quiche::ConnectionParams> {
        let ticket = self.tickets.get(id)?;

        // Check if ticket expired
        if ticket.created_at.elapsed() > self.max_ticket_age {
            self.tickets.remove(id);
            return None;
        }

        Some(ticket.params.clone())
    }

    /// Cleanup expired tickets
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.tickets.retain(|_, ticket| {
            now.duration_since(ticket.created_at) <= self.max_ticket_age
        });
    }
}
```

**Testing**:
- Test session resumption with 0-RTT
- Test early data acceptance
- Test ticket expiration
- Test security (replay attacks)

---

### Task 7: Comprehensive Testing (20-25 hours) 🔴 CRITICAL

#### 7.1 Unit Tests (10 hours)
- Address validation token generation/verification
- Connection migration logic
- Session ticket management
- Alt-svc header generation
- Error handling

#### 7.2 Integration Tests (10 hours)
- HTTP/3 server startup
- Connection establishment
- Request/response flow
- Protocol upgrade from HTTP/1.1
- Load balancing with HTTP/3

#### 7.3 Load Tests (5 hours)
- 1M concurrent HTTP/3 connections
- Mixed HTTP/1.1, HTTP/2, HTTP/3 traffic
- Connection migration under load
- 0-RTT performance

---

## Task Breakdown Summary

| Task | Priority | Hours | Status |
|------|----------|-------|--------|
| 1. Wire Http3Server | 🔴 Critical | 5 | Pending |
| 2. Alt-Svc Headers | 🔴 Critical | 3 | Pending |
| 3. Address Validation | 🔴 Critical | 8 | Pending |
| 4. Connection Migration | 🟡 High | 10 | Pending |
| 5. Routing Integration | 🟡 High | 8 | Pending |
| 6. 0-RTT Support | 🟢 Medium | 6 | Pending |
| 7. Testing | 🔴 Critical | 20-25 | Pending |
| **Total** | | **60-65** | |

---

## Dependencies

### External Crates

Already in `Cargo.toml`:
- ✅ `quiche` - QUIC/HTTP/3 implementation
- ✅ `ring` - Cryptography for tokens
- ✅ `rustls` - TLS for QUIC

No new dependencies needed!

---

## Configuration Schema

### Minimal HTTP/3 Config

```yaml
server:
  bind: ["0.0.0.0:443"]
  http3:
    enabled: true
    bind: "0.0.0.0:443"  # Same port, UDP
    max_idle_timeout: 30000  # 30 seconds
    max_udp_payload_size: 1350

  # Alt-svc settings
  http3_advertise:
    enabled: true
    max_age: 86400  # 24 hours

  # Address validation
  address_validation:
    enabled: true
    token_lifetime: 3600  # 1 hour
    secret: "<random-secret>"  # Auto-generated if not provided

  # Connection migration
  migration:
    enabled: true
    max_path_challenges: 10

tls:
  # Same TLS config used for HTTP/3
  certificates:
    - cert_file: /path/to/cert.pem
      key_file: /path/to/key.pem
```

---

## Testing Strategy

### Scenario 05: HTTP/3 + QUIC (From Plan)

**Setup**:
- 3 HTTP/3 backends
- Gateway with HTTP/3 enabled

**Tests**:
1. HTTP/3 upgrade from HTTP/1.1 via alt-svc
2. 0-RTT connections working
3. Connection migration (IP change)
4. 1M concurrent connections (mix of HTTP/1.1, HTTP/2, HTTP/3)
5. 400K RPS total throughput

**Success Criteria**:
- ✅ P99 < 5ms
- ✅ Connection migration working
- ✅ 0-RTT reduces latency by >50%
- ✅ No connection drops during migration

---

## Timeline

### Week 1 (20-25 hours)
- ✅ Day 1-2: Wire Http3Server (5h)
- ✅ Day 2-3: Alt-Svc headers (3h)
- ✅ Day 3-5: Address validation (8h)
- ✅ Day 5: Basic testing (4-6h)

### Week 2 (25-30 hours)
- ✅ Day 1-2: Connection migration (10h)
- ✅ Day 3: Routing integration (8h)
- ✅ Day 4: 0-RTT support (6h)
- ✅ Day 5: Integration testing (6h)

### Week 3 (10-15 hours)
- ✅ Day 1-3: Load testing (5h)
- ✅ Day 3-4: Bug fixes (5h)
- ✅ Day 5: Documentation (2-3h)

**Total**: 55-70 hours (target: 60 hours)

---

## Success Criteria

### Implementation Complete When:
- ✅ HTTP/3 server starts without errors
- ✅ Alt-svc headers advertise HTTP/3
- ✅ Address validation prevents DDoS
- ✅ Connection migration works
- ✅ Routing/load balancing integrated
- ✅ 0-RTT reduces latency
- ✅ Unit tests passing (>90%)
- ✅ Integration tests passing (100%)
- ✅ Load tests meet targets

---

## Next Steps

1. **Begin Task 1**: Wire Http3Server into main server startup
2. **Create HTTP/3 branch**: For isolated development
3. **Set up HTTP/3 test environment**: Local test servers
4. **Start documentation**: As implementation progresses

---

## Related Documentation

- [Phase 2.1 Final Summary](PHASE_2.1_FINAL_SUMMARY_2025-12-13.md)
- [Implementation Plan](~/.claude/plans/serialized-painting-narwhal.md)
- [QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html)
- [HTTP/3 RFC 9114](https://www.rfc-editor.org/rfc/rfc9114.html)

---

**Generated**: December 13, 2025
**Status**: Ready to Begin
**Phase 2.1**: ✅ Complete (prerequisite satisfied)
**Estimated Duration**: 60 hours over 3 weeks
