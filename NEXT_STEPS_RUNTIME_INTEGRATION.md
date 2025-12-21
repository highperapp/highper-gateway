# Next Steps: Runtime Integration

**Date**: December 21, 2025
**Current Status**: DSL Layer Complete ✅
**Next Phase**: Runtime Handler Integration

---

## Current State Summary

### ✅ COMPLETE: DSL Layer (Phase 1)

We've successfully completed the DSL parsing and conversion layer:

1. **Grammar Definitions** (`dsl.pest`) - ✅ Complete
   - php_fpm_directive with 8 options
   - static_files_directive, root_directive, index_directive, try_files_directive
   - All patterns and rules working correctly

2. **Parser Implementation** (`dsl_parser.rs`) - ✅ Complete
   - All parsing functions implemented
   - 14 parser tests passing
   - Handles nested rules, defaults, and edge cases

3. **YAML Converter** (`dsl_converter.rs`) - ✅ Complete
   - Full conversion support for all directives
   - 10 converter tests passing
   - Generates valid YAML for runtime

4. **Documentation & Examples** - ✅ Complete
   - PHP_FPM_DSL_COMPLETE.md - Full implementation docs
   - PHP_FPM_QUICK_START.md - User guide
   - examples/php-fpm-scenarios.dsl - 8 real-world scenarios

### ✅ COMPLETE: Runtime Components (Pre-existing)

The runtime layer has significant implementation already in place:

1. **FastCGI Protocol** (`webserver/php_fpm.rs`) - ✅ 100% Complete (332 lines)
   - Complete wire protocol implementation
   - Request/response encoding/decoding
   - All record types supported
   - Multi-record response handling

2. **Connection Pooling** (`webserver/php_fpm.rs`) - ✅ 90% Complete
   - Connection pool with configurable size
   - Idle connection reuse
   - Connection expiration
   - Unix socket and TCP support
   - Timeouts configured

3. **Static File Serving** (`webserver/static_files.rs`) - ✅ 80% Complete (224 lines)
   - Path resolution with security
   - Directory traversal prevention
   - URL decoding
   - Index file handling
   - MIME type detection
   - ETag generation
   - Zero-copy sendfile
   - kTLS integration

4. **MIME Type Cache** (`webserver/mime.rs`) - ✅ 100% Complete (261 lines)
   - Lock-free concurrent cache (DashMap)
   - Pre-populated common types
   - Extension-based detection
   - Compressibility detection

5. **Configuration** (`webserver/config.rs`) - ✅ 95% Complete (241 lines)
   - WebServerConfig struct
   - PhpFpmConfig struct
   - All fields defined

**Total Runtime Code**: 1,117 lines (significantly complete!)

---

## ❌ NOT COMPLETE: Handler Integration (Critical Path)

### What's Missing

The gap between DSL/runtime and actual functionality is the **Handler Integration**:

**Location**: `src/proxy/handler.rs`
**Status**: Not integrated
**Estimated Work**: 8-12 hours

### Required Implementation

#### 1. Request Routing Logic (2-3 hours)

Need to add routing logic to detect and route requests:

```rust
// In Handler::handle_request()
async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>> {
    // NEW: Check if this route has static file or PHP-FPM config
    let route_config = self.get_route_config(&req)?;

    if route_config.static_files || route_config.root.is_some() {
        // Static file or PHP handling
        return self.handle_webserver_request(req, route_config).await;
    }

    // Existing proxy logic
    self.handle_proxy_request(req).await
}
```

#### 2. Static File Handler (2-3 hours)

Integrate existing `static_files.rs` module:

```rust
async fn handle_webserver_request(
    &self,
    req: Request<Body>,
    config: &RouteConfig,
) -> Result<Response<Body>> {
    let uri_path = req.uri().path();

    // Resolve file path
    let file_info = self.static_handler.resolve_path(
        uri_path,
        &config.root,
        &config.index,
    )?;

    // Check if it's a PHP file
    if file_info.is_php && config.php_fpm.is_some() {
        return self.handle_php_request(req, file_info, config).await;
    }

    // Serve static file
    self.serve_static_file(req, file_info).await
}
```

#### 3. FastCGI Params Builder (3-4 hours)

Build FastCGI parameters from HTTP request:

```rust
fn build_fastcgi_params(
    req: &Request<Body>,
    file_info: &FileInfo,
    config: &RouteConfig,
) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Script parameters
    params.insert("SCRIPT_FILENAME".to_string(), file_info.absolute_path.clone());
    params.insert("SCRIPT_NAME".to_string(), req.uri().path().to_string());
    params.insert("REQUEST_METHOD".to_string(), req.method().as_str().to_string());
    params.insert("REQUEST_URI".to_string(), req.uri().to_string());

    // Query string
    if let Some(query) = req.uri().query() {
        params.insert("QUERY_STRING".to_string(), query.to_string());
    }

    // Content headers
    if let Some(content_type) = req.headers().get("content-type") {
        params.insert("CONTENT_TYPE".to_string(), content_type.to_str().unwrap().to_string());
    }
    if let Some(content_length) = req.headers().get("content-length") {
        params.insert("CONTENT_LENGTH".to_string(), content_length.to_str().unwrap().to_string());
    }

    // Server variables
    params.insert("DOCUMENT_ROOT".to_string(), config.root.clone().unwrap_or_default());
    params.insert("SERVER_SOFTWARE".to_string(), "highper-gateway/1.0".to_string());
    params.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    params.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());

    // HTTP headers as HTTP_*
    for (name, value) in req.headers().iter() {
        let header_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', '_'));
        params.insert(header_name, value.to_str().unwrap_or("").to_string());
    }

    params
}
```

#### 4. CGI Response Parser (2-3 hours)

Parse CGI headers from FastCGI response:

```rust
fn parse_cgi_response(data: &[u8]) -> Result<(HeaderMap, Vec<u8>)> {
    let mut headers = HeaderMap::new();
    let mut status = StatusCode::OK;

    // Find the blank line separating headers from body
    let body_start = data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("No blank line in CGI response"))?;

    let header_section = &data[..body_start];
    let body = &data[body_start + 4..];

    // Parse headers
    for line in header_section.split(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(line).trim().to_string();

        if line.is_empty() {
            continue;
        }

        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();

            // Handle special Status header
            if name.eq_ignore_ascii_case("status") {
                if let Some(code_str) = value.split_whitespace().next() {
                    if let Ok(code) = code_str.parse::<u16>() {
                        status = StatusCode::from_u16(code)?;
                    }
                }
            } else {
                // Regular header
                headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(value)?,
                );
            }
        }
    }

    Ok((headers, body.to_vec()))
}
```

#### 5. PHP Request Handler (2-3 hours)

Tie everything together:

```rust
async fn handle_php_request(
    &self,
    req: Request<Body>,
    file_info: FileInfo,
    config: &RouteConfig,
) -> Result<Response<Body>> {
    let php_config = config.php_fpm.as_ref()
        .ok_or_else(|| anyhow!("PHP-FPM not configured"))?;

    // Get request body
    let body_bytes = hyper::body::to_bytes(req.into_body()).await?;

    // Build FastCGI params
    let params = build_fastcgi_params(&req, &file_info, config);

    // Get connection from pool
    let mut conn = self.php_fpm_pool.get_connection(&php_config.socket).await?;

    // Execute FastCGI request
    let response_data = conn.execute(&params, &body_bytes).await?;

    // Parse CGI response
    let (headers, body) = parse_cgi_response(&response_data)?;

    // Build HTTP response
    let mut response = Response::new(Body::from(body));
    *response.headers_mut() = headers;

    Ok(response)
}
```

---

## Implementation Plan

### Week 1: Core Integration (8-12 hours)

**Day 1-2: Handler Routing (3-4 hours)**
- [ ] Add route config detection to Handler
- [ ] Implement webserver request routing
- [ ] Add static file handler integration
- [ ] Test with simple static file

**Day 3: FastCGI Params Builder (3-4 hours)**
- [ ] Implement build_fastcgi_params()
- [ ] Add all required CGI parameters
- [ ] Add HTTP header mapping (HTTP_*)
- [ ] Test parameter generation

**Day 4: CGI Response Parser (2-3 hours)**
- [ ] Implement parse_cgi_response()
- [ ] Handle Status: header
- [ ] Handle all CGI headers
- [ ] Test with sample CGI responses

**Day 5: PHP Request Handler (2-3 hours)**
- [ ] Implement handle_php_request()
- [ ] Integrate with PHP-FPM pool
- [ ] Test end-to-end with real PHP-FPM
- [ ] Handle errors gracefully

**Deliverable**: Basic PHP-FPM and static serving works!

### Week 2: Enhancement (4-6 hours)

**Enhancement 1: Conditional Requests (2 hours)**
- [ ] Implement If-Modified-Since handling
- [ ] Implement If-None-Match (ETag) handling
- [ ] Return 304 Not Modified when appropriate

**Enhancement 2: Try Files Support (2 hours)**
- [ ] Implement try_files pattern matching
- [ ] Support $uri, $uri/, and fallback files
- [ ] Support status code returns (=404)

**Enhancement 3: Error Pages (1-2 hours)**
- [ ] Custom 404 pages
- [ ] Custom 500 pages
- [ ] Proper error responses

**Deliverable**: Production-ready webserver!

### Week 3: Advanced Features (Optional, 6-8 hours)

**Feature 1: Range Requests (3-4 hours)**
- [ ] Parse Range header
- [ ] Support single byte ranges
- [ ] Return 206 Partial Content
- [ ] Return 416 for invalid ranges

**Feature 2: Directory Listing (3-4 hours)**
- [ ] Generate HTML directory listings
- [ ] Show file sizes and dates
- [ ] Add sorting options

**Deliverable**: Full Nginx replacement!

---

## Testing Strategy

### Unit Tests
- [ ] Test build_fastcgi_params() with various requests
- [ ] Test parse_cgi_response() with sample CGI output
- [ ] Test file resolution with try_files patterns
- [ ] Test conditional request logic

### Integration Tests
- [ ] Static file serving (HTML, CSS, JS, images)
- [ ] PHP file execution via PHP-FPM
- [ ] WordPress installation test
- [ ] Laravel application test
- [ ] Mixed static + PHP content

### Load Tests
- [ ] Static file throughput (target: 50K+ req/s)
- [ ] PHP-FPM throughput (target: 5K+ req/s)
- [ ] Connection pool efficiency
- [ ] Latency measurements (p50, p95, p99)

---

## Dependencies

All required dependencies already in Cargo.toml:
- ✅ hyper - HTTP framework
- ✅ tokio - Async runtime
- ✅ dashmap - Concurrent maps
- ✅ percent-encoding - URL decoding
- ✅ anyhow - Error handling

No new dependencies needed!

---

## Configuration Flow

```
DSL File (my-site.dsl)
         ↓
    DSL Parser (parse_dsl)
         ↓
    DSL AST (Config)
         ↓
  YAML Converter (generate_yaml_from_dsl)
         ↓
    YAML File (temp.yaml)
         ↓
  YAML Loader (load_config)
         ↓
  Runtime Config (schema::Config)
         ↓
    Handler (handler.rs) ← NEEDS INTEGRATION
         ↓
  Static Files / PHP-FPM
```

---

## Success Criteria

### Minimum Viable Product (MVP)
- [ ] Static HTML files served correctly
- [ ] PHP files executed via PHP-FPM
- [ ] Index files work (index.php, index.html)
- [ ] Basic error handling (404, 500)

### Production Ready
- [ ] Conditional requests (304 Not Modified)
- [ ] Try files pattern matching
- [ ] Custom error pages
- [ ] Connection pool metrics
- [ ] Health checks working

### Full Features
- [ ] Range requests for video streaming
- [ ] Directory listing
- [ ] All Nginx-equivalent features

---

## Risk Mitigation

### Low Risk ✅
- FastCGI protocol (already implemented and tested)
- Static file serving (core implemented)
- MIME detection (complete)

### Medium Risk ⚠️
- Handler integration (touching critical path)
- CGI parameter generation (must match Nginx exactly)
- Response parsing (edge cases)

### High Risk ❌
- PHP-FPM compatibility across versions
- Performance under high load
- Edge cases in try_files matching

---

## Quick Start for Implementation

### 1. Add Handler Fields

```rust
// In src/proxy/handler.rs
pub struct Handler {
    // ... existing fields ...

    // NEW: Webserver support
    static_handler: Option<Arc<StaticFileHandler>>,
    php_fpm_pool: Option<Arc<PhpFpmPool>>,
    webserver_config: Option<WebServerConfig>,
}
```

### 2. Initialize in Constructor

```rust
impl Handler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        let cfg = config.read().unwrap();

        let (static_handler, php_fpm_pool, webserver_config) =
            if cfg.webserver.is_some() {
                // Initialize webserver components
                (Some(...), Some(...), Some(...))
            } else {
                (None, None, None)
            };

        Self {
            // ... existing fields ...
            static_handler,
            php_fpm_pool,
            webserver_config,
        }
    }
}
```

### 3. Add Routing in handle_request()

```rust
async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>> {
    // NEW: Check for webserver routing
    if self.webserver_config.is_some() {
        if let Some(response) = self.try_webserver_request(&req).await? {
            return Ok(response);
        }
    }

    // Existing proxy logic
    self.handle_proxy_request(req).await
}
```

---

## Resources

- **Implementation Guide**: PHP_FPM_IMPLEMENTATION_STATUS.md
- **DSL Guide**: PHP_FPM_DSL_COMPLETE.md
- **User Guide**: PHP_FPM_QUICK_START.md
- **Examples**: examples/php-fpm-scenarios.dsl
- **Existing Code**:
  - src/webserver/php_fpm.rs (332 lines)
  - src/webserver/static_files.rs (224 lines)
  - src/webserver/mime.rs (261 lines)
  - src/webserver/config.rs (241 lines)

---

## Conclusion

We've completed the DSL layer (100%) and most of the runtime components exist (80-90%). The critical missing piece is the **Handler Integration** which connects everything together.

**Estimated Time to MVP**: 8-12 hours of focused development
**Estimated Time to Production**: 12-18 hours total
**Estimated Time to Full Features**: 18-26 hours total

The foundation is solid. The integration is straightforward. Ready to proceed!

---

**Document**: NEXT_STEPS_RUNTIME_INTEGRATION.md
**Date**: December 21, 2025
**Status**: ✅ DSL Complete, ⏭️ Handler Integration Next
