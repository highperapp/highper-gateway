# Streaming Response Body - Architectural Analysis

**Feature:** 1.2 Zero-Copy Sendfile - Streaming Response (P1)
**Branch:** `feature/streaming-response-body`
**Date:** 2025-11-16
**Status:** Analysis Complete, Implementation In Progress

---

## 🎯 Objective

Replace buffered file serving with streaming responses to:
- ✅ Support files of any size without memory constraints
- ✅ Reduce memory usage from O(file_size) to O(buffer_size)
- ✅ Enable constant memory usage (~16-64KB buffer)
- ✅ Improve performance for large file downloads (>100MB)
- ✅ Foundation for future sendfile() zero-copy optimization

---

## 📊 Current State Analysis

### Current Implementation

**File:** `src/proxy/handler.rs:776-849`

```rust
async fn serve_static_file(
    &self,
    file_info: &FileInfo,
    static_handler: &StaticFileHandler,
    req: &Request<Incoming>,
) -> Result<Response<Full<Bytes>>> {
    // Problem: Entire file loaded into memory
    let mut file = File::open(&file_info.path)?;  // Blocking I/O
    let mut contents = Vec::with_capacity(file_size as usize);  // Allocate file_size
    file.read_to_end(&mut contents)?;  // Read entire file

    Ok(Response::builder()
        .body(Full::new(Bytes::from(contents)))?)  // Buffer entire file
}
```

**Issues:**
1. **Memory:** Allocates `file_size` bytes per request
2. **Blocking I/O:** Uses `std::fs::File` (blocks thread)
3. **Scalability:** 10 concurrent 1GB file downloads = 10GB RAM
4. **Latency:** Must read entire file before sending first byte

### Return Type Chain

The `Response<Full<Bytes>>` return type propagates through:

```
Handler::handle() -> Response<Full<Bytes>>
├── serve_static_file() -> Response<Full<Bytes>>
├── serve_php_file() -> Response<Full<Bytes>>
├── parse_cgi_response() -> Response<Full<Bytes>>
├── error_response() -> Response<Full<Bytes>>
└── [15+ inline Response::builder() calls]
```

**Files Affected:**
- `src/proxy/handler.rs` (~950 lines)
- `src/proxy/server.rs` (calls handler)
- All tests that create mock responses
- Middleware that processes responses

---

## 🏗️ Proposed Architecture

### New ResponseBody Type

**File:** `src/http/streaming_body.rs` (✅ Created)

```rust
pub enum ResponseBody {
    /// Empty body (0 bytes) - for 304 Not Modified, etc.
    Empty,

    /// Buffered body - for small responses (<64KB)
    Buffered(Full<Bytes>),

    /// Streaming body - for large files, proxied responses
    Stream(UnsyncBoxBody<Bytes, std::io::Error>),
}

impl Body for ResponseBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(...) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        match self {
            Empty => Poll::Ready(None),
            Buffered(full) => Pin::new(full).poll_frame(cx),
            Stream(stream) => Pin::new(stream).poll_frame(cx),
        }
    }
}
```

**Key Methods:**
```rust
impl ResponseBody {
    pub fn empty() -> Self;
    pub fn buffered(bytes: Bytes) -> Self;
    pub fn from_reader<R: AsyncRead>(reader: R) -> Self;
    pub fn from_file(file: tokio::fs::File) -> Self;
}
```

### New serve_static_file Implementation

```rust
async fn serve_static_file(
    &self,
    file_info: &FileInfo,
    static_handler: &StaticFileHandler,
    req: &Request<Incoming>,
) -> Result<Response<ResponseBody>> {
    // Check ETag/If-Modified-Since (unchanged)
    if etag_match {
        return Ok(Response::builder()
            .status(304)
            .body(ResponseBody::empty())?);  // ← Empty body
    }

    // Open file async (non-blocking)
    let file = tokio::fs::File::open(&file_info.path).await?;

    // Return streaming response
    Ok(Response::builder()
        .status(200)
        .header("content-type", mime_type)
        .header("content-length", file_size.to_string())
        .body(ResponseBody::from_file(file))?)  // ← Stream body
}
```

**Benefits:**
- **Memory:** O(16KB buffer) instead of O(file_size)
- **Non-blocking:** `tokio::fs::File` uses async I/O
- **Latency:** First chunk sent immediately after headers
- **Scalability:** 1000 concurrent 1GB downloads = ~16MB RAM

---

## 🔄 Migration Strategy

### Phase 1: Foundation (✅ Completed)

**Completed:**
- [x] Create `ResponseBody` enum with Body trait
- [x] Add dependencies: `http-body`, `tokio-util`
- [x] Implement `from_reader()` and `from_file()` methods
- [x] Write unit tests for ResponseBody
- [x] Export from `http` module

**Files Created:**
- `src/http/streaming_body.rs` (155 lines)

**Dependencies Added:**
```toml
http-body = "1.0"
tokio-util = { version = "0.7", features = ["io"] }
```

### Phase 2: Handler Refactoring (🚧 In Progress)

**Changes Required:**

1. **Update Handler Signature** (✅ Partially Done)
   ```rust
   // Old
   pub async fn handle(&self, req: Request<Incoming>)
       -> Result<Response<Full<Bytes>>>

   // New
   pub async fn handle(&self, req: Request<Incoming>)
       -> Result<Response<ResponseBody>>
   ```

2. **Update All Response Creations** (~15 locations)

   | Location | Current | New | Lines |
   |----------|---------|-----|-------|
   | ACME challenges | `Full::new(Bytes::from(...))` | `ResponseBody::buffered(...)` | 241, 802, 818 |
   | serve_static_file | `Full::new(Bytes::from(contents))` | `ResponseBody::from_file(file)` | 849 |
   | serve_php_file | `Full::new(Bytes::from(...))` | `ResponseBody::buffered(...)` | 916 |
   | parse_cgi_response | `Full::new(Bytes::from(...))` | `ResponseBody::buffered(...)` | 928, 978 |
   | Proxied responses | `Full::new(body_bytes)` | `ResponseBody::buffered(...)` | 540 |
   | error_response | `Full::new(Bytes::from(...))` | `ResponseBody::buffered(...)` | 990 ✅ |

3. **Update serve_static_file** (Priority: High)
   - Replace `std::fs::File::open()` → `tokio::fs::File::open()`
   - Replace `file.read_to_end()` → `ResponseBody::from_file(file)`
   - Keep 304 responses as `ResponseBody::empty()`

4. **Update Middleware** (If Applicable)
   - Check if compression middleware accesses body
   - Verify middleware chain compatibility

### Phase 3: Testing & Validation

**Test Plan:**

1. **Unit Tests** (✅ Completed for ResponseBody)
   - Empty body collection
   - Buffered body collection
   - Streaming body from reader

2. **Integration Tests** (TODO)
   - Large file download (1GB)
   - Multiple concurrent downloads
   - 304 Not Modified responses
   - ETag/If-Modified-Since caching
   - PHP-FPM responses
   - Proxied responses

3. **Performance Benchmarks** (TODO)
   ```bash
   # Memory usage (should be constant)
   wrk -t4 -c100 -d30s http://localhost:8080/large-file.bin

   # Monitor with:
   ps aux | grep highper-gateway  # RSS should stay constant
   ```

4. **Load Testing** (TODO)
   - 100 concurrent 1GB file downloads
   - Verify memory stays under 100MB
   - Measure throughput vs buffered approach

---

## 📋 Detailed Implementation Checklist

### Step 1: Complete Handler Response Updates

**File:** `src/proxy/handler.rs`

- [ ] **Line 241:** ACME challenge response
  ```rust
  // Old
  .body(Full::new(Bytes::from(key_auth)))?

  // New
  .body(ResponseBody::buffered(Bytes::from(key_auth)))?
  ```

- [ ] **Line 802-803:** 304 Not Modified (ETag)
  ```rust
  // Old
  .body(Full::new(Bytes::new()))?

  // New
  .body(ResponseBody::empty())?
  ```

- [ ] **Line 818-819:** 304 Not Modified (If-Modified-Since)
  ```rust
  .body(ResponseBody::empty())?
  ```

- [ ] **Lines 825-849:** serve_static_file main path
  ```rust
  // Old
  let mut file = File::open(&file_info.path)?;
  let mut contents = Vec::with_capacity(file_size as usize);
  file.read_to_end(&mut contents)?;
  response.body(Full::new(Bytes::from(contents)))?

  // New
  let file = tokio::fs::File::open(&file_info.path).await?;
  response.body(ResponseBody::from_file(file))?
  ```

- [ ] **Line 916:** serve_php_file success response
  ```rust
  .body(ResponseBody::buffered(Bytes::from(body.to_vec())))?
  ```

- [ ] **Line 928:** parse_cgi_response (no headers)
  ```rust
  .body(ResponseBody::buffered(Bytes::from(output.to_vec())))?
  ```

- [ ] **Line 978:** parse_cgi_response (with headers)
  ```rust
  .body(ResponseBody::buffered(Bytes::from(body.to_vec())))?
  ```

- [ ] **Line 540:** Proxied response (middleware applied)
  ```rust
  .body(ResponseBody::buffered(body_bytes))?
  ```

- [x] **Line 990:** error_response ✅
  ```rust
  .body(ResponseBody::buffered(Bytes::from(message.to_string())))?
  ```

### Step 2: Update serve_static_file Signature

**File:** `src/proxy/handler.rs:777`

```rust
// Old
async fn serve_static_file(
    &self,
    file_info: &crate::webserver::FileInfo,
    static_handler: &crate::webserver::StaticFileHandler,
    req: &Request<Incoming>,
) -> Result<Response<Full<Bytes>>>

// New
async fn serve_static_file(
    &self,
    file_info: &crate::webserver::FileInfo,
    static_handler: &crate::webserver::StaticFileHandler,
    req: &Request<Incoming>,
) -> Result<Response<ResponseBody>>
```

### Step 3: Update serve_php_file Signature

**File:** `src/proxy/handler.rs:838`

```rust
// Old
async fn serve_php_file(
    &self,
    file_info: &crate::webserver::FileInfo,
    php_pool: &crate::webserver::PhpFpmPool,
    req: &Request<Empty<Bytes>>,
    host: &str,
    path: &str,
    body: CollectedBody,
) -> Result<Response<Full<Bytes>>>

// New
async fn serve_php_file(
    &self,
    file_info: &crate::webserver::FileInfo,
    php_pool: &crate::webserver::PhpFpmPool,
    req: &Request<Empty<Bytes>>,
    host: &str,
    path: &str,
    body: CollectedBody,
) -> Result<Response<ResponseBody>>
```

### Step 4: Update parse_cgi_response Signature

**File:** `src/proxy/handler.rs:920`

```rust
// Old
fn parse_cgi_response(&self, output: &[u8])
    -> Result<Response<Full<Bytes>>>

// New
fn parse_cgi_response(&self, output: &[u8])
    -> Result<Response<ResponseBody>>
```

### Step 5: Fix Remaining Inline Responses

Search for all `Full::new` in handler.rs:
```bash
grep -n "Full::new" src/proxy/handler.rs
```

Update each to use appropriate ResponseBody variant.

---

## 🔍 Risk Assessment

### High Risk

**Breaking Changes:**
- Handler return type changes (affects server.rs)
- Middleware compatibility (compression might need updates)

**Mitigation:**
- Feature branch isolation
- Comprehensive testing before merge
- Gradual rollout (feature flag if needed)

### Medium Risk

**Performance Regression:**
- Small files (<64KB) might be slower with streaming overhead

**Mitigation:**
- Use `ResponseBody::Buffered` for files <64KB
- Benchmark comparison: buffered vs streaming
- Add size threshold configuration

### Low Risk

**Backwards Compatibility:**
- External API remains unchanged (HTTP protocol)
- Client behavior unaffected

---

## 📈 Success Metrics

### Performance Targets

| Metric | Current | Target | Method |
|--------|---------|--------|--------|
| **Memory (1GB file)** | 1GB | <64MB | Monitor RSS during download |
| **Memory (100x1GB concurrent)** | 100GB | <100MB | Load test with `wrk` |
| **Latency (TTFB)** | file_read_time | <10ms | Benchmark with `curl -w` |
| **Throughput (single)** | ~Disk I/O | ~Disk I/O | No regression expected |
| **Throughput (concurrent)** | Limited by RAM | High | Should improve significantly |

### Functional Requirements

- [x] ResponseBody enum created and tested
- [ ] All handler responses converted to ResponseBody
- [ ] serve_static_file uses tokio::fs + streaming
- [ ] Large file downloads work correctly
- [ ] 304 Not Modified responses work
- [ ] PHP-FPM responses work
- [ ] Proxied responses work
- [ ] ETag caching works
- [ ] If-Modified-Since caching works

### Test Coverage

- [x] Unit tests for ResponseBody (3/3)
- [ ] Integration test: large file download
- [ ] Integration test: concurrent downloads
- [ ] Integration test: 304 responses
- [ ] Load test: memory usage under load
- [ ] Load test: throughput comparison

---

## 🚀 Next Steps

### Immediate (This Session)

1. ✅ Create feature branch
2. ⏭️ **Commit current progress**
3. ⏭️ **Complete handler.rs response updates** (Steps 1-5 above)
4. ⏭️ **Update serve_static_file implementation**
5. ⏭️ **Fix compilation errors**
6. ⏭️ **Run existing tests**

### Short Term (Next Session)

1. Add integration tests for streaming
2. Benchmark memory usage
3. Add size threshold for buffered vs streaming
4. Update documentation
5. Code review and merge to master

### Long Term (Future)

1. Implement true sendfile() syscall (Linux zero-copy)
2. Add HTTP range request support (partial content)
3. Optimize buffer sizes based on file type
4. Add streaming for proxied responses

---

## 📝 Notes & Decisions

### Why UnsyncBoxBody?

**Decision:** Use `UnsyncBoxBody` instead of `BoxBody`

**Rationale:**
- Handler runs in single-threaded tokio context
- No need for `Sync` overhead
- Slightly better performance

**Trade-off:** Cannot move handler across threads (acceptable for HTTP handler)

### Why Not Just StreamBody?

**Decision:** Enum with Empty/Buffered/Stream variants

**Rationale:**
- **Empty:** Zero allocation for 304 responses (common case)
- **Buffered:** Fast path for small responses (<64KB)
- **Stream:** Memory-efficient for large files

**Alternative Considered:** Always use StreamBody
**Rejected Because:** More overhead for small responses

### Buffered vs Streaming Threshold

**Decision:** TBD (needs benchmarking)

**Options:**
- 64KB (typical TCP window)
- 128KB (L3 cache size)
- 256KB (conservative)
- Configurable via settings

**Recommendation:** Start with 256KB, make configurable later

---

## 🔗 Related Work

**Dependencies:**
- POST Body Streaming (P0) ✅ Completed
- Enables: Response Streaming for Proxied Requests (P2)
- Enables: Zero-Copy Sendfile Syscall (Future)

**Blocked By:** None

**Blocks:**
- HTTP Range Requests (1.5)
- Sendfile() Zero-Copy (Future)

---

**Document Version:** 1.0
**Last Updated:** 2025-11-16
**Author:** Claude Code
**Status:** Ready for Implementation

