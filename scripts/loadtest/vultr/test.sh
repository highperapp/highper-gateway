#!/bin/bash
# Vultr load test execution script

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/../common/utils.sh"

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Parse arguments
STATE_FILE="${1:-}"
SCENARIO="${2:-scenario-02}"

if [ -z "$STATE_FILE" ] || [ ! -f "$STATE_FILE" ]; then
    log_error "State file not found: ${STATE_FILE}"
    log_info "Usage: $0 <state_file> [scenario]"
    exit 1
fi

# Test configuration
DURATION=${LOAD_TEST_DURATION:-180}
TARGET_RPS=${LOAD_TEST_TARGET_RPS:-600000}
CONNECTIONS=${LOAD_TEST_TARGET_CONCURRENCY:-3000000}

log_info "============================================"
log_info "Vultr Load Test Execution"
log_info "============================================"
log_info "Scenario: ${SCENARIO}"
log_info "Duration: ${DURATION}s"
log_info "Target RPS: ${TARGET_RPS}"
log_info "Connections: ${CONNECTIONS}"
log_info "============================================"

# Parse state file
PROXY_IP=$(jq -r '.proxy.ip' "$STATE_FILE")
GENERATOR_IPS=($(jq -r '.generators[].ip' "$STATE_FILE"))
TEST_ID=$(jq -r '.tag' "$STATE_FILE")

log_info "Proxy IP: ${PROXY_IP}"
log_info "Generators: ${#GENERATOR_IPS[@]}"

# Results directory
RESULTS_DIR="${PROJECT_ROOT}/results/${TEST_ID}"
mkdir -p "${RESULTS_DIR}/metrics"

# Calculate RPS per generator
RPS_PER_GEN=$((TARGET_RPS / ${#GENERATOR_IPS[@]}))
CONNECTIONS_PER_GEN=$((CONNECTIONS / ${#GENERATOR_IPS[@]}))

log_info "RPS per generator: ${RPS_PER_GEN}"
log_info "Connections per generator: ${CONNECTIONS_PER_GEN}"

# Pre-test health check
health_check() {
    log_info "Running pre-test health check..."

    local health_url="http://${PROXY_IP}:8080/health"

    if curl -sf -m 5 "$health_url" > /dev/null 2>&1; then
        log_success "Gateway is healthy"
        return 0
    else
        log_warn "Health check failed, trying root endpoint..."
        if curl -sf -m 5 "http://${PROXY_IP}:8080/" > /dev/null 2>&1; then
            log_success "Gateway is responding"
            return 0
        else
            log_error "Gateway is not responding"
            return 1
        fi
    fi
}

# Run load test on a single generator
run_generator() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.txt"

    log_info "Starting generator ${gen_num} (${gen_ip})..."

    # Use wrk2 for precise RPS control
    ssh_exec "$gen_ip" "wrk2 -t 32 -c ${CONNECTIONS_PER_GEN} -d ${DURATION}s -R ${RPS_PER_GEN} --latency http://${PROXY_IP}:8080/ > /tmp/wrk2-results.txt 2>&1" "root" &

    local pid=$!
    echo "$pid" > "${RESULTS_DIR}/metrics/generator-${gen_num}.pid"

    log_success "Generator ${gen_num} started (PID: $pid)"
}

# Collect results from a generator
collect_results() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.txt"

    log_info "Collecting results from generator ${gen_num}..."

    # Fetch results
    ssh_exec "$gen_ip" "cat /tmp/wrk2-results.txt" "root" > "$output_file"

    log_success "Results saved: ${output_file}"
}

# Collect Prometheus metrics from gateway
collect_gateway_metrics() {
    log_info "Collecting gateway metrics..."

    local metrics_url="http://${PROXY_IP}:9090/metrics"
    local metrics_file="${RESULTS_DIR}/metrics/gateway-metrics.txt"

    if curl -sf -m 10 "$metrics_url" > "$metrics_file" 2>&1; then
        log_success "Gateway metrics saved: ${metrics_file}"
    else
        log_warn "Failed to collect gateway metrics"
    fi
}

# Parse and aggregate results
aggregate_results() {
    log_info "Aggregating results..."

    local total_requests=0
    local total_errors=0
    local avg_latency=0
    local p50_latency=0
    local p99_latency=0
    local p999_latency=0

    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_num=$((i + 1))
        local results_file="${RESULTS_DIR}/metrics/generator-${gen_num}.txt"

        if [ ! -f "$results_file" ]; then
            log_warn "Results file not found: ${results_file}"
            continue
        fi

        # Parse wrk2 output
        local requests=$(grep "Requests/sec" "$results_file" | awk '{print $2}' || echo "0")
        local errors=$(grep "Non-2xx" "$results_file" | awk '{print $4}' || echo "0")

        total_requests=$(echo "$total_requests + $requests" | bc)
        total_errors=$(echo "$total_errors + $errors" | bc)
    done

    # Create summary report
    cat > "${RESULTS_DIR}/summary.txt" <<EOF
============================================
Load Test Summary
============================================
Test ID: ${TEST_ID}
Scenario: ${SCENARIO}
Duration: ${DURATION}s
Target RPS: ${TARGET_RPS}
Target Connections: ${CONNECTIONS}

Results:
--------
Total Requests/sec: ${total_requests}
Total Errors: ${total_errors}
Success Rate: $(echo "scale=2; (1 - $total_errors / ($total_requests * $DURATION)) * 100" | bc)%

Generators: ${#GENERATOR_IPS[@]}
RPS per Generator: ${RPS_PER_GEN}

Detailed results in: ${RESULTS_DIR}/metrics/

============================================
EOF

    cat "${RESULTS_DIR}/summary.txt"
}

# Monitor test progress
monitor_test() {
    local duration=$1

    log_info "Test running for ${duration} seconds..."

    for ((i=1; i<=duration; i++)); do
        if [ $((i % 30)) -eq 0 ]; then
            echo -ne "${CYAN}[⏳]${NC} Test progress: ${i}/${duration}s ($(echo "scale=1; $i * 100 / $duration" | bc)%)\r"
        fi
        sleep 1
    done

    echo -ne "\033[2K\r"  # Clear line
    log_success "Test duration complete!"
}

# Main execution
main() {
    log_info "Starting load test execution..."

    # Health check
    if ! health_check; then
        log_error "Pre-test health check failed"
        exit 1
    fi

    # Start all generators
    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_ip="${GENERATOR_IPS[$i]}"
        local gen_num=$((i + 1))
        run_generator "$gen_ip" "$gen_num"
    done

    # Wait for all generators to start
    sleep 5
    log_success "All generators started!"

    # Monitor test progress
    monitor_test "$DURATION"

    # Wait for all generators to finish
    log_info "Waiting for generators to complete..."
    wait

    # Collect results
    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_ip="${GENERATOR_IPS[$i]}"
        local gen_num=$((i + 1))
        collect_results "$gen_ip" "$gen_num"
    done

    # Collect gateway metrics
    collect_gateway_metrics

    # Aggregate and display results
    aggregate_results

    log_success "============================================"
    log_success "Load Test Complete!"
    log_success "============================================"
    log_info "Results directory: ${RESULTS_DIR}"
    log_info "Summary: ${RESULTS_DIR}/summary.txt"
    log_info "Metrics: ${RESULTS_DIR}/metrics/"
    log_success "============================================"

    exit 0
}

# Run main function
main "$@"
