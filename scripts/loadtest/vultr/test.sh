#!/bin/bash
# Load test execution script with support for multiple tools
# Primary: Vegeta | Fallback: wrk2

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
LOAD_TEST_TOOL="${LOAD_TEST_TOOL:-vegeta}"  # vegeta (default) or wrk2
DURATION=${LOAD_TEST_DURATION:-180}
TARGET_RPS=${LOAD_TEST_TARGET_RPS:-600000}
CONNECTIONS=${LOAD_TEST_TARGET_CONCURRENCY:-3000000}

log_info "============================================"
log_info "Load Test Execution"
log_info "============================================"
log_info "Tool: ${LOAD_TEST_TOOL} (vegeta=primary, wrk2=fallback)"
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

# Run load test with vegeta
run_generator_vegeta() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.json"

    log_info "Starting vegeta on generator ${gen_num} (${gen_ip})..."

    # Vegeta attack command
    # -rate: requests per second
    # -duration: test duration
    # -connections: max connections (vegeta uses workers internally)
    ssh_exec "$gen_ip" "echo 'GET http://${PROXY_IP}:8080/' | vegeta attack -rate ${RPS_PER_GEN} -duration ${DURATION}s -workers 32 -max-connections ${CONNECTIONS_PER_GEN} > /tmp/vegeta-results.bin 2>&1" "root" &

    local pid=$!
    echo "$pid" > "${RESULTS_DIR}/metrics/generator-${gen_num}.pid"

    log_success "Vegeta generator ${gen_num} started (PID: $pid)"
}

# Run load test with wrk2
run_generator_wrk2() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.txt"

    log_info "Starting wrk2 on generator ${gen_num} (${gen_ip})..."

    # wrk2 command
    ssh_exec "$gen_ip" "wrk2 -t 32 -c ${CONNECTIONS_PER_GEN} -d ${DURATION}s -R ${RPS_PER_GEN} --latency http://${PROXY_IP}:8080/ > /tmp/wrk2-results.txt 2>&1" "root" &

    local pid=$!
    echo "$pid" > "${RESULTS_DIR}/metrics/generator-${gen_num}.pid"

    log_success "wrk2 generator ${gen_num} started (PID: $pid)"
}

# Collect results from vegeta
collect_results_vegeta() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.json"
    local report_file="${RESULTS_DIR}/metrics/generator-${gen_num}-report.txt"

    log_info "Collecting vegeta results from generator ${gen_num}..."

    # Generate JSON report
    ssh_exec "$gen_ip" "cat /tmp/vegeta-results.bin | vegeta report -type=json" "root" > "$output_file"

    # Generate text report for human readability
    ssh_exec "$gen_ip" "cat /tmp/vegeta-results.bin | vegeta report -type=text" "root" > "$report_file"

    log_success "Results saved: ${output_file}"
}

# Collect results from wrk2
collect_results_wrk2() {
    local gen_ip=$1
    local gen_num=$2
    local output_file="${RESULTS_DIR}/metrics/generator-${gen_num}.txt"

    log_info "Collecting wrk2 results from generator ${gen_num}..."

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

# Aggregate results from vegeta
aggregate_results_vegeta() {
    log_info "Aggregating vegeta results..."

    local total_requests=0
    local total_success=0
    local total_errors=0
    local min_latency=9999999
    local max_latency=0
    local avg_latency=0
    local p50_latency=0
    local p95_latency=0
    local p99_latency=0
    local p999_latency=0
    local throughput=0

    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_num=$((i + 1))
        local results_file="${RESULTS_DIR}/metrics/generator-${gen_num}.json"

        if [ ! -f "$results_file" ]; then
            log_warn "Results file not found: ${results_file}"
            continue
        fi

        # Parse vegeta JSON output
        local gen_requests=$(jq -r '.requests // 0' "$results_file")
        local gen_success=$(jq -r '.success // 0' "$results_file")
        local gen_rate=$(jq -r '.rate // 0' "$results_file")
        local gen_throughput=$(jq -r '.throughput // 0' "$results_file")
        local gen_min=$(jq -r '.latencies.min // 0' "$results_file")
        local gen_max=$(jq -r '.latencies.max // 0' "$results_file")
        local gen_mean=$(jq -r '.latencies.mean // 0' "$results_file")
        local gen_p50=$(jq -r '.latencies."50th" // 0' "$results_file")
        local gen_p95=$(jq -r '.latencies."95th" // 0' "$results_file")
        local gen_p99=$(jq -r '.latencies."99th" // 0' "$results_file")
        local gen_p999=$(jq -r '.latencies."99.9th" // 0' "$results_file")

        total_requests=$((total_requests + gen_requests))
        total_success=$(echo "$total_success + $gen_success" | bc)
        total_errors=$((gen_requests - gen_success))
        throughput=$(echo "$throughput + $gen_throughput" | bc)

        # Track min/max latencies
        if [ "$gen_min" != "0" ] && [ $(echo "$gen_min < $min_latency" | bc) -eq 1 ]; then
            min_latency=$gen_min
        fi
        if [ $(echo "$gen_max > $max_latency" | bc) -eq 1 ]; then
            max_latency=$gen_max
        fi

        # Average the percentiles (simple average, not weighted)
        avg_latency=$(echo "$avg_latency + $gen_mean" | bc)
        p50_latency=$(echo "$p50_latency + $gen_p50" | bc)
        p95_latency=$(echo "$p95_latency + $gen_p95" | bc)
        p99_latency=$(echo "$p99_latency + $gen_p99" | bc)
        p999_latency=$(echo "$p999_latency + $gen_p999" | bc)
    done

    # Average latencies across generators
    local num_gens=${#GENERATOR_IPS[@]}
    avg_latency=$(echo "scale=2; $avg_latency / $num_gens / 1000000" | bc)  # Convert to ms
    p50_latency=$(echo "scale=2; $p50_latency / $num_gens / 1000000" | bc)
    p95_latency=$(echo "scale=2; $p95_latency / $num_gens / 1000000" | bc)
    p99_latency=$(echo "scale=2; $p99_latency / $num_gens / 1000000" | bc)
    p999_latency=$(echo "scale=2; $p999_latency / $num_gens / 1000000" | bc)
    min_latency=$(echo "scale=2; $min_latency / 1000000" | bc)
    max_latency=$(echo "scale=2; $max_latency / 1000000" | bc)

    local success_rate=$(echo "scale=2; ($total_success / $total_requests) * 100" | bc)
    local actual_rps=$(echo "scale=2; $total_requests / $DURATION" | bc)

    # Create summary report
    cat > "${RESULTS_DIR}/summary.txt" <<EOF
============================================
Load Test Summary (Vegeta)
============================================
Test ID: ${TEST_ID}
Scenario: ${SCENARIO}
Duration: ${DURATION}s
Target RPS: ${TARGET_RPS}
Actual RPS: ${actual_rps}
Target Connections: ${CONNECTIONS}

Results:
--------
Total Requests: ${total_requests}
Successful: ${total_success}
Errors: ${total_errors}
Success Rate: ${success_rate}%
Throughput: $(echo "scale=2; $throughput / 1000000" | bc) MB/s

Latency (ms):
-------------
Min: ${min_latency}
Max: ${max_latency}
Mean: ${avg_latency}
P50: ${p50_latency}
P95: ${p95_latency}
P99: ${p99_latency}
P99.9: ${p999_latency}

Generators: ${num_gens}
RPS per Generator: ${RPS_PER_GEN}

Detailed results in: ${RESULTS_DIR}/metrics/

============================================
EOF

    cat "${RESULTS_DIR}/summary.txt"
}

# Aggregate results from wrk2
aggregate_results_wrk2() {
    log_info "Aggregating wrk2 results..."

    local total_requests=0
    local total_errors=0
    local actual_rps=0

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

        total_requests=$(echo "$total_requests + $requests * $DURATION" | bc)
        total_errors=$(echo "$total_errors + $errors" | bc)
        actual_rps=$(echo "$actual_rps + $requests" | bc)
    done

    local success_rate=$(echo "scale=2; (1 - $total_errors / $total_requests) * 100" | bc)

    # Create summary report
    cat > "${RESULTS_DIR}/summary.txt" <<EOF
============================================
Load Test Summary (wrk2)
============================================
Test ID: ${TEST_ID}
Scenario: ${SCENARIO}
Duration: ${DURATION}s
Target RPS: ${TARGET_RPS}
Actual RPS: ${actual_rps}
Target Connections: ${CONNECTIONS}

Results:
--------
Total Requests: ${total_requests}
Total Errors: ${total_errors}
Success Rate: ${success_rate}%

Generators: ${#GENERATOR_IPS[@]}
RPS per Generator: ${RPS_PER_GEN}

Detailed results in: ${RESULTS_DIR}/metrics/

Note: For detailed latency percentiles, see individual generator reports.

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
            echo -ne "${CYAN}[⏳]${NC} Test progress: ${i}/${duration}s ($(echo "scale=1; $i * 100 / $duration" | bc)%)\\r"
        fi
        sleep 1
    done

    echo -ne "\\033[2K\\r"  # Clear line
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

    # Start all generators based on selected tool
    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_ip="${GENERATOR_IPS[$i]}"
        local gen_num=$((i + 1))

        if [ "$LOAD_TEST_TOOL" = "vegeta" ]; then
            run_generator_vegeta "$gen_ip" "$gen_num"
        elif [ "$LOAD_TEST_TOOL" = "wrk2" ]; then
            run_generator_wrk2 "$gen_ip" "$gen_num"
        else
            log_error "Unknown load test tool: ${LOAD_TEST_TOOL}"
            log_info "Supported tools: vegeta, wrk2"
            exit 1
        fi
    done

    # Wait for all generators to start
    sleep 5
    log_success "All generators started!"

    # Monitor test progress
    monitor_test "$DURATION"

    # Wait for all generators to finish
    log_info "Waiting for generators to complete..."
    wait

    # Collect results based on tool
    for i in "${!GENERATOR_IPS[@]}"; do
        local gen_ip="${GENERATOR_IPS[$i]}"
        local gen_num=$((i + 1))

        if [ "$LOAD_TEST_TOOL" = "vegeta" ]; then
            collect_results_vegeta "$gen_ip" "$gen_num"
        elif [ "$LOAD_TEST_TOOL" = "wrk2" ]; then
            collect_results_wrk2 "$gen_ip" "$gen_num"
        fi
    done

    # Collect gateway metrics
    collect_gateway_metrics

    # Aggregate and display results
    if [ "$LOAD_TEST_TOOL" = "vegeta" ]; then
        aggregate_results_vegeta
    elif [ "$LOAD_TEST_TOOL" = "wrk2" ]; then
        aggregate_results_wrk2
    fi

    log_success "============================================"
    log_success "Load Test Complete!"
    log_success "============================================"
    log_info "Tool used: ${LOAD_TEST_TOOL}"
    log_info "Results directory: ${RESULTS_DIR}"
    log_info "Summary: ${RESULTS_DIR}/summary.txt"
    log_info "Metrics: ${RESULTS_DIR}/metrics/"
    log_success "============================================"

    exit 0
}

# Run main function
main "$@"
