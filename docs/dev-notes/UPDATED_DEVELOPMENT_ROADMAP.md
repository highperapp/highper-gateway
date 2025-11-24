# Updated Development Roadmap - Rust Reverse Proxy & API Gateway
## Comprehensive Enhancement Plan with Pingora-Level Optimizations

**Date**: November 9, 2025 (Updated)
**Current Status**:
- ✅ Plugin System Complete (WASM + FFI + Hot Reload)
- ✅ 92% Feature Complete (HTTP/1-3, TLS, Load Balancing, Gateway Features)
- ⚠️ Connection pooling partially implemented (needs Pingora-level optimization)
- ❌ Layer 4 TCP proxy not yet implemented

**Project Goal**: Production-grade, Pingora-level performance reverse proxy and API gateway

---

## 📊 Current State vs. Target State

### Architecture Analysis

| Feature | Current State | Target State | Priority |
|---------|--------------|--------------|----------|
| **Connection Pooling** | ✅ Per-process (hyper pool) | 🎯 Multi-process shared pool | **HIGH** |
| **Connection Reuse** | ✅ ~90%+ (estimated) | 🎯 99%+ (measured) | **HIGH** |
| **TCP Load Balancing** | ❌ HTTP/HTTPS only (L7) | 🎯 Full L4 + L7 support | **MEDIUM** |
| **Pool Observability** | ❌ No metrics | 🎯 Full pool metrics | **HIGH** |
| **Threading Model** | ✅ Tokio multithreading | ✅ Already optimal | ✓ DONE |
| **Database Support** | ❌ Not supported | 🎯 MySQL/PostgreSQL TCP proxy | **MEDIUM** |
| **Plugin System** | ✅ WASM + FFI complete | ✅ Production ready | ✓ DONE |

### Key Findings from Architecture Review

#### ✅ **What's Already Pingora-Level:**
1. **Multithreading**: Using tokio's work-stealing scheduler (same as Pingora)
2. **Shared Connection Pool**: hyper's client is Arc-wrapped and shared across all tasks
3. **Connection Keepalive**: TCP + HTTP/2 keepalive configured (90s idle timeout)
4. **Connection Limits**: 100 idle connections per host

#### ⚠️ **What Needs Improvement:**
1. **No Multi-Process Pool Sharing**: Each process has isolated pool (unlike Pingora)
2. **No Pool Metrics**: Can't measure reuse ratio, handshakes/sec, etc.
3. **No Layer 4 Support**: Can't proxy MySQL, PostgreSQL, Redis, etc.
4. **No Pool Tuning**: Limited configuration options

#### ❌ **Major Gaps:**
1. **TCP Proxy Mode**: Only HTTP/HTTPS supported
2. **Connection Pool Coordination**: No inter-process communication
3. **Database Protocol Support**: No MySQL/PostgreSQL wire protocol

---

## 🚀 UPDATED DEVELOPMENT PLAN

### **PHASE 1: CONNECTION POOL OPTIMIZATION** (2 weeks) 🔥 **NEW PRIORITY**

#### Week 1: Connection Pool Observability & Metrics

**Goal**: Add comprehensive connection pool metrics to understand current performance

##### Day 1-2: Connection Pool Metrics Infrastructure
**Files to Create/Modify:**
- `src/proxy/pool_metrics.rs` (new)
- `src/proxy/client.rs` (enhance)
- `src/observability/metrics.rs` (add pool metrics)

**Implementation:**
```rust
// src/proxy/pool_metrics.rs

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Connection pool metrics tracker
#[derive(Clone)]
pub struct ConnectionPoolMetrics {
    // Connection lifecycle counters
    pub total_connections_created: Arc<AtomicU64>,
    pub total_connections_reused: Arc<AtomicU64>,
    pub total_connections_closed: Arc<AtomicU64>,

    // Current state
    pub active_connections: Arc<AtomicU64>,
    pub idle_connections: Arc<AtomicU64>,

    // Handshake metrics
    pub tcp_handshakes_total: Arc<AtomicU64>,
    pub tls_handshakes_total: Arc<AtomicU64>,
    pub handshake_time_total_ms: Arc<AtomicU64>,

    // Per-backend tracking
    pub per_backend: Arc<DashMap<String, BackendPoolStats>>,
}

impl ConnectionPoolMetrics {
    pub fn new() -> Self {
        Self {
            total_connections_created: Arc::new(AtomicU64::new(0)),
            total_connections_reused: Arc::new(AtomicU64::new(0)),
            total_connections_closed: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU64::new(0)),
            idle_connections: Arc::new(AtomicU64::new(0)),
            tcp_handshakes_total: Arc::new(AtomicU64::new(0)),
            tls_handshakes_total: Arc::new(AtomicU64::new(0)),
            handshake_time_total_ms: Arc::new(AtomicU64::new(0)),
            per_backend: Arc::new(DashMap::new()),
        }
    }

    /// Calculate connection reuse ratio (Pingora's key metric)
    pub fn reuse_ratio(&self) -> f64 {
        let created = self.total_connections_created.load(Ordering::Relaxed);
        let reused = self.total_connections_reused.load(Ordering::Relaxed);

        if created == 0 {
            return 0.0;
        }

        (reused as f64) / ((created + reused) as f64) * 100.0
    }

    /// Average handshake time
    pub fn avg_handshake_time_ms(&self) -> f64 {
        let total_handshakes = self.tcp_handshakes_total.load(Ordering::Relaxed);
        let total_time = self.handshake_time_total_ms.load(Ordering::Relaxed);

        if total_handshakes == 0 {
            return 0.0;
        }

        (total_time as f64) / (total_handshakes as f64)
    }

    /// Record new connection
    pub fn record_new_connection(&self, backend: &str, handshake_time_ms: u64) {
        self.total_connections_created.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.tcp_handshakes_total.fetch_add(1, Ordering::Relaxed);
        self.handshake_time_total_ms.fetch_add(handshake_time_ms, Ordering::Relaxed);

        // Update per-backend stats
        let mut stats = self.per_backend
            .entry(backend.to_string())
            .or_insert_with(BackendPoolStats::new);
        stats.connections_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record reused connection
    pub fn record_reused_connection(&self, backend: &str) {
        self.total_connections_reused.fetch_add(1, Ordering::Relaxed);

        let mut stats = self.per_backend
            .entry(backend.to_string())
            .or_insert_with(BackendPoolStats::new);
        stats.connections_reused.fetch_add(1, Ordering::Relaxed);
    }

    /// Export Prometheus metrics
    pub fn export_prometheus(&self) -> String {
        let reuse_ratio = self.reuse_ratio();
        let avg_handshake = self.avg_handshake_time_ms();

        format!(
            "# HELP connection_pool_reuse_ratio Connection reuse ratio percentage\n\
             # TYPE connection_pool_reuse_ratio gauge\n\
             connection_pool_reuse_ratio {}\n\
             \n\
             # HELP connection_pool_handshake_time_avg Average handshake time in ms\n\
             # TYPE connection_pool_handshake_time_avg gauge\n\
             connection_pool_handshake_time_avg {}\n\
             \n\
             # HELP connection_pool_active Active connections\n\
             # TYPE connection_pool_active gauge\n\
             connection_pool_active {}\n\
             \n\
             # HELP connection_pool_idle Idle connections\n\
             # TYPE connection_pool_idle gauge\n\
             connection_pool_idle {}\n",
            reuse_ratio,
            avg_handshake,
            self.active_connections.load(Ordering::Relaxed),
            self.idle_connections.load(Ordering::Relaxed)
        )
    }
}

#[derive(Default)]
pub struct BackendPoolStats {
    pub connections_created: AtomicU64,
    pub connections_reused: AtomicU64,
    pub active: AtomicU64,
    pub idle: AtomicU64,
}

impl BackendPoolStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reuse_ratio(&self) -> f64 {
        let created = self.connections_created.load(Ordering::Relaxed);
        let reused = self.connections_reused.load(Ordering::Relaxed);

        if created == 0 {
            return 0.0;
        }

        (reused as f64) / ((created + reused) as f64) * 100.0
    }
}
```

**Tasks:**
- [ ] Implement ConnectionPoolMetrics struct
- [ ] Add metrics to Client creation/reuse paths
- [ ] Add Prometheus export endpoints
- [ ] Create Grafana dashboard for pool metrics
- [ ] Add alerting rules for low reuse ratio (<90%)

**Success Criteria:**
- Can measure exact connection reuse ratio
- Can track handshakes per second
- Per-backend pool statistics available
- Prometheus metrics exported

---

##### Day 3-4: Enhanced Connection Pool Configuration
**Files to Modify:**
- `src/proxy/client.rs`
- `src/config/schema.rs`
- `config/example.yaml`

**Implementation:**
```rust
// Enhanced connection pool configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionPoolConfig {
    /// Maximum idle connections per host
    pub max_idle_per_host: usize,

    /// Idle timeout (duration before closing idle connection)
    pub idle_timeout: Duration,

    /// Maximum connection lifetime (force refresh)
    pub max_lifetime: Option<Duration>,

    /// Minimum idle connections to maintain (warm connections)
    pub min_idle: usize,

    /// Enable connection health checks
    pub health_check_interval: Option<Duration>,

    /// Enable connection pre-warming on startup
    pub prewarm_connections: bool,

    /// Number of connections to prewarm per backend
    pub prewarm_count: usize,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 100,
            idle_timeout: Duration::from_secs(90),
            max_lifetime: Some(Duration::from_secs(300)),
            min_idle: 10,
            health_check_interval: Some(Duration::from_secs(30)),
            prewarm_connections: true,
            prewarm_count: 10,
        }
    }
}
```

**YAML Configuration:**
```yaml
# Connection pool configuration (Pingora-inspired)
connection_pool:
  # Maximum idle connections per backend
  max_idle_per_host: 100

  # Idle timeout before closing connection
  idle_timeout: 90s

  # Maximum connection lifetime (force refresh to avoid stale connections)
  max_lifetime: 300s

  # Minimum idle connections to keep warm
  min_idle: 10

  # Health check idle connections every 30s
  health_check_interval: 30s

  # Pre-warm connections on startup
  prewarm_connections: true
  prewarm_count: 10
```

**Tasks:**
- [ ] Add ConnectionPoolConfig to schema
- [ ] Implement connection pre-warming
- [ ] Add periodic health checks for idle connections
- [ ] Implement max_lifetime enforcement
- [ ] Add min_idle maintenance

---

##### Day 5: Connection Pool Monitoring Dashboard
**Files to Create:**
- `grafana/connection-pool-dashboard.json`
- `prometheus/pool-alerts.yml`
- `docs/CONNECTION_POOL_MONITORING.md`

**Grafana Panels:**
1. Connection Reuse Ratio (gauge + time series)
2. Handshakes per Second (graph)
3. Active vs Idle Connections (stacked area)
4. Per-Backend Reuse Ratio (table)
5. Average Handshake Time (graph)
6. Pool Efficiency Score (calculated metric)

**Prometheus Alerts:**
```yaml
groups:
  - name: connection_pool
    interval: 30s
    rules:
      # Alert if reuse ratio drops below 90%
      - alert: LowConnectionReuseRatio
        expr: connection_pool_reuse_ratio < 90
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Connection reuse ratio below 90%"
          description: "Connection reuse is {{ $value }}%, indicating pool inefficiency"

      # Alert if too many handshakes
      - alert: HighHandshakeRate
        expr: rate(connection_pool_handshakes_total[1m]) > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High connection handshake rate"
          description: "{{ $value }} handshakes/sec, check pool configuration"
```

---

#### Week 2: Multi-Process Pool Coordination (Advanced)

**Goal**: Enable connection pool sharing across multiple proxy processes

##### Option A: Redis-Based Pool Coordination (Easier, Recommended)
**Approach**: Use Redis to coordinate pool state across processes

**Files to Create:**
- `src/proxy/shared_pool.rs` (new)
- `src/proxy/pool_coordinator.rs` (new)

**Architecture:**
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Process 1  │     │  Process 2  │     │  Process 3  │
│             │     │             │     │             │
│ Local Pool  │     │ Local Pool  │     │ Local Pool  │
│  (100 max)  │     │  (100 max)  │     │  (100 max)  │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                    ┌──────▼──────┐
                    │    Redis    │
                    │ Pool State  │
                    │ Coordinator │
                    └─────────────┘
```

**Implementation:**
```rust
// src/proxy/pool_coordinator.rs

use redis::aio::ConnectionManager;
use std::time::Duration;

pub struct PoolCoordinator {
    redis: ConnectionManager,
    process_id: String,
}

impl PoolCoordinator {
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let redis = ConnectionManager::new(client).await?;
        let process_id = format!("proxy-{}", std::process::id());

        Ok(Self { redis, process_id })
    }

    /// Register a new connection
    pub async fn register_connection(
        &self,
        backend: &str,
        connection_id: &str,
    ) -> Result<()> {
        let key = format!("pool:{}:connections", backend);
        let value = format!("{}:{}", self.process_id, connection_id);

        // Add to Redis set with expiry
        self.redis.sadd(&key, &value).await?;
        self.redis.expire(&key, 300).await?; // 5 min TTL

        Ok(())
    }

    /// Get global pool size for backend
    pub async fn get_global_pool_size(&self, backend: &str) -> Result<usize> {
        let key = format!("pool:{}:connections", backend);
        let size: usize = self.redis.scard(&key).await?;
        Ok(size)
    }

    /// Check if global pool is full
    pub async fn is_global_pool_full(
        &self,
        backend: &str,
        max_global: usize,
    ) -> Result<bool> {
        let size = self.get_global_pool_size(backend).await?;
        Ok(size >= max_global)
    }
}
```

**Tasks:**
- [ ] Implement Redis pool coordinator
- [ ] Add global pool limits
- [ ] Implement connection registration/deregistration
- [ ] Add process-level pool stats aggregation
- [ ] Test with multiple processes

**Configuration:**
```yaml
connection_pool:
  # Enable multi-process coordination
  multi_process: true

  # Redis coordinator
  coordinator:
    redis_url: "redis://localhost:6379"

  # Global limits across all processes
  global_max_connections: 1000
  global_max_per_backend: 300

  # Per-process limits
  local_max_idle_per_host: 100
```

##### Option B: Shared Memory Pool (More Complex, Better Performance)
**Approach**: Use shared memory for zero-latency pool coordination

**Note**: This is more complex and Linux-specific. Defer to future if needed.

---

### **PHASE 2: LAYER 4 TCP PROXY** (2 weeks) 🆕 **DATABASE SUPPORT**

#### Week 3: TCP Proxy Foundation

**Goal**: Add Layer 4 TCP proxy capability for database load balancing

##### Day 1-2: TCP Proxy Core Implementation
**Files to Create:**
- `src/tcp/mod.rs` (new)
- `src/tcp/proxy.rs` (new)
- `src/tcp/listener.rs` (new)
- `src/config/tcp.rs` (new)

**Architecture:**
```
┌──────────────┐
│ TCP Client   │
│ (MySQL CLI)  │
└──────┬───────┘
       │
       │ TCP connection
       │
┌──────▼───────┐
│ Highper Gateway   │
│ TCP Mode     │
│              │
│ - Protocol   │
│   Detection  │
│ - Load       │
│   Balancing  │
│ - Health     │
│   Checks     │
└──────┬───────┘
       │
       │ Select backend
       │
┌──────▼───────┐
│ Backend Pool │
│              │
│ [MySQL1]     │
│ [MySQL2]     │
│ [MySQL3]     │
└──────────────┘
```

**Implementation:**
```rust
// src/tcp/proxy.rs

use tokio::net::{TcpListener, TcpStream};
use tokio::io::copy_bidirectional;
use std::net::SocketAddr;
use crate::proxy::LoadBalancer;

pub struct TcpProxy {
    /// Listen address
    listen_addr: SocketAddr,

    /// Backend load balancer
    load_balancer: Arc<LoadBalancer>,

    /// Optional protocol detection
    protocol_detector: Option<ProtocolDetector>,

    /// Metrics
    metrics: Arc<TcpProxyMetrics>,
}

impl TcpProxy {
    pub fn new(
        listen_addr: SocketAddr,
        load_balancer: Arc<LoadBalancer>,
    ) -> Self {
        Self {
            listen_addr,
            load_balancer,
            protocol_detector: None,
            metrics: Arc::new(TcpProxyMetrics::new()),
        }
    }

    /// Start TCP proxy server
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        info!("TCP proxy listening on {}", self.listen_addr);

        loop {
            let (client_stream, client_addr) = listener.accept().await?;

            let load_balancer = self.load_balancer.clone();
            let metrics = self.metrics.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    client_stream,
                    client_addr,
                    load_balancer,
                    metrics,
                ).await {
                    error!("TCP proxy error: {}", e);
                }
            });
        }
    }

    /// Handle a single TCP connection
    async fn handle_connection(
        mut client_stream: TcpStream,
        client_addr: SocketAddr,
        load_balancer: Arc<LoadBalancer>,
        metrics: Arc<TcpProxyMetrics>,
    ) -> Result<()> {
        // Select backend
        let backend = load_balancer
            .select(Some(&client_addr.ip().to_string()), None)
            .ok_or_else(|| anyhow::anyhow!("No healthy backend"))?;

        let backend_addr = backend.server.url
            .parse::<SocketAddr>()
            .or_else(|_| {
                // Try resolving hostname:port
                tokio::net::lookup_host(&backend.server.url)
                    .await?
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Cannot resolve backend"))
            })?;

        // Connect to backend
        let mut backend_stream = TcpStream::connect(backend_addr).await?;

        info!(
            "TCP proxy: {} -> {} ({})",
            client_addr,
            backend.server.url,
            backend_addr
        );

        metrics.record_connection(&backend.server.url);

        // Bidirectional copy
        let (bytes_client_to_backend, bytes_backend_to_client) =
            copy_bidirectional(&mut client_stream, &mut backend_stream).await?;

        info!(
            "TCP proxy closed: {} -> {} ({} bytes up, {} bytes down)",
            client_addr,
            backend_addr,
            bytes_client_to_backend,
            bytes_backend_to_client
        );

        metrics.record_bytes_transferred(
            &backend.server.url,
            bytes_client_to_backend,
            bytes_backend_to_client,
        );

        Ok(())
    }
}

/// TCP proxy metrics
pub struct TcpProxyMetrics {
    connections_total: Arc<AtomicU64>,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
    active_connections: Arc<AtomicU64>,
}

impl TcpProxyMetrics {
    pub fn new() -> Self {
        Self {
            connections_total: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn record_connection(&self, _backend: &str) {
        self.connections_total.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_bytes_transferred(
        &self,
        _backend: &str,
        sent: u64,
        received: u64,
    ) {
        self.bytes_sent.fetch_add(sent, Ordering::Relaxed);
        self.bytes_received.fetch_add(received, Ordering::Relaxed);
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
```

**Configuration:**
```yaml
# TCP proxy configuration (NEW)
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
          weight: 1
        - url: "mysql2.internal:3306"
          weight: 1
        - url: "mysql3.internal:3306"
          weight: 1

      health_check:
        enabled: true
        interval: 10s
        timeout: 2s
        # TCP health check
        tcp_connect: true

  - name: postgres-proxy
    listen: "0.0.0.0:5432"
    mode: tcp

    upstream:
      name: postgres-cluster
      servers:
        - url: "pg1.internal:5432"
        - url: "pg2.internal:5432"
```

**Tasks:**
- [ ] Implement TcpProxy struct
- [ ] Add TCP listener and connection handler
- [ ] Integrate with existing LoadBalancer
- [ ] Add TCP-specific health checks
- [ ] Add configuration schema
- [ ] Integration testing with MySQL/PostgreSQL

---

##### Day 3-4: Database Protocol Support
**Files to Create:**
- `src/tcp/protocols/mod.rs` (new)
- `src/tcp/protocols/mysql.rs` (new)
- `src/tcp/protocols/postgres.rs` (new)

**Features:**
- Protocol detection (MySQL vs PostgreSQL)
- Database-specific health checks
- Connection pooling for database connections
- Query routing (optional, advanced)

**Implementation:**
```rust
// src/tcp/protocols/mysql.rs

/// MySQL protocol detector
pub struct MySQLProtocolDetector;

impl MySQLProtocolDetector {
    /// Detect if this is a MySQL connection
    pub fn detect(first_packet: &[u8]) -> bool {
        // MySQL handshake starts with protocol version (0x0a)
        first_packet.get(4) == Some(&0x0a)
    }

    /// Parse MySQL handshake packet
    pub fn parse_handshake(packet: &[u8]) -> Option<MySQLHandshake> {
        // Basic MySQL handshake parsing
        // In production, use mysql_async or similar
        None
    }
}

/// MySQL health checker
pub struct MySQLHealthChecker {
    connection_string: String,
}

impl MySQLHealthChecker {
    /// Check MySQL server health
    pub async fn check(&self) -> Result<bool> {
        // Connect to MySQL and run SELECT 1
        // Use mysql_async crate
        Ok(true)
    }
}
```

**Tasks:**
- [ ] Implement MySQL protocol detection
- [ ] Implement PostgreSQL protocol detection
- [ ] Add database-specific health checks
- [ ] Add connection string parsing
- [ ] Test with real MySQL/PostgreSQL servers

---

##### Day 5: TCP Proxy Testing & Documentation
**Files to Create:**
- `tests/tcp_proxy_tests.rs`
- `docs/TCP_PROXY_GUIDE.md`
- `examples/tcp-mysql-proxy.yaml`
- `examples/tcp-postgres-proxy.yaml`

**Tests:**
- TCP connection establishment
- Load balancing across backends
- Health check integration
- Connection pooling
- Protocol detection
- Metrics collection

**Documentation:**
- TCP proxy setup guide
- MySQL load balancing example
- PostgreSQL load balancing example
- Performance tuning
- Troubleshooting

---

#### Week 4: TCP Proxy Production Features

##### Day 1-2: TCP Connection Pooling
**Goal**: Implement connection pooling for TCP backends

**Features:**
- Maintain pool of connections to each backend
- Reuse connections when possible
- Connection health monitoring
- Automatic reconnection

##### Day 3-4: Advanced TCP Features
**Features:**
- SSL/TLS passthrough for encrypted database connections
- Connection limiting (per client, per backend)
- Bandwidth throttling
- Connection timeout configuration

##### Day 5: TCP Proxy Benchmarking
**Tools:**
- sysbench for MySQL
- pgbench for PostgreSQL
- Custom TCP benchmark tool

**Metrics to Measure:**
- Throughput (queries/sec)
- Latency (p50, p95, p99)
- Connection overhead
- Connection reuse ratio

---

### **PHASE 3: EXISTING FEATURES COMPLETION** (2 weeks)

Continue with existing priorities from NEXT_STEPS.md:

#### Week 5: Runtime Integration (Priority 1 from NEXT_STEPS.md)
- ProxyState + LoadBalancer integration
- Health checker integration
- Request handler metrics tracking
- Wire everything together

#### Week 6: Enhanced Metrics (Priority 2 from NEXT_STEPS.md)
- Per-route metrics tracking
- Per-backend metrics
- Latency histograms
- Advanced Prometheus metrics

---

## 📋 COMPLETE TODO LIST

### ✅ COMPLETED
- [x] Plugin System (WASM + FFI + Hot Reload)
- [x] Core HTTP/HTTPS proxy functionality
- [x] Basic connection pooling (hyper)
- [x] Load balancing algorithms (7 total)
- [x] TLS/mTLS support
- [x] Admin API
- [x] Observability basics

### 🔥 HIGH PRIORITY (Next 4 Weeks)

#### Phase 1: Connection Pool Optimization (Week 1-2)
- [ ] **Week 1: Observability**
  - [ ] Day 1-2: Implement ConnectionPoolMetrics
  - [ ] Day 3-4: Enhanced pool configuration
  - [ ] Day 5: Grafana dashboard + alerts
- [ ] **Week 2: Multi-Process Coordination**
  - [ ] Day 1-2: Redis-based pool coordinator
  - [ ] Day 3-4: Global pool limits
  - [ ] Day 5: Multi-process testing

**Success Metrics:**
- ✅ Can measure connection reuse ratio
- ✅ Reuse ratio > 95%
- ✅ Grafana dashboard functional
- ✅ Multi-process coordination working

#### Phase 2: TCP Proxy (Week 3-4)
- [ ] **Week 3: TCP Proxy Foundation**
  - [ ] Day 1-2: TCP proxy core implementation
  - [ ] Day 3-4: Database protocol support
  - [ ] Day 5: Testing & documentation
- [ ] **Week 4: TCP Production Features**
  - [ ] Day 1-2: TCP connection pooling
  - [ ] Day 3-4: Advanced TCP features
  - [ ] Day 5: Benchmarking

**Success Metrics:**
- ✅ MySQL load balancing working
- ✅ PostgreSQL load balancing working
- ✅ TCP health checks functional
- ✅ Performance: <1ms proxy overhead

---

### ⚠️ MEDIUM PRIORITY (Weeks 5-6)

#### Phase 3: Runtime Integration
- [ ] ProxyState integration with LoadBalancer
- [ ] Health checker integration
- [ ] Request handler metrics
- [ ] Wire all components together

#### Phase 4: Enhanced Metrics
- [ ] Per-route metrics
- [ ] Per-backend metrics
- [ ] Latency histograms
- [ ] Prometheus export

---

### 🔮 FUTURE ENHANCEMENTS

#### Layer 4+ Features
- [ ] Redis protocol support
- [ ] MongoDB wire protocol
- [ ] Custom protocol plugins
- [ ] SNI-based TCP routing

#### Advanced Connection Pool
- [ ] Per-backend pool strategies
- [ ] Connection health scoring
- [ ] Adaptive pool sizing
- [ ] Connection age management

#### Performance Optimizations
- [ ] io_uring integration (Linux)
- [ ] Zero-copy TCP forwarding
- [ ] DPDK integration (optional)
- [ ] eBPF integration (optional)

---

## 🎯 SUCCESS CRITERIA

### Pingora-Level Performance Targets

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| **Connection Reuse Ratio** | ~90% (unmeasured) | >99% | 🟡 In Progress |
| **Handshake Overhead** | Unknown | <5% of requests | 🟡 In Progress |
| **TCP Proxy Latency** | N/A | <1ms p99 | 🔴 Not Started |
| **Database Load Balancing** | Not Supported | Full MySQL/PG support | 🔴 Not Started |
| **Multi-Process Pool** | No | Yes (Redis-coordinated) | 🔴 Not Started |
| **Pool Observability** | None | Full metrics + dashboard | 🔴 Not Started |

### Production Readiness

| Requirement | Status |
|-------------|--------|
| ✅ HTTP/HTTPS proxy | Complete |
| 🟡 Connection pool optimization | In Progress |
| 🔴 TCP proxy | Not Started |
| 🟡 Database support | Not Started |
| ✅ Plugin system | Complete |
| 🟡 Full observability | Partial |
| 🟡 Production testing | Needs TCP testing |
| 🟡 Documentation | Needs TCP docs |

---

## 📊 EFFORT ESTIMATION

### Phase 1: Connection Pool (2 weeks)
- **Week 1** (Metrics): 40 hours
- **Week 2** (Multi-Process): 40 hours
- **Total**: 80 hours

### Phase 2: TCP Proxy (2 weeks)
- **Week 3** (Foundation): 40 hours
- **Week 4** (Production): 40 hours
- **Total**: 80 hours

### Phase 3: Runtime Integration (2 weeks)
- **Week 5-6**: 80 hours
- **Total**: 80 hours

### **Grand Total**: 240 hours (~6 weeks full-time)

---

## 🚀 QUICK START

### To Start Phase 1 (Connection Pool Optimization):

```bash
# Create feature branch
git checkout -b feature/connection-pool-metrics

# Create new files
mkdir -p src/proxy
touch src/proxy/pool_metrics.rs

# Add to src/proxy/mod.rs
echo "pub mod pool_metrics;" >> src/proxy/mod.rs

# Start implementing ConnectionPoolMetrics
# See implementation above
```

### To Start Phase 2 (TCP Proxy):

```bash
# Create feature branch
git checkout -b feature/tcp-proxy

# Create new module
mkdir -p src/tcp
touch src/tcp/mod.rs
touch src/tcp/proxy.rs
touch src/tcp/listener.rs

# Add to src/lib.rs
echo "pub mod tcp;" >> src/lib.rs

# Create config module
mkdir -p src/config
touch src/config/tcp.rs
```

---

## 📚 REFERENCES

### Pingora Architecture
- Blog: https://blog.cloudflare.com/how-we-built-pingora-the-proxy-that-connects-cloudflare-to-the-internet/
- Key Learnings:
  - Multithreading > Multiprocessing for pool sharing
  - Connection reuse ratio is critical metric
  - 99.92% reuse ratio achieved
  - 160x reduction in new connections
  - 434 years of handshake time saved daily

### Existing Documentation
- Plugin System: `PLUGIN_SYSTEM_FINAL_SUMMARY.md`
- Next Steps: `NEXT_STEPS.md`
- Development Plan: `COMPREHENSIVE_DEVELOPMENT_PLAN_V2.md`
- Optimizations: `OPTIMIZATIONS_AND_ARCHITECTURE.md`

---

## 💡 NOTES

### Why This Order?

1. **Connection Pool First**: Immediate performance wins, foundational for TCP proxy
2. **TCP Proxy Second**: Unlocks database load balancing use case
3. **Runtime Integration Third**: Completes existing features

### Alternative Approaches

If time is limited, consider:
- **Minimum Viable**: Phase 1 (Week 1 only) - Get metrics, skip multi-process
- **Database Focus**: Skip Phase 1, go straight to Phase 2
- **Existing Features**: Skip both, focus on Phase 3 (Runtime Integration)

### Risk Mitigation

- **Phase 1 Risk**: Multi-process coordination is complex
  - **Mitigation**: Start with metrics only, defer Redis coordination
- **Phase 2 Risk**: Database protocol complexity
  - **Mitigation**: Start with simple TCP proxy, add protocol detection later
- **Phase 3 Risk**: Breaking existing functionality
  - **Mitigation**: Comprehensive testing, feature flags

---

## ✅ DEFINITION OF DONE

Each phase is complete when:

1. ✅ Code implemented and tested
2. ✅ Integration tests passing
3. ✅ Documentation written
4. ✅ Metrics/observability added
5. ✅ Performance benchmarks met
6. ✅ Examples/guides provided
7. ✅ Grafana dashboards created (if applicable)

---

**Last Updated**: November 9, 2025
**Next Review**: After Phase 1 completion
