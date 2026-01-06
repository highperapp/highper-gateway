#!/bin/bash
# Common helper functions for load testing scripts
# Used across all 15 scenario scripts

set -euo pipefail

# ========================================
# Color Definitions
# ========================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ========================================
# Logging Functions
# ========================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_debug() {
    if [ "${LOAD_TEST_VERBOSE:-false}" = "true" ]; then
        echo -e "${CYAN}[DEBUG]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
    fi
}

# ========================================
# Directory & Path Functions
# ========================================

get_script_dir() {
    local source="${BASH_SOURCE[0]}"
    while [ -h "$source" ]; do
        local dir="$(cd -P "$(dirname "$source")" && pwd)"
        source="$(readlink "$source")"
        [[ $source != /* ]] && source="$dir/$source"
    done
    cd -P "$(dirname "$source")" && pwd
}

get_project_root() {
    local script_dir="$(get_script_dir)"
    # Go up to workspace root (one level above package root)
    cd "$script_dir/../../../.." && pwd
}

get_load_test_dir() {
    local script_dir="$(get_script_dir)"
    cd "$script_dir/.." && pwd
}

# ========================================
# Environment Setup
# ========================================

setup_environment() {
    # Set defaults if not already set
    export LOAD_TEST_DURATION="${LOAD_TEST_DURATION:-60}"
    export LOAD_TEST_WARMUP="${LOAD_TEST_WARMUP:-10}"
    export LOAD_TEST_MAX_RPS="${LOAD_TEST_MAX_RPS:-500000}"
    export LOAD_TEST_VERBOSE="${LOAD_TEST_VERBOSE:-false}"
    export LOAD_TEST_CI="${LOAD_TEST_CI:-false}"
    export LOAD_TEST_KEEP_INSTANCES="${LOAD_TEST_KEEP_INSTANCES:-false}"

    # Get paths
    export PROJECT_ROOT="$(get_project_root)"
    export LOAD_TEST_DIR="$(get_load_test_dir)"
    export GATEWAY_BINARY="${PROJECT_ROOT}/target/release/highper-gateway"

    # Results directory
    export LOAD_TEST_RESULTS_DIR="${LOAD_TEST_RESULTS_DIR:-${LOAD_TEST_DIR}/results}"

    log_debug "Environment setup complete"
    log_debug "Project root: ${PROJECT_ROOT}"
    log_debug "Load test dir: ${LOAD_TEST_DIR}"
    log_debug "Results dir: ${LOAD_TEST_RESULTS_DIR}"
}

# ========================================
# Binary & Dependency Checks
# ========================================

check_binary() {
    local binary=$1
    local install_hint=${2:-""}

    if ! command -v "$binary" &> /dev/null; then
        log_error "Required binary '$binary' not found"
        if [ -n "$install_hint" ]; then
            log_info "Install hint: $install_hint"
        fi
        return 1
    fi
    log_debug "Found binary: $binary ($(command -v $binary))"
    return 0
}

check_gateway_binary() {
    if [ ! -f "${GATEWAY_BINARY}" ]; then
        log_error "Highper Gateway binary not found: ${GATEWAY_BINARY}"
        log_info "Build it with: cd ${PROJECT_ROOT} && cargo build --release"
        return 1
    fi
    log_debug "Gateway binary: ${GATEWAY_BINARY}"
    return 0
}

ensure_tool() {
    local tool=$1

    case "$tool" in
        vegeta)
            if ! check_binary vegeta ""; then
                log_info "Installing vegeta..."
                "${LOAD_TEST_DIR}/helpers/install-tools.sh" vegeta
            fi
            ;;
        k6)
            if ! check_binary k6 ""; then
                log_info "Installing k6..."
                "${LOAD_TEST_DIR}/helpers/install-tools.sh" k6
            fi
            ;;
        ghz)
            if ! check_binary ghz ""; then
                log_info "Installing ghz..."
                "${LOAD_TEST_DIR}/helpers/install-tools.sh" ghz
            fi
            ;;
        iperf3)
            check_binary iperf3 "sudo apt-get install iperf3" || return 1
            ;;
        jq)
            check_binary jq "sudo apt-get install jq" || return 1
            ;;
        *)
            log_warn "Unknown tool: $tool"
            return 1
            ;;
    esac
    return 0
}

# ========================================
# Wait & Retry Functions
# ========================================

wait_for_ready() {
    local url=$1
    local timeout=${2:-30}
    local interval=${3:-1}

    log_info "Waiting for service to be ready: $url"

    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        if curl -s -f -o /dev/null "$url" 2>/dev/null; then
            log_success "Service is ready at $url"
            return 0
        fi
        sleep $interval
        elapsed=$((elapsed + interval))
        log_debug "Waiting... ${elapsed}/${timeout}s"
    done

    log_error "Service did not become ready within ${timeout}s"
    return 1
}

wait_for_port() {
    local host=${1:-localhost}
    local port=$2
    local timeout=${3:-30}

    log_info "Waiting for port ${port} on ${host}"

    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        if timeout 1 bash -c "cat < /dev/null > /dev/tcp/${host}/${port}" 2>/dev/null; then
            log_success "Port ${port} is open on ${host}"
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
        log_debug "Waiting for port... ${elapsed}/${timeout}s"
    done

    log_error "Port ${port} did not open within ${timeout}s"
    return 1
}

# ========================================
# Process Management
# ========================================

start_background_process() {
    local name=$1
    local command=$2
    local log_file=$3

    log_info "Starting background process: $name"
    log_debug "Command: $command"
    log_debug "Log file: $log_file"

    # Create log directory if needed
    mkdir -p "$(dirname "$log_file")"

    # Start process in background
    eval "$command" > "$log_file" 2>&1 &
    local pid=$!

    # Wait a moment to check if it crashed immediately
    sleep 1
    if ! kill -0 $pid 2>/dev/null; then
        log_error "Process $name failed to start (PID: $pid)"
        log_error "Log tail:"
        tail -20 "$log_file"
        return 1
    fi

    log_success "Started $name (PID: $pid)"
    echo $pid
}

stop_process() {
    local name=$1
    local pid=$2
    local timeout=${3:-10}

    if [ -z "$pid" ] || ! kill -0 $pid 2>/dev/null; then
        log_debug "Process $name (PID: $pid) is not running"
        return 0
    fi

    log_info "Stopping process: $name (PID: $pid)"

    # Try graceful shutdown
    kill -TERM $pid 2>/dev/null || true

    # Wait for graceful shutdown
    local elapsed=0
    while kill -0 $pid 2>/dev/null && [ $elapsed -lt $timeout ]; do
        sleep 1
        elapsed=$((elapsed + 1))
    done

    # Force kill if still running
    if kill -0 $pid 2>/dev/null; then
        log_warn "Process did not stop gracefully, forcing..."
        kill -9 $pid 2>/dev/null || true
        sleep 1
    fi

    if kill -0 $pid 2>/dev/null; then
        log_error "Failed to stop process $name (PID: $pid)"
        return 1
    fi

    log_success "Stopped $name"
    return 0
}

# ========================================
# Result Collection & Analysis
# ========================================

create_result_dir() {
    local scenario=$1
    local mode=${2:-local}
    local provider=${3:-""}

    local result_path="${LOAD_TEST_RESULTS_DIR}/${mode}"
    if [ -n "$provider" ] && [ "$mode" = "cloud" ]; then
        result_path="${result_path}/${provider}"
    fi
    result_path="${result_path}/${scenario}/$(date '+%Y%m%d-%H%M%S')"

    mkdir -p "$result_path"

    # Log to stderr to avoid polluting return value
    log_info "Results will be saved to: $result_path" >&2
    echo "$result_path"
}

save_test_metadata() {
    local result_dir=$1
    local scenario=$2
    local mode=$3
    local provider=${4:-""}

    local metadata_file="${result_dir}/metadata.json"

    cat > "$metadata_file" <<EOF
{
  "scenario": "$scenario",
  "mode": "$mode",
  "provider": "$provider",
  "timestamp": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "duration": ${LOAD_TEST_DURATION},
  "warmup": ${LOAD_TEST_WARMUP},
  "max_rps": ${LOAD_TEST_MAX_RPS},
  "gateway_version": "$(${GATEWAY_BINARY} --version 2>/dev/null || echo 'unknown')",
  "hostname": "$(hostname)",
  "os": "$(uname -s)",
  "kernel": "$(uname -r)"
}
EOF

    log_debug "Saved metadata to $metadata_file"
}

# ========================================
# Performance Validation
# ========================================

validate_performance() {
    local result_file=$1
    local expected_rps=$2
    local expected_p99_ms=$3
    local expected_success_rate=${4:-99.0}

    log_info "Validating performance against baselines..."

    # Parse vegeta JSON report
    local actual_rps=$(jq -r '.rate' "$result_file" 2>/dev/null || echo "0")
    local actual_p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "$result_file" 2>/dev/null || echo "0")
    local actual_success=$(jq -r '.success * 100' "$result_file" 2>/dev/null || echo "0")

    local pass=true

    # Validate RPS (allow 10% variance)
    local min_rps=$(echo "$expected_rps * 0.9" | bc)
    if (( $(echo "$actual_rps < $min_rps" | bc -l) )); then
        log_error "RPS too low: ${actual_rps} < ${min_rps} (target: ${expected_rps})"
        pass=false
    else
        log_success "RPS: ${actual_rps} (target: ${expected_rps}) ✓"
    fi

    # Validate P99 latency
    if (( $(echo "$actual_p99 > $expected_p99_ms" | bc -l) )); then
        log_error "P99 latency too high: ${actual_p99}ms > ${expected_p99_ms}ms"
        pass=false
    else
        log_success "P99 Latency: ${actual_p99}ms (target: <${expected_p99_ms}ms) ✓"
    fi

    # Validate success rate
    if (( $(echo "$actual_success < $expected_success_rate" | bc -l) )); then
        log_error "Success rate too low: ${actual_success}% < ${expected_success_rate}%"
        pass=false
    else
        log_success "Success Rate: ${actual_success}% (target: >${expected_success_rate}%) ✓"
    fi

    if [ "$pass" = "true" ]; then
        return 0
    else
        return 1
    fi
}

# ========================================
# Report Generation
# ========================================

generate_summary_report() {
    local result_dir=$1
    local scenario=$2

    local summary_file="${result_dir}/summary.txt"

    log_info "Generating summary report..."

    cat > "$summary_file" <<EOF
========================================
Load Test Summary
========================================

Scenario: $scenario
Date: $(date)
Duration: ${LOAD_TEST_DURATION}s per load level
Warmup: ${LOAD_TEST_WARMUP}s

Results Directory: $result_dir

EOF

    # Add results from each load level
    for result_json in "$result_dir"/vegeta-*.json; do
        if [ -f "$result_json" ]; then
            local rate=$(jq -r '.rate' "$result_json")
            local p50=$(jq -r '.latencies."50th" | tonumber / 1000000' "$result_json")
            local p95=$(jq -r '.latencies."95th" | tonumber / 1000000' "$result_json")
            local p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "$result_json")
            local success=$(jq -r '.success * 100' "$result_json")

            cat >> "$summary_file" <<EOF
----------------------------------------
Rate: ${rate} req/s
  P50: ${p50}ms
  P95: ${p95}ms
  P99: ${p99}ms
  Success: ${success}%
EOF
        fi
    done

    cat >> "$summary_file" <<EOF

========================================
EOF

    log_success "Summary saved to: $summary_file"
    cat "$summary_file"
}

# ========================================
# Cleanup Functions
# ========================================

cleanup_pids() {
    local pids=("$@")

    for pid in "${pids[@]}"; do
        if [ -n "$pid" ] && kill -0 $pid 2>/dev/null; then
            log_info "Cleaning up process: $pid"
            stop_process "cleanup" "$pid" 5
        fi
    done
}

trap_cleanup() {
    log_info "Received interrupt signal, cleaning up..."
    # Cleanup will be handled by individual scripts
}

# ========================================
# CI/CD Helpers
# ========================================

is_ci() {
    [ "${LOAD_TEST_CI}" = "true" ] || [ -n "${CI:-}" ]
}

exit_for_ci() {
    local exit_code=$1
    local message=$2

    if is_ci; then
        if [ $exit_code -eq 0 ]; then
            log_success "$message"
        else
            log_error "$message"
        fi
        exit $exit_code
    else
        if [ $exit_code -eq 0 ]; then
            log_success "$message"
        else
            log_warn "$message (continuing in interactive mode)"
        fi
    fi
}

# ========================================
# Initialization
# ========================================

# Auto-setup environment when sourced
setup_environment

log_debug "Common helpers loaded"
