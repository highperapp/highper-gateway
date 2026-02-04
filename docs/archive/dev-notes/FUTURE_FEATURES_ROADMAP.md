# Future Features Roadmap - Weeks 12-18
## Advanced Features for Production-Grade Application Delivery

## Executive Summary

Three strategic features identified to transform the reverse proxy into a **complete Nginx replacement** and high-performance application delivery platform:

1. **API Gateway Enhancement**: Per-hostname JSON route definitions with ultra-fast in-memory parsing
2. **Kernel TLS (kTLS)**: OS-level TLS offload for 20-30% TLS performance improvement
3. **Web Server Features**: PHP-FPM support and static file serving for full Nginx compatibility

**Combined Impact**: Complete Nginx replacement with superior performance

---

## Feature 1: API Gateway - Per-Hostname Route Definitions

### Overview

**Goal**: Enable JSON-based API route definitions on a per-hostname basis with ultra-fast loading and parsing

**Use Case**: API Gateway managing thousands of routes across multiple domains
```
api.example.com -> 5,000 API routes
api.staging.example.com -> 3,000 API routes
partner-api.example.com -> 2,000 API routes
```

### Current State Analysis

**Existing Capabilities** ✅:
- Route matching by path, host, method, headers
- Dynamic route management via Admin API
- In-memory route storage
- Hot reload support

**Gaps to Address** ⚠️:
- Routes not organized by hostname (flat structure)
- JSON format not optimized for API gateway use case
- No specialized in-memory index for fast lookup
- Route loading could be faster for large route sets

### Proposed Implementation

#### 1. Per-Hostname Route Storage Structure

```rust
/// Per-hostname route collection
pub struct HostnameRoutes {
    /// Routes indexed by hostname
    routes: DashMap<String, HostRouteIndex>,

    /// Global fallback routes (no hostname specified)
    global_routes: Arc<RwLock<Vec<Route>>>,
}

/// Fast route index for a single hostname
pub struct HostRouteIndex {
    /// Exact path matches (O(1) lookup)
    exact_paths: DashMap<String, Arc<Route>>,

    /// Prefix matches sorted by length (longest first)
    prefix_paths: Arc<RwLock<Vec<(String, Arc<Route>)>>>,

    /// Regex patterns (evaluated last)
    regex_patterns: Arc<RwLock<Vec<(regex::Regex, Arc<Route>)>>>,

    /// Method-based index for faster filtering
    method_index: DashMap<Method, Vec<Arc<Route>>>,
}
```

**Lookup Algorithm**:
```rust
async fn find_route(&self, host: &str, path: &str, method: Method) -> Option<Arc<Route>> {
    // 1. Get hostname-specific index (O(1))
    let host_index = self.routes.get(host)?;

    // 2. Try exact path match (O(1))
    if let Some(route) = host_index.exact_paths.get(path) {
        if route.matches_method(method) {
            return Some(route.clone());
        }
    }

    // 3. Try prefix matches (O(n) but n is small, sorted by length)
    let prefixes = host_index.prefix_paths.read().await;
    for (prefix, route) in prefixes.iter() {
        if path.starts_with(prefix) && route.matches_method(method) {
            return Some(route.clone());
        }
    }

    // 4. Try regex patterns (O(m) where m = regex count)
    let patterns = host_index.regex_patterns.read().await;
    for (pattern, route) in patterns.iter() {
        if pattern.is_match(path) && route.matches_method(method) {
            return Some(route.clone());
        }
    }

    // 5. Fallback to global routes
    self.find_global_route(path, method).await
}
```

**Complexity Analysis**:
- Exact match: **O(1)** ✅
- Prefix match: **O(n)** where n = prefix count (typically < 100)
- Regex match: **O(m)** where m = regex count (typically < 50)
- **Overall**: Sub-microsecond for most cases

#### 2. JSON Format for API Routes

```json
{
  "version": "1.0",
  "hostname": "api.example.com",
  "routes": [
    {
      "name": "get_user",
      "path": "/api/v1/users/:id",
      "method": "GET",
      "upstream": "user_service",
      "timeout_ms": 5000,
      "rate_limit": {
        "requests_per_second": 100,
        "burst": 200
      },
      "middleware": ["auth", "logging", "metrics"],
      "response_cache": {
        "enabled": true,
        "ttl_seconds": 60
      }
    },
    {
      "name": "create_user",
      "path": "/api/v1/users",
      "method": "POST",
      "upstream": "user_service",
      "timeout_ms": 10000,
      "middleware": ["auth", "validation", "logging"]
    }
  ],
  "upstreams": {
    "user_service": {
      "servers": [
        {"url": "http://user-svc-1:8080", "weight": 1},
        {"url": "http://user-svc-2:8080", "weight": 1}
      ],
      "load_balancing": "least_conn",
      "health_check": {
        "path": "/health",
        "interval_seconds": 5
      }
    }
  }
}
```

#### 3. Fast Loading with In-Memory Storage

**Option 1: Native Rust Collections (Recommended)**
```rust
use dashmap::DashMap;  // Already in use, lock-free concurrent hashmap
use parking_lot::RwLock;  // Faster than std::sync::RwLock

/// Ultra-fast in-memory route storage
pub struct RouteStorage {
    /// Primary storage: Hostname -> Routes
    hosts: DashMap<String, HostRouteIndex>,

    /// Secondary index: Route name -> Route (for lookups by name)
    by_name: DashMap<String, Arc<Route>>,

    /// Metadata
    version: AtomicU64,
    last_updated: AtomicU64,  // Unix timestamp
}

impl RouteStorage {
    /// Load routes from JSON (optimized)
    pub async fn load_from_json(&self, json_path: &Path) -> Result<()> {
        // 1. Parse JSON in parallel using serde_json
        let json_str = tokio::fs::read_to_string(json_path).await?;
        let config: HostnameRoutesConfig = serde_json::from_str(&json_str)?;

        // 2. Build indices in parallel
        let routes = config.routes.into_par_iter()  // Rayon parallel iterator
            .map(|route| self.build_route_index(route))
            .collect::<Vec<_>>();

        // 3. Insert into DashMap (concurrent)
        for route in routes {
            self.insert_route(route);
        }

        // 4. Update metadata
        self.version.fetch_add(1, Ordering::SeqCst);
        self.last_updated.store(current_timestamp(), Ordering::SeqCst);

        Ok(())
    }

    /// Hot reload (zero downtime)
    pub async fn hot_reload(&self, json_path: &Path) -> Result<()> {
        // 1. Load new routes into temporary storage
        let new_storage = RouteStorage::new();
        new_storage.load_from_json(json_path).await?;

        // 2. Atomic swap (using Arc)
        // Existing requests continue with old routes
        // New requests use new routes
        self.hosts.clear();
        for entry in new_storage.hosts.iter() {
            self.hosts.insert(entry.key().clone(), entry.value().clone());
        }

        Ok(())
    }
}
```

**Performance Characteristics**:
- JSON parsing: ~500 MB/s (serde_json)
- Index building: ~1M routes/sec (parallel)
- Hot reload: ~50-100ms for 10k routes
- Memory overhead: ~500 bytes per route

**Option 2: Redis Support (Optional)**
```rust
/// Redis-backed route storage (for distributed setups)
pub struct RedisRouteStorage {
    client: redis::Client,
    local_cache: Arc<RouteStorage>,  // Local cache for speed
    cache_ttl: Duration,
}

impl RedisRouteStorage {
    /// Load routes from Redis with local caching
    pub async fn load_routes(&self, hostname: &str) -> Result<Vec<Route>> {
        // 1. Check local cache first (L1 cache)
        if let Some(routes) = self.local_cache.get_host_routes(hostname) {
            return Ok(routes);
        }

        // 2. Fetch from Redis (L2 cache)
        let key = format!("routes:{}", hostname);
        let json: String = self.client.get(&key).await?;
        let routes: Vec<Route> = serde_json::from_str(&json)?;

        // 3. Update local cache
        self.local_cache.insert_host_routes(hostname, routes.clone());

        Ok(routes)
    }

    /// Subscribe to Redis pub/sub for route updates
    pub async fn subscribe_updates(&self) -> Result<()> {
        let mut pubsub = self.client.get_async_connection().await?.into_pubsub();
        pubsub.subscribe("route_updates").await?;

        while let Some(msg) = pubsub.on_message().next().await {
            let payload: String = msg.get_payload()?;
            self.handle_route_update(payload).await?;
        }

        Ok(())
    }
}
```

**Redis Benefits**:
- Distributed route management
- Multi-instance synchronization
- Pub/sub for instant updates
- Persistence and backup

**Redis Tradeoffs**:
- Additional dependency
- Network latency (~0.1-1ms)
- Complexity increase

**Recommendation**: Start with native Rust collections, add Redis support later if needed

#### 4. SIMD-Accelerated Route Parsing

**Use SIMD for Fast Path Matching**:
```rust
use crate::runtime::simd_helpers::{find_char, simd_memcmp};

/// Fast path parsing with SIMD
pub fn parse_path_fast(path: &[u8]) -> PathSegments {
    let mut segments = Vec::with_capacity(8);
    let mut start = 0;

    // Find '/' characters using SIMD (20x faster than scalar)
    for i in 0..path.len() {
        if let Some(pos) = simd_find_pattern(&path[start..], b'/') {
            let segment = &path[start..start + pos];
            segments.push(segment);
            start += pos + 1;
        } else {
            break;
        }
    }

    PathSegments { segments }
}

/// Fast route matching with SIMD
pub fn match_route_fast(path: &[u8], pattern: &[u8]) -> bool {
    // Use SIMD for exact match (25x faster than scalar)
    if path.len() == pattern.len() {
        return simd_memcmp(path, pattern);
    }

    // Use SIMD for prefix match
    if pattern.ends_with(b"/*") {
        let prefix_len = pattern.len() - 2;
        if path.len() >= prefix_len {
            return simd_memcmp(&path[..prefix_len], &pattern[..prefix_len]);
        }
    }

    // Fallback to regex for complex patterns
    false
}
```

**Expected Performance**: 10-20x faster path matching than pure regex

### Implementation Plan

**Week 12-13: Core Implementation** (40 hours)
1. Design per-hostname route storage structure
2. Implement `HostnameRoutes` and `HostRouteIndex`
3. Add JSON schema and parser
4. Build fast lookup algorithm
5. Integrate with existing routing

**Week 14: Optimization** (16 hours)
6. SIMD-accelerated path parsing
7. Parallel route loading
8. Benchmark and optimize

**Week 15: Hot Reload & Testing** (16 hours)
9. Implement hot reload mechanism
10. Add comprehensive tests
11. Load testing with 10k+ routes

**Week 16 (Optional): Redis Support** (16 hours)
12. Redis integration
13. Pub/sub for updates
14. Distributed testing

**Total Estimated Effort**: 72-88 hours (9-11 working days)

### Expected Performance

**Route Lookup**:
- Exact match: **< 100ns** (DashMap + O(1) lookup)
- Prefix match: **< 500ns** (sorted prefix list)
- Regex match: **< 5µs** (compiled regex)

**Route Loading**:
- 10,000 routes: **< 50ms** (parallel parsing)
- 100,000 routes: **< 500ms** (parallel parsing)

**Memory Usage**:
- ~500 bytes per route
- 10,000 routes: ~5 MB
- 100,000 routes: ~50 MB

**Hot Reload**:
- Zero downtime (atomic swap)
- < 100ms for 10k routes
- Existing connections unaffected

### Integration Points

**With Existing System**:
1. Route matching in `proxy/handler.rs`
2. Admin API for route management
3. Configuration hot reload
4. Metrics collection

**New Modules**:
- `src/gateway/hostname_routes.rs` - Per-hostname storage
- `src/gateway/route_index.rs` - Fast indexing
- `src/gateway/json_loader.rs` - JSON loading
- `src/gateway/redis_storage.rs` - Redis integration (optional)

---

## Feature 2: Kernel TLS (kTLS) Offload

### Overview

**Goal**: Offload TLS encryption/decryption to the Linux kernel for 20-30% performance improvement

**Technology**: Linux Kernel TLS (kTLS)
- Introduced in Linux 4.13+
- Supported by OpenSSL 3.0+
- Used by Nginx, Cloudflare, Facebook

### Performance Benefits

**Current (Userspace TLS)**:
```
Request → Userspace TLS (rustls) → Kernel → Network
        ↑ Context switches, memory copies
```

**With kTLS (Kernel TLS)**:
```
Request → Kernel TLS → Network
        ↑ Zero copy, no context switches
```

**Expected Improvements**:
- CPU usage: **-20-30%** (offload to kernel)
- Latency: **-10-20%** (no context switches)
- Throughput: **+20-30%** (zero-copy sendfile)
- Memory: **-15-25%** (kernel manages buffers)

### Technical Architecture

#### 1. kTLS Integration Layer

```rust
#[cfg(target_os = "linux")]
pub mod ktls {
    use std::os::unix::io::AsRawFd;
    use libc::{setsockopt, SOL_TLS, TLS_TX, TLS_RX};

    /// kTLS configuration
    pub struct KtlsConfig {
        pub enabled: bool,
        pub cipher_suites: Vec<CipherSuite>,
        pub min_tls_version: TlsVersion,
    }

    /// Enable kTLS on a socket
    pub fn enable_ktls(socket: &TcpStream, tls_session: &TlsSession) -> Result<()> {
        let fd = socket.as_raw_fd();

        // 1. Complete TLS handshake in userspace (OpenSSL/rustls)
        // 2. Extract cipher parameters
        let crypto_info = extract_crypto_info(tls_session)?;

        // 3. Enable kTLS for TX (transmit)
        unsafe {
            setsockopt(
                fd,
                SOL_TLS,
                TLS_TX,
                &crypto_info as *const _ as *const _,
                std::mem::size_of_val(&crypto_info) as u32,
            );
        }

        // 4. Enable kTLS for RX (receive)
        unsafe {
            setsockopt(
                fd,
                SOL_TLS,
                TLS_RX,
                &crypto_info as *const _ as *const _,
                std::mem::size_of_val(&crypto_info) as u32,
            );
        }

        Ok(())
    }

    /// Check if kTLS is supported
    pub fn is_ktls_supported() -> bool {
        // Check kernel version >= 4.13
        // Check TLS module loaded
        // Check cipher suite support
        check_kernel_version() && check_tls_module() && check_cipher_support()
    }
}
```

#### 2. Hybrid TLS Strategy

```rust
/// Hybrid TLS implementation
pub enum TlsBackend {
    /// Userspace TLS (rustls) - fallback
    Userspace(RustlsConnection),

    /// Kernel TLS (kTLS) - preferred when available
    Kernel(KtlsConnection),
}

impl TlsBackend {
    /// Create optimal TLS backend
    pub fn new(config: &TlsConfig) -> Self {
        if config.ktls_enabled && ktls::is_ktls_supported() {
            // Use kTLS if available
            Self::Kernel(KtlsConnection::new(config))
        } else {
            // Fallback to userspace
            Self::Userspace(RustlsConnection::new(config))
        }
    }

    /// Perform TLS handshake
    pub async fn handshake(&mut self, socket: TcpStream) -> Result<TlsStream> {
        match self {
            Self::Kernel(ktls) => {
                // 1. Do handshake in userspace
                let tls_session = ktls.handshake_userspace(&socket).await?;

                // 2. Offload to kernel
                ktls::enable_ktls(&socket, &tls_session)?;

                // 3. Return kTLS-enabled stream
                Ok(TlsStream::Kernel(socket))
            }
            Self::Userspace(rustls) => {
                // Normal userspace TLS
                rustls.handshake(socket).await
            }
        }
    }
}
```

#### 3. Zero-Copy Sendfile with kTLS

```rust
/// Zero-copy file transmission with kTLS
pub async fn sendfile_ktls(
    socket: &TlsStream,
    file: &File,
    offset: u64,
    count: usize,
) -> Result<usize> {
    match socket {
        TlsStream::Kernel(sock) => {
            // Use sendfile(2) syscall - zero copy!
            let sent = unsafe {
                libc::sendfile(
                    sock.as_raw_fd(),
                    file.as_raw_fd(),
                    &mut offset as *mut _ as *mut _,
                    count,
                )
            };

            if sent < 0 {
                Err(io::Error::last_os_error().into())
            } else {
                Ok(sent as usize)
            }
        }
        TlsStream::Userspace(_) => {
            // Fallback: read into buffer, then write (copy)
            send_file_userspace(socket, file, offset, count).await
        }
    }
}
```

**Benefit**: Static file serving becomes **zero-copy** with kTLS + sendfile
- No userspace buffer allocation
- No read() syscall
- No write() syscall
- **Direct kernel-to-kernel transfer**

### mTLS Support

**kTLS works with mTLS**:
```rust
pub async fn enable_mtls_ktls(
    socket: &TcpStream,
    server_cert: &Certificate,
    client_cert: &Certificate,
) -> Result<()> {
    // 1. Complete mTLS handshake in userspace
    //    - Verify client certificate
    //    - Verify server certificate
    let tls_session = complete_mtls_handshake(socket, server_cert, client_cert).await?;

    // 2. Offload to kernel (works same as regular TLS)
    ktls::enable_ktls(socket, &tls_session)?;

    Ok(())
}
```

**No difference from regular kTLS** - encryption offload works the same way

### OS Compatibility

**Supported Operating Systems**:
| OS | kTLS Support | Min Version | Notes |
|----|--------------|-------------|-------|
| Linux | ✅ Yes | 4.13+ | Full support |
| FreeBSD | ✅ Yes | 13.0+ | Full support |
| macOS | ❌ No | N/A | Use userspace |
| Windows | ❌ No | N/A | Use userspace |

**Configuration**:
```toml
[tls]
enabled = true

# Kernel TLS configuration
[tls.ktls]
enabled = true  # Enable on supported platforms
auto_detect = true  # Auto-detect kernel support
fallback_to_userspace = true  # Use rustls if kTLS unavailable

# Cipher suites (must be kTLS-compatible)
cipher_suites = [
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
]
```

### Implementation Plan

**Week 12-13: Research & Prototyping** (24 hours)
1. Study Linux kTLS APIs
2. Prototype basic kTLS enablement
3. Benchmark performance gains
4. Verify mTLS compatibility

**Week 14-15: Core Implementation** (32 hours)
5. Implement `ktls` module
6. Add kernel support detection
7. Implement hybrid TLS strategy
8. Integrate with existing TLS code
9. Add zero-copy sendfile

**Week 16: Testing & Optimization** (16 hours)
10. Unit tests for kTLS
11. Integration tests
12. Performance benchmarking
13. Documentation

**Total Estimated Effort**: 72 hours (9 working days)

### Expected Performance

**TLS Handshake**:
- No change (still userspace)
- kTLS only offloads data transfer

**Data Transfer** (Encrypted):
- CPU usage: **-20-30%**
- Throughput: **+20-30%**
- Latency: **-10-20%**

**Static File Serving** (with sendfile):
- CPU usage: **-40-50%** (zero-copy)
- Throughput: **+50-70%**
- Memory: **-30-40%**

**mTLS**:
- Same benefits as regular TLS
- No additional overhead

### Risks & Mitigation

**Risk 1: Kernel Version Incompatibility**
- **Mitigation**: Auto-detect and fallback to userspace TLS
- **Impact**: Graceful degradation

**Risk 2: Cipher Suite Limitations**
- **Mitigation**: Document supported cipher suites, validate config
- **Impact**: Limited cipher choice with kTLS

**Risk 3: Platform Portability**
- **Mitigation**: Feature-gated compilation (#[cfg(target_os = "linux")])
- **Impact**: Works on Linux, degrades gracefully on others

---

## Feature 3: Web Server - PHP-FPM & Static Files

### Overview

**Goal**: Add web server capabilities to replace Nginx in LEMP (Linux, Nginx, MySQL, PHP) stacks

**Use Cases**:
1. Static file serving (HTML, CSS, JS, images)
2. PHP application hosting (WordPress, Laravel, Symfony)
3. Combined reverse proxy + web server

### Architecture

#### 1. Static File Serving

```rust
pub mod webserver {
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;

    /// Web server configuration
    pub struct WebServerConfig {
        pub enabled: bool,
        pub document_roots: HashMap<String, PathBuf>,  // hostname -> root
        pub index_files: Vec<String>,  // ["index.html", "index.php"]
        pub autoindex: bool,  // Directory listing
        pub sendfile: bool,  // Use sendfile(2) for efficiency
        pub gzip_static: bool,  // Serve .gz files if available
    }

    /// Static file handler
    pub struct StaticFileHandler {
        config: WebServerConfig,
        mime_types: MimeTypeMap,
        cache: Arc<FileCache>,  // Optional file caching
    }

    impl StaticFileHandler {
        /// Serve static file
        pub async fn serve_file(
            &self,
            path: &Path,
            req: &Request,
        ) -> Result<Response> {
            // 1. Security check (prevent path traversal)
            let safe_path = sanitize_path(path)?;

            // 2. Check if file exists
            let metadata = tokio::fs::metadata(&safe_path).await?;

            // 3. Handle directory (index file or autoindex)
            if metadata.is_dir() {
                return self.serve_directory(&safe_path, req).await;
            }

            // 4. Check if-modified-since (304 Not Modified)
            if let Some(since) = req.headers().get(header::IF_MODIFIED_SINCE) {
                if !is_modified(&metadata, since)? {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .body(Body::empty())?);
                }
            }

            // 5. Try gzip version if gzip_static enabled
            if self.config.gzip_static {
                if let Some(gzip_path) = check_gzip_variant(&safe_path).await {
                    return self.serve_compressed_file(gzip_path, req).await;
                }
            }

            // 6. Determine MIME type
            let mime_type = self.mime_types.get_for_path(&safe_path);

            // 7. Serve file (with sendfile if enabled and kTLS)
            if self.config.sendfile && is_ktls_enabled() {
                // Zero-copy sendfile
                self.serve_with_sendfile(&safe_path, mime_type).await
            } else {
                // Read file into buffer
                self.serve_buffered(&safe_path, mime_type).await
            }
        }

        /// Zero-copy file serving
        async fn serve_with_sendfile(
            &self,
            path: &Path,
            mime_type: &str,
        ) -> Result<Response> {
            let file = File::open(path).await?;
            let metadata = file.metadata().await?;

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type)
                .header(header::CONTENT_LENGTH, metadata.len())
                .header(header::LAST_MODIFIED, format_http_date(metadata.modified()?))
                .body(Body::from_file(file))?)  // Uses sendfile internally
        }
    }
}
```

**Features**:
- ✅ MIME type detection
- ✅ Conditional requests (If-Modified-Since, ETag)
- ✅ Range requests (partial content)
- ✅ Gzip pre-compression
- ✅ Directory listing (optional)
- ✅ Zero-copy sendfile

#### 2. PHP-FPM Integration (FastCGI)

```rust
pub mod fastcgi {
    use tokio::net::TcpStream;

    /// PHP-FPM connection pool
    pub struct PhpFpmPool {
        address: SocketAddr,  // php-fpm socket (e.g., 127.0.0.1:9000)
        pool: Pool<FastCgiConnection>,
        config: PhpFpmConfig,
    }

    /// PHP-FPM configuration
    pub struct PhpFpmConfig {
        pub address: String,  // "127.0.0.1:9000" or "/var/run/php-fpm.sock"
        pub max_connections: usize,
        pub timeout: Duration,
        pub buffer_size: usize,
    }

    /// FastCGI protocol implementation
    pub struct FastCgiConnection {
        stream: TcpStream,
        request_id: u16,
    }

    impl PhpFpmPool {
        /// Execute PHP script via FastCGI
        pub async fn execute_php(
            &self,
            script_path: &Path,
            req: &Request,
        ) -> Result<Response> {
            // 1. Get connection from pool
            let mut conn = self.pool.get().await?;

            // 2. Build FastCGI request
            let fastcgi_req = self.build_fastcgi_request(script_path, req)?;

            // 3. Send request to PHP-FPM
            conn.send_request(&fastcgi_req).await?;

            // 4. Receive response from PHP-FPM
            let fastcgi_resp = conn.receive_response().await?;

            // 5. Parse CGI headers and body
            let (headers, body) = parse_cgi_response(fastcgi_resp)?;

            // 6. Build HTTP response
            Ok(Response::builder()
                .status(headers.status_code)
                .headers(headers.http_headers)
                .body(body)?)
        }

        /// Build FastCGI request from HTTP request
        fn build_fastcgi_request(
            &self,
            script_path: &Path,
            req: &Request,
        ) -> Result<FastCgiRequest> {
            let mut params = HashMap::new();

            // CGI environment variables
            params.insert("SCRIPT_FILENAME", script_path.display().to_string());
            params.insert("REQUEST_METHOD", req.method().as_str().to_string());
            params.insert("REQUEST_URI", req.uri().path().to_string());
            params.insert("QUERY_STRING", req.uri().query().unwrap_or("").to_string());
            params.insert("SERVER_PROTOCOL", "HTTP/1.1".to_string());
            params.insert("GATEWAY_INTERFACE", "CGI/1.1".to_string());

            // HTTP headers as CGI variables
            for (name, value) in req.headers() {
                let cgi_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', '_'));
                params.insert(cgi_name, value.to_str()?.to_string());
            }

            // Content-Type and Content-Length
            if let Some(ct) = req.headers().get(header::CONTENT_TYPE) {
                params.insert("CONTENT_TYPE", ct.to_str()?.to_string());
            }
            if let Some(cl) = req.headers().get(header::CONTENT_LENGTH) {
                params.insert("CONTENT_LENGTH", cl.to_str()?.to_string());
            }

            // Remote address
            params.insert("REMOTE_ADDR", req.remote_addr.ip().to_string());

            Ok(FastCgiRequest {
                params,
                stdin: req.body_bytes().to_vec(),
            })
        }
    }
}
```

**FastCGI Protocol**:
- ✅ Binary protocol implementation
- ✅ Connection pooling for performance
- ✅ Supports Unix sockets and TCP
- ✅ Environment variable mapping
- ✅ Streaming request/response

#### 3. Routing Configuration

```yaml
# Combined proxy + web server configuration
server:
  listen: "0.0.0.0:80"

# Web server hosts
web_servers:
  - hostname: "example.com"
    document_root: "/var/www/example.com"
    index: ["index.php", "index.html"]

    # PHP-FPM for .php files
    php_fpm:
      enabled: true
      address: "127.0.0.1:9000"
      script_pattern: "\.php$"

    # Static file serving
    static_files:
      enabled: true
      gzip: true
      cache_control: "public, max-age=3600"

    # Proxy API requests
    proxy_rules:
      - path: "/api/*"
        upstream: "api_backend"

  - hostname: "app.example.com"
    document_root: "/var/www/app"
    index: ["index.php"]

    php_fpm:
      enabled: true
      address: "/var/run/php-fpm.sock"  # Unix socket

# Upstreams for proxying
upstreams:
  api_backend:
    servers:
      - url: "http://api-server-1:8080"
      - url: "http://api-server-2:8080"
```

**Request Routing Logic**:
```rust
pub async fn route_request(req: Request) -> Response {
    let hostname = req.headers().get(header::HOST)?;
    let path = req.uri().path();

    // 1. Find web server config for hostname
    if let Some(webserver) = WEBSERVERS.get(hostname) {

        // 2. Check proxy rules first (highest priority)
        for rule in &webserver.proxy_rules {
            if rule.matches(path) {
                return proxy_to_upstream(req, rule.upstream).await;
            }
        }

        // 3. Check if PHP file (and PHP-FPM enabled)
        if webserver.php_fpm.enabled && path.ends_with(".php") {
            return execute_php(webserver, req).await;
        }

        // 4. Try static file serving
        if webserver.static_files.enabled {
            let file_path = webserver.document_root.join(path);
            if file_path.exists() {
                return serve_static_file(file_path, req).await;
            }
        }

        // 5. Try index files for directories
        if path.ends_with('/') {
            for index_file in &webserver.index {
                let index_path = webserver.document_root.join(path).join(index_file);
                if index_path.exists() {
                    if index_file.ends_with(".php") && webserver.php_fpm.enabled {
                        return execute_php_file(webserver, index_path, req).await;
                    } else {
                        return serve_static_file(index_path, req).await;
                    }
                }
            }
        }
    }

    // 6. No match - 404 Not Found
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Not Found".into())
}
```

### Implementation Plan

**Week 12-13: Static File Serving** (32 hours)
1. Implement `StaticFileHandler`
2. MIME type detection
3. Conditional requests (If-Modified-Since, ETag)
4. Range requests (partial content)
5. Gzip pre-compression
6. Zero-copy sendfile integration
7. Directory listing

**Week 14-15: PHP-FPM Integration** (32 hours)
8. FastCGI protocol implementation
9. PHP-FPM connection pool
10. Environment variable mapping
11. Request/response streaming
12. Unix socket support
13. Error handling

**Week 16-17: Integration & Testing** (24 hours)
14. Combined routing logic
15. Configuration schema
16. Integration with existing proxy
17. Comprehensive testing
18. WordPress/Laravel testing

**Week 18: Optimization & Documentation** (16 hours)
19. Performance benchmarking
20. File caching
21. Documentation
22. Example configurations

**Total Estimated Effort**: 104 hours (13 working days)

### Expected Performance

**Static File Serving**:
- Small files (< 100KB): **< 50µs** (with cache)
- Large files (> 10MB): **Zero-copy sendfile** (kTLS)
- Throughput: **10+ GB/s** (memory → network)

**PHP-FPM**:
- Connection pooling: **< 100µs** overhead
- FastCGI protocol: **< 10µs** encoding/decoding
- Total latency: **~5-50ms** (dominated by PHP execution)

**Combined (Proxy + Web Server)**:
- No performance degradation
- Routing adds: **< 1µs** overhead
- Memory: **< 10MB** overhead

### Nginx Compatibility

**Features Parity**:
| Feature | Nginx | This Proxy | Status |
|---------|-------|------------|--------|
| Static files | ✅ | ✅ | Equivalent |
| PHP-FPM | ✅ | ✅ | Equivalent |
| Gzip | ✅ | ✅ | Equivalent |
| Sendfile | ✅ | ✅ | Better (kTLS) |
| HTTP/2 | ✅ | ✅ | Equivalent |
| HTTP/3 | ✅ | ✅ | Equivalent |
| Reverse proxy | ✅ | ✅ | Better (more features) |
| Load balancing | ✅ | ✅ | Better (more algorithms) |

**Migration Path**:
```nginx
# Nginx configuration
server {
    listen 80;
    server_name example.com;
    root /var/www/example.com;
    index index.php index.html;

    location / {
        try_files $uri $uri/ /index.php?$query_string;
    }

    location ~ \.php$ {
        fastcgi_pass 127.0.0.1:9000;
        fastcgi_index index.php;
        include fastcgi_params;
    }
}
```

**Equivalent Configuration (YAML)**:
```yaml
web_servers:
  - hostname: "example.com"
    document_root: "/var/www/example.com"
    index: ["index.php", "index.html"]

    php_fpm:
      enabled: true
      address: "127.0.0.1:9000"

    static_files:
      enabled: true
      try_files: ["$uri", "$uri/", "/index.php?$query_string"]
```

**Migration Tool**: Auto-convert Nginx configs to YAML

---

## Combined Feature Impact

### Performance Improvements

| Feature | CPU | Latency | Throughput | Memory |
|---------|-----|---------|------------|--------|
| Hostname Routes | -5% | -20% | +25% | +5% |
| Kernel TLS | -25% | -15% | +30% | -20% |
| Web Server | 0% | 0% | N/A | +2% |
| **Combined** | **-30%** | **-35%** | **+55%** | **-13%** |

### Feature Comparison

| Feature | Nginx | Caddy | Envoy | **This Proxy** |
|---------|-------|-------|-------|----------------|
| HTTP/1.1 | ✅ | ✅ | ✅ | ✅ |
| HTTP/2 | ✅ | ✅ | ✅ | ✅ |
| HTTP/3 | ✅ | ✅ | ✅ | ✅ |
| Static files | ✅ | ✅ | ❌ | ✅ |
| PHP-FPM | ✅ | ❌ | ❌ | ✅ |
| Reverse proxy | ✅ | ✅ | ✅ | ✅ |
| Load balancing | ✅ | ✅ | ✅ | ✅ Better |
| WAF | ❌ | ❌ | ✅ | ✅ Better |
| API Gateway | ❌ | ❌ | ✅ | ✅ Better |
| kTLS | ✅ | ❌ | ❌ | ✅ |
| SIMD | ❌ | ❌ | ❌ | ✅ |
| io_uring | ❌ | ❌ | ❌ | ✅ |

**Conclusion**: With these features, this proxy becomes **the most feature-complete and performant** option available

---

## Implementation Timeline

### Weeks 12-18 (42 working days)

**Week 12: API Gateway Foundation**
- Design per-hostname route storage
- Implement DashMap-based indexing
- JSON schema and parser

**Week 13: API Gateway Optimization**
- SIMD-accelerated parsing
- Parallel route loading
- Benchmarking

**Week 14: Kernel TLS Research**
- Linux kTLS API study
- Prototype implementation
- Performance testing

**Week 15: Kernel TLS Implementation**
- Full kTLS integration
- Hybrid TLS strategy
- Zero-copy sendfile

**Week 16: Web Server - Static Files**
- Static file handler
- MIME types, caching
- Zero-copy integration

**Week 17: Web Server - PHP-FPM**
- FastCGI protocol
- PHP-FPM pool
- Integration testing

**Week 18: Polish & Testing**
- End-to-end testing
- Performance benchmarking
- Documentation
- Example configurations

### Resource Requirements

**Development Time**: 260-280 hours (6-7 weeks)
**Testing Time**: 40-60 hours (1 week)
**Documentation**: 20-30 hours

**Total**: 320-370 hours (~8-9 weeks with 1 developer)

---

## Risk Assessment

### Technical Risks

**Risk 1: Kernel TLS Compatibility**
- **Probability**: Medium
- **Impact**: Low
- **Mitigation**: Auto-detect and fallback to userspace

**Risk 2: PHP-FPM Protocol Complexity**
- **Probability**: Low
- **Impact**: Medium
- **Mitigation**: Reference existing implementations (Nginx, lighttpd)

**Risk 3: Performance Regression**
- **Probability**: Low
- **Impact**: High
- **Mitigation**: Comprehensive benchmarking at each step

### Operational Risks

**Risk 1: Configuration Complexity**
- **Probability**: Medium
- **Impact**: Medium
- **Mitigation**: Provide migration tool from Nginx

**Risk 2: PHP-FPM Version Incompatibility**
- **Probability**: Low
- **Impact**: Low
- **Mitigation**: Test with multiple PHP versions (7.4, 8.0, 8.1, 8.2)

---

## Success Criteria

### API Gateway
- ✅ 10,000 routes load in < 100ms
- ✅ Route lookup in < 100ns (exact match)
- ✅ Hot reload with zero downtime
- ✅ Memory < 500 bytes per route

### Kernel TLS
- ✅ Auto-detect kernel support
- ✅ 20%+ CPU reduction for TLS workloads
- ✅ Works with mTLS
- ✅ Graceful fallback on unsupported platforms

### Web Server
- ✅ Static file serving at 10+ GB/s
- ✅ PHP-FPM works with WordPress/Laravel
- ✅ Zero-copy sendfile with kTLS
- ✅ Nginx config migration tool

---

## Conclusion

These three features transform the reverse proxy into a **production-grade application delivery platform** that can completely replace Nginx while offering superior performance.

**Key Advantages**:
1. **API Gateway**: 20x faster route lookup than regex-based systems
2. **Kernel TLS**: 25% less CPU, 30% more throughput
3. **Web Server**: Full Nginx compatibility + better performance

**Production Readiness**: After implementation, this will be ready for high-traffic production deployments handling millions of requests per second.

**Next Steps**: Begin Week 12 implementation of API Gateway per-hostname routes.
