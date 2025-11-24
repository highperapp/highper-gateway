# TCP Proxy Implementation Plan
## Layer 4 Load Balancing for Database & Service Proxying

**Priority**: ⚡ CRITICAL (Week 5-6)
**Goal**: Match or exceed HAProxy and Nginx Plus performance
**Target Services**: MySQL, PostgreSQL, Redis, HTTP/HTTPS, and any TCP-based protocol

---

## 🎯 Executive Summary

### Why This is Critical

Layer 4 TCP load balancing is **essential** for production deployments because:

1. **Database Load Balancing**: Distribute database connections across clusters
2. **High Performance**: Lower overhead than Layer 7 (<0.5ms vs ~2-5ms)
3. **Protocol Agnostic**: Works with ANY TCP service (not just HTTP)
4. **Market Requirement**: Standard feature in HAProxy, Nginx Plus, Envoy
5. **Use Case Expansion**: Opens new deployment scenarios

### Success Criteria

| Metric | Target | Comparison |
|--------|--------|------------|
| **p99 Latency Overhead** | <0.5ms | Match HAProxy |
| **Throughput** | >1M conn/sec | Exceed Nginx Plus |
| **Connection Reuse** | >99% | Match Pingora |
| **Memory per Connection** | <4KB | Match HAProxy |
| **CPU Efficiency** | <15% at 100k req/s | Match HAProxy |

---

## 📋 Implementation Phases

### Phase 1: Core TCP Proxy (Week 5, Days 1-2)

**Estimated Time**: 16 hours

#### Tasks:
1. Create TCP module structure
2. Implement TcpProxy with bidirectional forwarding
3. Integrate with existing LoadBalancer
4. Add basic metrics
5. Configuration schema

#### Files to Create:
```
src/tcp/
├── mod.rs              # Module exports
├── proxy.rs            # Core TcpProxy implementation
├── listener.rs         # TCP listener wrapper
├── connection.rs       # Connection state management
└── config.rs           # TCP configuration types
```

#### Core Implementation:

```rust
// src/tcp/proxy.rs
use tokio::net::{TcpListener, TcpStream};
use tokio::io;
use std::sync::Arc;
use crate::proxy::loadbalancer::LoadBalancer;

pub struct TcpProxy {
    listener: TcpListener,
    load_balancer: Arc<LoadBalancer>,
    metrics: Arc<TcpMetrics>,
}

impl TcpProxy {
    pub async fn run(self) -> Result<()> {
        info!("TCP proxy listening on {}", self.listener.local_addr()?);

        loop {
            let (client_stream, client_addr) = self.listener.accept().await?;

            let lb = self.load_balancer.clone();
            let metrics = self.metrics.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(client_stream, client_addr, lb, metrics).await {
                    error!("Client {} error: {}", client_addr, e);
                }
            });
        }
    }

    async fn handle_client(
        mut client: TcpStream,
        client_addr: SocketAddr,
        load_balancer: Arc<LoadBalancer>,
        metrics: Arc<TcpMetrics>,
    ) -> Result<()> {
        let start = Instant::now();

        // Select backend
        let backend = load_balancer.select_backend().await?;

        // Connect to backend
        let mut upstream = TcpStream::connect(&backend.address).await?;

        // Bidirectional copy
        let (client_to_upstream, upstream_to_client) =
            io::copy_bidirectional(&mut client, &mut upstream).await?;

        // Record metrics
        let duration = start.elapsed();
        metrics.record_connection(duration, client_to_upstream, upstream_to_client);

        Ok(())
    }
}
```

#### Configuration Schema:

```yaml
# config/tcp-proxy.yaml
tcp_proxies:
  - name: mysql-lb
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
        type: tcp
        interval: 10s
        timeout: 3s
```

**Deliverable**: Basic TCP proxy forwarding traffic to backends with load balancing.

---

### Phase 2: Protocol Detection & Database Support (Week 5, Days 3-4)

**Estimated Time**: 16 hours

#### Tasks:
1. Implement MySQL wire protocol detection
2. Implement PostgreSQL protocol detection
3. Database-specific health checks
4. Protocol-aware routing

#### Files to Create:
```
src/tcp/protocols/
├── mod.rs              # Protocol detection
├── mysql.rs            # MySQL wire protocol
├── postgres.rs         # PostgreSQL protocol
└── redis.rs            # Redis protocol (basic)
```

#### MySQL Protocol Implementation:

```rust
// src/tcp/protocols/mysql.rs

/// Detect MySQL handshake packet
pub fn is_mysql_handshake(buf: &[u8]) -> bool {
    // MySQL server greeting:
    // - Bytes 0-2: Packet length (little-endian)
    // - Byte 3: Sequence number (0x00)
    // - Byte 4: Protocol version (0x0a = version 10)

    if buf.len() < 5 {
        return false;
    }

    let sequence_num = buf[3];
    let protocol_version = buf[4];

    sequence_num == 0x00 && protocol_version == 0x0a
}

/// MySQL-specific health check
pub async fn mysql_health_check(addr: &str) -> Result<bool> {
    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(addr)
    ).await??;

    let mut buf = [0u8; 1024];

    // Read server greeting
    let n = stream.read(&mut buf).await?;

    if n == 0 {
        return Ok(false);
    }

    // Verify MySQL handshake
    Ok(is_mysql_handshake(&buf[..n]))
}

/// Parse MySQL connection attributes for routing decisions
pub struct MySQLConnection {
    pub username: String,
    pub database: String,
    pub client_addr: SocketAddr,
}

impl MySQLConnection {
    pub async fn parse_handshake_response(buf: &[u8]) -> Result<Self> {
        // Parse client handshake response to extract:
        // - Username
        // - Database name
        // - Client capabilities

        // This enables advanced features like:
        // - Route to specific backend based on database
        // - User-based routing
        // - Read/write split based on query type

        todo!("Parse MySQL client handshake response")
    }
}
```

#### PostgreSQL Protocol Implementation:

```rust
// src/tcp/protocols/postgres.rs

/// PostgreSQL startup message
pub fn is_postgres_startup(buf: &[u8]) -> bool {
    if buf.len() < 8 {
        return false;
    }

    // Startup message format:
    // - Bytes 0-3: Message length (big-endian)
    // - Bytes 4-7: Protocol version (196608 = 3.0)

    let protocol = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);

    protocol == 196608  // Protocol version 3.0
}

/// PostgreSQL health check
pub async fn postgres_health_check(addr: &str) -> Result<bool> {
    let mut stream = TcpStream::connect(addr).await?;

    // Build startup message
    let startup = build_startup_message();
    stream.write_all(&startup).await?;

    // Read authentication request
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await?;

    // 'R' = Authentication request
    Ok(n > 0 && buf[0] == b'R')
}

fn build_startup_message() -> Vec<u8> {
    // PostgreSQL startup message:
    // - Protocol version (3.0)
    // - Parameters (user, database, etc.)

    let mut msg = Vec::new();

    // Protocol version 3.0
    msg.extend_from_slice(&196608u32.to_be_bytes());

    // user parameter
    msg.extend_from_slice(b"user\0postgres\0");

    // database parameter
    msg.extend_from_slice(b"database\0postgres\0");

    // Terminator
    msg.push(0);

    // Prepend length
    let len = (msg.len() + 4) as u32;
    let mut packet = len.to_be_bytes().to_vec();
    packet.extend(msg);

    packet
}
```

**Deliverable**: Protocol-aware TCP proxy with MySQL and PostgreSQL support.

---

### Phase 3: Connection Pooling (Week 6, Days 1-2)

**Estimated Time**: 16 hours

#### Tasks:
1. Implement TCP connection pool
2. Connection reuse logic
3. Pool health monitoring
4. Pool metrics

#### Files to Create:
```
src/tcp/
├── pool.rs             # TCP connection pooling
└── pool_metrics.rs     # Pool metrics
```

#### Connection Pool Implementation:

```rust
// src/tcp/pool.rs

use dashmap::DashMap;
use std::collections::VecDeque;
use tokio::net::TcpStream;
use std::time::{Instant, Duration};

pub struct TcpConnectionPool {
    // Per-backend connection pools
    pools: DashMap<String, VecDeque<PooledConnection>>,
    config: PoolConfig,
    metrics: Arc<PoolMetrics>,
}

#[derive(Clone)]
pub struct PoolConfig {
    pub max_idle_per_backend: usize,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub min_idle: usize,
}

struct PooledConnection {
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
}

impl TcpConnectionPool {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            pools: DashMap::new(),
            config,
            metrics: Arc::new(PoolMetrics::default()),
        }
    }

    /// Get connection from pool or create new
    pub async fn get(&self, backend: &str) -> Result<TcpStream> {
        // Try pool first
        if let Some(conn) = self.try_get_from_pool(backend).await {
            self.metrics.record_hit();
            return Ok(conn);
        }

        // Create new connection
        self.metrics.record_miss();
        let stream = TcpStream::connect(backend).await?;

        Ok(stream)
    }

    /// Return connection to pool
    pub async fn put(&self, backend: String, stream: TcpStream) {
        let mut pool = self.pools.entry(backend).or_insert_with(VecDeque::new);

        // Check pool size
        if pool.len() >= self.config.max_idle_per_backend {
            // Pool full, drop oldest connection
            pool.pop_front();
        }

        // Add to pool
        pool.push_back(PooledConnection {
            stream,
            created_at: Instant::now(),
            last_used: Instant::now(),
        });
    }

    async fn try_get_from_pool(&self, backend: &str) -> Option<TcpStream> {
        let mut pool = self.pools.get_mut(backend)?;

        while let Some(conn) = pool.pop_front() {
            // Check if connection is still valid
            if self.is_connection_valid(&conn) {
                return Some(conn.stream);
            }
            // Connection expired, try next
        }

        None
    }

    fn is_connection_valid(&self, conn: &PooledConnection) -> bool {
        let now = Instant::now();

        // Check max lifetime
        if now.duration_since(conn.created_at) > self.config.max_lifetime {
            return false;
        }

        // Check idle timeout
        if now.duration_since(conn.last_used) > self.config.idle_timeout {
            return false;
        }

        true
    }

    /// Pre-warm connections for a backend
    pub async fn prewarm(&self, backend: &str, count: usize) -> Result<()> {
        for _ in 0..count {
            let stream = TcpStream::connect(backend).await?;
            self.put(backend.to_string(), stream).await;
        }

        Ok(())
    }

    /// Cleanup expired connections
    pub async fn cleanup_expired(&self) {
        for mut entry in self.pools.iter_mut() {
            let pool = entry.value_mut();

            // Remove expired connections
            pool.retain(|conn| self.is_connection_valid(conn));
        }
    }
}

#[derive(Default)]
pub struct PoolMetrics {
    hits: AtomicU64,
    misses: AtomicU64,
    total_connections: AtomicU64,
    expired_connections: AtomicU64,
}

impl PoolMetrics {
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn reuse_ratio(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let total = hits + self.misses.load(Ordering::Relaxed) as f64;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }
}
```

**Target**: >95% connection reuse ratio for TCP connections.

**Deliverable**: Production-ready connection pooling with >95% reuse ratio.

---

### Phase 4: Production Features & Benchmarking (Week 6, Days 3-5)

**Estimated Time**: 24 hours

#### Tasks:
1. SSL/TLS passthrough
2. Connection limiting
3. Comprehensive benchmarking
4. Performance optimization
5. Documentation

#### SSL/TLS Passthrough:

```rust
// src/tcp/tls_passthrough.rs

/// Detect TLS Client Hello
pub fn is_tls_handshake(buf: &[u8]) -> bool {
    // TLS handshake starts with:
    // - Byte 0: Content type (0x16 = Handshake)
    // - Bytes 1-2: TLS version
    // - Bytes 3-4: Length

    if buf.len() < 5 {
        return false;
    }

    buf[0] == 0x16 && // Handshake
    (buf[1] == 0x03) && // TLS version 3.x
    (buf[2] >= 0x01 && buf[2] <= 0x03) // TLS 1.0-1.2
}

/// Extract SNI (Server Name Indication) from TLS Client Hello
pub fn extract_sni(buf: &[u8]) -> Option<String> {
    // Parse TLS Client Hello to extract SNI extension
    // This enables routing based on hostname for TLS passthrough

    // TLS Client Hello structure:
    // - Content Type: 0x16
    // - Version
    // - Length
    // - Handshake Type: 0x01 (Client Hello)
    // - Extensions...

    todo!("Parse TLS Client Hello for SNI")
}

/// TLS passthrough handler
pub async fn handle_tls_passthrough(
    mut client: TcpStream,
    load_balancer: Arc<LoadBalancer>,
) -> Result<()> {
    // Peek at first bytes to detect TLS and extract SNI
    let mut peek_buf = [0u8; 512];
    let n = client.peek(&mut peek_buf).await?;

    if !is_tls_handshake(&peek_buf[..n]) {
        return Err(anyhow!("Not a TLS handshake"));
    }

    // Extract SNI for routing
    let sni = extract_sni(&peek_buf[..n]);

    // Select backend based on SNI (if available)
    let backend = if let Some(hostname) = sni {
        load_balancer.select_by_sni(&hostname).await?
    } else {
        load_balancer.select_backend().await?
    };

    // Forward encrypted traffic
    let mut upstream = TcpStream::connect(&backend.address).await?;
    tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;

    Ok(())
}
```

#### Connection Limiting:

```rust
// src/tcp/limiter.rs

use std::sync::Arc;
use dashmap::DashMap;

pub struct ConnectionLimiter {
    // Per-client connection count
    client_connections: DashMap<SocketAddr, usize>,
    // Per-backend connection count
    backend_connections: DashMap<String, usize>,
    config: LimiterConfig,
}

pub struct LimiterConfig {
    pub max_connections_global: usize,
    pub max_connections_per_client: usize,
    pub max_connections_per_backend: usize,
}

impl ConnectionLimiter {
    pub fn check_client(&self, addr: &SocketAddr) -> Result<()> {
        let count = self.client_connections
            .get(addr)
            .map(|v| *v)
            .unwrap_or(0);

        if count >= self.config.max_connections_per_client {
            return Err(anyhow!("Client connection limit exceeded"));
        }

        Ok(())
    }

    pub fn check_backend(&self, backend: &str) -> Result<()> {
        let count = self.backend_connections
            .get(backend)
            .map(|v| *v)
            .unwrap_or(0);

        if count >= self.config.max_connections_per_backend {
            return Err(anyhow!("Backend connection limit exceeded"));
        }

        Ok(())
    }

    pub fn acquire(&self, client: SocketAddr, backend: String) -> Guard {
        self.client_connections
            .entry(client)
            .and_modify(|c| *c += 1)
            .or_insert(1);

        self.backend_connections
            .entry(backend.clone())
            .and_modify(|c| *c += 1)
            .or_insert(1);

        Guard {
            limiter: self.clone(),
            client,
            backend,
        }
    }
}

/// RAII guard for automatic connection release
pub struct Guard {
    limiter: Arc<ConnectionLimiter>,
    client: SocketAddr,
    backend: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.limiter.client_connections
            .entry(self.client)
            .and_modify(|c| *c = c.saturating_sub(1));

        self.limiter.backend_connections
            .entry(self.backend.clone())
            .and_modify(|c| *c = c.saturating_sub(1));
    }
}
```

---

## 🧪 Comprehensive Benchmarking

### Benchmark 1: MySQL Load Balancing

**Setup:**
```bash
# Start 3 MySQL containers
docker run -d --name mysql1 -e MYSQL_ROOT_PASSWORD=test -p 3307:3306 mysql:8.0
docker run -d --name mysql2 -e MYSQL_ROOT_PASSWORD=test -p 3308:3306 mysql:8.0
docker run -d --name mysql3 -e MYSQL_ROOT_PASSWORD=test -p 3309:3306 mysql:8.0

# Configure highper-gateway
cat > config/tcp-mysql.yaml <<EOF
tcp_proxies:
  - name: mysql-lb
    listen: "0.0.0.0:3306"
    upstream:
      servers:
        - url: "localhost:3307"
        - url: "localhost:3308"
        - url: "localhost:3309"
      load_balancing:
        algorithm: least_connections
EOF

# Start proxy
./target/release/highper-gateway --config config/tcp-mysql.yaml
```

**Run Benchmark:**
```bash
# Install sysbench
sudo apt-get install sysbench

# Prepare database
sysbench /usr/share/sysbench/oltp_common.lua \
  --mysql-host=127.0.0.1 \
  --mysql-port=3306 \
  --mysql-user=root \
  --mysql-password=test \
  --mysql-db=sbtest \
  --tables=10 \
  --table-size=100000 \
  prepare

# Run benchmark
sysbench /usr/share/sysbench/oltp_read_write.lua \
  --mysql-host=127.0.0.1 \
  --mysql-port=3306 \
  --mysql-user=root \
  --mysql-password=test \
  --mysql-db=sbtest \
  --tables=10 \
  --table-size=100000 \
  --threads=64 \
  --time=300 \
  --report-interval=10 \
  run
```

**Expected Results:**
```
Threads: 64
Queries performed:
    read:                            1400000
    write:                           400000
    other:                           200000
    total:                           2000000

Transactions:                        100000 (333.33 per sec.)
Queries:                             2000000 (6666.67 per sec.)
Latency (ms):
    min:                             2.45
    avg:                             192.03
    max:                             856.32
    95th percentile:                 344.08
    99th percentile:                 458.96

Proxy overhead: <0.5ms (target met ✅)
```

### Benchmark 2: PostgreSQL Load Balancing

```bash
# Start 3 PostgreSQL containers
docker run -d --name pg1 -e POSTGRES_PASSWORD=test -p 5433:5432 postgres:15
docker run -d --name pg2 -e POSTGRES_PASSWORD=test -p 5434:5432 postgres:15
docker run -d --name pg3 -e POSTGRES_PASSWORD=test -p 5435:5432 postgres:15

# Run pgbench
pgbench -h 127.0.0.1 -p 5432 -U postgres \
  -c 64 -j 8 -T 300 -P 10 \
  -i testdb
```

### Benchmark 3: Comparison vs HAProxy

**HAProxy Config:**
```
global
    maxconn 50000

defaults
    mode tcp
    timeout connect 5s
    timeout client 30s
    timeout server 30s

frontend mysql-front
    bind *:3306
    default_backend mysql-back

backend mysql-back
    balance leastconn
    server mysql1 127.0.0.1:3307 check
    server mysql2 127.0.0.1:3308 check
    server mysql3 127.0.0.1:3309 check
```

**Compare Results:**

| Metric | highper-gateway | HAProxy 2.8 | Target | Status |
|--------|------------|-------------|--------|--------|
| Throughput (QPS) | TBD | 65,000 | >63,000 | ⏳ |
| p99 Latency | TBD | 1.8ms | <1.8ms | ⏳ |
| CPU Usage | TBD | 15% | <15% | ⏳ |
| Memory | TBD | 450MB | <450MB | ⏳ |
| Connection Reuse | TBD | 95% | >95% | ⏳ |

---

## 📊 Performance Targets Summary

### Must-Have Metrics:

✅ **Latency**: <0.5ms p99 proxy overhead
✅ **Throughput**: >1M connections/sec
✅ **Reuse**: >95% connection reuse ratio
✅ **Memory**: <4KB per connection
✅ **CPU**: <15% at 100k req/s

### Competitive Position:

| Feature | HAProxy | Nginx Plus | highper-gateway |
|---------|---------|------------|------------|
| Layer 4 LB | ✅ | ✅ | 🎯 Target |
| Performance | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | 🎯 ⭐⭐⭐⭐⭐ |
| Protocol Support | MySQL, PG, Redis | MySQL, PG | 🎯 MySQL, PG, Redis |
| Connection Pool | ✅ | ✅ | 🎯 ✅ (99%+ reuse) |
| Modern Features | ❌ No WASM | ❌ Limited | ✅ WASM + FFI |

---

## 📝 Implementation Checklist

### Week 5:
- [ ] Day 1-2: Core TCP proxy implementation
- [ ] Day 2: Basic metrics and configuration
- [ ] Day 3: MySQL protocol detection
- [ ] Day 4: PostgreSQL protocol detection
- [ ] Day 5: Protocol-specific health checks

### Week 6:
- [ ] Day 1: Connection pooling implementation
- [ ] Day 2: Pool metrics and monitoring
- [ ] Day 3: SSL/TLS passthrough
- [ ] Day 4: Connection limiting
- [ ] Day 5: Comprehensive benchmarking

### Deliverables:
- [ ] TCP proxy working for MySQL
- [ ] TCP proxy working for PostgreSQL
- [ ] TCP proxy working for Redis
- [ ] Connection pooling >95% reuse
- [ ] Performance benchmarks documented
- [ ] Comparison vs HAProxy completed
- [ ] Production-ready configuration examples
- [ ] Complete documentation

---

## 🚀 Next Steps After TCP Proxy

1. **Week 7-8**: Caddy-like DSL for easier configuration
2. **Week 9-10**: Complete GraphQL Gateway
3. **Week 11+**: Performance optimizations (zero-copy, SIMD)

---

**Document Version**: 1.0
**Last Updated**: November 9, 2025
**Status**: Ready for Implementation
**Priority**: ⚡ CRITICAL
