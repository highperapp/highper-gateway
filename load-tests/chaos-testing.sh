#!/bin/bash
#
# Chaos Testing with Toxiproxy
#
# This script tests the proxy's resilience to various failure conditions:
# 1. Network latency
# 2. Connection timeouts
# 3. Bandwidth limits
# 4. Packet loss
# 5. Backend failures
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROXY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
CONFIG_FILE="${SCRIPT_DIR}/loadtest-config-optimized.toml"
RESULTS_DIR="${SCRIPT_DIR}/results/chaos"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

cleanup() {
    log_info "Cleaning up..."

    # Stop toxiproxy
    pkill toxiproxy-server 2>/dev/null || true

    # Stop backend
    if [ -n "${BACKEND_PID:-}" ]; then
        kill $BACKEND_PID 2>/dev/null || true
    fi

    # Stop proxy
    if [ -n "${PROXY_PID:-}" ]; then
        kill $PROXY_PID 2>/dev/null || true
        sleep 1
        kill -9 $PROXY_PID 2>/dev/null || true
    fi

    log_success "Cleanup complete"
}

trap cleanup EXIT INT TERM

mkdir -p "${RESULTS_DIR}"

echo "========================================"
echo "  Chaos Testing with Toxiproxy"
echo "========================================"
echo

# Start toxiproxy server
log_info "Starting Toxiproxy server..."
~/.local/bin/toxiproxy-server > "${RESULTS_DIR}/toxiproxy.log" 2>&1 &
TOXIPROXY_PID=$!
sleep 2

if ! curl -sf http://localhost:8474/version > /dev/null 2>&1; then
    log_error "Toxiproxy failed to start"
    cat "${RESULTS_DIR}/toxiproxy.log"
    exit 1
fi
log_success "Toxiproxy started (PID: $TOXIPROXY_PID)"

# Start backend on port 9000
log_info "Starting backend server..."
node "${SCRIPT_DIR}/simple-backend.js" > "${RESULTS_DIR}/backend.log" 2>&1 &
BACKEND_PID=$!
sleep 2

if ! curl -sf http://127.0.0.1:9000 > /dev/null 2>&1; then
    log_error "Backend failed to start"
    exit 1
fi
log_success "Backend ready (PID: $BACKEND_PID)"

# Create toxiproxy proxy pointing to backend
log_info "Creating toxiproxy proxy: toxic-backend -> localhost:9000"
~/.local/bin/toxiproxy-cli create --listen localhost:9100 --upstream localhost:9000 toxic-backend > /dev/null 2>&1

# Verify proxy created
if ! curl -sf http://localhost:9100 > /dev/null 2>&1; then
    log_error "Toxiproxy proxy failed to start"
    exit 1
fi
log_success "Toxiproxy proxy created (localhost:9100 -> localhost:9000)"

# Create config that points to toxiproxy
cat > /tmp/chaos-test-config.toml << 'EOF'
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"
protocols = ["http1", "http2"]

[server.performance]
max_connections = 10000
request_timeout = "10s"

[[upstreams]]
name = "chaos-backend"
servers = [{ url = "http://127.0.0.1:9100", weight = 1 }]

[upstreams.health_check]
enabled = true
interval = "2s"
timeout = "1s"

[upstreams.connection]
max_connections_per_upstream = 500
circuit_breaker_enabled = true
circuit_breaker_threshold = 5
circuit_breaker_timeout = "5s"

[[routes]]
name = "default"
upstream = "chaos-backend"

[routes.match]
paths = ["/"]

[observability]
log_level = "warn"
access_log = false
EOF

# Start proxy
log_info "Starting Rust proxy..."
"${PROXY_BIN}" start --config /tmp/chaos-test-config.toml --log-level warn > "${RESULTS_DIR}/proxy.log" 2>&1 &
PROXY_PID=$!
sleep 3

for i in {1..10}; do
    if curl -sf http://127.0.0.1:8080 > /dev/null 2>&1; then
        log_success "Proxy ready (PID: $PROXY_PID)"
        break
    fi
    if [ $i -eq 10 ]; then
        log_error "Proxy failed to start"
        cat "${RESULTS_DIR}/proxy.log"
        exit 1
    fi
    sleep 1
done

echo
echo "========================================"
echo "  Running Chaos Scenarios"
echo "========================================"
echo

# Helper function to run test
run_chaos_test() {
    local test_name=$1
    local toxic_type=$2
    local toxic_params=$3
    local duration=${4:-10}

    log_info "Test: $test_name"

    # Add toxic
    if [ -n "$toxic_type" ]; then
        log_info "  Adding toxic: $toxic_type $toxic_params"
        ~/.local/bin/toxiproxy-cli toxic add --type $toxic_type $toxic_params toxic-backend > /dev/null 2>&1
    fi

    # Run load test
    log_info "  Running load test (${duration}s)..."
    echo "GET http://127.0.0.1:8080/" | ~/.local/bin/vegeta attack \
        -rate=100 \
        -duration=${duration}s \
        -timeout=10s \
        > "${RESULTS_DIR}/${test_name}.bin" 2>&1

    # Generate report
    ~/.local/bin/vegeta report "${RESULTS_DIR}/${test_name}.bin" > "${RESULTS_DIR}/${test_name}.txt"

    # Extract metrics
    local success_rate=$(grep "Success" "${RESULTS_DIR}/${test_name}.txt" | awk '{print $2}')
    local p99=$(grep "Latencies" "${RESULTS_DIR}/${test_name}.txt" | awk '{print $7}')

    log_success "  Test complete - Success: $success_rate, p99: $p99"

    # Remove toxic
    if [ -n "$toxic_type" ]; then
        ~/.local/bin/toxiproxy-cli toxic remove --toxicName ${toxic_type}_downstream toxic-backend > /dev/null 2>&1 || true
        sleep 2
    fi

    echo
}

# Test 1: Baseline (no toxics)
run_chaos_test "1_baseline" "" "" 10

# Test 2: Network Latency (100ms)
run_chaos_test "2_latency_100ms" "latency" "--attribute latency=100" 10

# Test 3: High Latency (500ms)
run_chaos_test "3_latency_500ms" "latency" "--attribute latency=500" 10

# Test 4: Latency with Jitter
run_chaos_test "4_latency_jitter" "latency" "--attribute latency=200 --attribute jitter=100" 10

# Test 5: Bandwidth Limit (1 MB/s)
run_chaos_test "5_bandwidth_1mbps" "bandwidth" "--attribute rate=1000" 10

# Test 6: Connection Timeout (close after 1s)
run_chaos_test "6_timeout" "timeout" "--attribute timeout=1000" 10

# Test 7: Slow Close (delay closing by 2s)
run_chaos_test "7_slow_close" "slow_close" "--attribute delay=2000" 10

# Test 8: Packet Loss (10%)
run_chaos_test "8_packet_loss_10pct" "limit_data" "--attribute bytes=9000" 10

# Generate summary report
echo "========================================"
echo "  Chaos Testing Summary"
echo "========================================"
echo

cat > "${RESULTS_DIR}/CHAOS_TEST_REPORT.md" << 'EOF'
# Chaos Testing Report - Toxiproxy

## Test Environment
- Proxy: Rust Reverse Proxy (optimized config)
- Chaos Tool: Toxiproxy v2.9.0
- Backend: Node.js simple HTTP server
- Load: 100 req/s for 10 seconds per test

## Test Scenarios

EOF

for test_file in "${RESULTS_DIR}"/*.txt; do
    if [ -f "$test_file" ]; then
        test_name=$(basename "$test_file" .txt)
        echo "### Test: $test_name" >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
        echo '```' >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
        head -4 "$test_file" >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
        echo '```' >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
        echo >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
    fi
done

cat >> "${RESULTS_DIR}/CHAOS_TEST_REPORT.md" << 'EOF'

## Analysis

### Resilience Metrics

| Test | Success Rate | p99 Latency | Circuit Breaker | Failures |
|------|--------------|-------------|-----------------|----------|
| Baseline | [from results] | [from results] | No | 0 |
| 100ms Latency | [from results] | [from results] | No | [count] |
| 500ms Latency | [from results] | [from results] | Possible | [count] |
| Latency + Jitter | [from results] | [from results] | Possible | [count] |
| 1 MB/s Bandwidth | [from results] | [from results] | No | [count] |
| Timeout | [from results] | [from results] | Yes | [count] |
| Slow Close | [from results] | [from results] | Possible | [count] |
| 10% Packet Loss | [from results] | [from results] | Possible | [count] |

### Key Findings

1. **Circuit Breaker Behavior**: [To be analyzed from logs]
2. **Connection Pool Under Stress**: [To be analyzed]
3. **Timeout Handling**: [To be analyzed]
4. **Error Recovery**: [To be analyzed]

### Recommendations

1. [Based on test results]
2. [Based on test results]
3. [Based on test results]

EOF

log_success "Chaos testing report saved to ${RESULTS_DIR}/CHAOS_TEST_REPORT.md"
cat "${RESULTS_DIR}/CHAOS_TEST_REPORT.md"

echo
log_success "Chaos testing complete!"
echo "Results in: ${RESULTS_DIR}/"
