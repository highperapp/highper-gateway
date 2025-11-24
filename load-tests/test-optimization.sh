#!/bin/bash
#
# Test optimization impact on p99 latency
#
# Compares:
# - Old config (no connection pool tuning)
# - New config (optimized connection pool)
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROXY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
OLD_CONFIG="${SCRIPT_DIR}/loadtest-config.toml"
NEW_CONFIG="${SCRIPT_DIR}/loadtest-config-optimized.toml"
RESULTS_DIR="${SCRIPT_DIR}/results/optimization"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

cleanup() {
    log_info "Cleaning up..."
    pkill -P $$ 2>/dev/null || true
    killall node 2>/dev/null || true
    log_success "Cleanup complete"
}

trap cleanup EXIT INT TERM

mkdir -p "${RESULTS_DIR}"

echo "========================================"
echo "  P99 Latency Optimization Test"
echo "========================================"
echo

# Start backend once (reuse for both tests)
log_info "Starting backend server..."
node "${SCRIPT_DIR}/simple-backend.js" > "${RESULTS_DIR}/backend.log" 2>&1 &
BACKEND_PID=$!
sleep 2

if ! curl -sf http://127.0.0.1:9000 > /dev/null; then
    echo "Backend failed to start"
    exit 1
fi
log_success "Backend ready (PID: $BACKEND_PID)"

# Function to run test with a config
run_test() {
    local config_name=$1
    local config_file=$2
    local output_prefix=$3

    echo
    echo "========================================"
    echo "  Testing: $config_name"
    echo "========================================"
    echo

    log_info "Starting proxy with $config_name..."
    "${PROXY_BIN}" start --config "$config_file" --log-level warn > "${RESULTS_DIR}/${output_prefix}-proxy.log" 2>&1 &
    local proxy_pid=$!
    sleep 3

    # Wait for proxy to be ready
    for i in {1..10}; do
        if curl -sf http://127.0.0.1:8080 > /dev/null 2>&1; then
            log_success "Proxy ready (PID: $proxy_pid)"
            break
        fi
        if [ $i -eq 10 ]; then
            echo "Proxy failed to start"
            kill $proxy_pid 2>/dev/null || true
            return 1
        fi
        sleep 1
    done

    # Run 30-second test @ 10k req/s
    log_info "Running load test (10k req/s for 30s)..."
    echo "GET http://127.0.0.1:8080/" | ~/.local/bin/vegeta attack \
        -rate=10000 \
        -duration=30s \
        -timeout=10s \
        > "${RESULTS_DIR}/${output_prefix}.bin"

    # Generate report
    ~/.local/bin/vegeta report "${RESULTS_DIR}/${output_prefix}.bin" > "${RESULTS_DIR}/${output_prefix}.txt"

    # Extract key metrics
    local p50=$(grep "Latencies" "${RESULTS_DIR}/${output_prefix}.txt" | awk '{print $4}')
    local p95=$(grep "Latencies" "${RESULTS_DIR}/${output_prefix}.txt" | awk '{print $6}')
    local p99=$(grep "Latencies" "${RESULTS_DIR}/${output_prefix}.txt" | awk '{print $7}')
    local max=$(grep "Latencies" "${RESULTS_DIR}/${output_prefix}.txt" | awk '{print $8}')
    local success=$(grep "Success" "${RESULTS_DIR}/${output_prefix}.txt" | awk '{print $2}')

    log_success "Test complete"
    echo "  p50: $p50"
    echo "  p95: $p95"
    echo "  p99: $p99"
    echo "  max: $max"
    echo "  success: $success"

    # Stop proxy
    log_info "Stopping proxy..."
    kill $proxy_pid 2>/dev/null || true
    sleep 2
    kill -9 $proxy_pid 2>/dev/null || true

    # Save metrics for comparison
    echo "$p50,$p95,$p99,$max,$success" > "${RESULTS_DIR}/${output_prefix}-metrics.csv"
}

# Test 1: Old configuration
run_test "Old Configuration (No Tuning)" "$OLD_CONFIG" "old-config"

sleep 5

# Test 2: New configuration
run_test "New Configuration (Optimized)" "$NEW_CONFIG" "new-config"

# Generate comparison report
echo
echo "========================================"
echo "  Comparison Report"
echo "========================================"
echo

# Read metrics
old_metrics=$(cat "${RESULTS_DIR}/old-config-metrics.csv")
new_metrics=$(cat "${RESULTS_DIR}/new-config-metrics.csv")

old_p99=$(echo $old_metrics | cut -d',' -f3)
new_p99=$(echo $new_metrics | cut -d',' -f3)

cat > "${RESULTS_DIR}/COMPARISON.md" << EOF
# P99 Latency Optimization Results

## Test Configuration
- Load: 10,000 requests/second
- Duration: 30 seconds
- Total Requests: ~300,000
- Backend: Node.js simple HTTP server

## Results

### Old Configuration (Baseline)
\`\`\`
$(cat "${RESULTS_DIR}/old-config.txt" | head -4)
\`\`\`

### New Configuration (Optimized)
\`\`\`
$(cat "${RESULTS_DIR}/new-config.txt" | head -4)
\`\`\`

## Comparison

| Metric | Old Config | New Config | Improvement |
|--------|------------|------------|-------------|
| p50 | $(echo $old_metrics | cut -d',' -f1) | $(echo $new_metrics | cut -d',' -f1) | TBD |
| p95 | $(echo $old_metrics | cut -d',' -f2) | $(echo $new_metrics | cut -d',' -f2) | TBD |
| p99 | $(echo $old_metrics | cut -d',' -f3) | $(echo $new_metrics | cut -d',' -f3) | **PRIMARY METRIC** |
| max | $(echo $old_metrics | cut -d',' -f4) | $(echo $new_metrics | cut -d',' -f4) | TBD |
| Success Rate | $(echo $old_metrics | cut -d',' -f5) | $(echo $new_metrics | cut -d',' -f5) | TBD |

## Optimization Changes

1. **Connection Pool Size**: 100 → 500
2. **Pre-warming**: Disabled → 50 idle connections
3. **Per-server Limit**: Default → 1,000 connections
4. **Connection Timeout**: Default → 5s
5. **TCP Tuning**: Basic → Optimized (nodelay, larger buffers)

## Latency Histograms

### Old Configuration
\`\`\`
$(~/.local/bin/vegeta report -type='hist[0,1ms,2ms,5ms,10ms,20ms,50ms,100ms,200ms,500ms]' "${RESULTS_DIR}/old-config.bin")
\`\`\`

### New Configuration
\`\`\`
$(~/.local/bin/vegeta report -type='hist[0,1ms,2ms,5ms,10ms,20ms,50ms,100ms,200ms,500ms]' "${RESULTS_DIR}/new-config.bin")
\`\`\`

## Conclusion

$(if [ "$new_p99" \< "$old_p99" 2>/dev/null ]; then
    echo "✅ **OPTIMIZATION SUCCESSFUL** - p99 latency improved from $old_p99 to $new_p99"
else
    echo "⚠️ **NO IMPROVEMENT** - Further investigation needed"
fi)

EOF

log_success "Comparison report saved to ${RESULTS_DIR}/COMPARISON.md"
cat "${RESULTS_DIR}/COMPARISON.md"

echo
log_success "Optimization test complete!"
echo "Results in: ${RESULTS_DIR}/"
