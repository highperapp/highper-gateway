#!/bin/bash
# Local testing framework for all 15 scenarios
# Usage: ./scripts/test-local-scenarios.sh [scenario-number]
# Example: ./scripts/test-local-scenarios.sh 01
# Or run all: ./scripts/test-local-scenarios.sh

set -e

PROJECT_ROOT="/mnt/e/my-opensource/highper-gateway"
GATEWAY_BIN="${PROJECT_ROOT}/target/release/highper-gateway"
SCENARIOS_DIR="${PROJECT_ROOT}/configs/scenarios"
RESULTS_DIR="${PROJECT_ROOT}/load-tests/results/local-$(date +%Y%m%d-%H%M%S)"
BACKEND_SCRIPT="${PROJECT_ROOT}/load-tests/simple-backend-local.py"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# PIDs tracking
BACKEND_PIDS=()
GATEWAY_PID=""

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Cleanup function
cleanup() {
    log_info "Cleaning up processes..."

    # Kill gateway
    if [ -n "$GATEWAY_PID" ] && ps -p "$GATEWAY_PID" > /dev/null 2>&1; then
        log_info "Stopping gateway (PID: $GATEWAY_PID)..."
        kill "$GATEWAY_PID" 2>/dev/null || true
        sleep 1
        kill -9 "$GATEWAY_PID" 2>/dev/null || true
    fi

    # Kill backends
    for pid in "${BACKEND_PIDS[@]}"; do
        if ps -p "$pid" > /dev/null 2>&1; then
            kill "$pid" 2>/dev/null || true
        fi
    done

    # Fallback: kill by name
    pkill -f "simple-backend-local.py" 2>/dev/null || true
    pkill -f "highper-gateway start" 2>/dev/null || true

    sleep 1
    log_success "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if [ ! -f "$GATEWAY_BIN" ]; then
        log_error "Gateway binary not found at $GATEWAY_BIN"
        log_info "Please build first: cargo build --release"
        exit 1
    fi

    if [ ! -f "$BACKEND_SCRIPT" ]; then
        log_error "Backend script not found at $BACKEND_SCRIPT"
        exit 1
    fi

    if ! command -v python3 &> /dev/null; then
        log_error "python3 not found - required for backend servers"
        exit 1
    fi

    if ! command -v curl &> /dev/null; then
        log_error "curl not found - required for testing"
        exit 1
    fi

    log_success "Prerequisites OK"
}

# Start backend servers
start_backends() {
    local num_backends=${1:-3}
    local base_port=${2:-8081}

    log_info "Starting $num_backends backend servers..."

    for i in $(seq 0 $((num_backends - 1))); do
        local port=$((base_port + i))
        log_info "  Starting backend on port $port..."
        python3 "$BACKEND_SCRIPT" $port > "${RESULTS_DIR}/backend-${port}.log" 2>&1 &
        local pid=$!
        BACKEND_PIDS+=("$pid")
        sleep 0.5
    done

    sleep 2

    # Verify backends are running
    local failed=0
    for i in $(seq 0 $((num_backends - 1))); do
        local port=$((base_port + i))
        if curl -s --connect-timeout 2 "http://127.0.0.1:$port/" > /dev/null 2>&1; then
            log_success "  Backend on port $port is responding"
        else
            log_error "  Backend on port $port is NOT responding"
            failed=1
        fi
    done

    if [ $failed -eq 1 ]; then
        log_error "Some backends failed to start"
        return 1
    fi

    log_success "All backends started successfully"
    return 0
}

# Prepare configuration for testing
prepare_config() {
    local scenario_file=$1
    local temp_config="${RESULTS_DIR}/$(basename $scenario_file)"

    mkdir -p "$RESULTS_DIR"

    # Replace BACKEND_* placeholders with localhost addresses
    sed -e 's/BACKEND_1:8080/127.0.0.1:8081/g' \
        -e 's/BACKEND_2:8080/127.0.0.1:8082/g' \
        -e 's/BACKEND_3:8080/127.0.0.1:8083/g' \
        -e 's/BACKEND_1/127.0.0.1:8081/g' \
        -e 's/BACKEND_2/127.0.0.1:8082/g' \
        -e 's/BACKEND_3/127.0.0.1:8083/g' \
        "$scenario_file" > "$temp_config"

    echo "$temp_config"
}

# Test a single scenario
test_scenario() {
    local scenario_num=$1
    # Find scenario file (may have descriptive name, try .proxy then .yaml)
    local scenario_file=$(ls "${SCENARIOS_DIR}/scenario-${scenario_num}"*.proxy 2>/dev/null | head -1)
    if [ -z "$scenario_file" ]; then
        scenario_file=$(ls "${SCENARIOS_DIR}/scenario-${scenario_num}"*.yaml 2>/dev/null | head -1)
    fi

    if [ ! -f "$scenario_file" ]; then
        log_error "Scenario file not found for scenario $scenario_num"
        log_error "Searched for: ${SCENARIOS_DIR}/scenario-${scenario_num}*.yaml"
        return 1
    fi

    echo ""
    log_info "=========================================="
    log_info "Testing Scenario $scenario_num"
    local scenario_name=$(head -1 "$scenario_file" | sed 's/^# Scenario [0-9]*: //')
    log_info "Name: $scenario_name"
    log_info "File: $(basename $scenario_file)"
    log_info "=========================================="

    # Prepare configuration
    local temp_config=$(prepare_config "$scenario_file")
    log_info "Configuration: $temp_config"

    # Start gateway
    log_info "Starting gateway..."
    "$GATEWAY_BIN" start -c "$temp_config" > "${RESULTS_DIR}/scenario-${scenario_num}-gateway.log" 2>&1 &
    GATEWAY_PID=$!
    sleep 3

    # Check if gateway is running
    if ! ps -p $GATEWAY_PID > /dev/null 2>&1; then
        log_error "Gateway failed to start"
        log_error "Last 20 lines of gateway log:"
        tail -20 "${RESULTS_DIR}/scenario-${scenario_num}-gateway.log"
        return 1
    fi

    log_success "Gateway started (PID: $GATEWAY_PID)"

    # Run basic connectivity test
    log_info "Testing basic connectivity (10 requests)..."
    local success_count=0
    local response_file="${RESULTS_DIR}/scenario-${scenario_num}-test-responses.txt"
    > "$response_file"  # Clear file

    for i in {1..10}; do
        if curl -s --connect-timeout 2 --max-time 5 "http://127.0.0.1:8080/" >> "$response_file" 2>&1; then
            ((success_count++))
        fi
        sleep 0.1
    done

    if [ $success_count -eq 10 ]; then
        log_success "Basic connectivity: 10/10 requests succeeded"
    elif [ $success_count -gt 5 ]; then
        log_warn "Basic connectivity: $success_count/10 requests succeeded"
    else
        log_error "Basic connectivity: $success_count/10 requests succeeded"
        log_error "Gateway may not be working correctly"
    fi

    # Test load balancing distribution
    log_info "Testing load balancing distribution (30 requests)..."
    local dist_file="${RESULTS_DIR}/scenario-${scenario_num}-distribution.txt"
    > "$dist_file"

    for i in {1..30}; do
        curl -s --connect-timeout 2 --max-time 5 "http://127.0.0.1:8080/" >> "$dist_file" 2>&1
        sleep 0.05
    done

    # Analyze distribution
    if [ -f "$dist_file" ]; then
        log_info "Distribution analysis:"
        if grep -q '"backend"' "$dist_file" 2>/dev/null; then
            grep -o '"backend":"[^"]*"' "$dist_file" 2>/dev/null | sort | uniq -c | while read count backend; do
                local percentage=$(awk "BEGIN {printf \"%.1f\", ($count/30)*100}")
                log_info "  $backend: $count requests ($percentage%)"
            done
        else
            log_warn "Could not parse backend responses for distribution"
        fi
    fi

    # Check metrics endpoint
    log_info "Checking metrics endpoint..."
    if curl -s --connect-timeout 2 "http://127.0.0.1:9090/metrics" > "${RESULTS_DIR}/scenario-${scenario_num}-metrics.txt" 2>&1; then
        log_success "Metrics endpoint accessible"
        local metric_count=$(wc -l < "${RESULTS_DIR}/scenario-${scenario_num}-metrics.txt")
        log_info "Metrics captured: $metric_count lines"
    else
        log_warn "Metrics endpoint not accessible (may not be enabled for this scenario)"
    fi

    # Stop gateway for next test
    log_info "Stopping gateway..."
    kill $GATEWAY_PID 2>/dev/null || true
    sleep 2
    kill -9 $GATEWAY_PID 2>/dev/null || true
    GATEWAY_PID=""

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
    start_backends 3 8081 || exit 1

    if [ -n "$scenario_num" ]; then
        # Test single scenario
        log_info "Testing single scenario: $scenario_num"
        test_scenario "$scenario_num"
        local result=$?
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
                log_error "Scenario $num failed, continuing with next..."
            fi
            sleep 2  # Brief pause between scenarios
        done

        echo ""
        log_info "================================================"
        log_info "Test Summary"
        log_info "================================================"
        log_success "Passed: $passed/15"
        if [ $failed -gt 0 ]; then
            log_error "Failed: $failed/15"
        else
            log_info "Failed: $failed/15"
        fi
        log_info "Results directory: $RESULTS_DIR"
        local result=$failed
    fi

    return $result
}

# Run main with scenario number argument
main "$@"
