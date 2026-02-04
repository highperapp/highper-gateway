# Local Load Testing Plan - 15 Scenarios

**Date**: 2025-12-03
**Purpose**: Comprehensive local validation of all 15 use case scenarios before cloud deployment
**Context**: Post-WSL2 corruption recovery, methodical approach to ensure stability

---

## Executive Summary

This plan outlines a systematic approach to test all 15 highper-gateway scenarios locally on Windows 11 with WSL2-mounted code. The goal is to identify and fix all bugs locally (where debugging is FREE) before attempting any cloud deployments.

**Lessons Learned**:
- ✅ Never deploy to cloud without local validation ($3.91 lesson)
- ✅ Local testing catches 90%+ of issues at zero cost
- ✅ Systematic approach prevents panic mode during failures
- ✅ Documentation at each step prevents context loss

---

## Phase 1: Build and Infrastructure Setup

### 1.1 Build Highper Gateway

**Objective**: Build optimized release binary for testing

```bash
# Navigate to project
cd /mnt/e/my-opensource/highper-gateway

# Clean previous builds
cargo clean

# Build with optimizations
cargo build --release --manifest-path highper-gateway/Cargo.toml

# Verify binary
ls -lh highper-gateway/target/release/highper-gateway
./highper-gateway/target/release/highper-gateway --version
```

**Success Criteria**:
- [ ] Binary builds without errors
- [ ] Binary size reasonable (< 50MB)
- [ ] Version information displays correctly
- [ ] No missing dynamic library dependencies

**Estimated Time**: 10-15 minutes (initial build), 2-5 minutes (incremental)

---

### 1.2 Create Local Backend Infrastructure

**Objective**: Create flexible, fast mock backends for testing

#### Option A: Rust Fast Backend (Recommended)

**File**: `load-tests/fast-backend-local.rs`

```rust
// Simple, fast HTTP server for load testing
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::env;

async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let backend_id = env::var("BACKEND_ID").unwrap_or_else(|_| "unknown".to_string());
    Ok(Response::new(Body::from(format!(
        "{{\"backend\":\"{}\",\"status\":\"ok\"}}\n",
        backend_id
    ))))
}

#[tokio::main]
async fn main() {
    let port = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(handle))
    });

    let server = Server::bind(&addr).serve(make_svc);
    println!("Backend listening on {}", addr);

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}
```

**Build and test**:
```bash
cd load-tests
rustc --edition 2021 -O fast-backend-local.rs
./fast-backend-local 8081 &
./fast-backend-local 8082 &
./fast-backend-local 8083 &

# Test
curl http://127.0.0.1:8081/
```

#### Option B: Python Backend (Quick alternative)

**File**: `load-tests/simple-backend-local.py`

```python
#!/usr/bin/env python3
import sys
import socket
from http.server import HTTPServer, BaseHTTPRequestHandler

class BackendHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        backend_id = socket.gethostname() + ":" + str(self.server.server_port)
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(f'{{"backend":"{backend_id}","status":"ok"}}\n'.encode())

    def log_message(self, format, *args):
        pass  # Suppress logs for performance

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8081
    server = HTTPServer(('127.0.0.1', port), BackendHandler)
    print(f"Backend listening on 127.0.0.1:{port}")
    server.serve_forever()
```

**Run**:
```bash
chmod +x load-tests/simple-backend-local.py
python3 load-tests/simple-backend-local.py 8081 &
python3 load-tests/simple-backend-local.py 8082 &
python3 load-tests/simple-backend-local.py 8083 &
```

---

### 1.3 Create Local Test Runner Framework

**File**: `scripts/test-local-scenarios.sh`

```bash
#!/bin/bash
# Local testing framework for all 15 scenarios

set -e

PROJECT_ROOT="/mnt/e/my-opensource/highper-gateway"
GATEWAY_BIN="${PROJECT_ROOT}/highper-gateway/target/release/highper-gateway"
SCENARIOS_DIR="${PROJECT_ROOT}/configs/scenarios"
RESULTS_DIR="${PROJECT_ROOT}/load-tests/results/local-$(date +%Y%m%d-%H%M%S)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    pkill -f "highper-gateway" 2>/dev/null || true
    pkill -f "fast-backend-local" 2>/dev/null || true
    pkill -f "simple-backend-local.py" 2>/dev/null || true
}

trap cleanup EXIT

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if [ ! -f "$GATEWAY_BIN" ]; then
        log_error "Gateway binary not found at $GATEWAY_BIN"
        log_info "Please build first: cargo build --release"
        exit 1
    fi

    if ! command -v curl &> /dev/null; then
        log_error "curl not found - required for testing"
        exit 1
    fi

    log_success "Prerequisites OK"
}

# Start backends
start_backends() {
    local num_backends=${1:-3}
    local base_port=${2:-8081}

    log_info "Starting $num_backends backend servers..."

    for i in $(seq 0 $((num_backends - 1))); do
        local port=$((base_port + i))
        BACKEND_ID="backend-$((i+1))" python3 load-tests/simple-backend-local.py $port &
        sleep 0.5
    done

    sleep 2

    # Verify backends are running
    for i in $(seq 0 $((num_backends - 1))); do
        local port=$((base_port + i))
        if curl -s --connect-timeout 2 "http://127.0.0.1:$port/" > /dev/null; then
            log_success "Backend on port $port is responding"
        else
            log_error "Backend on port $port is NOT responding"
            return 1
        fi
    done
}

# Replace backend placeholders
prepare_config() {
    local scenario_file=$1
    local temp_config="${RESULTS_DIR}/$(basename $scenario_file)"

    mkdir -p "$RESULTS_DIR"

    # Replace BACKEND_* placeholders with localhost addresses
    sed -e 's/BACKEND_1:8080/127.0.0.1:8081/g' \
        -e 's/BACKEND_2:8080/127.0.0.1:8082/g' \
        -e 's/BACKEND_3:8080/127.0.0.1:8083/g' \
        "$scenario_file" > "$temp_config"

    echo "$temp_config"
}

# Test a single scenario
test_scenario() {
    local scenario_num=$1
    local scenario_file="${SCENARIOS_DIR}/scenario-${scenario_num}.yaml"

    if [ ! -f "$scenario_file" ]; then
        log_error "Scenario file not found: $scenario_file"
        return 1
    fi

    log_info "=========================================="
    log_info "Testing Scenario $scenario_num"
    log_info "File: $(basename $scenario_file)"
    log_info "=========================================="

    # Prepare configuration
    local temp_config=$(prepare_config "$scenario_file")
    log_info "Configuration prepared: $temp_config"

    # Start gateway
    log_info "Starting gateway..."
    "$GATEWAY_BIN" start -c "$temp_config" > "${RESULTS_DIR}/scenario-${scenario_num}-gateway.log" 2>&1 &
    local gateway_pid=$!
    sleep 3

    # Check if gateway is running
    if ! ps -p $gateway_pid > /dev/null; then
        log_error "Gateway failed to start"
        cat "${RESULTS_DIR}/scenario-${scenario_num}-gateway.log"
        return 1
    fi

    log_success "Gateway started (PID: $gateway_pid)"

    # Run basic connectivity test
    log_info "Testing basic connectivity (10 requests)..."
    local success_count=0
    for i in {1..10}; do
        if curl -s --connect-timeout 2 "http://127.0.0.1:8080/" > /dev/null 2>&1; then
            ((success_count++))
        fi
    done

    if [ $success_count -eq 10 ]; then
        log_success "Basic connectivity: 10/10 requests succeeded"
    else
        log_warn "Basic connectivity: $success_count/10 requests succeeded"
    fi

    # Run load balancing distribution test
    log_info "Testing load balancing distribution (30 requests)..."
    local responses_file="${RESULTS_DIR}/scenario-${scenario_num}-responses.txt"
    for i in {1..30}; do
        curl -s "http://127.0.0.1:8080/" >> "$responses_file" 2>&1
    done

    # Analyze distribution
    if [ -f "$responses_file" ]; then
        log_info "Distribution:"
        grep -o '"backend":"[^"]*"' "$responses_file" | sort | uniq -c | while read count backend; do
            local percentage=$(awk "BEGIN {printf \"%.1f\", ($count/30)*100}")
            log_info "  $backend: $count requests ($percentage%)"
        done
    fi

    # Check metrics endpoint
    log_info "Checking metrics endpoint..."
    if curl -s "http://127.0.0.1:9090/metrics" > "${RESULTS_DIR}/scenario-${scenario_num}-metrics.txt" 2>&1; then
        log_success "Metrics endpoint accessible"
        local request_count=$(grep -c "^http_requests_total" "${RESULTS_DIR}/scenario-${scenario_num}-metrics.txt" || echo 0)
        log_info "Metrics captured: $request_count metric types"
    else
        log_warn "Metrics endpoint not accessible"
    fi

    # Stop gateway
    log_info "Stopping gateway..."
    kill $gateway_pid 2>/dev/null || true
    sleep 2

    log_success "Scenario $scenario_num test complete"
    echo ""

    return 0
}

# Main execution
main() {
    local scenario_num=${1:-}

    echo ""
    log_info "================================================"
    log_info "Highper Gateway - Local Scenario Testing"
    log_info "================================================"
    echo ""

    check_prerequisites

    # Create results directory
    mkdir -p "$RESULTS_DIR"
    log_info "Results will be saved to: $RESULTS_DIR"

    # Start backends
    start_backends 3 8081

    if [ -n "$scenario_num" ]; then
        # Test single scenario
        test_scenario "$scenario_num"
    else
        # Test all scenarios
        log_info "Testing all 15 scenarios..."
        local passed=0
        local failed=0

        for num in $(seq -f "%02g" 1 15); do
            if test_scenario "$num"; then
                ((passed++))
            else
                ((failed++))
            fi
        done

        echo ""
        log_info "================================================"
        log_info "Test Summary"
        log_info "================================================"
        log_success "Passed: $passed"
        if [ $failed -gt 0 ]; then
            log_error "Failed: $failed"
        else
            log_info "Failed: $failed"
        fi
        log_info "Results directory: $RESULTS_DIR"
    fi
}

# Run main with scenario number argument
main "$@"
```

**Make executable**:
```bash
chmod +x scripts/test-local-scenarios.sh
```

---

## Phase 2: Individual Scenario Testing

### Testing Approach

For each scenario, we will:
1. **Prepare**: Update config with local backend addresses
2. **Validate**: Check configuration syntax
3. **Execute**: Start gateway and backends
4. **Test**: Run connectivity and load distribution tests
5. **Observe**: Check metrics, logs, and behavior
6. **Document**: Record any bugs, issues, or unexpected behavior

### 2.1 Scenario 01 - Layer 4 TCP Load Balancer

**Configuration**: `configs/scenarios/scenario-01-layer4-tcp.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 01
```

**What to Verify**:
- [ ] TCP connections accepted
- [ ] Load balancing works (round-robin)
- [ ] Health checks mark unhealthy backends
- [ ] Connection limits enforced (max_conns: 3000000)
- [ ] Timeouts work correctly (idle_timeout: 300s)

**Expected Behavior**:
- Pure TCP proxying without HTTP awareness
- Minimal latency overhead (< 0.5ms added)
- Equal distribution to all backends

**Potential Issues to Watch**:
- Connection pool leaks
- Health check failures on TCP (no HTTP /health endpoint)
- Performance degradation under load

---

### 2.2 Scenario 02 - Layer 7 HTTP Load Balancer

**Configuration**: `configs/scenarios/scenario-02-layer7-http.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 02
```

**What to Verify**:
- [ ] HTTP/1.1 requests proxied correctly
- [ ] Load balancing with health checks at /health
- [ ] Keepalive connections maintained (90s)
- [ ] Compression works (gzip, brotli)
- [ ] Rate limiting enforced (700K RPS limit)
- [ ] Connection pooling statistics available

**Expected Behavior**:
- HTTP-aware routing
- Request/response header manipulation
- Even distribution across backends

**Potential Issues to Watch**:
- Health check endpoint not found (404)
- Compression errors or missing Content-Encoding
- Rate limit not enforced correctly
- Connection pool exhaustion

---

### 2.3 Scenario 03 - Layer 7 HTTPS/TLS Termination

**Configuration**: `configs/scenarios/scenario-03-layer7-tls.yaml`

**Prerequisites**:
- Generate self-signed certificates for testing

**Setup**:
```bash
# Generate test certificates
mkdir -p /tmp/test-certs
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /tmp/test-certs/key.pem \
  -out /tmp/test-certs/cert.pem \
  -days 365 \
  -subj "/CN=localhost"
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 03
```

**What to Verify**:
- [ ] TLS 1.2/1.3 negotiation works
- [ ] Certificate validation (self-signed OK for local)
- [ ] HTTP/2 over TLS works
- [ ] TLS termination overhead measured
- [ ] Backend connections use HTTP (not HTTPS)

**Expected Behavior**:
- HTTPS on frontend (port 443 or 8443)
- HTTP to backends (port 8080)
- Performance degradation due to TLS overhead (~10-15%)

**Potential Issues to Watch**:
- Certificate loading failures
- TLS version negotiation errors
- HTTP/2 ALPN issues
- Performance bottlenecks in TLS termination

---

### 2.4 Scenario 04 - API Gateway

**Configuration**: `configs/scenarios/scenario-04-api-gateway.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 04
```

**What to Verify**:
- [ ] CORS headers added correctly
- [ ] Rate limiting per IP
- [ ] Authentication (JWT or API key)
- [ ] Request transformation
- [ ] Response caching

**Expected Behavior**:
- CORS preflight requests handled
- Rate limiting prevents abuse
- Invalid auth tokens rejected

**Potential Issues to Watch**:
- CORS misconfigurations
- Rate limit bypass
- Auth token validation bugs
- Cache invalidation issues

---

### 2.5 Scenario 05 - HTTP/3 QUIC Multi-Protocol

**Configuration**: `configs/scenarios/scenario-05-http3-quic.yaml`

**Prerequisites**:
- HTTP/3 client (curl with HTTP/3 support or custom client)

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 05
```

**What to Verify**:
- [ ] HTTP/3 over QUIC works
- [ ] HTTP/2 fallback works
- [ ] HTTP/1.1 fallback works
- [ ] Alt-Svc header advertises HTTP/3
- [ ] UDP port (443) open and listening

**Expected Behavior**:
- HTTP/3 preferred when client supports it
- Graceful fallback to HTTP/2 or HTTP/1.1
- Performance improvement on lossy networks

**Potential Issues to Watch**:
- HTTP/3 handshake failures
- QUIC connection migration issues
- Alt-Svc header missing or incorrect
- UDP firewall issues (even locally)

**Note**: This is the most complex scenario - may require significant debugging.

---

### 2.6 Scenario 06 - WebSocket Load Balancer

**Configuration**: `configs/scenarios/scenario-06-websocket.yaml`

**Prerequisites**:
- WebSocket test client (wscat, websocat, or custom)

**Install wscat**:
```bash
npm install -g wscat
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 06

# After gateway starts, test WebSocket
wscat -c ws://127.0.0.1:8080/ws
```

**What to Verify**:
- [ ] WebSocket upgrade works
- [ ] Sticky sessions (same client → same backend)
- [ ] Bidirectional messaging
- [ ] Connection timeout (5 minutes)
- [ ] Ping/pong keepalive

**Expected Behavior**:
- WebSocket connections maintained
- Messages delivered in order
- Sticky session prevents connection hopping

**Potential Issues to Watch**:
- Upgrade header missing or incorrect
- Sticky session not working
- Message delivery failures
- Connection drops on timeout

---

### 2.7 Scenario 07 - gRPC Gateway

**Configuration**: `configs/scenarios/scenario-07-grpc.yaml`

**Prerequisites**:
- gRPC client (grpcurl or custom client)
- gRPC backend service

**Install grpcurl**:
```bash
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 07

# After gateway starts, test gRPC
grpcurl -plaintext 127.0.0.1:8080 list
```

**What to Verify**:
- [ ] gRPC over HTTP/2 works
- [ ] Unary RPC
- [ ] Server streaming RPC
- [ ] Client streaming RPC
- [ ] Bidirectional streaming RPC
- [ ] Health check via gRPC protocol

**Expected Behavior**:
- gRPC requests proxied correctly
- Streaming maintained
- Low latency (< 2ms added)

**Potential Issues to Watch**:
- HTTP/2 framing errors
- Streaming connection drops
- Metadata/header loss
- Load balancing breaks streaming

---

### 2.8 Scenario 08 - Database Load Balancer

**Configuration**: `configs/scenarios/scenario-08-database-lb.yaml`

**Prerequisites**:
- MySQL/PostgreSQL client
- Mock database backend

**Setup Mock MySQL**:
```bash
# Use netcat as simple TCP echo server for testing
# Real test would use actual MySQL instance
while true; do nc -l 3306 -c 'echo "OK"'; done &
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 08

# Test MySQL connection
mysql -h 127.0.0.1 -P 8080 -u test -p
```

**What to Verify**:
- [ ] TCP connection pooling works
- [ ] Read/write split (if configured)
- [ ] Connection limits enforced
- [ ] Idle connection timeout
- [ ] Health checks for database

**Expected Behavior**:
- Database connections pooled efficiently
- Queries execute correctly
- Connection reuse minimizes overhead

**Potential Issues to Watch**:
- Connection pool exhaustion
- Idle connection leaks
- Protocol-specific issues (MySQL wire protocol)
- Transaction isolation problems

---

### 2.9 Scenario 09 - WAF + mTLS

**Configuration**: `configs/scenarios/scenario-09-waf-mtls.yaml`

**Prerequisites**:
- Client certificates
- WAF rules configured

**Setup Certificates**:
```bash
# Generate CA
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /tmp/test-certs/ca-key.pem \
  -out /tmp/test-certs/ca-cert.pem \
  -days 365 -subj "/CN=Test CA"

# Generate client cert
openssl req -new -newkey rsa:2048 -nodes \
  -keyout /tmp/test-certs/client-key.pem \
  -out /tmp/test-certs/client-csr.pem \
  -subj "/CN=Test Client"

openssl x509 -req -in /tmp/test-certs/client-csr.pem \
  -CA /tmp/test-certs/ca-cert.pem \
  -CAkey /tmp/test-certs/ca-key.pem \
  -CAcreateserial \
  -out /tmp/test-certs/client-cert.pem \
  -days 365
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 09

# Test with client cert
curl --cert /tmp/test-certs/client-cert.pem \
     --key /tmp/test-certs/client-key.pem \
     --cacert /tmp/test-certs/ca-cert.pem \
     https://127.0.0.1:8443/
```

**What to Verify**:
- [ ] mTLS certificate validation
- [ ] WAF rules block malicious requests
- [ ] SQL injection blocked
- [ ] XSS attempts blocked
- [ ] Rate limiting with certificate identity

**Expected Behavior**:
- Only clients with valid certs can connect
- WAF blocks attacks
- Performance impact measured

**Potential Issues to Watch**:
- Certificate chain validation errors
- WAF false positives
- WAF bypass vulnerabilities
- Performance degradation from WAF

---

### 2.10 Scenario 10 - Hybrid Multi-Protocol

**Configuration**: `configs/scenarios/scenario-10-hybrid-multiprotocol.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 10
```

**What to Verify**:
- [ ] HTTP/1.1, HTTP/2, HTTP/3 all work
- [ ] WebSocket connections work
- [ ] gRPC connections work
- [ ] TCP proxying works
- [ ] Protocol detection automatic

**Expected Behavior**:
- Different protocols to different backends
- Protocol upgrade headers handled
- No protocol interference

**Potential Issues to Watch**:
- Protocol detection failures
- Port conflicts
- Performance degradation from complexity

---

### 2.11 Scenario 11 - CDN Edge with Caching

**Configuration**: `configs/scenarios/scenario-11-cdn-edge-caching.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 11
```

**What to Verify**:
- [ ] Cache hit/miss tracking
- [ ] Cache expiration (TTL)
- [ ] Cache invalidation
- [ ] Cache-Control headers respected
- [ ] High cache hit ratio (> 80%)

**Expected Behavior**:
- First request misses cache (fetches from backend)
- Subsequent requests hit cache (no backend call)
- Significant latency reduction on cache hits

**Potential Issues to Watch**:
- Cache key collision
- Memory exhaustion from cache
- Stale content served
- Cache invalidation not working

---

### 2.12 Scenario 12 - Microservices with Discovery

**Configuration**: `configs/scenarios/scenario-12-microservices-discovery.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 12
```

**What to Verify**:
- [ ] Service discovery works (Consul/etcd)
- [ ] Circuit breaker opens on failures
- [ ] Retry logic works (with backoff)
- [ ] Timeouts enforced
- [ ] Health checks integrated

**Expected Behavior**:
- Services discovered automatically
- Failures trigger circuit breaker
- Retries succeed after transient failures

**Potential Issues to Watch**:
- Service discovery delays
- Circuit breaker stuck open
- Retry storms
- Cascading failures

---

### 2.13 Scenario 13 - GraphQL Gateway

**Configuration**: `configs/scenarios/scenario-13-graphql.yaml`

**Prerequisites**:
- GraphQL client

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 13

# Test GraphQL query
curl -X POST http://127.0.0.1:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ hello }"}'
```

**What to Verify**:
- [ ] GraphQL queries executed
- [ ] Query batching works
- [ ] Query complexity limiting
- [ ] Response caching
- [ ] Schema stitching (multiple backends)

**Expected Behavior**:
- GraphQL queries parsed and routed
- Multiple queries batched efficiently
- Complex queries rejected or limited

**Potential Issues to Watch**:
- Query parsing errors
- Complexity calculation bugs
- Cache key generation
- Schema stitching conflicts

---

### 2.14 Scenario 14 - Static + PHP-FPM

**Configuration**: `configs/scenarios/scenario-14-static-php-fpm.yaml`

**Prerequisites**:
- PHP-FPM running (or mock)
- Static files directory

**Setup**:
```bash
# Create static files
mkdir -p /tmp/static-content
echo "Static content" > /tmp/static-content/index.html

# Install/start PHP-FPM (Ubuntu/Debian)
# sudo apt install php-fpm
# sudo systemctl start php7.4-fpm  # or php8.x-fpm
```

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 14

# Test static file
curl http://127.0.0.1:8080/index.html

# Test PHP
curl http://127.0.0.1:8080/index.php
```

**What to Verify**:
- [ ] Static files served directly
- [ ] PHP requests forwarded to PHP-FPM
- [ ] FastCGI protocol works
- [ ] Performance (static vs PHP)
- [ ] Caching of static content

**Expected Behavior**:
- Static files served very fast (< 0.5ms)
- PHP files executed via FastCGI
- Clear performance difference

**Potential Issues to Watch**:
- FastCGI protocol errors
- PHP-FPM connection issues
- Static file caching not working
- Incorrect MIME types

---

### 2.15 Scenario 15 - Geographic Load Balancer

**Configuration**: `configs/scenarios/scenario-15-geo-routing.yaml`

**Test Command**:
```bash
./scripts/test-local-scenarios.sh 15

# Test with different IPs (mock GeoIP)
curl -H "X-Forwarded-For: 1.2.3.4" http://127.0.0.1:8080/  # US
curl -H "X-Forwarded-For: 185.1.1.1" http://127.0.0.1:8080/  # EU
```

**What to Verify**:
- [ ] GeoIP lookup works
- [ ] Routing based on geography
- [ ] Fallback region configured
- [ ] Latency-based routing (if configured)
- [ ] Regional health checks

**Expected Behavior**:
- US IPs routed to US backends
- EU IPs routed to EU backends
- Unknown IPs use default region

**Potential Issues to Watch**:
- GeoIP database not found or outdated
- Incorrect region assignments
- Fallback not working
- Performance impact of GeoIP lookup

---

## Phase 3: Bug Documentation and Analysis

### 3.1 Bug Tracking Template

**File**: `docs/dev-notes/LOCAL_TESTING_BUGS_FOUND.md`

For each bug found:

```markdown
## Bug #N: [Short Description]

**Scenario**: Scenario XX - [Name]
**Severity**: Critical / High / Medium / Low
**Date Found**: YYYY-MM-DD

### Description
[Detailed description of the bug]

### Steps to Reproduce
1. Step 1
2. Step 2
3. Observed behavior

### Expected Behavior
[What should happen]

### Actual Behavior
[What actually happens]

### Error Messages / Logs
```
[Paste relevant logs]
```

### Root Cause Analysis
[Analysis of why this happens]

### Proposed Fix
[How to fix this]

### Workaround
[Temporary workaround if any]

### Impact
- [ ] Blocks deployment
- [ ] Affects performance
- [ ] Security issue
- [ ] User experience issue

### Status
[ ] Open
[ ] In Progress
[ ] Fixed
[ ] Verified
[ ] Closed
```

---

### 3.2 Bug Priority Matrix

| Severity | Criteria | Action |
|----------|----------|--------|
| **Critical** | Gateway crashes, data loss, security vulnerability | Fix immediately |
| **High** | Feature doesn't work, major performance issue | Fix before cloud deployment |
| **Medium** | Feature partially works, minor performance issue | Fix before production |
| **Low** | Cosmetic issue, edge case | Fix when convenient |

---

## Phase 4: Bug Fixes and Retesting

### 4.1 Fix Workflow

For each bug:
1. **Isolate**: Create minimal reproduction case
2. **Analyze**: Understand root cause
3. **Fix**: Implement fix with tests
4. **Test**: Verify fix works
5. **Regress**: Ensure no new bugs introduced
6. **Document**: Update bug tracker

### 4.2 Regression Testing

After each bug fix:
```bash
# Re-run the specific scenario
./scripts/test-local-scenarios.sh XX

# If critical fix, run all scenarios
./scripts/test-local-scenarios.sh
```

---

## Phase 5: Performance Baseline and Analysis

### 5.1 Performance Testing Tools

**Install vegeta** (load generator):
```bash
go install github.com/tsenart/vegeta@latest
# Or download binary from GitHub releases
```

**Install wrk2** (latency-focused):
```bash
git clone https://github.com/giltene/wrk2.git
cd wrk2
make
sudo cp wrk /usr/local/bin/wrk2
```

### 5.2 Performance Test Script

**File**: `scripts/perf-test-local.sh`

```bash
#!/bin/bash
# Performance baseline testing

GATEWAY_URL="http://127.0.0.1:8080"
DURATION="30s"
RESULTS_DIR="load-tests/results/perf-baseline-$(date +%Y%m%d-%H%M%S)"

mkdir -p "$RESULTS_DIR"

echo "=== Performance Baseline Testing ==="
echo "Duration: $DURATION"
echo "URL: $GATEWAY_URL"
echo ""

# Test 1: Vegeta - Increasing load
for rate in 100 500 1000 5000 10000; do
    echo "Testing at ${rate} req/s..."
    echo "GET $GATEWAY_URL" | vegeta attack -rate=$rate -duration=$DURATION \
        | vegeta report -type=text > "$RESULTS_DIR/vegeta-${rate}rps.txt"
    echo "  Complete. Results: $RESULTS_DIR/vegeta-${rate}rps.txt"
    sleep 5
done

# Test 2: wrk2 - Latency distribution
echo ""
echo "Testing latency distribution (1000 req/s for $DURATION)..."
wrk2 -t4 -c100 -d$DURATION -R1000 --latency $GATEWAY_URL \
    > "$RESULTS_DIR/wrk2-latency.txt"

echo ""
echo "=== Performance Baseline Complete ==="
echo "Results saved to: $RESULTS_DIR"
```

### 5.3 Expected Local Performance

**Baseline Expectations** (localhost testing):

| Metric | Conservative | Target | Stretch |
|--------|--------------|--------|---------|
| **RPS** | 10K | 50K | 100K+ |
| **P50 Latency** | < 1ms | < 0.5ms | < 0.2ms |
| **P99 Latency** | < 10ms | < 5ms | < 2ms |
| **P99.9 Latency** | < 50ms | < 25ms | < 10ms |
| **Error Rate** | < 0.1% | < 0.01% | 0% |
| **CPU (Gateway)** | 70% | 50% | 30% |
| **Memory (Gateway)** | 500MB | 300MB | 100MB |

**Note**: Local testing will have different characteristics than cloud:
- Lower latency (no network latency)
- Higher contention (single machine)
- Different bottlenecks (CPU vs network)

---

## Phase 6: Final Validation and Documentation

### 6.1 Completion Checklist

**Before declaring local testing complete**:

```
Build & Setup:
├─ [ ] Binary built successfully
├─ [ ] Backend infrastructure scripts created
├─ [ ] Test runner framework operational
└─ [ ] All prerequisites installed

Scenario Testing:
├─ [ ] Scenario 01 - Layer 4 TCP: PASSED
├─ [ ] Scenario 02 - Layer 7 HTTP: PASSED
├─ [ ] Scenario 03 - Layer 7 HTTPS/TLS: PASSED
├─ [ ] Scenario 04 - API Gateway: PASSED
├─ [ ] Scenario 05 - HTTP/3 QUIC: PASSED
├─ [ ] Scenario 06 - WebSocket: PASSED
├─ [ ] Scenario 07 - gRPC: PASSED
├─ [ ] Scenario 08 - Database LB: PASSED
├─ [ ] Scenario 09 - WAF + mTLS: PASSED
├─ [ ] Scenario 10 - Hybrid Multi-Protocol: PASSED
├─ [ ] Scenario 11 - CDN Edge Caching: PASSED
├─ [ ] Scenario 12 - Microservices Discovery: PASSED
├─ [ ] Scenario 13 - GraphQL: PASSED
├─ [ ] Scenario 14 - Static + PHP-FPM: PASSED
└─ [ ] Scenario 15 - Geographic LB: PASSED

Bug Fixing:
├─ [ ] All critical bugs fixed
├─ [ ] All high priority bugs fixed
├─ [ ] Medium/low bugs documented for later
└─ [ ] Regression tests passing

Performance:
├─ [ ] Performance baseline established
├─ [ ] All scenarios meet minimum performance targets
├─ [ ] No memory leaks detected
├─ [ ] No connection leaks detected
└─ [ ] Resource usage within acceptable limits

Documentation:
├─ [ ] All bugs documented
├─ [ ] Bug fixes documented
├─ [ ] Performance results documented
├─ [ ] Known limitations documented
└─ [ ] Final testing report written
```

---

### 6.2 Final Testing Report Template

**File**: `docs/dev-notes/LOCAL_TESTING_FINAL_REPORT_2025-12.md`

```markdown
# Local Load Testing - Final Report

**Date Completed**: YYYY-MM-DD
**Testing Duration**: X days
**Total Scenarios Tested**: 15
**Scenarios Passed**: X/15
**Critical Bugs Found**: X
**Critical Bugs Fixed**: X

---

## Executive Summary

[Brief summary of testing outcomes]

---

## Scenario Results

| # | Scenario | Status | Issues Found | Performance |
|---|----------|--------|--------------|-------------|
| 01 | Layer 4 TCP | ✅/❌ | X issues | XXK RPS |
| 02 | Layer 7 HTTP | ✅/❌ | X issues | XXK RPS |
| ... | ... | ... | ... | ... |

---

## Bugs Found and Fixed

### Critical Bugs (X found, X fixed)

[List critical bugs with status]

### High Priority Bugs (X found, X fixed)

[List high priority bugs]

### Medium/Low Priority Bugs (X found, X deferred)

[List medium/low bugs]

---

## Performance Analysis

[Detailed performance analysis]

---

## Known Limitations

[List any known limitations that are not bugs]

---

## Recommendations

### Before Cloud Deployment
1. [Recommendation 1]
2. [Recommendation 2]

### For Production
1. [Recommendation 1]
2. [Recommendation 2]

---

## Conclusion

[Overall assessment of readiness]

**Ready for Cloud Testing**: YES / NO / WITH CAVEATS
```

---

## Phase 7: Cloud Deployment Planning (Post Local Testing)

**Only proceed here after local testing is 100% complete and successful**

### 7.1 Cloud Deployment Checklist

```
Pre-Deployment:
├─ [ ] All 15 scenarios working locally
├─ [ ] All critical/high bugs fixed
├─ [ ] Performance baselines established
├─ [ ] Final testing report complete
└─ [ ] Deployment scripts updated

Provider Selection:
├─ [ ] Choose provider based on test type (see INFRASTRUCTURE_SELECTION_GUIDE.md)
├─ [ ] API keys configured
├─ [ ] Budget allocated
└─ [ ] Cost monitoring configured

First Cloud Test:
├─ [ ] Start with Scenario 01 or 02 (simplest)
├─ [ ] 1-2 hour smoke test (cost: ~$1-2)
├─ [ ] Verify configuration works
└─ [ ] If successful, proceed to other scenarios
```

---

## Timeline Estimate

### Realistic Timeline (Single Person)

| Phase | Duration | Notes |
|-------|----------|-------|
| **Phase 1**: Build & Setup | 2-4 hours | One-time setup |
| **Phase 2**: Scenario Testing (15 scenarios) | 15-30 hours | 1-2 hours per scenario |
| **Phase 3**: Bug Documentation | Ongoing | As bugs found |
| **Phase 4**: Bug Fixes | 10-40 hours | Depends on bug complexity |
| **Phase 5**: Performance Testing | 4-8 hours | After bugs fixed |
| **Phase 6**: Final Validation | 4-8 hours | Documentation, reports |
| **Phase 7**: Cloud Planning | 2-4 hours | Script updates |

**Total**: 37-94 hours (5-12 days of focused work)

### Aggressive Timeline

- **Week 1**: Phases 1-2 (Build, setup, test all scenarios) - 30 hours
- **Week 2**: Phases 3-4 (Document bugs, fix critical/high) - 30 hours
- **Week 3**: Phases 5-6 (Performance, validation, docs) - 15 hours
- **Week 4**: Phase 7 + First cloud test - 5 hours

**Total**: 80 hours (~10 days of full-time work)

---

## Success Criteria

### Local Testing Successful When:

1. ✅ **Functional**:
   - All 15 scenarios working
   - Load balancing verified
   - Health checks working
   - No crashes or panics

2. ✅ **Performance**:
   - Meeting baseline targets
   - No memory leaks
   - No connection leaks
   - Acceptable resource usage

3. ✅ **Quality**:
   - All critical bugs fixed
   - All high priority bugs fixed
   - Medium/low bugs documented
   - Comprehensive documentation

4. ✅ **Confidence**:
   - Team confident in stability
   - Clear understanding of limitations
   - Known issues documented
   - Rollback plan in place

---

## Risk Mitigation

### Potential Risks

1. **Risk**: Testing takes longer than expected
   - **Mitigation**: Focus on critical scenarios first (01, 02, 03, 04)
   - **Fallback**: Test subset of scenarios, document others as "untested"

2. **Risk**: Critical bugs block testing
   - **Mitigation**: Document bug, work on other scenarios
   - **Fallback**: Skip problematic scenario, focus on stable ones

3. **Risk**: Performance not meeting targets
   - **Mitigation**: Profile and optimize critical paths
   - **Fallback**: Lower targets, document reasons

4. **Risk**: Local testing doesn't reveal cloud issues
   - **Mitigation**: Use docker-compose for more realistic setup
   - **Fallback**: Be prepared for cloud debugging (budgeted)

---

## Cost Analysis

### Local Testing Costs

| Item | Cost | Notes |
|------|------|-------|
| **Time** | 80 hours | Developer time |
| **Infrastructure** | $0 | All local testing |
| **Tools** | $0 | All open source |
| **Total** | 80 hours + $0 | Pure time investment |

### ROI Calculation

**Without Local Testing** (based on Vultr experience):
- Cost per failed cloud test: ~$3-5
- Time per failed cloud test: ~1-2 hours
- Expected failures without testing: 5-10
- **Total cost**: $15-50 + 5-20 hours wasted

**With Local Testing**:
- Local testing cost: $0 + 80 hours
- Expected cloud test success rate: 90%+
- Expected cloud failures: 1-2
- **Total cost**: $3-10 + 1-4 hours

**Savings**: $12-40 + 4-16 hours
**ROI**: Positive after first 2-3 cloud tests

---

## Conclusion

This comprehensive plan provides a systematic approach to validate all 15 highper-gateway scenarios locally before any cloud deployment. By following this plan:

1. ✅ **Zero Cloud Costs**: All testing done locally
2. ✅ **High Confidence**: Issues found and fixed before spending money
3. ✅ **Complete Coverage**: All 15 scenarios tested
4. ✅ **Clear Documentation**: All issues tracked and understood
5. ✅ **Performance Baseline**: Know what to expect in cloud

**Next Step**: Begin Phase 1 - Build and Infrastructure Setup

**Ready to proceed when you are!**
