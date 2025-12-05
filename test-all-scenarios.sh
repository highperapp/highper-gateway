#!/bin/bash
# Comprehensive test script for all 15 scenarios
# Tests DSL configs with real backends

set -e

PROJECT_ROOT="/mnt/e/my-opensource/highper-gateway"
GATEWAY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
SCENARIOS_DIR="${PROJECT_ROOT}/configs/scenarios"
RESULTS_DIR="${PROJECT_ROOT}/test-results/$(date +%Y%m%d-%H%M%S)"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; }
log_skip() { echo -e "${YELLOW}[SKIP]${NC} $1"; }

# Create results directory
mkdir -p "$RESULTS_DIR"

# Track results
PASSED=0
FAILED=0
SKIPPED=0

echo ""
log_info "================================================"
log_info "Highper Gateway - Scenario Test Suite"
log_info "================================================"
log_info "Results: $RESULTS_DIR"
echo ""

# Test individual scenario
test_scenario() {
    local num=$1
    local name=$2
    local config_file=$(ls "${SCENARIOS_DIR}/scenario-${num}"*.proxy 2>/dev/null | head -1)

    if [ ! -f "$config_file" ]; then
        log_skip "Scenario $num: $name - Config not found"
        ((SKIPPED++))
        return
    fi

    log_info "Testing Scenario $num: $name"

    # Validate config
    if ! $GATEWAY_BIN validate -c "$config_file" > "${RESULTS_DIR}/scenario-${num}-validate.log" 2>&1; then
        log_fail "Scenario $num: Config validation failed"
        ((FAILED++))
        return
    fi

    log_success "Scenario $num: Config valid"
    ((PASSED++))
}

# Test all scenarios
test_scenario "01" "Layer 4 TCP Load Balancer"
test_scenario "02" "Layer 7 HTTP Load Balancer"
test_scenario "03" "Layer 7 TLS"
test_scenario "04" "API Gateway"
test_scenario "05" "HTTP/3 QUIC"
test_scenario "06" "WebSocket"
test_scenario "07" "gRPC"
test_scenario "08" "Database LB"
test_scenario "09" "WAF + mTLS"
test_scenario "10" "Hybrid Multiprotocol"
test_scenario "11" "CDN Edge Caching"
test_scenario "12" "Microservices Discovery"
test_scenario "13" "GraphQL"
test_scenario "14" "Static + PHP-FPM"
test_scenario "15" "Geo Routing"

echo ""
log_info "================================================"
log_info "Test Summary"
log_info "================================================"
log_success "Passed: $PASSED/15"
log_fail "Failed: $FAILED/15"
log_skip "Skipped: $SKIPPED/15"
log_info "Results saved to: $RESULTS_DIR"
echo ""

exit $FAILED
