# Comprehensive TODO List - Rust Reverse Proxy & API Gateway
## Complete Development Roadmap with All Pending Features

**Date**: November 9, 2025
**Status**: Post-Plugin System Completion
**Current Completion**: ~85-92% of features implemented

---

## 📊 CURRENT STATE SUMMARY

### ✅ **COMPLETED** (As of Nov 9, 2025)
- ✅ **Stage 0**: Compression Adapter Pattern (COMPLETE)
- ✅ **Stage 1**: HTTP/3 Integration + Middleware (COMPLETE)
- ✅ **Stage 2**: Benchmarking Suite (COMPLETE)
- ✅ **Stage 3**: Maglev Load Balancing + Geographic LB (COMPLETE)
- ✅ **Plugin System**: WASM + FFI + Hot Reload (COMPLETE - Nov 9)
- ✅ **WAF**: Multi-Engine Implementation (COMPLETE)
- ✅ Core HTTP/1.1, HTTP/2, HTTP/3 support
- ✅ TLS/mTLS with ACME, OCSP stapling
- ✅ 7 Load balancing algorithms
- ✅ Admin API basics
- ✅ Observability (Prometheus, OpenTelemetry basics)

### ⚠️ **PARTIALLY IMPLEMENTED**
- ⚠️ **io_uring Runtime Integration** - Files exist but not integrated into server
- ⚠️ **Admin API** - Basic endpoints exist, needs completion
- ⚠️ **Test Suite** - 270/276 passing (93.4%, needs fixes)
- ⚠️ **GraphQL Gateway** - 85% complete
- ⚠️ **Connection Pool** - Basic pooling, needs Pingora-level optimization
- ⚠️ **Runtime Integration** - ProxyState not wired to LoadBalancer

### ❌ **NOT IMPLEMENTED**
- ❌ **Caddy-like Configuration DSL**
- ❌ **TCP Proxy (Layer 4)** for database load balancing
- ❌ **Zero-Copy I/O optimizations**
- ❌ **SIMD optimizations**
- ❌ **Lock-free data structures**
- ❌ **Admin Dashboard UI**
- ❌ **Multi-process connection pool coordination**

---

## 🔥 PRIORITY 1: CRITICAL PRODUCTION READINESS (4-6 Weeks)

### **Week 1: io_uring Runtime Integration** ⚡ URGENT

Status: Files exist (`io_uring_shim.rs`, `io_backend.rs`, `epoll_backend.rs`) but NOT integrated

#### Day 1-2: Fix HybridTcpStream & io_uring Integration
**Files to Modify:**
- `src/runtime/hybrid_stream.rs` - Fix borrow checker issues
- `src/proxy/server.rs` - Update accept loop
- `src/runtime/io_uring_backend.rs` - Complete implementation

**Tasks:**
- [ ] Fix HybridTcpStream borrow checker errors (currently commented out)
- [ ] Update server accept loop to use `GLOBAL_IO.accept()`
- [ ] Integrate io_uring read/write into handler
- [ ] Add io_uring stats to observability metrics
- [ ] Test fallback to epoll when io_uring unavailable
- [ ] Test on Linux 5.1+ kernel (io_uring support)
- [ ] Test on older kernels (epoll fallback)

**Success Criteria:**
- ✅ Server accepts connections via GLOBAL_IO adapter
- ✅ io_uring used by default on Linux 5.1+
- ✅ Graceful fallback to epoll on older kernels/macOS
- ✅ No connection drops during operation
- ✅ 15-20% latency reduction measured
- ✅ Stats show backend being used (epoll vs io_uring)

**Current Code Status:**
```rust
// src/runtime/mod.rs:18-20 - Currently commented out!
// TODO: HybridTcpStream has borrow checker issues, will fix in Day 3
// #[cfg(all(feature = "io-uring", target_os = "linux"))]
// mod hybrid_stream;
```

**References:**
- `COMPREHENSIVE_DEVELOPMENT_PLAN_V2.md` - Line 971-985 (Priority 1B)
- Existing implementation in `src/runtime/io_uring_shim.rs` (16,840 bytes)

---

#### Day 3-4: Test Suite Fixes & 100% Pass Rate
**Current Status**: 270/276 passing (6 failing)

**Files to Fix:**
- `src/config/watcher.rs` - File watcher test timing issues
- `src/observability/*.rs` - 5 failing API mismatch tests
- Integration tests for compression adapter

**Tasks:**
- [ ] Fix flaky file watcher test (already attempted, needs more work)
- [ ] Fix 5 observability test failures
- [ ] Add compression adapter integration tests
- [ ] Improve test isolation
- [ ] Add io_uring integration tests
- [ ] Add TCP proxy tests (when implemented)

**Success Criteria:**
- ✅ 100% test pass rate (276/276 or more)
- ✅ No flaky tests
- ✅ Test coverage maintained at 85%+
- ✅ All new features have tests

---

#### Day 5: Complete Admin API Endpoints
**Current Status**: Basic endpoints work, many TODOs

**Files to Modify:**
- `src/admin/server.rs` - Complete all endpoints
- `src/admin/backends.rs` - Backend management
- `src/admin/cache.rs` - Cache management
- `src/admin/metrics.rs` - Enhanced metrics

**Missing Endpoints:**
```
✅ GET  /api/health              - Implemented
✅ GET  /api/stats               - Implemented
✅ GET  /api/stats/compression   - Implemented (Stage 1)
⚠️ GET  /api/metrics             - Partial (needs enhancement)
❌ GET  /api/upstreams           - TODO
❌ POST /api/upstreams/:id       - TODO
❌ GET  /api/routes              - TODO
❌ POST /api/routes/:id          - TODO
❌ GET  /api/config              - TODO
❌ POST /api/reload              - TODO
❌ GET  /api/certificates        - TODO
❌ POST /api/certificates/renew  - TODO
❌ GET  /api/compressors         - TODO
❌ GET  /api/pool/stats          - TODO (NEW - connection pool)
❌ WS   /api/stream              - TODO (real-time metrics)
```

**Tasks:**
- [ ] Implement all missing endpoints
- [ ] Add authentication (JWT or API key)
- [ ] Add OpenAPI/Swagger documentation
- [ ] Add WebSocket endpoint for real-time metrics
- [ ] Add connection pool statistics endpoint
- [ ] Add io_uring backend stats endpoint

**Success Criteria:**
- ✅ All endpoints implemented and working
- ✅ Authentication enforced
- ✅ OpenAPI spec auto-generated
- ✅ WebSocket streaming functional
- ✅ Postman collection updated

---

### **Week 2: Connection Pool Optimization (Pingora-Level)**

#### Day 1-2: Connection Pool Metrics & Observability
**Goal**: Measure current connection reuse ratio

**Files to Create:**
- `src/proxy/pool_metrics.rs` (NEW)
- `src/observability/pool_dashboard.rs` (NEW)

**Files to Modify:**
- `src/proxy/client.rs` - Instrument connection creation/reuse
- `src/observability/metrics.rs` - Add pool metrics

**Tasks:**
- [ ] Implement `ConnectionPoolMetrics` struct
- [ ] Track connection creation events
- [ ] Track connection reuse events
- [ ] Calculate reuse ratio (target: >99%)
- [ ] Track handshakes per second
- [ ] Track per-backend pool statistics
- [ ] Add Prometheus metrics export
- [ ] Create Grafana dashboard
- [ ] Add Prometheus alerts (reuse ratio <90%)

**Metrics to Track:**
```rust
pub struct ConnectionPoolMetrics {
    total_connections_created: AtomicU64,
    total_connections_reused: AtomicU64,
    total_connections_closed: AtomicU64,
    active_connections: AtomicU64,
    idle_connections: AtomicU64,
    tcp_handshakes_total: AtomicU64,
    tls_handshakes_total: AtomicU64,
    handshake_time_total_ms: AtomicU64,
    per_backend_stats: DashMap<String, BackendPoolStats>,
}
```

**Success Criteria:**
- ✅ Can measure exact connection reuse ratio
- ✅ Grafana dashboard shows real-time stats
- ✅ Alerts trigger when reuse ratio drops
- ✅ Per-backend statistics available

**References:**
- `UPDATED_DEVELOPMENT_ROADMAP.md` - Phase 1, Week 1
- `ARCHITECTURE_ANALYSIS_SUMMARY.md` - Connection pool analysis

---

#### Day 3-4: Enhanced Pool Configuration
**Goal**: Improve pool efficiency

**Files to Modify:**
- `src/proxy/client.rs` - Enhanced pool config
- `src/config/schema.rs` - Add pool configuration

**Tasks:**
- [ ] Add `ConnectionPoolConfig` to schema
- [ ] Implement connection pre-warming
- [ ] Add min_idle connection maintenance
- [ ] Add max_lifetime enforcement
- [ ] Implement periodic health checks for idle connections
- [ ] Add pool configuration to YAML schema

**Configuration:**
```yaml
connection_pool:
  max_idle_per_host: 100
  idle_timeout: 90s
  max_lifetime: 300s       # NEW - force refresh
  min_idle: 10             # NEW - keep warm
  health_check_interval: 30s  # NEW
  prewarm_connections: true   # NEW
  prewarm_count: 10        # NEW
```

**Success Criteria:**
- ✅ Reuse ratio > 95%
- ✅ Cold start latency eliminated
- ✅ Connection freshness maintained
- ✅ No stale connections

---

#### Day 5: Multi-Process Pool Coordination (OPTIONAL)
**Goal**: Share pool across processes

**Files to Create:**
- `src/proxy/pool_coordinator.rs` (NEW)
- `src/proxy/shared_pool.rs` (NEW)

**Approach**: Redis-based coordination

**Tasks:**
- [ ] Implement `PoolCoordinator` using Redis
- [ ] Register/deregister connections globally
- [ ] Check global pool limits before creating connections
- [ ] Aggregate statistics across processes
- [ ] Test with multiple proxy processes

**Success Criteria:**
- ✅ Multiple processes coordinate pool usage
- ✅ Global limits enforced
- ✅ No over-provisioning of connections

**Note**: This is OPTIONAL - only needed if deploying with multiple processes. Single process deployment already has shared pool.

---

### **Week 3: Runtime Integration & ProxyState Wiring**

#### Day 1-2: ProxyState + LoadBalancer Integration
**Goal**: Wire ProxyState into runtime

**Files to Modify:**
- `src/proxy/loadbalancer.rs` - Accept ProxyState
- `src/proxy/handler.rs` - Pass ProxyState
- `src/main.rs` - Initialize ProxyState

**Tasks:**
- [ ] Pass ProxyState to LoadBalancer initialization
- [ ] Update backend selection to check enabled/draining flags
- [ ] Track active connections in ProxyState
- [ ] Skip disabled backends during selection
- [ ] Update connection counts on request start/end

**Success Criteria:**
- ✅ Admin API can enable/disable backends
- ✅ Draining backends stop receiving new requests
- ✅ Active connection counts accurate
- ✅ Backend selection respects ProxyState flags

**References:**
- `NEXT_STEPS.md` - Priority 1: Runtime Integration

---

#### Day 3-4: Health Checker Integration
**Goal**: Update ProxyState from health checks

**Files to Modify:**
- `src/proxy/health.rs` - Update ProxyState
- `src/state/proxy_state.rs` - Store health history

**Tasks:**
- [ ] Update health check results in ProxyState
- [ ] Store health check history (last 100 checks)
- [ ] Trigger health checks via Admin API
- [ ] Add health status to backend selection logic
- [ ] Add health status to Admin API responses

**Success Criteria:**
- ✅ Failed health checks mark backends unhealthy
- ✅ Unhealthy backends removed from rotation
- ✅ Health history accessible via API
- ✅ Manual health checks work via API

---

#### Day 5: Request Handler Metrics Integration
**Goal**: Track all metrics in ProxyState

**Files to Modify:**
- `src/proxy/handler.rs` - Track metrics
- `src/state/proxy_state.rs` - Store metrics

**Tasks:**
- [ ] Increment request counters on every request
- [ ] Record response status codes
- [ ] Measure response times
- [ ] Track error rates
- [ ] Update backend statistics
- [ ] Add per-route statistics

**Success Criteria:**
- ✅ Real-time statistics in Admin API
- ✅ Accurate request counts
- ✅ Response time percentiles
- ✅ Error rate tracking

---

### **Week 4: Enhanced Observability**

#### Day 1-3: Per-Route & Per-Backend Metrics
**Files to Create:**
- `src/observability/histogram.rs` (NEW)
- `src/observability/route_metrics.rs` (NEW)

**Tasks:**
- [ ] Implement histogram data structure for latency
- [ ] Track P50, P95, P99 latencies
- [ ] Add route identifier to request context
- [ ] Track metrics by route pattern
- [ ] Track requests per backend
- [ ] Measure backend response times
- [ ] Calculate backend error rates
- [ ] Monitor connection pool usage per backend

**Success Criteria:**
- ✅ Latency percentiles available
- ✅ Per-route RPS tracked
- ✅ Per-backend error rates visible
- ✅ Prometheus metrics exported

---

#### Day 4-5: Advanced Prometheus & Grafana
**Files to Create:**
- `grafana/advanced-dashboard.json` (NEW)
- `prometheus/advanced-alerts.yml` (NEW)

**Tasks:**
- [ ] Create request duration histograms
- [ ] Add connection pool gauges
- [ ] Add cache hit/miss ratio metrics
- [ ] Add rate limit usage metrics
- [ ] Create comprehensive Grafana dashboard
- [ ] Add alerting rules
- [ ] Add SLO/SLI tracking

**Success Criteria:**
- ✅ Production-grade dashboards
- ✅ Alerting on critical metrics
- ✅ SLO compliance tracked

---

## 🚀 PRIORITY 2: LAYER 4 TCP LOAD BALANCING (4-6 Weeks) ⚡ CRITICAL

**GOAL**: Match or exceed HAProxy, Nginx, and Nginx Plus performance for TCP load balancing

### **Strategic Importance**
Layer 4 TCP load balancing is a **CRITICAL** feature for production deployments. This implementation must support:
- ✅ **HTTP/HTTPS** - Web traffic (can also use Layer 7, but Layer 4 for raw performance)
- ✅ **MySQL** - Database cluster load balancing and read replica distribution
- ✅ **PostgreSQL** - Primary/replica routing and connection pooling
- ✅ **Redis** - Cache cluster and sentinel support
- ✅ **Any TCP-based service** - Universal protocol support

### **Performance Targets vs Competition**

| Metric | HAProxy | Nginx Plus | Our Target | Status |
|--------|---------|------------|------------|--------|
| **Latency Overhead** | <0.5ms | <0.8ms | <0.5ms p99 | 🔴 Not Started |
| **Throughput** | 1M+ conn/sec | 800k+ conn/sec | 1M+ conn/sec | 🔴 Not Started |
| **Connection Reuse** | 95%+ | 90%+ | 99%+ | 🔴 Not Started |
| **Memory per Connection** | ~4KB | ~6KB | <4KB | 🔴 Not Started |
| **SSL/TLS Passthrough** | Yes | Yes | Yes | 🔴 Not Started |
| **Health Checks** | Advanced | Advanced | Advanced | 🔴 Not Started |

**Reference Benchmarks**:
- HAProxy: https://www.haproxy.com/blog/haproxy-forwards-over-2-million-http-requests-per-second-on-a-single-aws-arm-instance/
- Nginx: https://www.nginx.com/blog/testing-the-performance-of-nginx-and-nginx-plus-web-servers/

### **Week 5-6: TCP Proxy Implementation**

#### Week 5, Day 1-2: TCP Proxy Core
**Files to Create:**
- `src/tcp/mod.rs` (NEW)
- `src/tcp/proxy.rs` (NEW)
- `src/tcp/listener.rs` (NEW)
- `src/config/tcp.rs` (NEW)

**Tasks:**
- [ ] Implement `TcpProxy` struct
- [ ] Add TCP listener
- [ ] Implement bidirectional TCP forwarding
- [ ] Integrate with existing LoadBalancer
- [ ] Add TCP-specific metrics
- [ ] Add configuration schema

**Configuration:**
```yaml
tcp_proxies:
  - name: mysql-proxy
    listen: "0.0.0.0:3306"
    mode: tcp
    upstream:
      name: mysql-cluster
      load_balancing:
        algorithm: least_connections
      servers:
        - url: "mysql1.internal:3306"
        - url: "mysql2.internal:3306"
```

**Success Criteria:**
- ✅ TCP connections forwarded to backends
- ✅ Load balancing working
- ✅ Metrics tracking connections

**References:**
- `UPDATED_DEVELOPMENT_ROADMAP.md` - Phase 2: TCP Proxy

---

#### Week 5, Day 3-4: Database Protocol Support
**Files to Create:**
- `src/tcp/protocols/mod.rs` (NEW)
- `src/tcp/protocols/mysql.rs` (NEW)
- `src/tcp/protocols/postgres.rs` (NEW)

**Tasks:**
- [ ] Implement MySQL protocol detection
- [ ] Implement PostgreSQL protocol detection
- [ ] Add database-specific health checks
- [ ] Add connection string parsing
- [ ] Test with real MySQL server
- [ ] Test with real PostgreSQL server

**Success Criteria:**
- ✅ MySQL load balancing working
- ✅ PostgreSQL load balancing working
- ✅ Database health checks functional

---

#### Week 5, Day 5: TCP Testing & Documentation
**Files to Create:**
- `tests/tcp_proxy_tests.rs` (NEW)
- `docs/TCP_PROXY_GUIDE.md` (NEW)
- `examples/tcp-mysql-proxy.yaml` (NEW)
- `examples/tcp-postgres-proxy.yaml` (NEW)

**Tasks:**
- [ ] Write TCP proxy integration tests
- [ ] Write documentation
- [ ] Create example configurations
- [ ] Add troubleshooting guide

---

#### Week 6, Day 1-2: TCP Connection Pooling
**Files to Modify:**
- `src/tcp/proxy.rs` - Add pooling
- `src/tcp/pool.rs` (NEW)

**Tasks:**
- [ ] Implement TCP connection pool
- [ ] Reuse connections to backends
- [ ] Add pool health monitoring
- [ ] Implement automatic reconnection

**Success Criteria:**
- ✅ Connections reused
- ✅ Connection pool metrics available
- ✅ Performance improvement measured

---

#### Week 6, Day 3-5: TCP Production Features & Benchmarking
**Tasks:**
- [ ] SSL/TLS passthrough for encrypted databases
- [ ] Connection limiting (per client, per backend)
- [ ] Bandwidth throttling
- [ ] Timeout configuration
- [ ] Benchmark with sysbench (MySQL)
- [ ] Benchmark with pgbench (PostgreSQL)
- [ ] Measure proxy overhead (<0.5ms target)
- [ ] Implement HAProxy-level health check features
- [ ] Add connection draining support
- [ ] Add session persistence (sticky sessions)

**Success Criteria:**
- ✅ <0.5ms p99 proxy overhead (HAProxy level)
- ✅ TLS passthrough working
- ✅ Production-ready features complete
- ✅ Performance benchmarks meet or exceed targets

---

#### TCP Proxy Benchmarking Methodology

**Benchmark Environment:**
- Hardware: AWS c5.2xlarge or similar (8 vCPU, 16GB RAM)
- OS: Ubuntu 22.04 LTS with Linux 5.15+ (io_uring support)
- Network: 10 Gbps

**Benchmark 1: MySQL Load Balancing**
```bash
# Tool: sysbench
# Test: OLTP read/write workload
sysbench /usr/share/sysbench/oltp_read_write.lua \
  --mysql-host=localhost \
  --mysql-port=3306 \
  --mysql-user=test \
  --mysql-password=test \
  --mysql-db=sbtest \
  --tables=10 \
  --table-size=1000000 \
  --threads=64 \
  --time=300 \
  --report-interval=10 \
  run
```

**Metrics to Measure:**
- Queries per second (QPS)
- Latency: min, avg, p95, p99, max
- Connection overhead vs direct connection
- CPU usage
- Memory usage

**Target:**
- <0.5ms additional latency vs direct connection
- >95% of direct connection QPS
- <2% CPU overhead per 10k connections

---

**Benchmark 2: PostgreSQL Load Balancing**
```bash
# Tool: pgbench
# Test: TPC-B like workload
pgbench -h localhost -p 5432 -U postgres \
  -c 64 -j 8 -T 300 -P 10 \
  -M prepared \
  testdb
```

**Metrics to Measure:**
- Transactions per second (TPS)
- Latency distribution
- Connection establishment time
- Read/write split accuracy
- Failover time (if primary fails)

**Target:**
- <0.5ms proxy overhead
- >98% of direct connection TPS
- <100ms failover time

---

**Benchmark 3: Redis Load Balancing**
```bash
# Tool: redis-benchmark
redis-benchmark -h localhost -p 6379 \
  -c 100 -n 1000000 \
  -t get,set,incr,lpush,rpush,lpop,rpop,sadd,hset,spop,zadd,zpopmin,lrange \
  -P 16 \
  -q
```

**Metrics:**
- Operations per second
- Latency percentiles
- Pipeline efficiency
- Memory usage

**Target:**
- >500k ops/sec
- <0.3ms p99 latency
- <1% memory overhead

---

**Benchmark 4: Raw TCP Performance**
```bash
# Tool: iperf3
# Test: Maximum throughput
iperf3 -c backend_server -p 8080 -t 60 -P 10
```

**Metrics:**
- Maximum throughput (Gbps)
- Packet loss
- CPU usage at saturation

**Target:**
- >9 Gbps on 10 Gbps link
- 0% packet loss
- <80% CPU usage

---

**Benchmark 5: Connection Throughput**
```bash
# Tool: wrk (for HTTP) + custom TCP tool
# Test: New connections per second
wrk -t 8 -c 1000 -d 60s --latency http://localhost:8080/
```

**Metrics:**
- New connections per second
- Connection establishment time
- TIME_WAIT socket count
- Connection reuse ratio

**Target:**
- >1M new connections/sec
- <0.2ms connection establishment
- >99% connection reuse

---

**Comparison Benchmarks vs HAProxy/Nginx:**

**Setup:**
```yaml
# Test same workload across:
# 1. highper-gateway (our implementation)
# 2. HAProxy 2.8+
# 3. Nginx Plus R30 (or Nginx 1.25+)
# 4. Direct connection (baseline)
```

**Workload:**
- 10,000 concurrent connections
- 100,000 requests/sec
- 50/50 read/write mix
- 1KB average payload
- 60 second test duration

**Comparison Matrix:**

| Proxy | p50 Latency | p99 Latency | Throughput | CPU % | Memory |
|-------|-------------|-------------|------------|-------|--------|
| Direct | 0.5ms | 1.2ms | 100k/s | - | - |
| HAProxy | 0.7ms | 1.8ms | 98k/s | 15% | 450MB |
| Nginx Plus | 0.9ms | 2.3ms | 95k/s | 18% | 520MB |
| **highper-gateway** | **<0.7ms** | **<1.8ms** | **>98k/s** | **<15%** | **<450MB** |

**Success Criteria:**
- ✅ p99 latency ≤ HAProxy
- ✅ Throughput ≥ HAProxy
- ✅ CPU usage ≤ HAProxy
- ✅ Memory usage ≤ HAProxy
- ✅ Feature parity with HAProxy essentials

---

**HAProxy Features to Implement:**

**Essential (Week 5-6):**
- [ ] Layer 4 TCP proxy
- [ ] Health checks (TCP, HTTP, custom scripts)
- [ ] Load balancing algorithms (round-robin, least-conn, source hash)
- [ ] Connection limits
- [ ] Timeout configuration
- [ ] SSL/TLS passthrough
- [ ] Stats endpoint
- [ ] Connection draining

**Advanced (Week 7+):**
- [ ] Stick tables (session persistence)
- [ ] Rate limiting
- [ ] ACLs (Access Control Lists)
- [ ] Header inspection (Layer 7 on TCP mode)
- [ ] Connection queuing
- [ ] Backend server weights
- [ ] Slow start for new backends
- [ ] Agent-based health checks

---

**Nginx Plus Features to Implement:**

**Essential (Week 5-6):**
- [ ] TCP/UDP load balancing
- [ ] Health checks (passive, active)
- [ ] SSL termination and passthrough
- [ ] Connection limiting
- [ ] Dynamic reconfiguration (Admin API)
- [ ] Statistics and monitoring

**Advanced (Week 7+):**
- [ ] Session persistence (sticky learn, cookie, route)
- [ ] Active health monitoring with API
- [ ] Dynamic upstream configuration
- [ ] TCP/UDP load balancing with health checks
- [ ] Bandwidth limiting
- [ ] Geographic load balancing (already implemented!)

---

**Performance Testing Commands:**

```bash
# 1. Build optimized release
cargo build --release --features io-uring

# 2. Run with production config
./target/release/highper-gateway --config tcp-benchmark.yaml

# 3. MySQL benchmark
sysbench oltp_read_write --mysql-host=localhost run

# 4. PostgreSQL benchmark
pgbench -c 100 -j 10 -T 300 testdb

# 5. Compare with HAProxy
# Start HAProxy with equivalent config
haproxy -f haproxy-tcp.cfg
# Run same benchmarks
# Compare results

# 6. Generate performance report
./scripts/benchmark-report.sh > TCP_PERFORMANCE_REPORT.md
```

---

**Success Metrics Summary:**

✅ **Latency**: <0.5ms p99 proxy overhead (match HAProxy)
✅ **Throughput**: >1M connections/sec (exceed Nginx Plus)
✅ **Reuse**: >99% connection reuse (match Pingora)
✅ **Memory**: <4KB per connection (match HAProxy)
✅ **Features**: All HAProxy essential features
✅ **Stability**: 99.99% uptime under load
✅ **Documentation**: Complete TCP proxy guide

---

### **Week 7-8: Caddy-like Configuration DSL**

#### Week 7: DSL Design & Parser
**Files to Create:**
- `src/config/dsl/mod.rs` (NEW)
- `src/config/dsl/parser.rs` (NEW)
- `src/config/dsl/ast.rs` (NEW)
- `src/config/dsl/compiler.rs` (NEW)

**Goal**: Simplify configuration from 45+ lines YAML to 3-5 lines

**Current YAML** (45 lines):
```yaml
server:
  bind: ["0.0.0.0:80", "0.0.0.0:443"]
  protocols: [http1, http2, http3]

upstreams:
  - name: backend
    load_balancing:
      algorithm: round_robin
    servers:
      - url: "http://10.0.1.10:8080"
        weight: 1
      - url: "http://10.0.1.11:8080"
        weight: 1
    health_check:
      enabled: true
      path: /health
      interval: 10s

routes:
  - path: /*
    upstream: backend

tls:
  acme:
    enabled: true
    email: admin@example.com
```

**Target DSL** (3-5 lines):
```
example.com
reverse_proxy 10.0.1.10:8080 10.0.1.11:8080
```

**Or with more options:**
```
api.example.com {
    reverse_proxy {
        to 10.0.1.10:8080 10.0.1.11:8080 10.0.1.12:8080
        lb_policy maglev
        health /health 10s
    }
    rate_limit 100/s
    compress br zstd gzip
}
```

**Tasks:**
- [ ] Design DSL syntax (Caddyfile-inspired)
- [ ] Implement parser (using nom or pest)
- [ ] Define AST (Abstract Syntax Tree)
- [ ] Implement compiler (DSL -> YAML)
- [ ] Add validation
- [ ] Add syntax highlighting (VSCode extension)

**Success Criteria:**
- ✅ Simple configs 10x shorter
- ✅ Human-readable and writable
- ✅ Backwards compatible with YAML
- ✅ Error messages helpful

---

#### Week 8: DSL Integration & Testing
**Tasks:**
- [ ] Integrate DSL parser into config loading
- [ ] Support both DSL and YAML
- [ ] Add config validation
- [ ] Write DSL examples
- [ ] Write migration guide
- [ ] Add DSL to CLI (--config-format dsl)

**Success Criteria:**
- ✅ Can load DSL configs
- ✅ Can mix DSL and YAML (include statements)
- ✅ Comprehensive examples
- ✅ Migration tooling

---

### **Week 9-10: GraphQL Gateway Completion**

**Current Status**: 85% complete

**Files to Modify:**
- `src/gateway/graphql/stitcher.rs` - Complete schema stitching
- `src/gateway/graphql/executor.rs` - Complete query execution
- `src/gateway/graphql/cache.rs` - Complete caching

**Missing Features:**
- [ ] Complete schema stitching implementation
- [ ] Add query batching
- [ ] Add DataLoader pattern
- [ ] Add persisted queries
- [ ] Add subscription support (WebSocket)
- [ ] Add GraphQL playground
- [ ] Add introspection caching

**Success Criteria:**
- ✅ 100% feature complete
- ✅ Production-ready
- ✅ Performance optimized

---

## ⚡ PRIORITY 3: PERFORMANCE OPTIMIZATIONS (4-6 Weeks)

### **Week 11-12: Zero-Copy I/O**

**Goal**: Eliminate unnecessary memory copies

**Files to Modify:**
- `src/proxy/handler.rs` - Use zero-copy techniques
- `src/http/mod.rs` - Zero-copy response building

**Techniques:**
- [ ] Use `sendfile()` for static files
- [ ] Use `splice()` for proxying (Linux)
- [ ] Avoid cloning `Bytes` unnecessarily
- [ ] Use `Arc<[u8]>` for shared buffers
- [ ] Implement `pin_project` for streaming

**Success Criteria:**
- ✅ 10-15% throughput improvement
- ✅ 20-30% reduction in allocations
- ✅ Memory usage reduced

---

### **Week 13-14: SIMD Optimizations**

**Goal**: Vectorize hot paths

**Files to Create:**
- `src/utils/simd_headers.rs` (NEW)
- `src/utils/simd_url.rs` (NEW)

**Tasks:**
- [ ] SIMD header parsing (using `simd-json`)
- [ ] SIMD URL parsing
- [ ] SIMD string matching for routing
- [ ] SIMD compression (if available)

**Success Criteria:**
- ✅ 5-10% improvement in header parsing
- ✅ 10-15% improvement in routing

---

### **Week 15-16: Lock-Free Data Structures**

**Goal**: Replace locking structures

**Files to Modify:**
- Replace `DashMap` with lock-free alternatives
- Use `crossbeam` lock-free queues
- Implement lock-free connection pool

**Tasks:**
- [ ] Benchmark current DashMap usage
- [ ] Implement lock-free alternatives
- [ ] Measure contention reduction
- [ ] Profile lock wait times

**Success Criteria:**
- ✅ Reduced lock contention
- ✅ Better multi-core scaling
- ✅ Higher concurrent request throughput

---

## 🔧 PRIORITY 4: TOOLING & UX (2-3 Weeks)

### **Week 17: Admin Dashboard UI**

**Goal**: Web-based management interface

**Technology**: htmx + Tailwind CSS (or similar)

**Files to Create:**
- `admin-ui/` directory (NEW)
- Serve static files from Admin API

**Features:**
- [ ] Real-time metrics dashboard
- [ ] Backend management (enable/disable/drain)
- [ ] Configuration editor
- [ ] Log viewer
- [ ] Certificate management
- [ ] Health check viewer

**Success Criteria:**
- ✅ Functional web UI
- ✅ Real-time updates
- ✅ Mobile responsive

---

### **Week 18: CLI Enhancements**

**Files to Modify:**
- `src/main.rs` - Enhanced CLI

**Features:**
- [ ] `proxy validate` - Validate configuration
- [ ] `proxy test` - Test backend connectivity
- [ ] `proxy reload` - Hot reload config
- [ ] `proxy status` - Show runtime status
- [ ] `proxy cert renew` - Force certificate renewal
- [ ] `proxy benchmark` - Built-in benchmarking

**Success Criteria:**
- ✅ All commands working
- ✅ Good UX with colors/formatting
- ✅ Man pages generated

---

## 📦 PRIORITY 5: PRODUCTION PACKAGING (1-2 Weeks)

### **Week 19: Packaging & Distribution**

**Tasks:**
- [ ] Create optimized Docker image
- [ ] Create Kubernetes manifests
- [ ] Create Helm chart
- [ ] Create systemd service file
- [ ] Create DEB package
- [ ] Create RPM package
- [ ] Create Homebrew formula
- [ ] CI/CD for releases

**Success Criteria:**
- ✅ Easy deployment on all platforms
- ✅ Auto-update mechanism
- ✅ Monitoring integration examples

---

## 📚 DOCUMENTATION (Ongoing)

### Documentation TODO:
- [ ] **Getting Started Guide** - Quick start for new users
- [ ] **Configuration Reference** - Complete YAML/DSL reference
- [ ] **Performance Tuning Guide** - Optimization best practices
- [ ] **Deployment Guide** - Production deployment guide
- [ ] **Security Hardening** - Security best practices
- [ ] **Troubleshooting Guide** - Common issues and solutions
- [ ] **API Reference** - Admin API documentation
- [ ] **Plugin Development Guide** - Already exists, update
- [ ] **TCP Proxy Guide** - Database load balancing
- [ ] **Migration Guides**:
  - [ ] From nginx
  - [ ] From HAProxy
  - [ ] From Caddy
  - [ ] From Envoy

---

## 📊 OVERALL TIMELINE SUMMARY

| Priority | Weeks | Features | Status |
|----------|-------|----------|--------|
| **P1: Critical** | 1-4 | io_uring, Tests, Admin API, Pool, Runtime Integration, Metrics | 🔴 Not Started |
| **P2: Features** | 5-10 | **TCP Proxy (⚡ CRITICAL)**, Caddy DSL, GraphQL | 🔴 Not Started |
| **P3: Performance** | 11-16 | Zero-copy, SIMD, Lock-free | 🔴 Not Started |
| **P4: Tooling** | 17-18 | Admin UI, CLI | 🔴 Not Started |
| **P5: Packaging** | 19 | Docker, K8s, packages | 🔴 Not Started |

**Total Timeline**: 19-24 weeks (~5-6 months)

### Priority Breakdown

**Weeks 1-4 (Priority 1)**: Foundation & Stability
- Get io_uring working → 15-20% latency reduction
- Fix all tests → 100% pass rate
- Complete Admin API → Production-ready management
- Optimize connection pool → Pingora-level reuse (99%+)

**Weeks 5-6 (Priority 2A - CRITICAL)**: Layer 4 TCP Proxy
- **THIS IS A CRITICAL FEATURE** for production deployments
- Enables database load balancing (MySQL, PostgreSQL, Redis)
- Must match/exceed HAProxy and Nginx Plus performance
- Opens new use cases and markets
- Performance target: <0.5ms p99 latency overhead

**Weeks 7-10 (Priority 2B)**: UX & Features
- Caddy-like DSL → 10x simpler configs
- GraphQL Gateway completion → 100% feature complete

**Weeks 11-16 (Priority 3)**: Performance Optimizations
- Zero-copy I/O → 10-15% throughput gain
- SIMD → 5-10% parsing speedup
- Lock-free structures → Better multi-core scaling

**Weeks 17-19 (Priority 4-5)**: Tooling & Deployment
- Admin UI, CLI, Packaging

---

## 🎯 SUCCESS METRICS

### Production Readiness Checklist:
- [ ] All tests passing (100%)
- [ ] io_uring working on Linux
- [ ] Connection reuse ratio > 99% (Pingora-level)
- [ ] Admin API complete
- [ ] **TCP proxy working for HTTP/HTTPS/MySQL/PostgreSQL/Redis**
- [ ] Caddy-like DSL working
- [ ] Documentation complete
- [ ] Deployment packages available
- [ ] Performance benchmarks met (see below)

### HTTP/HTTPS Performance Benchmarks:
- [ ] >100k req/sec (HTTP/1.1)
- [ ] >150k req/sec (HTTP/2)
- [ ] >200k req/sec (HTTP/3)
- [ ] <10ms p99 latency (HTTP)
- [ ] >99% connection reuse ratio
- [ ] <500ms p99 TLS handshake

### TCP Proxy Performance Benchmarks (⚡ CRITICAL):

**Layer 4 Load Balancing - Must Match/Exceed HAProxy & Nginx Plus:**

#### MySQL Load Balancing:
- [ ] <0.5ms p99 proxy overhead (vs direct connection)
- [ ] >95% of direct connection QPS
- [ ] >10,000 queries/sec per backend
- [ ] <100ms failover time on backend failure
- [ ] Connection pooling with >95% reuse ratio
- [ ] <2% CPU overhead per 10k connections

#### PostgreSQL Load Balancing:
- [ ] <0.5ms p99 proxy overhead
- [ ] >98% of direct connection TPS
- [ ] >5,000 transactions/sec per backend
- [ ] Read/write split working correctly
- [ ] Primary/replica failover <100ms
- [ ] Prepared statement routing working

#### Redis Load Balancing:
- [ ] <0.3ms p99 latency
- [ ] >500k operations/sec
- [ ] Pipeline efficiency >95%
- [ ] Cluster mode support
- [ ] Sentinel failover support

#### Raw TCP Performance:
- [ ] >1M new connections/sec
- [ ] <0.2ms connection establishment
- [ ] >9 Gbps throughput on 10 Gbps link
- [ ] 0% packet loss under normal load
- [ ] <80% CPU usage at saturation
- [ ] <4KB memory per connection

#### Comparative Benchmarks:
**Target: Match or exceed these metrics:**

| Metric | HAProxy 2.8+ | Nginx Plus R30 | highper-gateway Target |
|--------|--------------|----------------|-------------------|
| p99 Latency | 1.8ms | 2.3ms | **≤1.8ms** |
| Throughput | 98k req/s | 95k req/s | **≥98k req/s** |
| CPU Usage | 15% | 18% | **≤15%** |
| Memory | 450MB | 520MB | **≤450MB** |
| Connections/sec | 80k | 60k | **≥80k** |
| Connection Reuse | 95% | 90% | **≥99%** |

### Feature Parity Goals:

#### HAProxy Feature Parity (Essential):
- [ ] Layer 4 TCP proxy - **CRITICAL**
- [ ] Health checks (TCP, HTTP, custom scripts)
- [ ] Load balancing (round-robin, least-conn, source-hash)
- [ ] SSL/TLS passthrough
- [ ] Connection limiting
- [ ] Stats endpoint
- [ ] Connection draining
- [ ] Session persistence (stick tables)

#### Nginx Plus Feature Parity (Essential):
- [ ] TCP/UDP load balancing - **CRITICAL**
- [ ] Active health checks
- [ ] Dynamic reconfiguration (Admin API)
- [ ] Statistics and monitoring
- [ ] Session persistence
- [ ] Bandwidth limiting

#### Caddy Feature Parity (UX):
- [ ] Simple configuration DSL (3-5 lines vs 45+ lines YAML)
- [ ] Automatic HTTPS (ACME) - **Already implemented ✅**
- [ ] Zero-downtime config reload
- [ ] Easy-to-read syntax

#### Pingora Feature Parity (Performance):
- [ ] Multithreading architecture - **Already implemented ✅**
- [ ] >99% connection reuse ratio - **Target**
- [ ] Work-stealing scheduler - **Already implemented ✅ (tokio)**
- [ ] Shared connection pool - **Already implemented ✅**

---

## 💡 QUICK WINS (Can be done anytime)

### Low-Effort, High-Value Tasks:
1. **Fix Flaky Tests** (2-4 hours) - Get to 100% pass rate
2. **Add More Examples** (2-3 hours) - Config examples for common scenarios
3. **Docker Image** (2-3 hours) - Basic Dockerfile
4. **Performance Baseline** (2 hours) - Run benchmarks and document
5. **Prometheus Dashboard** (2-3 hours) - Basic Grafana dashboard
6. **Health Check Endpoint** (1 hour) - Already exists, enhance
7. **Version Command** (30 min) - Add `--version` with build info
8. **Configuration Validation** (2 hours) - `--validate` flag

---

## 🚨 BLOCKERS & DEPENDENCIES

### Critical Blockers:
1. **io_uring HybridTcpStream** - Borrow checker issues must be fixed
2. **Test Failures** - 6 tests failing, blocking release

### Dependencies:
- **TCP Proxy** depends on **io_uring** for best performance
- **Admin UI** depends on **Complete Admin API**
- **Caddy DSL** depends on **Config validation**
- **Zero-copy** depends on **io_uring**

---

## 📝 NOTES

### Architecture Decisions Needed:
1. **Multi-process vs Single-process**: Decide deployment model
2. **DSL Syntax**: Finalize Caddy-like syntax design
3. **Lock-free**: Which structures to replace first?
4. **SIMD**: Which crates to use (portable-simd vs platform-specific)?

### Known Issues:
1. HybridTcpStream borrow checker errors
2. 6 failing tests (observability + file watcher)
3. GraphQL gateway 15% incomplete
4. Connection pool reuse ratio unmeasured

---

---

## 📘 APPENDIX A: TCP Proxy Implementation Guide

### Architecture Design

**File Structure:**
```
src/tcp/
├── mod.rs              # Module exports
├── proxy.rs            # TcpProxy core
├── listener.rs         # TCP listener
├── connection.rs       # Connection handling
├── pool.rs             # TCP connection pooling
├── protocols/
│   ├── mod.rs         # Protocol detection
│   ├── mysql.rs       # MySQL wire protocol
│   ├── postgres.rs    # PostgreSQL wire protocol
│   └── redis.rs       # Redis protocol (future)
└── health.rs          # TCP health checks
```

**Core Implementation Pattern:**

```rust
// src/tcp/proxy.rs
pub struct TcpProxy {
    config: TcpProxyConfig,
    load_balancer: Arc<LoadBalancer>,
    connection_pool: Arc<TcpConnectionPool>,
    metrics: Arc<TcpProxyMetrics>,
}

impl TcpProxy {
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.listen_addr).await?;

        loop {
            // Use io_uring if available, epoll otherwise
            let (client_stream, client_addr) = GLOBAL_IO.accept(&listener).await?;

            let proxy = self.clone();
            tokio::spawn(async move {
                if let Err(e) = proxy.handle_connection(client_stream, client_addr).await {
                    error!("Connection failed: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        &self,
        mut client: TcpStream,
        client_addr: SocketAddr,
    ) -> Result<()> {
        // 1. Select backend using load balancer
        let backend = self.load_balancer.select_backend().await?;

        // 2. Get connection from pool or create new
        let mut upstream = self.connection_pool
            .get_or_create(&backend.address)
            .await?;

        // 3. Bidirectional forwarding with metrics
        let (client_to_upstream, upstream_to_client) = tokio::io::copy_bidirectional(
            &mut client,
            &mut upstream,
        ).await?;

        // 4. Update metrics
        self.metrics.record_bytes_sent(client_to_upstream);
        self.metrics.record_bytes_received(upstream_to_client);

        Ok(())
    }
}
```

### MySQL Protocol Support

**Wire Protocol Detection:**

```rust
// src/tcp/protocols/mysql.rs

/// MySQL handshake packet detection
pub fn detect_mysql_handshake(buf: &[u8]) -> bool {
    if buf.len() < 5 {
        return false;
    }

    // MySQL handshake starts with packet length + sequence number
    // First byte is packet length (LSB)
    // Byte 4 is protocol version (usually 10)
    buf.len() >= 5 && buf[4] == 10
}

/// MySQL health check
pub async fn mysql_health_check(addr: &str) -> Result<bool> {
    let mut stream = TcpStream::connect(addr).await?;
    let mut buf = vec![0u8; 1024];

    // Read handshake from server
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf)).await??;

    if n == 0 || !detect_mysql_handshake(&buf[..n]) {
        return Ok(false);
    }

    Ok(true)
}
```

**Configuration Example:**

```yaml
tcp_proxies:
  - name: mysql-cluster
    listen: "0.0.0.0:3306"
    protocol: mysql  # Optional: auto-detect if not specified

    upstream:
      name: mysql-backends
      load_balancing:
        algorithm: least_connections  # Best for long-lived connections

      servers:
        - url: "mysql1.internal:3306"
          weight: 1
        - url: "mysql2.internal:3306"
          weight: 1
        - url: "mysql3.internal:3306"
          weight: 1

      health_check:
        type: tcp
        interval: 10s
        timeout: 3s
        unhealthy_threshold: 3
        healthy_threshold: 2

    connection_pool:
      max_idle_per_backend: 100
      idle_timeout: 300s
      max_lifetime: 3600s

    limits:
      max_connections: 10000
      max_connections_per_client: 100
```

### PostgreSQL Protocol Support

```rust
// src/tcp/protocols/postgres.rs

/// PostgreSQL startup message detection
pub fn detect_postgres_startup(buf: &[u8]) -> bool {
    if buf.len() < 8 {
        return false;
    }

    // PostgreSQL startup message:
    // - Bytes 0-3: Message length (big-endian)
    // - Bytes 4-7: Protocol version (196608 = 3.0)
    let protocol_version = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    protocol_version == 196608
}

/// PostgreSQL health check
pub async fn postgres_health_check(addr: &str) -> Result<bool> {
    let mut stream = TcpStream::connect(addr).await?;

    // Send startup message
    let startup = build_postgres_startup_message();
    stream.write_all(&startup).await?;

    // Read response
    let mut buf = vec![0u8; 1024];
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf)).await??;

    // Check for authentication request ('R' message)
    Ok(n > 0 && buf[0] == b'R')
}
```

### Performance Optimizations

**1. Zero-Copy Forwarding (Linux):**

```rust
#[cfg(target_os = "linux")]
async fn forward_with_splice(
    client: &mut TcpStream,
    upstream: &mut TcpStream,
) -> Result<(u64, u64)> {
    use nix::fcntl::{splice, SpliceFFlags};

    // Create pipe for zero-copy transfer
    let (pipe_read, pipe_write) = nix::unistd::pipe()?;

    // Splice from client to pipe, then pipe to upstream
    // This avoids copying data to userspace
    let bytes_forwarded = splice(
        client.as_raw_fd(),
        None,
        pipe_write,
        None,
        65536,
        SpliceFFlags::SPLICE_F_MOVE,
    )?;

    splice(
        pipe_read,
        None,
        upstream.as_raw_fd(),
        None,
        bytes_forwarded as usize,
        SpliceFFlags::SPLICE_F_MOVE,
    )?;

    Ok((bytes_forwarded as u64, bytes_forwarded as u64))
}
```

**2. Connection Pooling:**

```rust
// src/tcp/pool.rs

pub struct TcpConnectionPool {
    pools: DashMap<String, VecDeque<PooledConnection>>,
    config: PoolConfig,
    metrics: Arc<PoolMetrics>,
}

impl TcpConnectionPool {
    pub async fn get_or_create(&self, backend: &str) -> Result<TcpStream> {
        // Try to get from pool first
        if let Some(mut pool) = self.pools.get_mut(backend) {
            while let Some(conn) = pool.pop_front() {
                if conn.is_healthy().await {
                    self.metrics.record_reuse();
                    return Ok(conn.stream);
                }
            }
        }

        // Create new connection if pool is empty
        let stream = TcpStream::connect(backend).await?;
        self.metrics.record_new_connection();
        Ok(stream)
    }

    pub async fn return_to_pool(&self, backend: String, stream: TcpStream) {
        let conn = PooledConnection {
            stream,
            created_at: Instant::now(),
            last_used: Instant::now(),
        };

        let mut pool = self.pools.entry(backend).or_insert_with(VecDeque::new);

        // Enforce max idle connections
        if pool.len() >= self.config.max_idle_per_backend {
            // Drop oldest connection
            pool.pop_front();
        }

        pool.push_back(conn);
    }
}
```

**3. Metrics Collection:**

```rust
// src/tcp/metrics.rs

pub struct TcpProxyMetrics {
    active_connections: AtomicU64,
    total_connections: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    connection_errors: AtomicU64,
    pool_hits: AtomicU64,
    pool_misses: AtomicU64,
}

impl TcpProxyMetrics {
    pub fn to_prometheus(&self) -> String {
        format!(
            r#"# HELP tcp_active_connections Active TCP connections
# TYPE tcp_active_connections gauge
tcp_active_connections {{}} {}

# HELP tcp_total_connections Total TCP connections
# TYPE tcp_total_connections counter
tcp_total_connections {{}} {}

# HELP tcp_bytes_sent Total bytes sent to backends
# TYPE tcp_bytes_sent counter
tcp_bytes_sent {{}} {}

# HELP tcp_connection_pool_hit_ratio Connection pool hit ratio
# TYPE tcp_connection_pool_hit_ratio gauge
tcp_connection_pool_hit_ratio {{}} {}
"#,
            self.active_connections.load(Ordering::Relaxed),
            self.total_connections.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
            self.pool_hit_ratio(),
        )
    }

    fn pool_hit_ratio(&self) -> f64 {
        let hits = self.pool_hits.load(Ordering::Relaxed) as f64;
        let total = hits + self.pool_misses.load(Ordering::Relaxed) as f64;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }
}
```

### Testing Strategy

**1. Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_mysql_detection() {
        let handshake = vec![
            0x4a, 0x00, 0x00, 0x00,  // Packet length
            0x0a,                     // Protocol version 10
            // ... rest of handshake
        ];

        assert!(detect_mysql_handshake(&handshake));
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let pool = TcpConnectionPool::new(PoolConfig::default());

        // Get connection (should create new)
        let conn1 = pool.get_or_create("127.0.0.1:3306").await.unwrap();

        // Return to pool
        pool.return_to_pool("127.0.0.1:3306".to_string(), conn1).await;

        // Get again (should reuse)
        let conn2 = pool.get_or_create("127.0.0.1:3306").await.unwrap();

        assert_eq!(pool.metrics.pool_hits.load(Ordering::Relaxed), 1);
    }
}
```

**2. Integration Tests:**
```rust
#[tokio::test]
async fn test_mysql_load_balancing() {
    // Start 3 MySQL backends (use testcontainers)
    let backends = start_mysql_cluster(3).await;

    // Start proxy
    let proxy = TcpProxy::new(config_with_backends(&backends));
    tokio::spawn(async move { proxy.run().await });

    // Connect via proxy
    let mut conn = mysql_async::Conn::new("mysql://localhost:3306/test").await.unwrap();

    // Run queries
    for _ in 0..1000 {
        conn.query_drop("SELECT 1").await.unwrap();
    }

    // Verify load balancing distributed queries
    let backend_counts = get_backend_connection_counts(&backends).await;
    assert_balanced_distribution(&backend_counts, 0.15); // Within 15%
}
```

**3. Benchmark Tests:**
```rust
#[bench]
fn bench_tcp_proxy_throughput(b: &mut Bencher) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    b.iter(|| {
        runtime.block_on(async {
            // Send 1000 requests through proxy
            for _ in 0..1000 {
                send_tcp_request().await.unwrap();
            }
        })
    });
}
```

---

## 📘 APPENDIX B: HAProxy vs Nginx Plus vs highper-gateway Feature Matrix

| Feature | HAProxy 2.8+ | Nginx Plus R30 | highper-gateway Status |
|---------|--------------|----------------|-------------------|
| **Layer 4 TCP LB** | ✅ Full | ✅ Full | 🔴 TODO (Week 5-6) |
| **Layer 7 HTTP LB** | ✅ Full | ✅ Full | ✅ Complete |
| **HTTP/2** | ✅ Yes | ✅ Yes | ✅ Yes |
| **HTTP/3** | ⚠️ Experimental | ✅ Yes | ✅ Yes (quiche) |
| **TLS Termination** | ✅ Yes | ✅ Yes | ✅ Yes |
| **TLS Passthrough** | ✅ Yes | ✅ Yes | 🔴 TODO (Week 6) |
| **ACME/Let's Encrypt** | ⚠️ Via plugin | ✅ Built-in | ✅ Built-in |
| **Health Checks** | ✅ Advanced | ✅ Advanced | ✅ Basic (needs enhancement) |
| **Load Balancing Algorithms** | ✅ 8+ algorithms | ✅ 6+ algorithms | ✅ 7 algorithms |
| **Maglev LB** | ❌ No | ❌ No | ✅ Yes |
| **Geographic LB** | ⚠️ Via ACLs | ✅ Yes | ✅ Yes |
| **Connection Pooling** | ✅ Advanced | ✅ Good | ✅ Good (needs metrics) |
| **Session Persistence** | ✅ Stick tables | ✅ Multiple methods | 🔴 TODO (Week 6) |
| **Rate Limiting** | ✅ Advanced | ✅ Advanced | ✅ Basic |
| **WAF** | ⚠️ Via ModSecurity | ⚠️ Via ModSecurity | ✅ Built-in (multi-engine) |
| **Plugin System** | ⚠️ Lua | ⚠️ NJS (JavaScript) | ✅ WASM + FFI |
| **Admin API** | ✅ Runtime API | ✅ Full API | ⚠️ Partial (Week 1) |
| **Metrics** | ✅ Prometheus | ✅ Prometheus | ✅ Prometheus + OTLP |
| **Config Format** | Custom DSL | Custom DSL | YAML (DSL in Week 7-8) |
| **Config Simplicity** | ⭐⭐⭐ Good | ⭐⭐ Complex | ⭐⭐⭐⭐ (with DSL) |
| **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ (Target: ⭐⭐⭐⭐⭐) |
| **Memory Efficiency** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ (Rust) |

**Legend:**
- ✅ Fully implemented
- ⚠️ Partially implemented or requires plugin
- 🔴 Not yet implemented (TODO)
- ❌ Not supported

---

**Last Updated**: November 9, 2025
**Next Review**: After Week 1 (io_uring integration) completion
**Priority Focus**:
1. **Week 1-4**: Get to 100% production ready (Priority 1)
2. **Week 5-6**: TCP Proxy for database load balancing ⚡ CRITICAL
3. **Week 7-8**: Caddy-like DSL for ease of use

