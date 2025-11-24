#!/bin/bash
#
# Run baseline load testing benchmarks
#
# This script:
# 1. Starts a simple HTTP backend server
# 2. Starts the Rust proxy
# 3. Runs quick baseline load tests
# 4. Cleans up
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROXY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
CONFIG_FILE="${SCRIPT_DIR}/loadtest-config.toml"
RESULTS_DIR="${SCRIPT_DIR}/results/baseline"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."

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

# Create results directory
mkdir -p "${RESULTS_DIR}"

echo "========================================"
echo "  Rust Reverse Proxy - Baseline Benchmark"
echo "========================================"
echo

# Check if proxy binary exists
if [ ! -f "${PROXY_BIN}" ]; then
    log_error "Proxy binary not found: ${PROXY_BIN}"
    log_info "Please build it first: cd highper-gateway && cargo build --release"
    exit 1
fi

# Check if config exists
if [ ! -f "${CONFIG_FILE}" ]; then
    log_error "Config file not found: ${CONFIG_FILE}"
    exit 1
fi

# Step 1: Start backend server
log_info "Starting backend server on port 9000..."
node "${SCRIPT_DIR}/simple-backend.js" > "${RESULTS_DIR}/backend.log" 2>&1 &
BACKEND_PID=$!
log_success "Backend server started (PID: $BACKEND_PID)"

# Wait for backend to be ready
sleep 2

# Verify backend is running
if ! curl -sf http://127.0.0.1:9000 > /dev/null; then
    log_error "Backend server failed to start"
    exit 1
fi
log_success "Backend server is ready"

# Step 2: Start proxy server
log_info "Starting Rust proxy..."
"${PROXY_BIN}" start --config "${CONFIG_FILE}" --log-level warn > "${RESULTS_DIR}/proxy.log" 2>&1 &
PROXY_PID=$!
log_success "Proxy started (PID: $PROXY_PID)"

# Wait for proxy to be ready
log_info "Waiting for proxy to be ready..."
for i in {1..10}; do
    if curl -sf http://127.0.0.1:8080 > /dev/null 2>&1; then
        log_success "Proxy is ready!"
        break
    fi
    if [ $i -eq 10 ]; then
        log_error "Proxy failed to start within 10 seconds"
        cat "${RESULTS_DIR}/proxy.log"
        exit 1
    fi
    sleep 1
done

echo
echo "========================================"
echo "  Running Load Tests"
echo "========================================"
echo

# Test 1: Quick vegeta test - 1000 req/s for 10 seconds
log_info "Test 1: vegeta @ 1000 req/s for 10 seconds"
TARGET_URL="http://127.0.0.1:8080"
echo "GET ${TARGET_URL}/" | ~/.local/bin/vegeta attack \
    -rate=1000 \
    -duration=10s \
    -timeout=5s \
    > "${RESULTS_DIR}/vegeta-1000rps.bin"

~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-1000rps.bin" | tee "${RESULTS_DIR}/vegeta-1000rps.txt"
log_success "Test 1 complete"

echo

# Test 2: vegeta test - 5000 req/s for 10 seconds
log_info "Test 2: vegeta @ 5000 req/s for 10 seconds"
echo "GET ${TARGET_URL}/" | ~/.local/bin/vegeta attack \
    -rate=5000 \
    -duration=10s \
    -timeout=5s \
    > "${RESULTS_DIR}/vegeta-5000rps.bin"

~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-5000rps.bin" | tee "${RESULTS_DIR}/vegeta-5000rps.txt"
log_success "Test 2 complete"

echo

# Test 3: vegeta test - 10000 req/s for 10 seconds
log_info "Test 3: vegeta @ 10000 req/s for 10 seconds"
echo "GET ${TARGET_URL}/" | ~/.local/bin/vegeta attack \
    -rate=10000 \
    -duration=10s \
    -timeout=5s \
    > "${RESULTS_DIR}/vegeta-10000rps.bin"

~/.local/bin/vegeta report "${RESULTS_DIR}/vegeta-10000rps.bin" | tee "${RESULTS_DIR}/vegeta-10000rps.txt"
log_success "Test 3 complete"

echo

# Test 4: Quick k6 test
log_info "Test 4: k6 simple HTTP test (30 seconds)"
export TARGET_URL="http://127.0.0.1:8080"
~/.local/bin/k6 run \
    --vus 100 \
    --duration 30s \
    --out json="${RESULTS_DIR}/k6-simple.json" \
    --quiet \
    "${SCRIPT_DIR}/scripts/k6-http-simple.js"
log_success "Test 4 complete"

echo
echo "========================================"
echo "  Benchmark Results Summary"
echo "========================================"
echo

# Parse and display summary
echo "vegeta @ 1000 req/s:"
grep -E "Requests|Latencies|Success" "${RESULTS_DIR}/vegeta-1000rps.txt" | head -3

echo
echo "vegeta @ 5000 req/s:"
grep -E "Requests|Latencies|Success" "${RESULTS_DIR}/vegeta-5000rps.txt" | head -3

echo
echo "vegeta @ 10000 req/s:"
grep -E "Requests|Latencies|Success" "${RESULTS_DIR}/vegeta-10000rps.txt" | head -3

echo
echo "========================================"
echo "  Results Location"
echo "========================================"
echo "All results saved to: ${RESULTS_DIR}/"
echo
echo "Files:"
ls -lh "${RESULTS_DIR}/"

echo
log_success "Baseline benchmarking complete!"
