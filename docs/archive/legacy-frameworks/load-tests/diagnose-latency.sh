#!/bin/bash
#
# Diagnostic script to investigate p99 latency issue at 10k req/s
#
# This script:
# 1. Starts backend and proxy with monitoring
# 2. Runs load test while collecting system metrics
# 3. Analyzes results to identify bottleneck
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROXY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
CONFIG_FILE="${SCRIPT_DIR}/loadtest-config.toml"
RESULTS_DIR="${SCRIPT_DIR}/results/diagnostic"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Cleanup function
cleanup() {
    log_info "Cleaning up..."

    # Stop monitoring
    if [ -n "${MONITOR_PID:-}" ]; then
        kill $MONITOR_PID 2>/dev/null || true
    fi

    if [ -n "${BACKEND_PID:-}" ]; then
        log_info "Stopping backend server (PID: $BACKEND_PID)"
        kill $BACKEND_PID 2>/dev/null || true
    fi

    if [ -n "${PROXY_PID:-}" ]; then
        log_info "Stopping proxy server (PID: $PROXY_PID)"
        kill $PROXY_PID 2>/dev/null || true
        sleep 1
        kill -9 $PROXY_PID 2>/dev/null || true
    fi

    log_success "Cleanup complete"
}

trap cleanup EXIT INT TERM

mkdir -p "${RESULTS_DIR}"

echo "========================================"
echo "  P99 Latency Diagnostic Analysis"
echo "========================================"
echo

# Start backend
log_info "Starting backend server..."
node "${SCRIPT_DIR}/simple-backend.js" > "${RESULTS_DIR}/backend.log" 2>&1 &
BACKEND_PID=$!
sleep 2

if ! curl -sf http://127.0.0.1:9000 > /dev/null; then
    log_error "Backend failed to start"
    exit 1
fi
log_success "Backend ready (PID: $BACKEND_PID)"

# Start proxy
log_info "Starting proxy..."
"${PROXY_BIN}" start --config "${CONFIG_FILE}" --log-level info > "${RESULTS_DIR}/proxy.log" 2>&1 &
PROXY_PID=$!
sleep 3

for i in {1..10}; do
    if curl -sf http://127.0.0.1:8080 > /dev/null 2>&1; then
        log_success "Proxy ready (PID: $PROXY_PID)"
        break
    fi
    if [ $i -eq 10 ]; then
        log_error "Proxy failed to start"
        exit 1
    fi
    sleep 1
done

echo
echo "========================================"
echo "  Collecting System Metrics"
echo "========================================"
echo

# Start system monitoring in background
log_info "Starting system monitoring..."
(
    while true; do
        timestamp=$(date +%s)

        # CPU and memory for proxy process
        if [ -d "/proc/$PROXY_PID" ]; then
            cpu=$(ps -p $PROXY_PID -o %cpu= | tr -d ' ')
            mem=$(ps -p $PROXY_PID -o %mem= | tr -d ' ')
            rss=$(ps -p $PROXY_PID -o rss= | tr -d ' ')

            # Thread count
            threads=$(ps -p $PROXY_PID -o nlwp= | tr -d ' ')

            # File descriptors
            fds=$(ls /proc/$PROXY_PID/fd 2>/dev/null | wc -l)

            # TCP connections
            tcp_conns=$(ss -tnp 2>/dev/null | grep -c "pid=$PROXY_PID" || echo 0)

            echo "$timestamp,$cpu,$mem,$rss,$threads,$fds,$tcp_conns" >> "${RESULTS_DIR}/proxy-metrics.csv"
        fi

        sleep 1
    done
) &
MONITOR_PID=$!
log_success "Monitoring started (PID: $MONITOR_PID)"

# Create metrics CSV header
echo "timestamp,cpu_percent,mem_percent,rss_kb,threads,file_descriptors,tcp_connections" > "${RESULTS_DIR}/proxy-metrics.csv"

# Wait for metrics to stabilize
sleep 3

echo
echo "========================================"
echo "  Running Diagnostic Load Tests"
echo "========================================"
echo

# Test 1: Baseline - 1k req/s
log_info "Test 1: Baseline @ 1k req/s (should have good p99)"
echo "GET http://127.0.0.1:8080/" | ~/.local/bin/vegeta attack \
    -rate=1000 \
    -duration=30s \
    -timeout=10s \
    > "${RESULTS_DIR}/vegeta-1k.bin"

~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-1k.bin" > "${RESULTS_DIR}/vegeta-1k.txt"
p99_1k=$(grep "99th percentile" "${RESULTS_DIR}/vegeta-1k.txt" | awk '{print $3}' || echo "N/A")
log_success "Test 1 complete - p99: $p99_1k"

sleep 5

# Test 2: Problem load - 10k req/s
log_info "Test 2: Problem load @ 10k req/s (should show high p99)"
echo "GET http://127.0.0.1:8080/" | ~/.local/bin/vegeta attack \
    -rate=10000 \
    -duration=30s \
    -timeout=10s \
    > "${RESULTS_DIR}/vegeta-10k.bin"

~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-10k.bin" > "${RESULTS_DIR}/vegeta-10k.txt"
p99_10k=$(grep "99th percentile" "${RESULTS_DIR}/vegeta-10k.txt" | awk '{print $3}' || echo "N/A")
log_success "Test 2 complete - p99: $p99_10k"

sleep 5

# Test 3: Ramp test to identify degradation point
log_info "Test 3: Ramp test (2k → 4k → 6k → 8k → 10k)"
for rate in 2000 4000 6000 8000 10000; do
    log_info "  Testing @ ${rate} req/s..."
    echo "GET http://127.0.0.1:8080/" | ~/.local/bin/vegeta attack \
        -rate=${rate} \
        -duration=15s \
        -timeout=10s \
        > "${RESULTS_DIR}/vegeta-${rate}.bin"

    ~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-${rate}.bin" > "${RESULTS_DIR}/vegeta-${rate}.txt"

    p99=$(grep "99th percentile" "${RESULTS_DIR}/vegeta-${rate}.txt" | awk '{print $3}' || echo "N/A")
    success=$(grep "Success" "${RESULTS_DIR}/vegeta-${rate}.txt" | awk '{print $2}' || echo "N/A")

    log_info "    p99: $p99, success: $success"

    sleep 3
done

log_success "Ramp test complete"

echo
echo "========================================"
echo "  Analysis Results"
echo "========================================"
echo

# Analyze proxy logs for errors
log_info "Analyzing proxy logs..."
error_count=$(grep -i "error" "${RESULTS_DIR}/proxy.log" | wc -l || echo 0)
warn_count=$(grep -i "warn" "${RESULTS_DIR}/proxy.log" | wc -l || echo 0)
log_info "Errors: $error_count, Warnings: $warn_count"

if [ $error_count -gt 0 ]; then
    log_warn "Sample errors:"
    grep -i "error" "${RESULTS_DIR}/proxy.log" | head -5
fi

# Analyze metrics
log_info "Analyzing resource usage..."
if [ -f "${RESULTS_DIR}/proxy-metrics.csv" ]; then
    # Peak CPU
    peak_cpu=$(tail -n +2 "${RESULTS_DIR}/proxy-metrics.csv" | cut -d',' -f2 | sort -n | tail -1)
    avg_cpu=$(tail -n +2 "${RESULTS_DIR}/proxy-metrics.csv" | cut -d',' -f2 | awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print 0}')

    # Peak memory
    peak_mem=$(tail -n +2 "${RESULTS_DIR}/proxy-metrics.csv" | cut -d',' -f4 | sort -n | tail -1)

    # Peak FDs
    peak_fds=$(tail -n +2 "${RESULTS_DIR}/proxy-metrics.csv" | cut -d',' -f6 | sort -n | tail -1)

    # Peak TCP connections
    peak_tcp=$(tail -n +2 "${RESULTS_DIR}/proxy-metrics.csv" | cut -d',' -f7 | sort -n | tail -1)

    log_info "Peak CPU: ${peak_cpu}%"
    log_info "Avg CPU: ${avg_cpu}%"
    log_info "Peak Memory: ${peak_mem} KB"
    log_info "Peak File Descriptors: ${peak_fds}"
    log_info "Peak TCP Connections: ${peak_tcp}"
fi

# Compare latencies
echo
log_info "Latency Comparison:"
echo "Rate     | p99 Latency"
echo "---------|------------"
for rate in 1k 2000 4000 6000 8000 10k; do
    if [ -f "${RESULTS_DIR}/vegeta-${rate}.txt" ]; then
        p99=$(grep "Latencies" "${RESULTS_DIR}/vegeta-${rate}.txt" -A 1 | tail -1 | awk '{print $7}')
        printf "%-8s | %s\n" "$rate" "$p99"
    fi
done

echo
echo "========================================"
echo "  Diagnostic Summary"
echo "========================================"
echo

# Generate recommendations based on findings
log_info "Generating diagnostic report..."

cat > "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md" << 'EOF'
# P99 Latency Diagnostic Report

## Test Configuration
- Backend: Node.js simple HTTP server
- Proxy: Rust Reverse Proxy (release build)
- Test Duration: 15-30 seconds per rate
- Rates Tested: 1k, 2k, 4k, 6k, 8k, 10k req/s

## Results Summary

EOF

# Add latency comparison to report
echo "### Latency by Load" >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"
echo "" >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"
echo "| Rate | p99 Latency | p95 Latency | p50 Latency | Success Rate |" >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"
echo "|------|-------------|-------------|-------------|--------------|" >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"

for rate in 1k 2000 4000 6000 8000 10k; do
    if [ -f "${RESULTS_DIR}/vegeta-${rate}.txt" ]; then
        p99=$(grep "Latencies" "${RESULTS_DIR}/vegeta-${rate}.txt" -A 1 | tail -1 | awk '{print $7}')
        p95=$(grep "Latencies" "${RESULTS_DIR}/vegeta-${rate}.txt" -A 1 | tail -1 | awk '{print $6}')
        p50=$(grep "Latencies" "${RESULTS_DIR}/vegeta-${rate}.txt" -A 1 | tail -1 | awk '{print $4}')
        success=$(grep "Success" "${RESULTS_DIR}/vegeta-${rate}.txt" | awk '{print $2}')
        echo "| $rate req/s | $p99 | $p95 | $p50 | $success |" >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"
    fi
done

# Add resource usage
cat >> "${RESULTS_DIR}/DIAGNOSTIC_REPORT.md" << EOF

### Resource Usage

- **Peak CPU:** ${peak_cpu:-N/A}%
- **Average CPU:** ${avg_cpu:-N/A}%
- **Peak Memory:** ${peak_mem:-N/A} KB
- **Peak File Descriptors:** ${peak_fds:-N/A}
- **Peak TCP Connections:** ${peak_tcp:-N/A}

### Error Analysis

- **Total Errors:** $error_count
- **Total Warnings:** $warn_count

## Next Steps

Based on this diagnostic, the next steps are:

1. **Review proxy logs** for specific error patterns
2. **Profile with flamegraph** to identify hot code paths
3. **Check Tokio runtime** metrics for task scheduling delays
4. **Analyze connection pool** behavior and wait times
5. **Test with faster backend** to isolate proxy vs backend issue

## Files Generated

- \`vegeta-*.bin\` - Raw vegeta results
- \`vegeta-*.txt\` - Vegeta reports
- \`proxy-metrics.csv\` - System metrics over time
- \`proxy.log\` - Proxy application logs
- \`backend.log\` - Backend server logs

EOF

log_success "Diagnostic report saved to ${RESULTS_DIR}/DIAGNOSTIC_REPORT.md"

echo
log_info "All results saved to: ${RESULTS_DIR}/"
echo
log_success "Diagnostic complete! Review DIAGNOSTIC_REPORT.md for analysis."
