# Implementation Plan - Complete All Validation Gaps

Based on COMPREHENSIVE_VALIDATION.md analysis, this plan addresses all identified gaps and creates complete configuration templates and load testing for all 15 use case scenarios.

---

## Plan Overview

**Total Phases**: 5
**Estimated Scope**: Complete all Priority 1-3 gaps + templates + load tests
**Git Strategy**: Commit after each phase completion

---

## Phase 1: Protocol-Specific Observability Metrics (Priority 1 - High)

### 1.1 TCP-Specific Metrics Module

**File**: `src/observability/tcp_metrics.rs`

**Metrics to implement**:
- `tcp_active_connections` (Gauge) - Current active TCP connections
- `tcp_total_connections` (Counter) - Total TCP connections established
- `tcp_bytes_sent` (Counter) - Total bytes sent over TCP
- `tcp_bytes_received` (Counter) - Total bytes received over TCP
- `tcp_connection_duration_seconds` (Histogram) - TCP connection duration
- `tcp_connection_errors` (Counter) - TCP connection errors by type
- `tcp_pool_active` (Gauge) - Active connections in pool
- `tcp_pool_idle` (Gauge) - Idle connections in pool
- `tcp_pool_wait_duration_seconds` (Histogram) - Time waiting for pool connection

**Integration points**:
- Hook into TCP proxy handler
- Add connection lifecycle tracking
- Update metrics on connection open/close/error
- Export via `/metrics` endpoint

**Files to modify**:
- `src/proxy/tcp_proxy.rs` (if exists) or relevant TCP handler
- `src/observability/metrics.rs` - Register TCP metrics
- `src/observability/mod.rs` - Export tcp_metrics module

---

### 1.2 TLS-Specific Metrics Module

**File**: `src/observability/tls_metrics.rs`

**Metrics to implement**:
- `tls_handshakes_total` (Counter) - Total TLS handshakes by result (success/failure)
- `tls_handshake_duration_seconds` (Histogram) - TLS handshake duration
- `tls_certificate_expiry_seconds` (Gauge) - Seconds until certificate expires
- `tls_cipher_suite_usage` (Counter) - Usage count by cipher suite
- `tls_protocol_version` (Counter) - Connections by TLS version (1.2, 1.3)
- `tls_client_cert_validations` (Counter) - Client cert validation by result
- `tls_errors` (Counter) - TLS errors by type

**Integration points**:
- Hook into TLS/HTTPS handler
- Track handshake lifecycle
- Monitor certificate expiry (background task)
- Capture cipher suite and protocol version from connection
- Export via `/metrics` endpoint

**Files to modify**:
- `src/tls/mod.rs` or relevant TLS handler
- `src/observability/metrics.rs` - Register TLS metrics
- `src/observability/mod.rs` - Export tls_metrics module

---

### 1.3 QUIC/HTTP/3-Specific Metrics Module

**File**: `src/observability/quic_metrics.rs`

**Metrics to implement**:
- `quic_connections_total` (Counter) - Total QUIC connections
- `quic_zero_rtt_usage` (Counter) - 0-RTT connection attempts by result
- `quic_packet_loss_ratio` (Gauge) - Packet loss ratio
- `quic_rtt_seconds` (Histogram) - Round-trip time
- `quic_streams_active` (Gauge) - Active streams per connection
- `quic_streams_total` (Counter) - Total streams created
- `quic_migration_events` (Counter) - Connection migration events
- `quic_congestion_events` (Counter) - Congestion control events

**Integration points**:
- Hook into HTTP/3 handler
- Access QUIC connection stats
- Track stream lifecycle
- Monitor packet loss and RTT
- Export via `/metrics` endpoint

**Files to modify**:
- `src/http3/mod.rs` or relevant HTTP/3 handler
- `src/observability/metrics.rs` - Register QUIC metrics
- `src/observability/mod.rs` - Export quic_metrics module

---

### 1.4 gRPC-Specific Metrics Module

**File**: `src/observability/grpc_metrics.rs`

**Metrics to implement**:
- `grpc_requests_total` (Counter) - Total gRPC requests by service/method
- `grpc_request_duration_seconds` (Histogram) - Request duration by service/method
- `grpc_streams_active` (Gauge) - Active gRPC streams
- `grpc_streams_total` (Counter) - Total streams by type (unary/client/server/bidi)
- `grpc_messages_sent` (Counter) - Messages sent by service/method
- `grpc_messages_received` (Counter) - Messages received by service/method
- `grpc_status_codes` (Counter) - gRPC status codes (OK, CANCELLED, etc.)
- `grpc_stream_duration_seconds` (Histogram) - Bidirectional stream duration

**Integration points**:
- Hook into gRPC handler
- Parse gRPC service/method from path
- Track stream lifecycle
- Capture gRPC status codes
- Export via `/metrics` endpoint

**Files to modify**:
- `src/grpc/mod.rs` or relevant gRPC handler
- `src/observability/metrics.rs` - Register gRPC metrics
- `src/observability/mod.rs` - Export grpc_metrics module

---

### 1.5 GraphQL-Specific Metrics Module

**File**: `src/observability/graphql_metrics.rs`

**Metrics to implement**:
- `graphql_queries_total` (Counter) - Total GraphQL queries by operation name
- `graphql_query_duration_seconds` (Histogram) - Query execution time by operation
- `graphql_query_complexity` (Histogram) - Query complexity score
- `graphql_query_depth` (Histogram) - Query depth
- `graphql_resolver_duration_seconds` (Histogram) - Resolver latency by field
- `graphql_batched_queries` (Counter) - Batched query count
- `graphql_errors` (Counter) - GraphQL errors by type

**Integration points**:
- Hook into GraphQL handler
- Parse GraphQL query to extract operation name
- Calculate complexity and depth
- Track resolver execution
- Export via `/metrics` endpoint

**Files to modify**:
- `src/graphql/mod.rs` or relevant GraphQL handler
- `src/observability/metrics.rs` - Register GraphQL metrics
- `src/observability/mod.rs` - Export graphql_metrics module

---

### 1.6 Enhanced Cache Metrics

**Extend**: `src/observability/cache_metrics.rs` (if exists) or add to main metrics

**Additional metrics**:
- `cache_size_bytes` (Gauge) - Current cache size in bytes
- `cache_entries` (Gauge) - Number of entries in cache
- `cache_evictions_total` (Counter) - Cache evictions by reason (ttl/lru/size)
- `cache_ttl_distribution_seconds` (Histogram) - TTL distribution of cached items
- `cache_key_size_bytes` (Histogram) - Cache key size distribution
- `cache_value_size_bytes` (Histogram) - Cache value size distribution

**Files to modify**:
- `src/cache/mod.rs` or relevant cache implementation
- `src/observability/metrics.rs` - Register cache metrics

---

### Phase 1 Completion Criteria
- [ ] All 5 protocol-specific metric modules created
- [ ] All metrics integrated into respective handlers
- [ ] Metrics visible in `/metrics` endpoint
- [ ] Unit tests for each metric module
- [ ] Documentation updated
- [ ] Git commit: "feat: Add protocol-specific observability metrics (TCP, TLS, QUIC, gRPC, GraphQL)"

---

## Phase 2: Enhanced Structured Logging (Priority 1)

### 2.1 TCP Session Logging

**File**: `src/logging/tcp_logger.rs`

**Log events**:
```json
{
  "event": "tcp_connection_established",
  "timestamp": "2025-12-22T10:00:00Z",
  "client_ip": "192.168.1.100",
  "client_port": 54321,
  "backend_ip": "10.0.0.1",
  "backend_port": 3306,
  "connection_id": "uuid"
}
```

```json
{
  "event": "tcp_connection_closed",
  "timestamp": "2025-12-22T10:05:00Z",
  "connection_id": "uuid",
  "duration_seconds": 300.5,
  "bytes_sent": 1024000,
  "bytes_received": 2048000,
  "error": null
}
```

**Integration**: Add to TCP proxy handler

---

### 2.2 TLS Handshake Logging

**File**: `src/logging/tls_logger.rs`

**Log events**:
```json
{
  "event": "tls_handshake_start",
  "timestamp": "2025-12-22T10:00:00Z",
  "client_ip": "192.168.1.100",
  "sni": "api.example.com",
  "connection_id": "uuid"
}
```

```json
{
  "event": "tls_handshake_complete",
  "timestamp": "2025-12-22T10:00:00.150Z",
  "connection_id": "uuid",
  "duration_ms": 150,
  "protocol_version": "TLSv1.3",
  "cipher_suite": "TLS_AES_256_GCM_SHA384",
  "client_cert": "CN=client.example.com",
  "success": true
}
```

**Integration**: Add to TLS handler

---

### 2.3 QUIC Connection Logging

**File**: `src/logging/quic_logger.rs`

**Log events**:
```json
{
  "event": "quic_connection_established",
  "timestamp": "2025-12-22T10:00:00Z",
  "client_ip": "192.168.1.100",
  "connection_id": "quic-uuid",
  "zero_rtt": true,
  "protocol_version": "h3-29"
}
```

```json
{
  "event": "quic_connection_migration",
  "timestamp": "2025-12-22T10:01:00Z",
  "connection_id": "quic-uuid",
  "old_ip": "192.168.1.100",
  "new_ip": "192.168.1.101"
}
```

**Integration**: Add to HTTP/3 handler

---

### 2.4 Logging Configuration Enhancement

**Files to modify**:
- `src/logging/mod.rs` - Export new loggers
- `src/config/logging.rs` - Add log level per protocol
- Update DSL to support:

```dsl
logging {
    format json
    level info
    protocols {
        tcp debug
        tls info
        quic debug
        grpc info
        graphql debug
    }
}
```

---

### Phase 2 Completion Criteria
- [ ] TCP, TLS, QUIC loggers implemented
- [ ] All protocol events logged in structured JSON
- [ ] Log levels configurable per protocol
- [ ] Logging tested with sample traffic
- [ ] Git commit: "feat: Add protocol-specific structured logging"

---

## Phase 3: Configuration Enhancements (Priority 2)

### 3.1 Connection Pooling for Database LB

**File**: `src/proxy/connection_pool.rs`

**Implementation**:
```rust
pub struct ConnectionPool {
    min_connections: usize,
    max_connections: usize,
    idle_timeout: Duration,
    active: Arc<Mutex<Vec<Connection>>>,
    idle: Arc<Mutex<Vec<Connection>>>,
    wait_queue: Arc<Mutex<VecDeque<Sender<Connection>>>>,
}

impl ConnectionPool {
    pub async fn acquire(&self) -> Result<Connection>;
    pub async fn release(&self, conn: Connection);
    pub fn stats(&self) -> PoolStats;
}
```

**DSL Support**:
```dsl
tcp://0.0.0.0:3306 {
    proxy mysql-primary:3306 mysql-secondary:3306
    connection_pool {
        min_connections 10
        max_connections 100
        idle_timeout 300s
        max_wait_time 5s
    }
}
```

**Files to modify**:
- Create `src/proxy/connection_pool.rs`
- Update `src/proxy/tcp_proxy.rs` to use pool
- Update `src/config/mod.rs` for DSL parsing
- Update `src/observability/tcp_metrics.rs` to track pool metrics

---

### 3.2 Security Headers Middleware

**File**: `src/middleware/security_headers.rs`

**Default headers**:
```rust
pub struct SecurityHeaders {
    x_frame_options: String,           // DENY
    x_content_type_options: String,    // nosniff
    x_xss_protection: String,          // 1; mode=block
    strict_transport_security: String, // max-age=31536000
    content_security_policy: String,   // default-src 'self'
    referrer_policy: String,           // strict-origin-when-cross-origin
    permissions_policy: String,        // configurable
}
```

**DSL Support**:
```dsl
https://api.example.com {
    security_headers {
        enabled true
        x_frame_options "DENY"
        hsts "max-age=31536000; includeSubDomains"
        csp "default-src 'self'; script-src 'self' 'unsafe-inline'"
        custom_headers {
            "X-Custom-Header" "value"
        }
    }
}
```

**Files to modify**:
- Create `src/middleware/security_headers.rs`
- Update `src/middleware/mod.rs`
- Update `src/config/mod.rs` for DSL parsing
- Apply middleware in HTTP/HTTPS handler

---

### 3.3 Zero-Code Default Configurations

**File**: `src/config/defaults.rs`

**Smart defaults by scenario**:
```rust
impl Config {
    pub fn with_smart_defaults(scheme: &str) -> Self {
        match scheme {
            "http" | "https" => Self::http_defaults(),
            "tcp" => Self::tcp_defaults(),
            "ws" | "wss" => Self::websocket_defaults(),
            _ => Self::default(),
        }
    }

    fn http_defaults() -> Self {
        Self {
            security_headers: SecurityHeaders::default(),
            rate_limit: Some(RateLimit::default()),
            cors: Some(CORS::permissive()),
            timeout: Duration::from_secs(60),
            ..Default::default()
        }
    }
}
```

**Files to modify**:
- Create `src/config/defaults.rs`
- Update `src/config/mod.rs` to apply defaults
- Update `src/main.rs` to use smart defaults

---

### Phase 3 Completion Criteria
- [ ] Connection pooling fully implemented and tested
- [ ] Security headers middleware working
- [ ] Smart defaults for all protocols
- [ ] DSL updated for all new features
- [ ] Git commit: "feat: Add connection pooling, security headers, and smart defaults"

---

## Phase 4: Configuration Templates for All 15 Scenarios

**Directory**: `examples/use-cases/`

### 4.1 Template Files to Create

1. `01-tcp-layer4-proxy.dsl` - Layer 4 TCP proxying
2. `02-http-load-balancing.dsl` - Layer 7 HTTP/1.1
3. `03-https-tls-termination.dsl` - HTTPS/TLS
4. `04-api-gateway.dsl` - REST API gateway
5. `05-http3-quic.dsl` - HTTP/3 QUIC
6. `06-websocket.dsl` - WebSocket
7. `07-grpc-gateway.dsl` - gRPC gateway
8. `08-database-load-balancer.dsl` - Database LB with pooling
9. `09-waf-mtls.dsl` - WAF + mTLS
10. `10-hybrid-multi-protocol.dsl` - All protocols
11. `11-cdn-edge-caching.dsl` - CDN caching
12. `12-microservices-discovery.dsl` - Microservices
13. `13-graphql-gateway.dsl` - GraphQL
14. `14-static-php-fpm.dsl` - Static + PHP-FPM
15. `15-geographic-load-balancing.dsl` - Geo routing

### 4.2 Template Structure

Each template should include:
- **Complete working configuration**
- **Inline comments explaining each directive**
- **Multiple backend examples**
- **Security best practices**
- **Observability configuration**
- **Environment variable usage**

**Example template structure**:
```dsl
# Use Case 01: Layer 4 TCP Proxying
# Purpose: Pure TCP load balancing for databases, Redis, etc.
# Security: IP filtering, connection limits
# Observability: TCP-specific metrics, structured logging

tcp://0.0.0.0:3306 {
    # Backend MySQL servers
    proxy db-primary.example.com:3306 db-secondary.example.com:3306

    # Load balancing algorithm
    load_balance least_connections

    # Health checks
    health_check tcp interval=5s timeout=2s

    # Connection pooling
    connection_pool {
        min_connections 10
        max_connections 100
        idle_timeout 300s
    }

    # Security
    limits {
        max_connections 1000
        max_connections_per_ip 10
    }

    # Observability
    logging {
        level debug
        format json
    }

    metrics enabled
}
```

### 4.3 README for Templates

**File**: `examples/use-cases/README.md`

Content:
- Overview of all 15 scenarios
- How to use each template
- Customization guide
- Environment variable reference
- Testing instructions

---

### Phase 4 Completion Criteria
- [ ] All 15 template files created
- [ ] README documentation complete
- [ ] All templates tested and validated
- [ ] Git commit: "docs: Add configuration templates for all 15 use case scenarios"

---

## Phase 5: Load Testing Scripts for All 15 Scenarios

**Directory**: `benchmarks/use-cases/`

### 5.1 Load Testing Infrastructure

**Tools**:
- `wrk` for HTTP/HTTPS benchmarks
- `h2load` for HTTP/2, gRPC
- `curl` for HTTP/3 QUIC testing
- Custom TCP client for TCP benchmarks
- `wscat` or custom WebSocket client
- `ghz` for gRPC benchmarks

**Structure**:
```
benchmarks/use-cases/
├── common/
│   ├── utils.sh           # Shared utilities
│   ├── metrics.sh         # Metrics collection
│   └── report.sh          # Report generation
├── 01-tcp/
│   ├── run.sh             # Main test runner
│   ├── tcp-client.sh      # TCP test client
│   └── README.md
├── 02-http/
│   ├── run.sh
│   ├── wrk-script.lua     # WRK Lua script
│   └── README.md
├── 03-https/
│   ├── run.sh
│   ├── wrk-script.lua
│   └── README.md
├── 04-api-gateway/
│   ├── run.sh
│   ├── test-api.sh        # API test scenarios
│   └── README.md
├── 05-http3/
│   ├── run.sh
│   ├── http3-client.sh
│   └── README.md
├── 06-websocket/
│   ├── run.sh
│   ├── ws-client.js       # WebSocket test client
│   └── README.md
├── 07-grpc/
│   ├── run.sh
│   ├── test.proto         # gRPC proto definition
│   ├── grpc-client.go     # gRPC client
│   └── README.md
├── 08-database/
│   ├── run.sh
│   ├── db-bench.sh        # Database benchmark
│   └── README.md
├── 09-waf-mtls/
│   ├── run.sh
│   ├── certs/             # Test certificates
│   └── README.md
├── 10-multi-protocol/
│   ├── run.sh
│   ├── test-all.sh        # Test all protocols
│   └── README.md
├── 11-cdn-caching/
│   ├── run.sh
│   ├── cache-test.sh      # Cache hit/miss test
│   └── README.md
├── 12-microservices/
│   ├── run.sh
│   ├── circuit-breaker-test.sh
│   └── README.md
├── 13-graphql/
│   ├── run.sh
│   ├── queries/           # GraphQL test queries
│   ├── graphql-bench.js
│   └── README.md
├── 14-php-fpm/
│   ├── run.sh
│   ├── php-bench.sh
│   └── README.md
├── 15-geo-routing/
│   ├── run.sh
│   ├── geo-test.sh        # Geo routing test
│   └── README.md
└── run-all.sh             # Run all benchmarks
```

---

### 5.2 Common Testing Framework

**File**: `benchmarks/use-cases/common/utils.sh`

```bash
#!/bin/bash

# Common utilities for load testing

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print functions
print_header() {
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}$1${NC}"
    echo -e "${GREEN}========================================${NC}"
}

print_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if command exists
check_command() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 is not installed"
        exit 1
    fi
}

# Wait for service to be ready
wait_for_service() {
    local host=$1
    local port=$2
    local timeout=${3:-30}

    print_info "Waiting for $host:$port to be ready..."

    for i in $(seq 1 $timeout); do
        if nc -z $host $port 2>/dev/null; then
            print_success "Service is ready"
            return 0
        fi
        sleep 1
    done

    print_error "Service not ready after ${timeout}s"
    return 1
}

# Collect metrics from Highper Gateway
collect_metrics() {
    local metrics_url=${1:-"http://localhost:9090/metrics"}
    local output_file=${2:-"metrics.txt"}

    print_info "Collecting metrics from $metrics_url"
    curl -s $metrics_url > $output_file
    print_success "Metrics saved to $output_file"
}

# Generate HTML report
generate_report() {
    local test_name=$1
    local results_dir=$2

    print_info "Generating HTML report for $test_name"

    cat > "$results_dir/report.html" <<EOF
<!DOCTYPE html>
<html>
<head>
    <title>Load Test Report - $test_name</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        h1 { color: #333; }
        table { border-collapse: collapse; width: 100%; margin-top: 20px; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #4CAF50; color: white; }
        .success { color: green; }
        .warning { color: orange; }
        .error { color: red; }
    </style>
</head>
<body>
    <h1>Load Test Report - $test_name</h1>
    <p><strong>Date:</strong> $(date)</p>
    <p><strong>Results Directory:</strong> $results_dir</p>

    <h2>Test Results</h2>
    <pre>$(cat $results_dir/results.txt 2>/dev/null || echo "No results available")</pre>

    <h2>Metrics</h2>
    <pre>$(cat $results_dir/metrics.txt 2>/dev/null || echo "No metrics available")</pre>
</body>
</html>
EOF

    print_success "Report generated: $results_dir/report.html"
}
```

---

### 5.3 Individual Test Scripts

**Example**: `benchmarks/use-cases/02-http/run.sh`

```bash
#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../common/utils.sh"

print_header "Use Case 02: HTTP/1.1 Load Balancing Test"

# Configuration
GATEWAY_URL="http://localhost:8080"
CONNECTIONS=${CONNECTIONS:-100}
DURATION=${DURATION:-30}
THREADS=${THREADS:-4}
RESULTS_DIR="$SCRIPT_DIR/results/$(date +%Y%m%d_%H%M%S)"

mkdir -p "$RESULTS_DIR"

# Check dependencies
check_command wrk
check_command curl

# Wait for gateway
wait_for_service localhost 8080 30

# Run WRK benchmark
print_info "Running WRK benchmark..."
print_info "Connections: $CONNECTIONS, Duration: ${DURATION}s, Threads: $THREADS"

wrk -t$THREADS -c$CONNECTIONS -d${DURATION}s \
    --latency \
    -s "$SCRIPT_DIR/wrk-script.lua" \
    "$GATEWAY_URL" \
    > "$RESULTS_DIR/wrk-results.txt"

print_success "WRK benchmark completed"

# Collect metrics
collect_metrics "http://localhost:9090/metrics" "$RESULTS_DIR/metrics.txt"

# Parse results
REQUESTS=$(grep "Requests/sec:" "$RESULTS_DIR/wrk-results.txt" | awk '{print $2}')
LATENCY_AVG=$(grep "Latency" "$RESULTS_DIR/wrk-results.txt" | awk '{print $2}')

# Generate summary
cat > "$RESULTS_DIR/results.txt" <<EOF
HTTP/1.1 Load Balancing Test Results
=====================================

Configuration:
- Connections: $CONNECTIONS
- Duration: ${DURATION}s
- Threads: $THREADS

Results:
- Requests/sec: $REQUESTS
- Average Latency: $LATENCY_AVG

Full WRK output:
$(cat "$RESULTS_DIR/wrk-results.txt")
EOF

# Generate HTML report
generate_report "HTTP/1.1 Load Balancing" "$RESULTS_DIR"

print_success "Test completed. Results saved to: $RESULTS_DIR"
print_info "View report: $RESULTS_DIR/report.html"
```

---

### 5.4 Master Test Runner

**File**: `benchmarks/use-cases/run-all.sh`

```bash
#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common/utils.sh"

print_header "Running All 15 Use Case Load Tests"

RESULTS_ROOT="$SCRIPT_DIR/results/all-tests-$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_ROOT"

# Array of test scenarios
declare -a tests=(
    "01-tcp"
    "02-http"
    "03-https"
    "04-api-gateway"
    "05-http3"
    "06-websocket"
    "07-grpc"
    "08-database"
    "09-waf-mtls"
    "10-multi-protocol"
    "11-cdn-caching"
    "12-microservices"
    "13-graphql"
    "14-php-fpm"
    "15-geo-routing"
)

# Run each test
for test in "${tests[@]}"; do
    print_header "Running $test"

    if [ -f "$SCRIPT_DIR/$test/run.sh" ]; then
        cd "$SCRIPT_DIR/$test"
        bash run.sh

        # Copy results to master results directory
        latest_result=$(ls -td results/* 2>/dev/null | head -1)
        if [ -n "$latest_result" ]; then
            cp -r "$latest_result" "$RESULTS_ROOT/$test"
            print_success "$test completed"
        else
            print_warning "$test completed but no results found"
        fi
    else
        print_warning "$test/run.sh not found, skipping"
    fi

    echo ""
done

# Generate master report
print_info "Generating master report..."

cat > "$RESULTS_ROOT/index.html" <<EOF
<!DOCTYPE html>
<html>
<head>
    <title>Highper Gateway - All Tests Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        h1 { color: #333; }
        .test-link { display: block; padding: 10px; margin: 5px 0; background: #f0f0f0; text-decoration: none; color: #333; border-radius: 5px; }
        .test-link:hover { background: #e0e0e0; }
    </style>
</head>
<body>
    <h1>Highper Gateway - All Tests Report</h1>
    <p><strong>Date:</strong> $(date)</p>

    <h2>Test Scenarios</h2>
EOF

for test in "${tests[@]}"; do
    if [ -d "$RESULTS_ROOT/$test" ]; then
        echo "    <a class='test-link' href='$test/report.html'>$test</a>" >> "$RESULTS_ROOT/index.html"
    fi
done

cat >> "$RESULTS_ROOT/index.html" <<EOF
</body>
</html>
EOF

print_success "All tests completed!"
print_success "Master report: $RESULTS_ROOT/index.html"
```

---

### 5.5 Documentation

**File**: `benchmarks/use-cases/README.md`

```markdown
# Load Testing Guide - All 15 Use Case Scenarios

## Overview

This directory contains load testing scripts for all 15 use case scenarios of Highper Gateway.

## Prerequisites

Install required tools:

```bash
# Ubuntu/Debian
sudo apt-get install wrk h2load curl netcat

# macOS
brew install wrk h2load curl netcat

# gRPC benchmarking
go install github.com/bojand/ghz/cmd/ghz@latest

# WebSocket testing
npm install -g wscat
```

## Running Individual Tests

Each test scenario has its own directory with a `run.sh` script:

```bash
cd benchmarks/use-cases/02-http
bash run.sh
```

## Running All Tests

To run all 15 scenarios:

```bash
bash run-all.sh
```

## Configuration

Tests can be configured via environment variables:

```bash
# HTTP test example
CONNECTIONS=200 DURATION=60 THREADS=8 bash 02-http/run.sh
```

Common variables:
- `CONNECTIONS` - Number of concurrent connections (default: 100)
- `DURATION` - Test duration in seconds (default: 30)
- `THREADS` - Number of threads (default: 4)
- `GATEWAY_URL` - Gateway URL (default varies by test)

## Results

Results are saved in each test's `results/` directory with:
- `results.txt` - Summary
- `wrk-results.txt` - Full WRK output
- `metrics.txt` - Prometheus metrics
- `report.html` - HTML report

## Test Scenarios

1. **01-tcp** - Layer 4 TCP proxying
2. **02-http** - HTTP/1.1 load balancing
3. **03-https** - HTTPS/TLS termination
4. **04-api-gateway** - REST API gateway
5. **05-http3** - HTTP/3 QUIC
6. **06-websocket** - WebSocket
7. **07-grpc** - gRPC gateway
8. **08-database** - Database load balancing
9. **09-waf-mtls** - WAF + mTLS
10. **10-multi-protocol** - All protocols
11. **11-cdn-caching** - CDN edge caching
12. **12-microservices** - Microservices discovery
13. **13-graphql** - GraphQL gateway
14. **14-php-fpm** - Static + PHP-FPM
15. **15-geo-routing** - Geographic load balancing
```

---

### Phase 5 Completion Criteria
- [ ] Common testing framework created
- [ ] All 15 load testing scripts implemented
- [ ] Master test runner working
- [ ] Documentation complete
- [ ] All tests validated
- [ ] Git commit: "test: Add load testing scripts for all 15 use case scenarios"

---

## Phase 6: Security Enhancements (Priority 3 - Optional)

### 6.1 TCP-Level IP Filtering

**File**: `src/security/tcp_firewall.rs`

**Features**:
- IP allowlist/blocklist
- CIDR range support
- Per-IP connection limits
- Rate limiting

**DSL**:
```dsl
tcp://0.0.0.0:3306 {
    firewall {
        allowlist [
            "10.0.0.0/8",
            "192.168.1.0/24"
        ]
        blocklist [
            "1.2.3.4"
        ]
        max_connections_per_ip 10
        rate_limit 100 per_second
    }
}
```

---

### 6.2 Advanced Authentication (JWT/OAuth)

**File**: `src/auth/jwt.rs`, `src/auth/oauth.rs`

**Built-in JWT support**:
```dsl
https://api.example.com {
    /api/* {
        auth {
            type jwt
            secret env("JWT_SECRET")
            algorithm HS256
            claims {
                required ["sub", "exp"]
                validate_exp true
                validate_nbf true
            }
            header "Authorization"
            prefix "Bearer "
        }
    }
}
```

**OAuth2 support**:
```dsl
https://api.example.com {
    /api/* {
        auth {
            type oauth2
            provider google
            client_id env("OAUTH_CLIENT_ID")
            client_secret env("OAUTH_CLIENT_SECRET")
            scopes ["email", "profile"]
            callback_url "https://api.example.com/oauth/callback"
        }
    }
}
```

---

### 6.3 Database TLS Support

**Enhancement**: Add TLS support for database proxy

**DSL**:
```dsl
tcp://0.0.0.0:3306 {
    tls {
        enabled true
        passthrough true  # Pass TLS through to backend
    }
    proxy mysql-primary:3306
}
```

---

### Phase 6 Completion Criteria (Optional)
- [ ] TCP firewall implemented
- [ ] JWT authentication built-in
- [ ] OAuth2 support added
- [ ] Database TLS support
- [ ] Git commit: "feat: Add TCP firewall, JWT/OAuth auth, and database TLS"

---

## Git Commit Strategy

### Commit Points

1. **After Phase 1**: `git commit -m "feat: Add protocol-specific observability metrics (TCP, TLS, QUIC, gRPC, GraphQL)"`
2. **After Phase 2**: `git commit -m "feat: Add protocol-specific structured logging"`
3. **After Phase 3**: `git commit -m "feat: Add connection pooling, security headers, and smart defaults"`
4. **After Phase 4**: `git commit -m "docs: Add configuration templates for all 15 use case scenarios"`
5. **After Phase 5**: `git commit -m "test: Add load testing scripts for all 15 use case scenarios"`
6. **After Phase 6** (optional): `git commit -m "feat: Add TCP firewall, JWT/OAuth auth, and database TLS"`

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**: feat, fix, docs, test, refactor, perf, chore

---

## Testing Strategy

### Unit Tests
- Each new module needs unit tests
- Test metrics increment/decrement
- Test logging output format
- Test configuration parsing

### Integration Tests
- Test with real traffic
- Verify metrics collection
- Verify log output
- Test each scenario template

### Load Tests
- Run all 15 scenario tests
- Verify performance under load
- Check for memory leaks
- Monitor metrics during load

---

## Success Criteria

### Phase 1-3 (Core Implementation)
- ✅ All protocol-specific metrics working
- ✅ All structured logging working
- ✅ Connection pooling functional
- ✅ Security headers applied
- ✅ Smart defaults working

### Phase 4 (Templates)
- ✅ All 15 templates created
- ✅ Templates tested and validated
- ✅ Documentation complete

### Phase 5 (Load Testing)
- ✅ All 15 load test scripts working
- ✅ Master test runner functional
- ✅ Reports generated successfully

### Overall Success
- ✅ All gaps from COMPREHENSIVE_VALIDATION.md addressed
- ✅ 100% of 15 scenarios fully supported
- ✅ Production-ready for all scenarios
- ✅ Complete observability across all protocols
- ✅ Zero-code deployment improved

---

## Execution Timeline

**Recommended Order**:
1. Phase 1: Protocol-specific metrics (High Priority)
2. Phase 2: Structured logging (High Priority)
3. Phase 3: Configuration enhancements (Medium Priority)
4. Phase 4: Configuration templates (Documentation)
5. Phase 5: Load testing scripts (Validation)
6. Phase 6: Security enhancements (Optional)

**Estimated Effort**:
- Phase 1: 2-3 days (5 metric modules)
- Phase 2: 1-2 days (3 loggers)
- Phase 3: 2-3 days (pooling + headers + defaults)
- Phase 4: 1-2 days (15 templates)
- Phase 5: 2-3 days (15 test scripts)
- Phase 6: 2-3 days (optional)

**Total**: 8-16 days depending on scope

---

## Notes

- All code should follow Rust best practices
- All new code needs documentation
- All metrics must be Prometheus-compatible
- All logs must be structured JSON
- All DSL changes must be backward-compatible
- All tests must pass before commit

---

**END OF IMPLEMENTATION PLAN**
