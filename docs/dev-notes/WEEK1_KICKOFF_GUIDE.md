# Week 1 Kickoff Guide
## Getting Started with Priority 1 Tasks

**Current Date**: November 9, 2025
**Focus**: Foundation & Stability (4 weeks)
**Immediate Priority**: Week 1 - io_uring Integration & Test Fixes

---

## 🎯 Week 1 Goals

### Success Criteria:
- ✅ io_uring integrated and working on Linux
- ✅ 100% test pass rate (276/276 or more)
- ✅ Admin API endpoints complete
- ✅ 15-20% latency reduction measured

### Time Estimate: 40 hours (1 week)

---

## 📅 Day-by-Day Breakdown

### **Day 1-2: Fix io_uring Integration** (16 hours)

#### Current Problem:
```rust
// src/runtime/mod.rs:18-20
// TODO: HybridTcpStream has borrow checker issues, will fix in Day 3
// #[cfg(all(feature = "io-uring", target_os = "linux"))]
// mod hybrid_stream;  ← COMMENTED OUT!
```

#### Status:
- ✅ Adapter pattern implemented (`io_backend.rs`)
- ✅ io_uring backend implemented (`io_uring_backend.rs` - 16KB)
- ✅ epoll backend implemented (`epoll_backend.rs`)
- ❌ HybridTcpStream has borrow checker errors
- ❌ Server not using GLOBAL_IO.accept()

#### Tasks:

**Step 1: Fix HybridTcpStream (4 hours)**
```bash
# Open the file
vim src/runtime/hybrid_stream.rs

# Look for borrow checker errors
# Likely issues:
# - Holding references across .await points
# - Mutable/immutable borrow conflicts
# - Lifetime issues with AsyncRead/AsyncWrite traits
```

**Common Fix Pattern:**
```rust
// BEFORE (Error):
impl AsyncRead for HybridTcpStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut ReadBuf) -> Poll<io::Result<()>> {
        match &self.inner {  // Immutable borrow
            Inner::IoUring(stream) => stream.poll_read(cx, buf),  // Error!
            Inner::Epoll(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

// AFTER (Fixed):
impl AsyncRead for HybridTcpStream {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context, buf: &mut ReadBuf) -> Poll<io::Result<()>> {
        match &mut self.get_mut().inner {  // Mutable borrow
            Inner::IoUring(stream) => Pin::new(stream).poll_read(cx, buf),
            Inner::Epoll(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}
```

**Step 2: Uncomment in mod.rs (1 hour)**
```rust
// src/runtime/mod.rs
#[cfg(all(feature = "io-uring", target_os = "linux"))]
mod hybrid_stream;

#[cfg(all(feature = "io-uring", target_os = "linux"))]
pub use hybrid_stream::HybridTcpStream;
```

**Step 3: Update server.rs to use GLOBAL_IO (4 hours)**
```rust
// src/proxy/server.rs

// BEFORE:
pub async fn run(&self) -> Result<()> {
    let listener = TcpListener::bind(&self.config.bind).await?;

    loop {
        let (stream, addr) = listener.accept().await?;  // Using tokio directly
        // ...
    }
}

// AFTER:
use crate::runtime::GLOBAL_IO;

pub async fn run(&self) -> Result<()> {
    let listener = TcpListener::bind(&self.config.bind).await?;

    loop {
        let (stream, addr) = GLOBAL_IO.accept(&listener).await?;  // Using adapter!
        // ...
    }
}
```

**Step 4: Test io_uring (4 hours)**
```bash
# Build with io-uring feature
cargo build --release --features io-uring

# Check kernel version (need 5.1+)
uname -r

# Run on Linux 5.1+
./target/release/highper-gateway --config config/example.yaml

# Check logs for io_uring usage
# Should see: "Using io_uring backend on Linux"

# Test on older kernel (should fallback to epoll)
# Should see: "Falling back to epoll backend"
```

**Step 5: Add io_uring metrics (3 hours)**
```rust
// src/observability/system.rs

pub struct IoBackendMetrics {
    pub backend_type: String,  // "io_uring" or "epoll"
    pub operations_total: u64,
    pub operations_completed: u64,
    pub operations_failed: u64,
}

// Add to Prometheus metrics
impl PrometheusMetrics {
    pub fn io_backend_info(&self) -> String {
        format!(
            "# HELP io_backend_type I/O backend in use\n\
             # TYPE io_backend_type gauge\n\
             io_backend_type{{backend=\"{}\"}} 1\n",
            self.backend_type
        )
    }
}
```

**Deliverable**: io_uring working on Linux with automatic epoll fallback.

---

### **Day 3-4: Fix Failing Tests** (16 hours)

#### Current Status:
```
Test Summary:
  Total: 276 tests
  Passed: 270
  Failed: 6
  Pass Rate: 93.4%
```

#### Failed Tests:
1. File watcher test (timing issue)
2. 5 observability tests (API mismatch)

**Step 1: Identify failing tests (2 hours)**
```bash
# Run tests with verbose output
cargo test --all-features -- --nocapture

# Look for failed tests
# Note: Test names and error messages
```

**Step 2: Fix file watcher test (4 hours)**
```bash
# Likely in src/config/watcher.rs
vim tests/config_watcher_test.rs

# Common issue: Race condition
# Fix: Add proper synchronization
```

Example fix:
```rust
#[tokio::test]
async fn test_config_reload() {
    // BEFORE: Race condition
    write_config("config.yaml", "version: 1").await;
    let watcher = ConfigWatcher::new("config.yaml").await?;

    write_config("config.yaml", "version: 2").await;
    // May not trigger immediately!

    // AFTER: Wait for event
    write_config("config.yaml", "version: 1").await;
    let mut watcher = ConfigWatcher::new("config.yaml").await?;

    write_config("config.yaml", "version: 2").await;

    // Wait for reload event with timeout
    tokio::time::timeout(
        Duration::from_secs(5),
        watcher.wait_for_reload()
    ).await??;

    assert_eq!(watcher.version(), 2);
}
```

**Step 3: Fix observability tests (6 hours)**
```bash
# Check src/observability/*.rs tests
# Likely issue: Prometheus metric format changes

# Run individual tests
cargo test --package highper-gateway --lib observability::tests
```

**Step 4: Run full test suite (2 hours)**
```bash
# Run all tests
cargo test --all-features

# Check coverage
cargo tarpaulin --all-features

# Target: 100% pass rate, 85%+ coverage
```

**Step 5: Add integration tests (2 hours)**
```rust
// tests/integration/io_uring_tests.rs

#[tokio::test]
#[cfg(all(feature = "io-uring", target_os = "linux"))]
async fn test_io_uring_accept() {
    // Test that GLOBAL_IO.accept() works
    // Verify io_uring is being used
}

#[tokio::test]
async fn test_epoll_fallback() {
    // Test fallback to epoll on non-Linux or old kernel
}
```

**Deliverable**: 100% test pass rate (276/276 or more).

---

### **Day 5: Complete Admin API** (8 hours)

#### Missing Endpoints:

```
Status:
✅ GET  /api/health              - Working
✅ GET  /api/stats               - Working
⚠️ GET  /api/metrics             - Needs enhancement
❌ GET  /api/upstreams           - TODO
❌ POST /api/upstreams/:id       - TODO
❌ GET  /api/routes              - TODO
❌ GET  /api/config              - TODO
❌ POST /api/reload              - TODO
❌ GET  /api/pool/stats          - TODO (NEW)
❌ WS   /api/stream              - TODO
```

**Step 1: Implement missing endpoints (6 hours)**
```rust
// src/admin/server.rs

// Add new endpoints
async fn get_upstreams() -> Result<Json<Vec<Upstream>>> {
    // Return list of all upstreams
    let upstreams = STATE.upstreams.read().await;
    Ok(Json(upstreams.values().cloned().collect()))
}

async fn update_upstream(
    Path(id): Path<String>,
    Json(update): Json<UpstreamUpdate>,
) -> Result<Json<Upstream>> {
    // Update upstream configuration
    let mut upstreams = STATE.upstreams.write().await;
    let upstream = upstreams.get_mut(&id).ok_or(NotFound)?;

    upstream.enabled = update.enabled;
    upstream.weight = update.weight;

    Ok(Json(upstream.clone()))
}

async fn get_pool_stats() -> Result<Json<PoolStats>> {
    // Return connection pool statistics
    Ok(Json(PoolStats {
        total_connections: POOL_METRICS.total_connections.load(Ordering::Relaxed),
        idle_connections: POOL_METRICS.idle_connections.load(Ordering::Relaxed),
        reuse_ratio: POOL_METRICS.reuse_ratio(),
    }))
}

// WebSocket for real-time metrics
async fn stream_metrics(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_metrics_stream(socket))
}

async fn handle_metrics_stream(mut socket: WebSocket) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let stats = get_current_stats().await;
        let json = serde_json::to_string(&stats).unwrap();

        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}
```

**Step 2: Add authentication (2 hours)**
```rust
// src/admin/auth.rs

pub struct ApiKeyAuth;

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let api_key = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        if api_key == CONFIG.admin_api_key {
            Ok(ApiKeyAuth)
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

// Use in routes
async fn protected_endpoint(_auth: ApiKeyAuth) -> Result<Json<Value>> {
    // Only accessible with valid API key
}
```

**Deliverable**: Complete Admin API with authentication.

---

## 🧪 Testing Your Progress

### After Day 1-2 (io_uring):
```bash
# Build and run
cargo build --release --features io-uring
./target/release/highper-gateway --config config/example.yaml

# Check logs
tail -f logs/proxy.log | grep -i "io_uring\|epoll"

# Expected: "Using io_uring backend on Linux 5.15+"

# Run benchmark
wrk -t 8 -c 100 -d 30s http://localhost:8080/

# Compare latency with/without io_uring
# Expected: 15-20% reduction
```

### After Day 3-4 (Tests):
```bash
# Run full test suite
cargo test --all-features

# Expected output:
# test result: ok. 276 passed; 0 failed; 0 ignored; 0 measured

# Check coverage
cargo tarpaulin --all-features --out Html
# Expected: >85% coverage
```

### After Day 5 (Admin API):
```bash
# Start server
./target/release/highper-gateway --config config/example.yaml

# Test endpoints
curl http://localhost:9090/api/health
curl http://localhost:9090/api/stats
curl http://localhost:9090/api/upstreams
curl -H "X-API-Key: secret" http://localhost:9090/api/pool/stats

# Test WebSocket
websocat ws://localhost:9090/api/stream
# Expected: Real-time metrics stream
```

---

## 📊 Week 1 Success Metrics

### Technical Metrics:
- ✅ io_uring working on Linux 5.1+
- ✅ Epoll fallback on older systems
- ✅ 100% test pass rate
- ✅ All Admin API endpoints working
- ✅ 15-20% latency reduction measured

### Performance Comparison:

| Metric | Before (Day 0) | After (Day 5) | Improvement |
|--------|----------------|---------------|-------------|
| p99 Latency | ~12ms | <10ms | ~15-20% ✅ |
| Test Pass Rate | 93.4% | 100% | +6.6% ✅ |
| Admin API | 30% complete | 100% complete | +70% ✅ |

---

## 🚀 What's Next (Week 2)

After completing Week 1, you'll move to:

### Week 2: Connection Pool Optimization
- Implement ConnectionPoolMetrics
- Measure current reuse ratio
- Create Grafana dashboard
- Enhanced pool configuration
- Target: >95% connection reuse

### Preparation for Week 2:
```bash
# Install Grafana
docker run -d -p 3000:3000 grafana/grafana

# Install Prometheus
docker run -d -p 9090:9090 prom/prometheus

# Prepare for metrics implementation
```

---

## 🆘 Troubleshooting

### io_uring not available:
```bash
# Check kernel version
uname -r
# Need: Linux 5.1+

# Check io_uring support
cat /boot/config-$(uname -r) | grep IO_URING
# Should see: CONFIG_IO_URING=y
```

### Tests failing:
```bash
# Run individual test with output
cargo test test_name -- --nocapture --test-threads=1

# Check for race conditions
# Add delays or proper synchronization
```

### Admin API not starting:
```bash
# Check port availability
netstat -tulpn | grep 9090

# Check config
cat config/example.yaml | grep admin

# Check logs
tail -f logs/proxy.log | grep admin
```

---

## 📚 Reference Documents

- **Comprehensive TODO List**: `COMPREHENSIVE_TODO_LIST.md`
- **TCP Proxy Plan**: `TCP_PROXY_IMPLEMENTATION_PLAN.md`
- **Architecture Analysis**: `ARCHITECTURE_ANALYSIS_SUMMARY.md`
- **Development Roadmap**: `UPDATED_DEVELOPMENT_ROADMAP.md`

---

**Good luck with Week 1!** 🚀

Remember:
- Take breaks every 2 hours
- Commit frequently with clear messages
- Test incrementally, not all at once
- Document any issues you encounter
- Ask questions if stuck >30 minutes

**End of Week 1 Goal**: Production-ready foundation with io_uring, 100% tests, and complete Admin API.
