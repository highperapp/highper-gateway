# Highper Gateway Architecture

> **Reconciliation banner — 2026-05-02.** This document predates the 2026-05-02
> interface-first audit. It does **not** mention the 8 weak/non-trait architecture
> boundaries identified in `docs/planning/ROADMAP.md` §4.4 (LoadBalancerStrategy,
> RateLimiter, AuthProvider, GeoProvider, ConnectionPool unification, CircuitBreaker
> unification, MetricsBackend/LogBackend, ConfigSource). The roadmap tracks the
> refactor work folded into Phases 0–4; this document will be superseded by an
> `ARCHITECTURE_v2.md` once those refactors land. Treat the diagrams below as the
> *current* topology, not the target topology.

This document provides a high-level overview of the Highper Gateway architecture, its components, and data flow.

## Overview

Highper Gateway is a high-performance reverse proxy and API gateway written in Rust, designed for:
- **Million-RPS throughput** with sub-millisecond latency
- **Multi-protocol support** (HTTP/1.1, HTTP/2, HTTP/3, gRPC, WebSocket, TCP)
- **Production-grade features** (TLS, mTLS, ACME, WAF, caching, rate limiting)
- **Cloud-native architecture** (service discovery, observability, hot reload)

## System Architecture

```
                                    +------------------+
                                    |   Admin API      |
                                    |   (Dashboard,    |
                                    |    REST API)     |
                                    +--------+---------+
                                             |
+------------+    +------------------+    +--+---------------+    +----------------+
|            |    |                  |    |                  |    |                |
|  Clients   +--->+   TLS Layer     +--->+   Gateway Core   +--->+   Backends     |
|            |    |   (rustls)      |    |   (Routing,      |    |   (Upstreams)  |
+------------+    +------------------+    |    Load Balance) |    +----------------+
                                          +------------------+
                                                   |
                  +--------------------------------+--------------------------------+
                  |                |               |               |                |
          +-------v------+  +-----v-----+  +------v------+  +-----v------+  +------v------+
          |  Middleware  |  |   Cache   |  | Observability|  |  Health    |  |  Service    |
          |  (WAF, Auth, |  |  (Memory, |  | (Metrics,   |  |  Checker   |  |  Discovery  |
          |   Rate Limit)|  |   Disk)   |  |  Tracing)   |  |            |  |  (Consul,   |
          +--------------+  +-----------+  +-------------+  +------------+  |   etcd)     |
                                                                            +-------------+
```

## Core Components

### 1. Entry Points (`main.rs`)

The main entry point initializes:
- Configuration loading (DSL or YAML)
- Runtime setup (Tokio with io_uring on Linux)
- Server binding and graceful shutdown
- Signal handling (SIGHUP for reload)

### 2. Configuration (`config/`)

| Component | Description |
|-----------|-------------|
| `schema.rs` | Strongly-typed configuration structures |
| `dsl_parser.rs` | Caddy-like DSL parser (pest grammar) |
| `dsl_ast.rs` | Abstract syntax tree definitions |
| `dsl_converter.rs` | Converts DSL to internal config |
| `validation.rs` | Configuration validation |
| `reloader.rs` | Hot reload support |
| `defaults.rs` | Protocol-specific defaults |

### 3. Proxy Layer (`proxy/`)

The core request handling layer:

| Component | Description |
|-----------|-------------|
| `handler.rs` | Main request handler, routing decisions |
| `server.rs` | Server socket management |
| `loadbalancer.rs` | Load balancing algorithms (round-robin, least-conn, IP-hash, Maglev) |
| `connection_pool.rs` | Connection pooling to backends |
| `stick_table.rs` | HAProxy-style session persistence |
| `geographic.rs` | Geo-aware routing |
| `database_pool.rs` | Database connection pooling |

### 4. Gateway Features (`gateway/`)

High-level gateway functionality:

| Component | Description |
|-----------|-------------|
| `routing/` | Route matching, hot reload, upstream state |
| `auth/` | Authentication (JWT, OAuth2, API keys, mTLS) |
| `cache/` | Response caching logic |
| `graphql/` | GraphQL gateway, schema stitching |
| `aggregation/` | Response aggregation (BFF pattern) |
| `bff/` | Backend-for-Frontend routing |
| `transform.rs` | Request/response transformation |
| `validation.rs` | Request validation (JSON schema) |

### 5. Protocol Support

#### HTTP (`http/`)
- `http3.rs`, `http3_quiche.rs` - HTTP/3 with QUIC (via quiche)
- `alt_svc.rs` - Alt-Svc header for HTTP/3 upgrade

#### gRPC (`grpc/`)
- `server.rs` - gRPC proxying
- `health.rs` - gRPC health checking

#### WebSocket (`websocket/`)
- `handler.rs` - Upgrade handling
- `session.rs` - Session management
- `keepalive.rs` - Connection keep-alive
- `recovery.rs` - Connection recovery
- `shutdown.rs` - Graceful shutdown

#### TCP (`tcp/`)
- Layer 4 TCP proxying
- Protocol detection (TLS, HTTP, MySQL, PostgreSQL)

### 6. TLS (`tls/`)

| Component | Description |
|-----------|-------------|
| `mod.rs` | TLS configuration and setup |
| `acme.rs` | Automatic certificate management (Let's Encrypt) |
| `cert_validator.rs` | Certificate chain validation |
| `ocsp_fetcher.rs` | OCSP stapling |
| `ocsp_stapler.rs` | OCSP response stapling |
| `crl_checker.rs` | Certificate revocation list checking |
| `client_verifier.rs` | mTLS client verification |
| `auto_https.rs` | Automatic HTTPS redirect |

### 7. Middleware (`middleware/`)

Request/response processing pipeline:

| Middleware | Description |
|------------|-------------|
| `waf/` | Web Application Firewall (ModSecurity, Coraza, AWS WAF) |
| `compression/` | Response compression (gzip, brotli, zstd) |
| `rate_limit.rs` | Rate limiting with token bucket |
| `circuit_breaker.rs` | Circuit breaker pattern |
| `request_validation.rs` | Input validation, injection prevention |
| `ddos_protection.rs` | DDoS mitigation |
| `transform.rs` | Header/body transformation |
| `security_audit.rs` | Security event logging |

### 8. Cache (`cache/`)

| Component | Description |
|-----------|-------------|
| `manager.rs` | Cache orchestration |
| `backends.rs` | Backend interfaces |
| `memory.rs` | In-memory caching (DashMap) |
| `disk.rs` | Disk-based persistent cache |
| `redis.rs` | Redis cache backend |

### 9. Observability (`observability/`)

| Component | Description |
|-----------|-------------|
| `metrics.rs` | Prometheus metrics |
| `tracing.rs` | Distributed tracing (OpenTelemetry) |
| `dashboard.rs` | Real-time metrics dashboard |
| `tcp_metrics.rs` | TCP connection metrics |
| `tls_metrics.rs` | TLS handshake metrics |
| `quic_metrics.rs` | QUIC connection metrics |
| `grpc_metrics.rs` | gRPC request metrics |
| `graphql_metrics.rs` | GraphQL query metrics |
| `cache_metrics.rs` | Cache hit/miss metrics |

### 10. Service Discovery (`discovery/`)

| Component | Description |
|-----------|-------------|
| `registry.rs` | Service registry interface |
| `consul.rs` | HashiCorp Consul integration |
| `etcd.rs` | etcd integration |
| `kubernetes.rs` | Kubernetes service discovery |
| `static.rs` | Static backend configuration |

### 11. Admin API (`admin/`)

| Component | Description |
|-----------|-------------|
| `server.rs` | Admin HTTP server |
| `api.rs` | REST API endpoints |
| `dashboard.rs` | Web dashboard |
| `auth.rs` | Admin authentication (JWT, SQLite) |
| `backends.rs` | Backend management |
| `metrics.rs` | Metrics endpoints |
| `cache.rs` | Cache management |
| `upstreams.rs` | Upstream management |
| `stats.rs` | Statistics endpoints |
| `config_persistence.rs` | Configuration persistence |

### 12. Runtime (`runtime/`)

| Component | Description |
|-----------|-------------|
| `mod.rs` | Runtime configuration |
| `buffer_pool.rs` | Zero-copy buffer pooling |
| `backpressure.rs` | Backpressure management |
| `io_backend.rs` | io_uring backend (Linux) |
| `simd_helpers.rs` | SIMD optimizations |

### 13. Plugin System (`plugin/`)

| Component | Description |
|-----------|-------------|
| `trait_def.rs` | Plugin trait definitions |
| `registry.rs` | Plugin registry |
| `manager.rs` | Plugin lifecycle management |
| `wasm_runtime.rs` | WASM plugin support |
| `ffi.rs` | Foreign function interface |
| `hot_reload.rs` | Plugin hot reload |

### 14. Web Server (`webserver/`)

Static file serving capabilities:

| Component | Description |
|-----------|-------------|
| `config.rs` | Web server configuration |
| `static_files.rs` | Static file handler |
| `php_fpm.rs` | PHP-FPM FastCGI support |
| `security.rs` | Security middleware |
| `mime.rs` | MIME type detection |
| `resource_limits.rs` | Resource limiting |

## Data Flow

### Request Processing Pipeline

```
1. Client Connection
   |
2. TLS Handshake (if HTTPS)
   |
3. Protocol Detection (HTTP/1, HTTP/2, gRPC, WebSocket)
   |
4. Request Parsing
   |
5. Middleware Chain (pre-processing)
   |-- WAF inspection
   |-- Rate limiting
   |-- Authentication
   |-- Request validation
   |
6. Route Matching
   |
7. Cache Lookup (if cacheable)
   |-- Cache hit -> Return cached response
   |
8. Load Balancing (select backend)
   |
9. Backend Connection (from pool)
   |
10. Proxy Request to Backend
    |
11. Receive Response
    |
12. Middleware Chain (post-processing)
    |-- Response transformation
    |-- Compression
    |-- Cache store
    |
13. Send Response to Client
```

## Configuration

Highper Gateway supports two configuration formats:

### DSL Format (Caddyfile-like)
```
https://api.example.com:443 {
    tls "/path/to/cert.pem" "/path/to/key.pem"
    proxy http://backend1:8080 http://backend2:8080
    lb round_robin
    health interval=10s path="/health"
    rate_limit 10000 burst=1000
}
```

### YAML Format
```yaml
server:
  listen: "0.0.0.0:443"
  tls:
    cert: "/path/to/cert.pem"
    key: "/path/to/key.pem"
upstreams:
  - name: backend
    servers:
      - address: "backend1:8080"
      - address: "backend2:8080"
    load_balancing: round_robin
```

## Performance Optimizations

1. **Zero-copy I/O** - Buffer pooling, io_uring on Linux
2. **Connection pooling** - Persistent connections to backends
3. **Lock-free data structures** - DashMap for concurrent access
4. **Async/await** - Full async with Tokio runtime
5. **SIMD** - Vectorized operations where applicable
6. **Memory-mapped files** - Efficient static file serving
7. **Backpressure** - Automatic load shedding under pressure

## Security Features

- TLS 1.2/1.3 with modern cipher suites
- mTLS for service-to-service authentication
- OCSP stapling and CRL checking
- Web Application Firewall (OWASP CRS)
- Rate limiting and DDoS protection
- Request validation and sanitization
- Security headers (HSTS, CSP, etc.)

## Deployment Modes

1. **Standalone** - Single binary deployment
2. **Cluster** - Multiple instances with shared state
3. **Sidecar** - Kubernetes sidecar container
4. **Edge** - CDN/edge deployment with caching

## 12-Factor App Compliance

Highper Gateway follows [12-Factor App](https://12factor.net/) methodology:

### I. Codebase - Single Repository
One codebase tracked in Git, multiple deployments via configuration.

### II. Dependencies - Explicit Declaration
All dependencies declared in `Cargo.toml`. No implicit system dependencies.

### III. Config - Environment Variables
All configuration can be overridden via environment variables:
```bash
HIGHPER_SERVER_LISTEN=0.0.0.0:8080
HIGHPER_TLS_ENABLED=true
HIGHPER_DISCOVERY_TYPE=consul
```

### IV. Backing Services - Uniform Resource Abstraction
Backing services (cache, discovery, databases) are treated as attached resources:

| Service Type | Abstraction | Implementations |
|--------------|-------------|-----------------|
| **Cache** | `CacheBackend` trait | Memory, Disk, Redis |
| **Discovery** | `RegistryClient` trait | Consul, etcd, Kubernetes, Static |
| **Metrics** | `MetricsExporter` trait | Prometheus, OpenTelemetry |
| **Tracing** | OpenTelemetry SDK | Jaeger, Zipkin, OTLP |
| **Config Store** | `ConfigProvider` trait | File, etcd, Consul KV |

Service bindings are managed through configuration, not code changes:
```yaml
# Switch from Redis to in-memory cache
cache:
  backend: memory  # or "redis://localhost:6379"

# Switch from Consul to Kubernetes discovery
discovery:
  type: kubernetes  # or "consul", "etcd", "static"
```

### V. Build, Release, Run - Strict Separation
- **Build**: `cargo build --release`
- **Release**: Tagged Docker images with config
- **Run**: Immutable deployment

### VI. Processes - Stateless
Gateway processes are stateless; any state is stored in backing services.
No sticky sessions required (stick tables use external store in cluster mode).

### VII. Port Binding - Self-Contained
Gateway binds directly to ports, no external web server needed.

### VIII. Concurrency - Process Model
Scale via process model (multiple instances behind load balancer).
Supports horizontal scaling with shared-nothing architecture.

### IX. Disposability - Fast Startup/Shutdown
- Startup: <100ms cold start
- Shutdown: Graceful drain with configurable timeout
- Connection draining for zero-downtime deployments

### X. Dev/Prod Parity - Minimal Gap
Same binary runs in all environments; only configuration differs.

### XI. Logs - Event Streams
Logs written to stdout/stderr in JSON format:
```json
{"level":"INFO","target":"highper_gateway","message":"Request processed","trace_id":"abc123"}
```

### XII. Admin Processes - One-off Tasks
Admin tasks via CLI or Admin API, not special processes.

## Service Discovery Architecture

Highper Gateway provides a unified service discovery abstraction:

```
                    +-------------------+
                    |   Gateway Core    |
                    |   (routes to      |
                    |    upstreams)     |
                    +---------+---------+
                              |
                              v
                    +-------------------+
                    | ServiceRegistry   |
                    | (Abstraction)     |
                    +---------+---------+
                              |
         +--------------------+--------------------+
         |                    |                    |
+--------v--------+  +--------v--------+  +-------v--------+
|     Consul      |  |      etcd       |  |   Kubernetes   |
|  Integration    |  |   Integration   |  |   Integration  |
+-----------------+  +-----------------+  +----------------+
```

### Common Interface

All discovery backends implement `RegistryClient`:

```rust
trait RegistryClient: Send + Sync {
    /// Discover services by name
    async fn discover(&self, service: &str) -> Result<Vec<ServiceInstance>>;

    /// Watch for service changes
    async fn watch(&self, service: &str) -> impl Stream<Item = ServiceChange>;

    /// Register this gateway instance
    async fn register(&self, instance: ServiceInstance) -> Result<()>;

    /// Deregister on shutdown
    async fn deregister(&self) -> Result<()>;

    /// Health check update
    async fn update_health(&self, status: HealthStatus) -> Result<()>;
}
```

### Configuration Examples

**Consul:**
```yaml
discovery:
  type: consul
  consul:
    address: "http://consul.service:8500"
    datacenter: "dc1"
    token: "${CONSUL_TOKEN}"
    health_check_interval: 10s
```

**etcd:**
```yaml
discovery:
  type: etcd
  etcd:
    endpoints:
      - "http://etcd1:2379"
      - "http://etcd2:2379"
    prefix: "/services"
    username: "${ETCD_USERNAME}"
    password: "${ETCD_PASSWORD}"
```

**Kubernetes:**
```yaml
discovery:
  type: kubernetes
  kubernetes:
    namespace: "default"
    label_selector: "app=backend"
    port_name: "http"
```

**Static (for development/testing):**
```yaml
discovery:
  type: static
  static_backends:
    - id: "backend-1"
      address: "192.168.1.10"
      port: 8080
    - id: "backend-2"
      address: "192.168.1.11"
      port: 8080
```

## Further Reading

- [DSL Syntax Reference](DSL_SYNTAX.md)
- [Deployment Guide](DEPLOYMENT_GUIDE.md)
- [Admin API Reference](ADMIN_API.md)
- [Security Features](SECURITY_FEATURES.md)
- [HTTP/3 Support](HTTP3.md)
- [mTLS Configuration](MTLS.md)
