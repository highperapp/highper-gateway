# Highper Gateway - Feature Improvement Roadmap

Based on the feature comparison matrix analysis, this document outlines improvements needed to achieve feature parity and competitive advantage against Nginx, Nginx Plus, HAProxy, Caddy, KrakenD, and Pingora.

---

## Priority Classification

| Priority | Meaning | Timeline |
|----------|---------|----------|
| P0 | Critical - Required for production | Immediate |
| P1 | High - Competitive necessity | Short-term |
| P2 | Medium - Nice to have | Medium-term |
| P3 | Low - Future consideration | Long-term |

---

## A. Load Balancing Algorithms

### A1. Slow Start (P2) ✅ IMPLEMENTED
**Gap:** HAProxy and Nginx Plus support slow start for new backends.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Gradually increases traffic to newly added/recovered backends
- Prevents overwhelming servers after maintenance
- Configurable ramp-up period with linear weight increase
- Integrates with health check recovery

**Configuration:**
```yaml
upstreams:
  - name: web-servers
    servers:
      - url: http://backend1:8080
      - url: http://backend2:8080
    slow_start:
      enabled: true
      duration_secs: 60           # Seconds to reach full weight
      initial_weight_percent: 10  # Start at 10% of full weight
```

**How it works:**
1. When a backend is added or recovers from unhealthy state, its join time is recorded
2. During the slow start period, effective weight is calculated as:
   - `effective_weight = base_weight * (initial% + (100% - initial%) * elapsed/duration)`
3. Load balancer uses effective weight for selection decisions
4. After duration expires, backend receives full traffic

**Implementation Details:**
- `BackendServer.join_time` tracks when backend was added/recovered
- `BackendServer.effective_weight()` calculates current weight based on elapsed time
- `LoadBalancer.weighted_selection()` uses effective weights for fair distribution
- `LoadBalancer.least_connections()` considers weight ratio for connection distribution
- Minimum effective weight is always 1 (never completely excluded)

**API Support:**
```bash
# Reset join time for a backend (triggers slow start)
# This happens automatically on health check recovery
```

**Example - Gradual Ramp-up:**
```
Time 0s:   10% weight (initial)
Time 15s:  32.5% weight
Time 30s:  55% weight
Time 45s:  77.5% weight
Time 60s:  100% weight (full traffic)
```

---

### A2. Least Response Time (P1) ✅ IMPLEMENTED
**Gap:** HAProxy, Nginx Plus, and Pingora support this.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Routes to backend with lowest average response time
- Better than least_conn for heterogeneous backends
- Adapts to varying backend performance
- New backends without data are preferred initially for fair probing

**Configuration:**
```yaml
# YAML config
backends:
  - name: api-servers
    algorithm: least_response_time
```

```hcl
# DSL config
example.com {
    proxy backend1:8080 backend2:8080
    lb least_response_time
}
```

**Implementation Details:**
- Rolling window of last 100 response times per backend
- Atomic response time tracking for thread-safety
- Tie-breaking with least connections
- Response times recorded on both success and failure
- Supports percentile queries (p50, p90, p99)

---

## B. Security Features

### B1. Automatic HTTPS / Let's Encrypt Integration (P1) ✅ IMPLEMENTED
**Gap:** Caddy's killer feature - zero-config HTTPS.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Automatic certificate provisioning via ACME (Let's Encrypt, ZeroSSL, Buypass)
- Automatic renewal before expiry (configurable, default 30 days)
- HTTP-01 challenge handling integrated with proxy
- Background renewal task for continuous certificate management
- No manual certificate management required

**Configuration:**
```yaml
# YAML config
tls:
  auto: true                    # Enable automatic HTTPS
  acme:
    provider: letsencrypt       # or zerossl, buypass
    email: admin@example.com
    staging: false              # Use staging for testing
    domains:                    # Optional: auto-detected from routes if empty
      - example.com
      - api.example.com
    challenge_type: http-01     # HTTP-01 challenge
    storage:
      storage_type: file
      path: /var/lib/highper-gateway/certs
    renewal_days: 30            # Renew 30 days before expiry
    renew_check_interval: 1h    # Check every hour
```

**Implementation Details:**
- Full ACME protocol implementation via `instant-acme`
- AutoHttpsManager for automatic certificate lifecycle
- ChallengeStore for HTTP-01 challenge token storage
- Background renewal task with configurable interval
- Certificate status monitoring and reporting
- Support for multiple ACME providers (Let's Encrypt, ZeroSSL, Buypass)
- Automatic staging/production URL selection

**Key Components:**
- `AutoHttpsManager` - Manages automatic certificate provisioning and renewal
- `AcmeClient` - ACME protocol client for certificate requests
- `ChallengeStore` - HTTP-01 challenge token storage
- `DynamicCertResolver` - SNI-based certificate resolution

---

## C. Caching & Performance

### C1. Disk Cache (P2) ✅ IMPLEMENTED
**Gap:** Nginx has robust disk caching for large objects.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Persistent cache survives restarts
- Handles objects larger than memory
- Tiered caching (memory hot, disk warm)
- LRU eviction when size limit reached
- Optional zstd compression

**Configuration:**
```yaml
# Disk-only cache
cache:
  type: disk
  path: /var/cache/highper-gateway
  max_size: 10737418240     # 10GB
  min_object_size: 0        # Cache everything (or set to skip small objects)
  compression: true         # Enable zstd compression

# Tiered cache (memory hot + disk warm) - RECOMMENDED
cache:
  type: tiered
  disk_path: /var/cache/highper-gateway
  disk_size: 10737418240    # 10GB disk
  hot_max_size: 1048576     # Objects up to 1MB cached in memory
  compression: true
```

**Cache Backend Types:**
| Type | Description | Persistence |
|------|-------------|-------------|
| `inmemory` | Fast DashMap-based | No |
| `disk` | File-based with LRU | Yes |
| `tiered` | Memory (hot) + Disk (warm) | Yes |
| `redis` | Distributed Redis | Yes |
| `multitier` | Local + Distributed | Yes |

**Implementation Features:**
- ✅ SHA256-based key to file path mapping
- ✅ Subdirectory structure (avoids too many files per directory)
- ✅ Atomic writes (temp file + rename)
- ✅ LRU eviction with configurable size limits
- ✅ Automatic cleanup of expired entries
- ✅ Optional zstd compression for large objects
- ✅ In-memory index for fast lookups
- ✅ Background cleanup task
- ✅ Tiered caching with automatic promotion

**Key Components:**
- `DiskBackend` - Persistent file-based cache
- `TieredBackend` - Memory hot tier + Disk warm tier
- `DiskCacheConfig` - Configuration options
- LRU eviction based on last access time

**Example Usage:**
```rust
use highper_gateway::cache::{CacheManager, CacheBackendType};

// Create tiered cache
let cache = CacheManager::new(CacheBackendType::Tiered {
    disk_path: "/var/cache/highper-gateway".into(),
    disk_size: 10 * 1024 * 1024 * 1024, // 10GB
    hot_max_size: 1024 * 1024, // 1MB
    compression: true,
}).await?;

// Use cache
cache.set("key", &value, Some(Duration::from_secs(3600))).await?;
let value: Option<MyType> = cache.get("key").await?;
```

---

## D. Observability

### D1. OpenTelemetry Native Support (P1) ✅ IMPLEMENTED
**Gap:** KrakenD and Nginx Plus have better tracing integration.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Unified observability (traces, metrics, logs)
- Vendor-agnostic export (Jaeger, OTLP, stdout)
- Automatic context propagation (W3C Trace Context)
- OTLP support for both traces and metrics

**Configuration:**
```yaml
# YAML config
tracing:
  enabled: true
  exporter: otlp              # Options: jaeger, otlp, stdout
  endpoint: "http://otel-collector:4317"
  sample_rate: 1.0
  service_name: "highper-gateway"
  resource_attributes:
    environment: production

  # OTLP-specific settings
  otlp:
    metrics_enabled: true     # Enable OTLP metrics export
    export_interval_secs: 60  # Metrics export interval
    compression: false        # gRPC compression
    timeout_secs: 30          # Export timeout
    headers: {}               # Custom headers for auth
```

**Implementation Details:**
- Full OTLP support via gRPC (opentelemetry-otlp)
- Batch exporter for traces (efficient batching)
- Periodic metrics export to any OTLP backend
- W3C Trace Context propagation (traceparent, tracestate)
- Integration with Rust tracing ecosystem
- Support for custom resource attributes

**Supported Backends:**
- Jaeger (native and via OTLP)
- Grafana Tempo
- Honeycomb
- Datadog
- Any OTLP-compatible collector

---

### D2. Status Dashboard (P2) ✅ IMPLEMENTED
**Gap:** HAProxy and KrakenD have built-in dashboards.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Real-time traffic visualization
- Backend health status with live monitoring
- Route configuration viewer
- Cache statistics and hit rate tracking
- System info (version, uptime, workers)
- Auto-refresh every 2 seconds
- No external tools required - embedded single-page app

**Access:**
```
http://admin-server:9091/dashboard
# or
http://admin-server:9091/
```

**Dashboard Features:**
- **Overview Stats:** RPS, active connections, avg latency, error rate
- **Backends Tab:** Server health status, connections, requests, latency per backend
- **Routes Tab:** Route configuration, match criteria, upstream mapping
- **Cache Tab:** Hit rate with progress bar, entries count, hits/misses, backend type
- **System Tab:** Version, uptime, workers, memory usage, CPU usage
- **Request History Chart:** Last 60 seconds of request rate

**Implementation Details:**
- Embedded HTML/JS dashboard (no build tools required)
- Polls `/api/stats`, `/api/backends`, `/api/routes`, `/api/cache/stats`
- Dark theme with responsive design
- Status badge (Healthy/Degraded/Unhealthy) based on error rate
- Tab-based navigation for organized information

**Configuration:**
```yaml
admin:
  bind: "127.0.0.1:9091"
  # Dashboard is automatically available at /dashboard
  # No additional configuration required
```

---

## E. API Gateway Features

### E1. Request Validation (P2) ✅ IMPLEMENTED
**Gap:** KrakenD validates requests against schemas.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- JSON Schema validation for request bodies
- Response validation (for debugging/monitoring)
- Reject malformed requests early with detailed errors
- Schema caching for performance

**Configuration:**
```yaml
routes:
  - name: create-user
    match:
      paths: ["/api/users"]
      methods: [POST]
    upstream: api-backend
    validation:
      request:
        # Option 1: External schema file
        schema_file: /etc/highper-gateway/schemas/user.json
        # Option 2: Inline JSON Schema
        json_schema:
          type: object
          required: [name, email]
          properties:
            name: { type: string, minLength: 1 }
            email: { type: string, format: email }
        # Content types to validate (default: ["application/json"])
        content_types:
          - application/json
        # Action on failure: "reject" (400) or "warn" (log only)
        on_failure: reject
        # Include validation errors in response
        include_errors: true
      response:
        # Response validation (for debugging)
        json_schema:
          type: object
          properties:
            id: { type: integer }
            name: { type: string }
        on_failure: warn  # Log only for responses
```

**Validation Actions:**
| Action | Request Behavior | Response Behavior |
|--------|-----------------|-------------------|
| `reject` | Return 400 Bad Request | Return 502 Bad Gateway |
| `warn` | Log warning, continue | Log warning, continue |

**Error Response Format:**
```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "details": [
    { "path": "/email", "message": "\"email\" is a required property" },
    { "path": "/age", "message": "must be a positive integer" }
  ]
}
```

**Implementation Details:**
- Uses `jsonschema` crate for JSON Schema Draft 7 validation
- Schema cache with compile-once pattern for performance
- Supports both inline schemas and external schema files
- Automatic content-type detection (validates JSON only)
- Non-JSON requests are skipped automatically
- Thread-safe schema cache using `parking_lot::RwLock`

**Key Components:**
- `SchemaCache` - Compiled schema cache
- `RequestValidator` - Request body validation
- `ResponseValidator` - Response body validation
- `ValidationConfig` - Route-level configuration

---

### E2. Response Aggregation (P3)
**Gap:** KrakenD's core feature - combine multiple backend responses.

**What it does:**
- Call multiple backends in parallel
- Merge responses into single JSON
- GraphQL-like composition without GraphQL

**Implementation Plan:**
```yaml
routes:
  - match:
      path: /api/dashboard
    aggregate:
      backends:
        - name: user-service
          path: /users/$user_id
          field: user
        - name: orders-service
          path: /orders?user=$user_id
          field: orders
        - name: notifications-service
          path: /notifications/$user_id
          field: notifications
      strategy: parallel  # or sequential
      timeout: 5000
      fail_strategy: partial  # Return partial results if one fails
```

**Effort:** High (2-3 weeks)

---

## F. Service Discovery

### F1. etcd Support (P2) ✅ IMPLEMENTED
**Gap:** KrakenD supports etcd for configuration and discovery.

**Status:** ✅ **Already Implemented** (feature-gated)

**What it does:**
- Watch etcd for backend changes with real-time updates
- Service registration with TTL-based leases
- Automatic lease keep-alive for registered services
- Background cache refresh
- Health status management

**Configuration:**
```yaml
discovery:
  type: Etcd
  addresses:
    - "http://etcd1:2379"
    - "http://etcd2:2379"
  refresh_interval: 30      # Cache refresh interval in seconds
  service_name: my-service  # Service to discover
  health_check_enabled: true
  only_healthy: true        # Only return healthy instances
```

**Features:**
- ✅ Service instance discovery with caching
- ✅ Healthy instance filtering
- ✅ Service registration with automatic TTL leases
- ✅ Service deregistration
- ✅ Health status updates
- ✅ Watch for service changes (real-time updates)
- ✅ Background cache refresh

**Service Instance Format (stored in etcd):**
```json
{
  "id": "service-instance-1",
  "name": "api-service",
  "address": "192.168.1.100",
  "port": 8080,
  "health": "Passing",
  "tags": ["production", "v1"],
  "metadata": {
    "version": "1.2.3",
    "region": "us-east-1"
  }
}
```

**Key Structure:**
- Services stored at: `/services/{service_name}/{instance_id}`
- Automatic cleanup via 60-second TTL leases
- Keep-alive every 20 seconds

**Usage:**
```rust
use highper_gateway::discovery::{create_discovery, DiscoveryConfig, DiscoveryBackend};

let config = DiscoveryConfig {
    backend_type: DiscoveryBackend::Etcd,
    addresses: vec!["http://localhost:2379".to_string()],
    refresh_interval: 30,
    service_name: Some("my-service".to_string()),
    health_check_enabled: true,
    only_healthy: true,
    ..Default::default()
};

let discovery = create_discovery(config).await?;
let instances = discovery.get_healthy_instances("my-service").await?;
```

**Feature Flag:**
Enable with `etcd-client` feature in Cargo.toml:
```toml
[dependencies]
highper-gateway = { version = "0.1", features = ["etcd-client"] }
```

---

## G. Configuration & Operations

### G1. JSON Configuration (P2) ✅ IMPLEMENTED
**Gap:** Caddy and KrakenD support JSON natively.

**Status:** ✅ **Already Implemented**

**What it does:**
- JSON as alternative to YAML/HCL/TOML
- Better for programmatic generation
- API configuration compatibility
- Auto-detect format from file extension

**Supported Formats:**
| Extension | Format |
|-----------|--------|
| `.yaml`, `.yml` | YAML |
| `.json` | JSON |
| `.toml` | TOML |
| `.proxy` | Caddy-like DSL |

**Usage:**
```bash
# Use JSON config
highper-gateway --config config.json

# Use YAML config
highper-gateway --config config.yaml

# Use TOML config
highper-gateway --config config.toml

# Use DSL config
highper-gateway --config config.proxy
```

**Example JSON Configuration:**
```json
{
  "server": {
    "bind": ["0.0.0.0:8080"],
    "workers": "auto"
  },
  "upstreams": [
    {
      "name": "api",
      "servers": [
        {"url": "http://backend1:8080", "weight": 100},
        {"url": "http://backend2:8080", "weight": 100}
      ]
    }
  ],
  "routes": [
    {
      "name": "api-route",
      "upstream": "api",
      "match": {
        "paths": ["/api/*"]
      }
    }
  ]
}
```

---

### G2. API-Based Configuration (P1) ✅ IMPLEMENTED
**Gap:** Nginx Plus and Caddy support runtime configuration via API.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Add/remove backends and upstreams without restart
- Full CRUD for routes and upstreams at runtime
- Configuration persistence to disk
- Real-time server management (add/remove backend servers)
- Dynamic load balancing algorithm updates

**Configuration:**
```yaml
admin:
  bind: "127.0.0.1:9091"
  auth_enabled: true
  api_keys:
    - "your-api-key-here"
  read_only: false         # Allow mutations
  cors_enabled: true
```

**API Endpoints:**

**Routes Management:**
```
GET    /api/routes              # List all routes
POST   /api/routes              # Create new route
GET    /api/routes/:name        # Get route details
PUT    /api/routes/:name        # Update route
DELETE /api/routes/:name        # Delete route
```

**Upstreams Management:**
```
GET    /api/upstreams                          # List all upstreams
POST   /api/upstreams                          # Create new upstream
GET    /api/upstreams/:name                    # Get upstream details
PUT    /api/upstreams/:name                    # Update upstream
DELETE /api/upstreams/:name                    # Delete upstream
GET    /api/upstreams/:name/health             # Get health status
POST   /api/upstreams/:name/servers            # Add server to upstream
DELETE /api/upstreams/:name/servers            # Remove server from upstream
PUT    /api/upstreams/:name/load-balancing     # Update load balancing config
```

**Backends Control:**
```
GET    /api/backends                    # List all backends
GET    /api/backends/:id                # Get backend details
POST   /api/backends/:id/enable         # Enable backend
POST   /api/backends/:id/disable        # Disable backend
POST   /api/backends/:id/drain          # Drain backend (graceful shutdown)
GET    /api/backends/:id/drain-status   # Get drain status
POST   /api/backends/:id/cancel-drain   # Cancel drain
POST   /api/backends/:id/health-check   # Force health check
```

**Configuration Persistence:**
```
POST   /api/config/save         # Save runtime config to disk
GET    /api/config/export       # Export current config as JSON
POST   /api/config/reload       # Trigger config reload
```

**Cache Management:**
```
GET    /api/cache/stats         # Get cache statistics
GET    /api/cache/keys          # List cache keys
POST   /api/cache/clear         # Clear cache
POST   /api/cache/invalidate    # Invalidate specific keys
```

**Health & Metrics:**
```
GET    /health                  # Health check
GET    /ready                   # Readiness check
GET    /metrics                 # Prometheus metrics
GET    /api/stats               # Runtime statistics
```

**Example API Usage:**

Create a new upstream:
```bash
curl -X POST http://localhost:9091/api/upstreams \
  -H "X-API-Key: your-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-upstream",
    "servers": [
      {"url": "http://backend1:8080", "weight": 100},
      {"url": "http://backend2:8080", "weight": 50}
    ],
    "load_balancing": {"algorithm": "round_robin"},
    "health_checks": {"enabled": true, "interval": 10, "timeout": 5}
  }'
```

Add a server to existing upstream:
```bash
curl -X POST http://localhost:9091/api/upstreams/my-upstream/servers \
  -H "X-API-Key: your-key" \
  -H "Content-Type: application/json" \
  -d '{"url": "http://backend3:8080", "weight": 75}'
```

Update load balancing algorithm:
```bash
curl -X PUT http://localhost:9091/api/upstreams/my-upstream/load-balancing \
  -H "X-API-Key: your-key" \
  -H "Content-Type: application/json" \
  -d '{"algorithm": "least_response_time"}'
```

Save configuration to disk:
```bash
curl -X POST http://localhost:9091/api/config/save \
  -H "X-API-Key: your-key"
```

**Implementation Details:**
- `UpstreamManager` - Runtime upstream CRUD operations
- `RouteManager` - Runtime route management
- `ConfigPersistence` - Atomic config save/load to YAML
- Authentication via API keys or JWT tokens
- Read-only mode for production safety
- CORS support for web dashboards

**Key Components:**
- `src/admin/upstreams.rs` - Upstream management endpoints
- `src/admin/routes.rs` - Route management endpoints
- `src/admin/config_persistence.rs` - Configuration persistence
- `src/admin/backends.rs` - Backend control endpoints
- `src/admin/server.rs` - Admin API server with all endpoints

---

### G3. HCL/DSL Improvements (P2)
**Gap:** Current DSL could be more expressive.

**Improvements:**
1. **Variables/Interpolation:**
   ```hcl
   variable "env" {
     default = "production"
   }

   backend "api" {
     server {
       address = "api-${var.env}.example.com:8080"
     }
   }
   ```

2. **Conditionals:**
   ```hcl
   listener "https" {
     tls = var.env == "production" ? "strict" : "permissive"
   }
   ```

3. **Loops:**
   ```hcl
   dynamic "server" {
     for_each = var.backend_ips
     content {
       address = "${server.value}:8080"
     }
   }
   ```

4. **Includes:**
   ```hcl
   include "common/tls.hcl"
   include "backends/*.hcl"
   ```

**Effort:** Medium-High (2 weeks)

---

## H. Web Server & Static File Improvements

### H1. Enhanced Static File Serving (P2)
**Gap:** Nginx excels at static file serving.

**Improvements:**
```yaml
static:
  root: /var/www/html

  # Directory index
  autoindex:
    enabled: true
    format: html  # or json, xml

  # Range requests for video streaming
  range_requests: true

  # Sendfile optimization
  sendfile: true
  tcp_nopush: true

  # Pre-compressed files
  gzip_static: true
  brotli_static: true

  # Fallback handling (SPA)
  try_files:
    - $uri
    - $uri/
    - /index.html
```

**Effort:** Medium (1 week)

---

## I. L4 Load Balancing & Database Focus

### I1. Database Protocol Awareness (P2)
**Gap:** HAProxy has deep MySQL/PostgreSQL protocol support.

**Improvements:**
```yaml
backends:
  - name: mysql-pool
    protocol: mysql
    mysql:
      # Read/write splitting
      read_write_split:
        enabled: true
        primary: mysql-primary:3306
        replicas:
          - mysql-replica-1:3306
          - mysql-replica-2:3306

      # Connection multiplexing
      multiplexing:
        enabled: true
        max_connections_per_server: 100

      # Query routing
      query_routing:
        - pattern: "^SELECT"
          target: replicas
        - pattern: ".*"
          target: primary
```

**Effort:** High (3-4 weeks per protocol)

---

### I2. Connection Draining (P1) ✅ IMPLEMENTED
**Gap:** HAProxy has robust connection draining.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Gracefully remove backends without dropping connections
- Wait for in-flight requests to complete
- Configurable drain timeout with background monitoring
- Real-time drain status monitoring
- Drain can be cancelled if needed

**Admin API Usage:**
```bash
# Start draining a backend (with 60 second timeout)
curl -X POST "http://localhost:9000/api/backends/upstream_0/drain" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Maintenance", "drain_timeout_seconds": 60}'

# Check drain status
curl "http://localhost:9000/api/backends/upstream_0/drain-status"

# Cancel drain
curl -X POST "http://localhost:9000/api/backends/upstream_0/cancel-drain"
```

**Implementation Details:**
- Background task monitors active connections and timeout
- Drain completes when connections reach 0 or timeout expires
- Load balancer automatically excludes draining backends
- Full drain status available via API (elapsed time, remaining time, active connections)

---

### I3. Stick Tables / Session Persistence (P2)
**Gap:** HAProxy's stick tables for stateful load balancing.

**What it does:**
- Track client → backend mappings
- Persist across connections
- Shared across cluster nodes (optional)

**Implementation:**
```yaml
stick_tables:
  - name: user_sessions
    type: ip
    size: 1000000
    expire: 30m
    store:
      - conn_cur
      - conn_rate(10s)
      - http_req_rate(10s)

backends:
  - name: api-servers
    stick:
      table: user_sessions
      on: src  # or cookie, header
```

**Effort:** High (2-3 weeks)

---

## J. Simplicity & Developer Experience

### J1. Zero-Config Mode (P1) ✅ IMPLEMENTED
**Gap:** Caddy works out of the box with minimal config.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Sensible defaults for everything
- Quick CLI startup without config files
- Minimal required configuration
- All 8 load balancing algorithms available via CLI flag
- Optional automatic HTTPS with Let's Encrypt

**Quick Start (CLI):**
```bash
# Simplest possible start - proxy to a single backend
highper-gateway run --backend localhost:3000

# Multiple backends with load balancing
highper-gateway run --backend backend1:8080 --backend backend2:8080

# Custom port and load balancing algorithm
highper-gateway run --backend api:8080 --port 80 --lb least_response_time

# With automatic HTTPS
highper-gateway run --backend localhost:3000 --tls --domain example.com --email admin@example.com

# With HTTP/3 and admin API
highper-gateway run --backend localhost:3000 --http3 --admin-port 9000

# With compression and custom timeout
highper-gateway run --backend localhost:3000 --compress --timeout 60
```

**CLI Options:**
```
Options:
  -b, --backend <URL>        Backend server URL(s) - can be repeated
  -p, --port <PORT>          Port to listen on [default: 8080]
      --bind <ADDR>          Bind address [default: 0.0.0.0]
      --tls                  Enable automatic HTTPS (Let's Encrypt)
      --domain <DOMAIN>      Domain for HTTPS (required with --tls)
      --email <EMAIL>        Email for Let's Encrypt
      --lb <ALGORITHM>       Load balancing: round_robin, least_conn,
                             least_response_time, random, ip_hash,
                             consistent_hash, power_of_two, geographic, maglev
      --admin-port <PORT>    Enable admin API on port
      --http3                Enable HTTP/3 (QUIC)
      --compress             Enable compression (gzip, brotli, zstd)
      --timeout <SECS>       Request timeout [default: 30]
      --max-conns <NUM>      Max connections per backend [default: 1000]
```

**Configuration File (still supported):**
```yaml
# Minimal config - everything else uses smart defaults
server:
  bind: ["0.0.0.0:8080"]
upstreams:
  - name: default
    servers:
      - url: http://localhost:3000
routes:
  - name: default
    upstream: default
    match:
      paths: ["/*"]
```

**Sensible Defaults:**
- Workers: auto-detected based on CPU cores
- Load balancing: round_robin
- Health checks: enabled
- Connection pooling: enabled
- Metrics: enabled on :9090
- Timeout: 30 seconds
- Keep-alive: 60 seconds

---

### J2. Quick Setup Script (P1) ✅ IMPLEMENTED
**Gap:** Caddy's installation is trivial.

**Status:** ✅ **Implemented in v1.1.0**

**One-liner Installation:**
```bash
# Basic installation (latest version)
curl -fsSL https://raw.githubusercontent.com/anthropics/highper-gateway/main/scripts/install.sh | bash

# Install with quick-start config
curl ... | bash -s -- --backend localhost:3000 --port 80

# Install with automatic HTTPS
curl ... | bash -s -- --backend localhost:3000 --tls auto --domain example.com --email admin@example.com

# Install specific version
curl ... | bash -s -- --version v0.1.0
```

**Script Options:**
```
Options:
  -p, --port PORT         HTTP port to listen on (default: 8080)
  -b, --backend URL       Backend server URL
  -t, --tls MODE          TLS mode: auto (Let's Encrypt) or manual
  -d, --domain DOMAIN     Domain for automatic TLS
  -e, --email EMAIL       Email for Let's Encrypt registration
  -v, --version VERSION   Version to install (default: latest)
  --skip-service          Don't create systemd service
  --uninstall             Uninstall Highper Gateway
  -h, --help              Show help message
```

**Features:**
- ✅ Linux-only (io_uring requires kernel 5.1+)
- ✅ Kernel version check with helpful warnings
- ✅ Support for x86_64, aarch64, armv7 architectures
- ✅ Distribution detection (Ubuntu, Debian, RHEL, Fedora, etc.)
- ✅ Download correct binary from GitHub releases
- ✅ Install systemd service with security hardening
- ✅ Generate minimal configuration file
- ✅ Auto-configure TLS with Let's Encrypt
- ✅ Create necessary directories
- ✅ Uninstall support
- ✅ Colored output and helpful messages

**Installation Directories:**
- Binary: `/usr/local/bin/highper-gateway`
- Config: `/etc/highper-gateway/config.yaml`
- Data: `/var/lib/highper-gateway/`
- Logs: `/var/log/highper-gateway/`

**Systemd Service Features:**
- Automatic restart on failure
- Security hardening (NoNewPrivileges, ProtectSystem, etc.)
- High file descriptor limit (65535)
- HUP signal for config reload

---

## K. KrakenD-Competitive Features

### K1. Backend-for-Frontend (BFF) Pattern (P3)
**Gap:** KrakenD excels at API composition.

**Implementation:**
```yaml
routes:
  - match:
      path: /mobile/home
    bff:
      name: mobile-home
      endpoints:
        - backend: user-service
          path: /me
          field: user
        - backend: product-service
          path: /featured
          field: products
          transform:
            - type: jq
              expression: ".items[:5]"
        - backend: notification-service
          path: /unread/count
          field: notification_count
```

**Effort:** High (3-4 weeks)

---

### K2. Response Transformation (P2) ✅ IMPLEMENTED
**Gap:** KrakenD has powerful response manipulation.

**Status:** ✅ **Implemented in v1.1.0**

**What it does:**
- Rename fields in JSON responses
- Remove sensitive/internal fields
- Add static fields (e.g., API version)
- Flatten nested objects
- Pick/whitelist specific fields
- Extract nested field as root
- Transform array elements
- Request body transformation (optional)

**Configuration:**
```yaml
routes:
  - name: api-v2
    match:
      paths: ["/api/v2/users"]
    upstream: user-service-v1
    transform:
      response:
        # Rename fields
        rename:
          user_name: username
          user_email: email
        # Remove sensitive fields
        remove:
          - internal_id
          - password_hash
          - created_at
        # Add fields
        add:
          api_version: "2.0"
          processed: true
        # Flatten nested objects (bring fields to root)
        flatten:
          - user_profile
        # Pick only specific fields (whitelist)
        pick:
          - id
          - username
          - email
        # Extract nested field as root
        extract: data
        # Apply to array elements
        map_array: true
        # Content types to transform
        content_types:
          - application/json
      request:
        # Optional: Transform request body
        rename:
          userName: user_name
        remove:
          - csrf_token
```

**Transformation Operations:**
| Operation | Description |
|-----------|-------------|
| `rename` | Rename fields (old → new) |
| `remove` | Remove fields by path |
| `add` | Add static fields |
| `flatten` | Bring nested object fields to root |
| `pick` | Keep only specified fields (whitelist) |
| `extract` | Use nested field as new root |
| `map_array` | Apply transforms to each array element |

**Example Transformations:**

1. **Rename fields:**
```json
// Before: {"user_name": "john"}
// After:  {"username": "john"}
```

2. **Remove sensitive data:**
```json
// Before: {"name": "john", "password": "secret", "internal_id": 123}
// After:  {"name": "john"}
```

3. **Flatten nested object:**
```json
// Before: {"id": 1, "user": {"name": "john", "email": "j@x.com"}}
// After:  {"id": 1, "name": "john", "email": "j@x.com"}
```

4. **Extract nested data:**
```json
// Before: {"status": "ok", "data": {"id": 1, "name": "john"}}
// After:  {"id": 1, "name": "john"}
```

**Implementation Details:**
- Efficient JSON parsing with serde_json
- Supports dot-notation for nested paths
- Content-type based filtering
- Non-JSON content passed through unchanged
- Request transformation for API versioning

---

## Implementation Priority Matrix

| Feature | Priority | Effort | Dependencies | Phase |
|---------|----------|--------|--------------|-------|
| Automatic HTTPS | P1 | High | None | 1 |
| API-Based Config | P1 | High | None | 1 |
| Least Response Time LB | P1 | Medium | None | 1 |
| Connection Draining | P1 | Medium | None | 1 |
| Zero-Config Mode | P1 | Medium | None | 1 |
| Quick Install Script | P1 | Low | None | 1 |
| OpenTelemetry | P1 | Medium | None | 1 |
| Slow Start | P2 | Medium | None | ✅ Done |
| Disk Cache | P2 | High | None | ✅ Done |
| Status Dashboard | P2 | High | API Config | ✅ Done |
| Request Validation | P2 | Medium | None | ✅ Done |
| etcd Discovery | P2 | Medium | None | ✅ Done |
| JSON Config | P2 | Low | None | ✅ Done |
| HCL Improvements | P2 | Medium | None | 2 |
| Static File Enhancements | P2 | Medium | None | 2 |
| Stick Tables | P2 | High | None | 2 |
| Database Protocol | P2 | High | None | 2 |
| Response Aggregation | P3 | High | None | 3 |
| BFF Pattern | P3 | High | Response Agg | 3 |
| Response Transformation | P2 | Medium | None | ✅ Done |

---

## Estimated Timeline

### Phase 1 (P1 Features) - 6-8 Weeks
- Automatic HTTPS / ACME
- API-Based Configuration
- Least Response Time LB
- Connection Draining
- Zero-Config Mode
- Quick Install Script
- OpenTelemetry Support

### Phase 2 (P2 Features) - 8-12 Weeks
- ✅ Slow Start (Implemented)
- ✅ Disk Cache (Implemented)
- ✅ Status Dashboard (Implemented)
- Request Validation
- etcd Discovery
- JSON Config
- HCL Improvements
- Static File Enhancements
- Stick Tables
- Response Transformation

### Phase 3 (P3 Features) - 6-8 Weeks
- Response Aggregation
- BFF Pattern
- Database Protocol Awareness (MySQL/PostgreSQL)

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Feature parity vs Caddy | 75% | 95% |
| Feature parity vs HAProxy | 70% | 90% |
| Feature parity vs KrakenD | 60% | 80% |
| Time to first request (fresh install) | 5 min | 30 sec |
| Configuration complexity score | Medium | Low |

---

## References

- [Caddy Automatic HTTPS](https://caddyserver.com/docs/automatic-https)
- [HAProxy Stick Tables](https://www.haproxy.com/blog/introduction-to-haproxy-stick-tables)
- [KrakenD Aggregation](https://www.krakend.io/docs/backends/data-manipulation/)
- [OpenTelemetry Rust SDK](https://docs.rs/opentelemetry/latest/opentelemetry/)
- [ACME Protocol RFC 8555](https://datatracker.ietf.org/doc/html/rfc8555)
