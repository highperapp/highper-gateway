# Architectural Changes Plan - Pending Features

**Date:** 2025-11-16
**Purpose:** Comprehensive list of features requiring architectural refactoring
**Priority:** Organized by impact and complexity

---

## 🎯 Executive Summary

This document outlines **27 pending features** requiring architectural changes, organized into 5 categories:

1. **Request/Response Pipeline** (7 features) - Core data flow changes
2. **State Management & Integration** (8 features) - Component wiring and state
3. **Advanced Protocols** (4 features) - Protocol-level enhancements
4. **Observability & Admin** (5 features) - Monitoring and control plane
5. **Performance Optimizations** (3 features) - Low-level improvements

**Estimated Total Effort:** 12-16 weeks (for all features)

---

## 📊 Priority Matrix

| Priority | Features | Estimated Effort | Impact |
|----------|----------|-----------------|--------|
| **P0 - Critical** | 5 | 4-5 weeks | High |
| **P1 - High** | 8 | 5-6 weeks | Medium-High |
| **P2 - Medium** | 9 | 2-3 weeks | Medium |
| **P3 - Low** | 5 | 1-2 weeks | Low-Medium |

---

## 1️⃣ Request/Response Pipeline (P0-P1)

### 1.1 POST Body Streaming for PHP-FPM (P0)

**Current State:**
```rust
// src/proxy/handler.rs:860-864
async fn serve_php_file(&self, req: &Request<Incoming>, ...) -> Result<...> {
    // Note: This requires req to be mutable, but it's passed as &Request
    // For now, we'll use an empty body as we can't consume the request
    // Full implementation would require changing the signature
    let body_bytes = vec![];  // Empty body placeholder
}
```

**Problem:**
- Request passed by reference (`&Request<Incoming>`)
- Cannot consume body from borrowed request
- Body is `Incoming` type (requires ownership to read)
- POST/PUT data is currently ignored

**Architectural Change Required:**

**Option A - Extract body before handler** (Recommended):
```rust
// In server.rs - before calling handler
pub async fn handle_connection(req: Request<Incoming>) -> Result<...> {
    let (parts, body) = req.into_parts();

    // Collect body with size limits
    let body_bytes = collect_body_with_limit(body, MAX_SIZE).await?;

    // Reconstruct request with empty body
    let req = Request::from_parts(parts, Empty::new());

    // Pass both request and body to handler
    handler.handle_with_body(req, body_bytes).await
}
```

**Option B - Change handler signature** (More invasive):
```rust
// Change all handler methods to accept owned request
async fn serve_php_file(
    &self,
    req: Request<Incoming>,  // Now owned, not borrowed
    ...
) -> Result<...>
```

**Impact:**
- **Files affected:** 3-5 (server.rs, handler.rs, possibly middleware)
- **Breaking change:** Method signatures
- **Testing effort:** High (need to test all request paths)
- **Estimated effort:** 2-3 days

**Benefits:**
- Enable POST/PUT for PHP applications
- Support file uploads to PHP
- Complete WordPress/Laravel compatibility

**Priority:** P0 (Critical for PHP-FPM production use)

---

### 1.2 Zero-Copy Sendfile for Large Static Files (P1)

**Current State:**
```rust
// src/proxy/handler.rs:776-799
async fn serve_static_file(...) -> Result<Response<Full<Bytes>>> {
    // Read entire file into memory
    let mut contents = Vec::with_capacity(file_size as usize);
    file.read_to_end(&mut contents)?;

    // Return Full<Bytes> (entire body in memory)
    Ok(response.body(Full::new(Bytes::from(contents)))?)
}
```

**Problem:**
- Returns `Full<Bytes>` which requires entire file in memory
- Inefficient for large files (100MB+ videos, ISOs, etc.)
- No zero-copy path available
- High memory usage under load

**Architectural Change Required:**

**Option A - Streaming Response Body** (Recommended):
```rust
use http_body_util::StreamBody;
use tokio_util::io::ReaderStream;

async fn serve_static_file(...) -> Result<Response<impl Body>> {
    let file = tokio::fs::File::open(&path).await?;
    let stream = ReaderStream::new(file);
    let body = StreamBody::new(stream);

    Ok(Response::builder()
        .status(200)
        .header("content-length", file_size)
        .body(body)?)
}
```

**Option B - Direct sendfile syscall** (Maximum performance):
```rust
// Requires low-level socket access
async fn serve_static_file_zerocopy(
    socket: &TcpStream,
    file: &File,
    offset: u64,
    count: usize,
) -> Result<usize> {
    // Use existing ktls::sendfile implementation
    ktls::sendfile_ktls(socket, file, offset, count)
}
```

**Challenge:**
- Hyper abstracts away socket access
- Would need to bypass Hyper for zero-copy path
- Complex integration with TLS (kTLS helps here)

**Impact:**
- **Files affected:** 5-7 (handler.rs, server.rs, response types)
- **Breaking change:** Response type signatures throughout
- **Testing effort:** High
- **Estimated effort:** 4-5 days

**Benefits:**
- Support serving large files (videos, ISOs, downloads)
- Reduce memory usage by 90%+ for large files
- Enable CDN-like behavior
- Better performance under load

**Priority:** P1 (High for media/download use cases)

---

### 1.3 Streaming Request Body Validation (P1)

**Current State:**
- Request size limits check `Content-Length` header only
- Body is consumed all at once (or ignored)
- No chunk-by-chunk validation

**Problem:**
- Client can send more data than declared in Content-Length
- No protection against slow-loris style attacks
- Cannot enforce limits during streaming

**Architectural Change Required:**

```rust
use http_body_util::BodyExt;

pub struct StreamingValidator<B> {
    inner: B,
    max_size: u64,
    consumed: u64,
}

impl<B: Body> Body for StreamingValidator<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.as_mut().project();

        match ready!(this.inner.poll_frame(cx)) {
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref() {
                    *this.consumed += data.len() as u64;
                    if *this.consumed > *this.max_size {
                        return Poll::Ready(Some(Err(...)));
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            other => Poll::Ready(other),
        }
    }
}
```

**Impact:**
- **Files affected:** 3-4
- **Estimated effort:** 2-3 days

**Priority:** P1 (Security concern)

---

### 1.4 Middleware Request Body Access (P2)

**Current State:**
- Middleware can modify headers, but not body
- Body is consumed by handler, not accessible to middleware
- WAF can't inspect POST data

**Problem:**
- WAF needs to inspect POST bodies for SQL injection, XSS
- Cannot validate JSON schema in middleware
- Cannot decompress/transform request bodies

**Architectural Change Required:**

```rust
pub trait Middleware {
    async fn process_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Request<Incoming>, Response> {
        // Middleware can now consume and transform body
    }
}

// In middleware chain
async fn process_request(mut req: Request<Incoming>) -> Result<...> {
    for middleware in &self.middlewares {
        // Each middleware can read/transform the body
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect().await?;

        // Middleware inspects/transforms
        let new_body = middleware.transform_body(body_bytes).await?;

        // Reconstruct request
        req = Request::from_parts(parts, Full::new(new_body));
    }
}
```

**Impact:**
- **Files affected:** 3-4 (middleware/mod.rs, waf modules)
- **Estimated effort:** 2-3 days

**Priority:** P2 (WAF enhancement)

---

### 1.5 Response Streaming for Proxied Requests (P2)

**Current State:**
```rust
// Proxy collects entire upstream response
let response_bytes = upstream_response.body().collect().await?;
Ok(Response::new(Full::new(response_bytes)))
```

**Problem:**
- Large responses consume memory
- Adds latency (must wait for complete response)
- Cannot stream large file downloads through proxy

**Architectural Change Required:**

```rust
// Stream directly from upstream to client
async fn proxy_request(...) -> Result<Response<impl Body>> {
    let upstream_response = client.request(upstream_req).await?;

    // Stream body directly (no buffering)
    Ok(Response::builder()
        .status(upstream_response.status())
        .body(upstream_response.into_body())?)
}
```

**Impact:**
- **Files affected:** 2-3
- **Estimated effort:** 2-3 days

**Priority:** P2 (Performance improvement)

---

### 1.6 Chunked Transfer Encoding Support (P2)

**Current State:**
- Relies on Hyper's default handling
- No explicit chunked encoding control
- Cannot optimize chunk sizes

**Architectural Change Required:**

```rust
use http_body_util::combinators::Chunk;

// Adaptive chunking based on network conditions
pub struct AdaptiveChunker<B> {
    inner: B,
    chunk_size: usize,
}

// Adjust chunk size based on RTT, bandwidth
```

**Impact:**
- **Files affected:** 2-3
- **Estimated effort:** 1-2 days

**Priority:** P2 (Optimization)

---

### 1.7 Multipart Form Data Handling (P3)

**Current State:**
- No multipart parsing
- File uploads to PHP not supported
- Form data ignored

**Architectural Change Required:**

```rust
use multer::Multipart;

async fn parse_multipart(
    body: Incoming,
    boundary: &str,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut multipart = Multipart::new(body, boundary);

    while let Some(field) = multipart.next_field().await? {
        // Parse field name, filename, content-type
        // Stream field data
    }
}
```

**Impact:**
- **Files affected:** 2-3
- **Estimated effort:** 2-3 days

**Priority:** P3 (Nice to have for PHP)

---

## 2️⃣ State Management & Integration (P0-P2)

### 2.1 Upstream State Tracking in HostnameRouter (P0)

**Current State:**
```rust
// src/gateway/routing/mod.rs:236
// Create minimal config (upstreams would need to be tracked separately in production)
let config = HostnameRoutesConfig {
    version: "1.0".to_string(),
    hosts: hosts_configs,
    upstreams: all_upstreams,  // Empty HashMap!
};
```

**Problem:**
- HostnameRouter doesn't track upstream configurations
- Routes reference upstream names as strings
- No connection to actual upstream servers
- Export loses upstream information

**Architectural Change Required:**

```rust
pub struct HostnameRouter {
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,

    // NEW: Track upstreams
    upstreams: Arc<DashMap<String, Arc<UpstreamConfig>>>,

    // NEW: Connection to load balancer
    load_balancer: Option<Arc<LoadBalancer>>,
}

impl HostnameRouter {
    pub fn add_upstream(&self, name: String, config: UpstreamConfig) {
        self.upstreams.insert(name, Arc::new(config));
    }

    pub fn get_upstream_for_route(&self, route: &Route) -> Option<Arc<UpstreamConfig>> {
        self.upstreams.get(&route.upstream).map(|u| u.clone())
    }

    pub async fn export_to_json(&self) -> Result<String> {
        // Export both routes AND upstreams
        let upstreams: HashMap<_, _> = self.upstreams
            .iter()
            .map(|e| (e.key().clone(), (*e.value()).clone()))
            .collect();

        let config = HostnameRoutesConfig {
            hosts: hosts_configs,
            upstreams,  // Now populated!
        };
    }
}
```

**Integration with Handler:**
```rust
impl Handler {
    async fn find_route_async(...) -> Option<(String, Arc<UpstreamConfig>)> {
        if let Some(router) = &self.hostname_router {
            if let Some(matched) = router.match_request_wildcard(...).await {
                // Get upstream config from router
                let upstream_config = router.get_upstream_for_route(&matched.route)?;
                return Some((matched.route.upstream.clone(), upstream_config));
            }
        }
        // Fallback to legacy config
    }
}
```

**Impact:**
- **Files affected:** 5-7 (routing/mod.rs, routing/loader.rs, routing/types.rs, handler.rs)
- **Breaking change:** HostnameRouter API
- **Estimated effort:** 3-4 days

**Benefits:**
- Complete route export (routes + upstreams)
- Self-contained router configuration
- Better separation of concerns
- Enable router-specific load balancing

**Priority:** P0 (Critical for production API Gateway)

---

### 2.2 Health Check Integration with Routes (P1)

**Current State:**
- Health checker exists but operates independently
- Routes don't know upstream health status
- No automatic failover based on health

**Problem:**
- Cannot mark routes as unhealthy
- No circuit breaker per route
- Manual health check triggering only

**Architectural Change Required:**

```rust
pub struct HostRouteIndex {
    routes: DashMap<String, Arc<Route>>,

    // NEW: Health checker per upstream
    health_checkers: Arc<DashMap<String, Arc<HealthChecker>>>,

    // NEW: Upstream health state
    upstream_health: Arc<DashMap<String, HealthStatus>>,
}

impl HostRouteIndex {
    pub async fn match_route_with_health(&self, path: &str) -> Option<Arc<Route>> {
        let route = self.match_route(path)?;

        // Check if upstream is healthy
        if let Some(health) = self.upstream_health.get(&route.upstream) {
            if matches!(*health, HealthStatus::Down) {
                warn!("Route {} has unhealthy upstream {}", route.name, route.upstream);
                return None;  // Try next route or return 503
            }
        }

        Some(route)
    }
}
```

**Impact:**
- **Files affected:** 6-8
- **Estimated effort:** 4-5 days

**Priority:** P1 (High availability)

---

### 2.3 Admin API Integration with HostnameRouter (P1)

**Current State:**
```rust
// src/admin/api.rs has many TODOs:
// TODO: Return actual configuration
// TODO: Return actual routes
// TODO: Parse body and create route
// TODO: Delete route
```

**Problem:**
- Admin API returns placeholder data
- Cannot dynamically add/remove hostname routes
- No runtime configuration changes

**Architectural Change Required:**

```rust
pub struct AdminServer {
    config: Arc<Config>,

    // NEW: Access to hostname router
    hostname_router: Option<Arc<HostnameRouter>>,

    // NEW: Access to handler for dynamic updates
    handler: Option<Arc<Handler>>,
}

impl AdminServer {
    // GET /api/routes/:hostname
    async fn get_routes(&self, hostname: &str) -> Result<Response> {
        if let Some(router) = &self.hostname_router {
            let routes = router.get_host_routes(hostname)?;
            let json = serde_json::to_string(&routes)?;
            return Ok(Response::new(Full::new(Bytes::from(json))));
        }
        // Fallback to config-based routes
    }

    // POST /api/routes/:hostname
    async fn add_route(&self, hostname: &str, route: RouteConfig) -> Result<Response> {
        if let Some(router) = &self.hostname_router {
            router.add_route(hostname, route).await?;
            // Trigger reload
            return Ok(Response::new(StatusCode::CREATED, ...));
        }
    }

    // DELETE /api/routes/:hostname/:route_name
    async fn delete_route(&self, hostname: &str, route_name: &str) -> Result<Response> {
        // Implementation
    }
}
```

**Impact:**
- **Files affected:** 8-10 (admin/*.rs, server.rs, handler.rs)
- **Estimated effort:** 5-6 days

**Priority:** P1 (Dynamic configuration)

---

### 2.4 Metrics Integration (P1)

**Current State:**
```rust
// src/admin/metrics.rs:
// TODO: Full integration with routing engine requires:
// - Request counting per route
// - Response time tracking
// - Error rate calculation
```

**Problem:**
- Metrics not tied to routes
- Cannot track per-route performance
- No per-upstream metrics

**Architectural Change Required:**

```rust
pub struct RouteMetrics {
    request_count: AtomicU64,
    error_count: AtomicU64,
    response_times: Arc<RwLock<Vec<Duration>>>,
}

pub struct HostnameRouter {
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,

    // NEW: Per-route metrics
    metrics: Arc<DashMap<String, Arc<RouteMetrics>>>,
}

impl Handler {
    async fn handle_request(&self, req: Request) -> Result<Response> {
        let start = Instant::now();

        let route_name = /* matched route name */;

        // Track request
        if let Some(metrics) = self.hostname_router.get_route_metrics(&route_name) {
            metrics.request_count.fetch_add(1, Ordering::Relaxed);
        }

        let result = self.proxy_request(req).await;

        // Track response time
        let duration = start.elapsed();
        if let Some(metrics) = self.hostname_router.get_route_metrics(&route_name) {
            metrics.record_response_time(duration);
            if result.is_err() {
                metrics.error_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        result
    }
}
```

**Impact:**
- **Files affected:** 6-8
- **Estimated effort:** 3-4 days

**Priority:** P1 (Observability)

---

### 2.5 Distributed Cache Integration (P2)

**Current State:**
```rust
// src/admin/cache.rs:
distributed: None, // TODO: Add distributed cache support
// TODO: Clear distributed cache if requested
// TODO: Invalidate from distributed cache if requested
```

**Problem:**
- Only in-memory caching
- No cache sharing across instances
- No persistent cache storage

**Architectural Change Required:**

```rust
pub struct CacheManager {
    local: Arc<DashMap<String, CacheEntry>>,

    // NEW: Redis connection pool
    redis: Option<Arc<RedisPool>>,
}

impl CacheManager {
    pub async fn get(&self, key: &str) -> Option<Bytes> {
        // Try local cache first
        if let Some(entry) = self.local.get(key) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }

        // Try distributed cache
        if let Some(redis) = &self.redis {
            if let Ok(value) = redis.get(key).await {
                // Populate local cache
                self.local.insert(key.to_string(), CacheEntry::new(value.clone()));
                return Some(value);
            }
        }

        None
    }
}
```

**Impact:**
- **Files affected:** 4-6
- **Dependencies:** redis crate
- **Estimated effort:** 4-5 days

**Priority:** P2 (Scalability)

---

### 2.6 Plugin System State Management (P3)

**Current State:**
```rust
// src/plugin/hot_reload.rs:
// TODO: Actual reload would need to be coordinated through PluginManager
```

**Problem:**
- Hot reload not fully implemented
- Plugin state not preserved across reloads
- No graceful plugin updates

**Impact:**
- **Files affected:** 5-7
- **Estimated effort:** 3-4 days

**Priority:** P3 (Plugin enhancement)

---

### 2.7 Configuration Validation at Runtime (P2)

**Current State:**
- Configuration loaded at startup
- No validation of runtime changes
- Admin API accepts invalid configs

**Architectural Change Required:**

```rust
pub struct ConfigValidator {
    schema: JsonSchema,
}

impl ConfigValidator {
    pub fn validate_route_config(&self, config: &RouteConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate upstream exists
        if !self.upstream_exists(&config.upstream) {
            errors.push(format!("Upstream '{}' not found", config.upstream));
        }

        // Validate regex patterns
        if let PathMatch::Pattern { pattern } = &config.path_match {
            if regex::Regex::new(pattern).is_err() {
                errors.push(format!("Invalid regex pattern: {}", pattern));
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

**Impact:**
- **Files affected:** 4-6
- **Estimated effort:** 2-3 days

**Priority:** P2 (Reliability)

---

### 2.8 WebSocket State Persistence (P3)

**Current State:**
- WebSocket connections are stateless
- No connection tracking
- Cannot list active WebSocket connections

**Impact:**
- **Files affected:** 3-4
- **Estimated effort:** 2-3 days

**Priority:** P3 (Enhancement)

---

## 3️⃣ Advanced Protocols (P1-P2)

### 3.1 kTLS Session Key Extraction (P1)

**Current State:**
```rust
// src/proxy/server.rs:51-52
// Note: Full kTLS integration requires additional work
// to extract session keys and configure kernel TLS after handshake
```

**Problem:**
- kTLS detection works
- Cannot enable kTLS due to session key extraction
- Rustls doesn't expose TLS 1.3 keys by default

**Architectural Change Required:**

```rust
use rustls::crypto::cipher::Tls13AeadAlgorithm;

pub struct KeyExtractor {
    keys: Arc<Mutex<Option<TlsKeys>>>,
}

impl rustls::ClientConfig {
    pub fn with_key_extractor(self, extractor: KeyExtractor) -> Self {
        // Hook into key derivation
        // Rustls would need to call extractor.set_keys() after handshake
    }
}

// After TLS handshake completes
async fn enable_ktls_on_connection(
    stream: &TcpStream,
    keys: &TlsKeys,
) -> Result<()> {
    ktls::enable_tx_ktls(stream, keys)?;
    ktls::enable_rx_ktls(stream, keys)?;
    Ok(())
}
```

**Challenge:**
- Rustls doesn't provide key extraction API
- May require forking Rustls or using `unsafe`
- TLS 1.3 key schedule is complex

**Alternative:**
- Use BoringSSL instead of Rustls (has kTLS support)
- Requires replacing entire TLS stack

**Impact:**
- **Files affected:** 8-12 (major TLS refactor)
- **Estimated effort:** 2-3 weeks
- **Risk:** High (TLS is security-critical)

**Priority:** P1 (Major performance gain if achieved)

---

### 3.2 HTTP/3 Request Body Streaming (P2)

**Current State:**
```rust
// src/http/http3_quiche.rs has basic HTTP/3 support
// Body streaming not fully implemented
```

**Problem:**
- HTTP/3 body handling incomplete
- QUIC streams not fully utilized
- Cannot stream large uploads over HTTP/3

**Impact:**
- **Files affected:** 5-7
- **Estimated effort:** 5-6 days

**Priority:** P2 (HTTP/3 enhancement)

---

### 3.3 gRPC Streaming Proxying (P2)

**Current State:**
```rust
// src/proxy/handler.rs:415
// Full gRPC-aware proxying with streaming will be added in a future update
```

**Problem:**
- gRPC treated as regular HTTP/2
- No streaming support for gRPC calls
- Cannot handle bidirectional streaming

**Architectural Change Required:**

```rust
pub async fn proxy_grpc_streaming(
    client_stream: impl Stream<Item = GrpcMessage>,
    upstream: &str,
) -> impl Stream<Item = GrpcMessage> {
    // Establish bidirectional gRPC channel
    let mut upstream_stream = grpc_client.connect(upstream).await?;

    // Relay messages bidirectionally
    tokio::spawn(async move {
        while let Some(msg) = client_stream.next().await {
            upstream_stream.send(msg).await?;
        }
    });

    upstream_stream
}
```

**Impact:**
- **Files affected:** 4-6
- **Dependencies:** tonic or grpc crate
- **Estimated effort:** 4-5 days

**Priority:** P2 (Microservices use case)

---

### 3.4 Protocol Negotiation Enhancement (P3)

**Current State:**
- ALPN negotiation basic
- No protocol upgrade handling
- Limited protocol fallback

**Impact:**
- **Files affected:** 3-4
- **Estimated effort:** 2-3 days

**Priority:** P3 (Compatibility)

---

## 4️⃣ Observability & Admin (P1-P3)

### 4.1 Jaeger Tracing Integration (P1)

**Current State:**
```rust
// src/observability/tracing.rs:50
// TODO: Complete Jaeger implementation with correct opentelemetry-jaeger API
```

**Problem:**
- Tracing infrastructure exists
- Jaeger exporter not configured
- Cannot send traces to Jaeger

**Architectural Change Required:**

```rust
use opentelemetry_jaeger::JaegerPipeline;

pub fn init_jaeger(config: &TracingConfig) -> Result<()> {
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(&config.service_name)
        .with_endpoint(&config.jaeger_endpoint)
        .install_batch(opentelemetry::runtime::Tokio)?;

    tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(())
}
```

**Impact:**
- **Files affected:** 2-3
- **Dependencies:** opentelemetry-jaeger, opentelemetry-otlp
- **Estimated effort:** 1-2 days

**Priority:** P1 (Critical for distributed tracing)

---

### 4.2 Real-Time Metrics Dashboard (P2)

**Current State:**
- Admin API exists
- No real-time updates
- No WebSocket for metrics streaming

**Architectural Change Required:**

```rust
// Admin API endpoint for metrics streaming
pub async fn metrics_stream(ws: WebSocket) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let metrics = collect_current_metrics().await;
        let json = serde_json::to_string(&metrics)?;

        ws.send(Message::text(json)).await?;
    }
}
```

**Impact:**
- **Files affected:** 4-6
- **Estimated effort:** 3-4 days

**Priority:** P2 (Observability enhancement)

---

### 4.3 Configuration Hot Reload Signal Handling (P2)

**Current State:**
- Hot reload via file watching exists
- No SIGHUP handling
- No graceful reload

**Impact:**
- **Files affected:** 2-3
- **Estimated effort:** 1-2 days

**Priority:** P2 (Operational)

---

### 4.4 Admin API Authentication (P1)

**Current State:**
```rust
// src/admin/api.rs:
// TODO: Verify JWT token
```

**Problem:**
- Admin API has no authentication
- Anyone can modify configuration
- Security risk in production

**Impact:**
- **Files affected:** 4-5
- **Estimated effort:** 2-3 days

**Priority:** P1 (Security critical)

---

### 4.5 Request/Response Logging Middleware (P2)

**Current State:**
```rust
// src/middleware/logging.rs:200
// Note: In a real implementation, we'd need to pass request metadata
// through the middleware chain. For now, we'll just log the response.
```

**Problem:**
- Cannot correlate request and response
- No request ID propagation
- Limited structured logging

**Impact:**
- **Files affected:** 3-4
- **Estimated effort:** 2-3 days

**Priority:** P2 (Debugging)

---

## 5️⃣ Performance Optimizations (P1-P2)

### 5.1 io_uring Registered Buffers (P1)

**Current State:**
```rust
// src/runtime/io_uring_buffers.rs:173
// Note: This requires the IORING_REGISTER_BUFFERS operation
// The io-uring crate may not expose this yet
```

**Problem:**
- Buffer registration not implemented
- Missing zero-copy optimization
- io_uring benefits not fully realized

**Architectural Change Required:**

```rust
use io_uring::{opcode, types, IoUring};

pub struct RegisteredBufferPool {
    ring: IoUring,
    buffers: Vec<Vec<u8>>,
    buffer_ids: Vec<usize>,
}

impl RegisteredBufferPool {
    pub fn new(count: usize, size: usize) -> Result<Self> {
        let mut ring = IoUring::new(4096)?;

        // Allocate buffers
        let mut buffers = Vec::new();
        for _ in 0..count {
            buffers.push(vec![0u8; size]);
        }

        // Register with io_uring
        let iovecs: Vec<_> = buffers.iter()
            .map(|buf| libc::iovec {
                iov_base: buf.as_ptr() as *mut _,
                iov_len: buf.len(),
            })
            .collect();

        unsafe {
            ring.submitter().register_buffers(&iovecs)?;
        }

        Ok(Self { ring, buffers, buffer_ids: (0..count).collect() })
    }

    pub async fn read_into_registered(&mut self, fd: RawFd, buf_id: usize) -> Result<usize> {
        let read_op = opcode::ReadFixed::new(
            types::Fd(fd),
            std::ptr::null_mut(),
            0,
            buf_id as u16,
        );

        unsafe {
            self.ring.submission().push(&read_op.build())?;
        }
        self.ring.submit_and_wait(1)?;

        // Get completion
        let cqe = self.ring.completion().next().unwrap();
        Ok(cqe.result() as usize)
    }
}
```

**Impact:**
- **Files affected:** 4-6
- **Estimated effort:** 4-5 days

**Priority:** P1 (Major performance gain)

---

### 5.2 SIMD Path Matching Optimization (P2)

**Current State:**
- SIMD helpers exist (runtime/simd_helpers.rs)
- Not used in route matching
- Regex patterns are slow

**Architectural Change Required:**

```rust
use std::arch::x86_64::*;

pub fn simd_path_match(path: &[u8], patterns: &[&[u8]]) -> Option<usize> {
    unsafe {
        for (idx, pattern) in patterns.iter().enumerate() {
            if simd_starts_with(path, pattern) {
                return Some(idx);
            }
        }
    }
    None
}

unsafe fn simd_starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }

    let h_ptr = haystack.as_ptr();
    let n_ptr = needle.as_ptr();

    // Process 16 bytes at a time
    let chunks = needle.len() / 16;
    for i in 0..chunks {
        let h_chunk = _mm_loadu_si128(h_ptr.add(i * 16) as *const __m128i);
        let n_chunk = _mm_loadu_si128(n_ptr.add(i * 16) as *const __m128i);
        let cmp = _mm_cmpeq_epi8(h_chunk, n_chunk);
        if _mm_movemask_epi8(cmp) != 0xFFFF {
            return false;
        }
    }

    // Handle remaining bytes
    true
}
```

**Impact:**
- **Files affected:** 3-4
- **Estimated effort:** 3-4 days

**Priority:** P2 (Optimization for high route counts)

---

### 5.3 Connection Pooling per Route (P2)

**Current State:**
- Global connection pool
- No per-upstream pools
- Cannot tune pool size per backend

**Impact:**
- **Files affected:** 4-5
- **Estimated effort:** 2-3 days

**Priority:** P2 (Resource optimization)

---

## 📋 Implementation Roadmap

### Phase 1: Critical Path (4-5 weeks) - P0 Items

**Week 1-2:**
1. POST Body Streaming for PHP-FPM
2. Upstream State Tracking in HostnameRouter

**Week 3-4:**
1. Admin API Integration with HostnameRouter
2. Admin API Authentication

**Week 5:**
1. Health Check Integration
2. Testing and validation

### Phase 2: High Priority (5-6 weeks) - P1 Items

**Week 6-7:**
1. Zero-Copy Sendfile
2. Streaming Request Body Validation

**Week 8-9:**
1. Metrics Integration
2. kTLS Session Key Extraction (spike/research)

**Week 10-11:**
1. Jaeger Tracing
2. io_uring Registered Buffers

### Phase 3: Medium Priority (2-3 weeks) - P2 Items

**Week 12-13:**
1. Response Streaming
2. gRPC Streaming
3. Distributed Cache

**Week 14:**
1. SIMD Optimizations
2. Configuration Validation

### Phase 4: Low Priority (1-2 weeks) - P3 Items

**Week 15-16:**
1. Multipart Form Data
2. WebSocket State Persistence
3. Cleanup and optimization

---

## 🎯 Quick Wins (Can be done in 1-2 days each)

1. **Jaeger Tracing** - Already have infrastructure, just wire up exporter
2. **Configuration Validation** - Add JSON schema validation
3. **Hot Reload Signal** - Add SIGHUP handler
4. **Request Logging** - Add correlation IDs

---

## 🚨 High-Risk Changes

### Risk Level: HIGH
1. **kTLS Session Key Extraction** - Requires Rustls changes or fork
2. **Zero-Copy Sendfile** - Major response type refactoring
3. **POST Body Streaming** - Core request handling flow

### Risk Level: MEDIUM
1. **io_uring Registered Buffers** - Low-level unsafe code
2. **Admin API Integration** - State management complexity
3. **Distributed Cache** - New dependency and failure modes

### Risk Level: LOW
1. **Metrics Integration** - Additive change
2. **Configuration Validation** - Isolated feature
3. **Jaeger Tracing** - Standard integration

---

## 📈 Expected Impact

### Performance Improvements
- **kTLS:** 20-30% CPU reduction for TLS workloads
- **io_uring Buffers:** 10-15% throughput increase
- **Zero-Copy Sendfile:** 90% memory reduction for large files
- **SIMD Path Matching:** 2-3x faster route matching

### Feature Completeness
- **POST Body Streaming:** Full PHP-FPM compatibility (WordPress, Laravel)
- **Upstream Tracking:** Complete API Gateway implementation
- **Admin API:** Dynamic runtime configuration
- **Health Checks:** High availability and auto-failover

### Operational Excellence
- **Jaeger Tracing:** Full distributed tracing capability
- **Metrics:** Per-route observability
- **Authentication:** Secure admin operations
- **Validation:** Prevent configuration errors

---

## 💡 Recommendations

### Start With (Maximum Impact, Lowest Risk):
1. **POST Body Streaming** (P0) - Critical for PHP use cases
2. **Upstream State Tracking** (P0) - Complete API Gateway
3. **Jaeger Tracing** (P1) - Quick win for observability
4. **Admin API Authentication** (P1) - Security requirement

### Defer Until Later:
1. **kTLS Session Keys** - Complex, requires deep research
2. **HTTP/3 Streaming** - Lower priority protocol
3. **Plugin Hot Reload** - Nice to have, not critical

### Consider Alternatives:
1. **kTLS** - Evaluate BoringSSL as alternative to Rustls
2. **Zero-Copy Sendfile** - Start with streaming body, add sendfile later
3. **Distributed Cache** - Evaluate need vs. complexity

---

## ✅ Next Steps

1. **Review and prioritize** this plan with stakeholders
2. **Create detailed design docs** for Phase 1 items
3. **Set up feature branches** for parallel development
4. **Establish testing strategy** for architectural changes
5. **Plan for backward compatibility** and migration path

---

**Document Version:** 1.0
**Last Updated:** 2025-11-16
**Status:** Ready for Review

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
