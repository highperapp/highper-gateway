#!/bin/bash
# Scenario 01: Layer 4 TCP - Pure TCP Proxying
# Protocol: TCP
# Target: 500K+ connections/sec
# Focus: Zero-copy forwarding, connection pooling, high throughput

set -euo pipefail

# ========================================
# Load Dependencies
# ========================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOAD_TEST_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

source "${LOAD_TEST_DIR}/helpers/common.sh"

# ========================================
# Scenario Configuration
# ========================================

SCENARIO_NAME="01-tcp-proxy"
SCENARIO_DESCRIPTION="Layer 4 TCP - Pure TCP Proxying"

# Performance targets (from VALIDATION_REPORT.md)
TARGET_CONNECTIONS_PER_SEC=500000
TARGET_P99_LATENCY_MS=1
TARGET_SUCCESS_RATE=99.9

# Test parameters
TCP_CONNECTIONS=${LOAD_TEST_TCP_CONNECTIONS:-10000}
TCP_DURATION=${LOAD_TEST_DURATION:-60}
TCP_WARMUP=${LOAD_TEST_WARMUP:-10}

# Infrastructure
MODE="${LOAD_TEST_MODE:-local}"  # local, vultr, phoenixnap, hetzner
GATEWAY_HOST="${LOAD_TEST_GATEWAY_HOST:-localhost}"
GATEWAY_PORT="${LOAD_TEST_GATEWAY_PORT:-9000}"
BACKEND_HOST="${LOAD_TEST_BACKEND_HOST:-localhost}"
BACKEND_PORT="${LOAD_TEST_BACKEND_PORT:-5432}"

# ========================================
# Cleanup Handler
# ========================================

cleanup() {
    log_info "Cleaning up scenario 01..."

    if [ "${MODE}" = "local" ]; then
        # Stop docker-compose
        docker-compose \
            -f "${LOAD_TEST_DIR}/docker/docker-compose.base.yml" \
            -f "${LOAD_TEST_DIR}/docker/docker-compose.scenario-01-tcp.yml" \
            down
    elif [ "${LOAD_TEST_KEEP_INSTANCES}" != "true" ]; then
        # Destroy cloud infrastructure
        if [ -f "${RESULT_DIR}/infrastructure.json" ]; then
            source "${LOAD_TEST_DIR}/providers/provider-interface.sh"
            destroy_load_test_infrastructure "${MODE}" "$(cat ${RESULT_DIR}/infrastructure.json)"
        fi
    fi

    log_success "Cleanup complete"
}

trap cleanup EXIT INT TERM

# ========================================
# Infrastructure Setup
# ========================================

setup_infrastructure() {
    log_info "Setting up infrastructure for ${SCENARIO_NAME}"

    if [ "${MODE}" = "local" ]; then
        setup_local_infrastructure
    else
        setup_cloud_infrastructure "${MODE}"
    fi
}

setup_local_infrastructure() {
    log_info "Starting local Docker infrastructure..."

    # Build and start containers
    docker-compose \
        -f "${LOAD_TEST_DIR}/docker/docker-compose.base.yml" \
        -f "${LOAD_TEST_DIR}/docker/docker-compose.scenario-01-tcp.yml" \
        up -d --build

    # Wait for services to be ready
    log_info "Waiting for services to be ready..."

    # Wait for PostgreSQL backend
    wait_for_port "${BACKEND_HOST}" "${BACKEND_PORT}" 60

    # Wait for gateway TCP port
    wait_for_port "${GATEWAY_HOST}" "${GATEWAY_PORT}" 60

    log_success "Local infrastructure ready"
}

setup_cloud_infrastructure() {
    local provider=$1

    log_info "Provisioning cloud infrastructure on ${provider}..."

    source "${LOAD_TEST_DIR}/providers/provider-interface.sh"

    local infrastructure=$(provision_load_test_infrastructure \
        "${provider}" \
        "${SCENARIO_NAME}" \
        "auto" \
        "auto")

    echo "$infrastructure" > "${RESULT_DIR}/infrastructure.json"

    # Extract IPs
    export GATEWAY_HOST=$(echo "$infrastructure" | jq -r '.gateway.ip')
    export BACKEND_HOST=$(echo "$infrastructure" | jq -r '.backends[0].ip')

    log_info "Gateway IP: ${GATEWAY_HOST}"
    log_info "Backend IP: ${BACKEND_HOST}"

    # Deploy Highper Gateway and backends via SSH
    deploy_to_cloud_servers "$infrastructure"

    log_success "Cloud infrastructure ready"
}

deploy_to_cloud_servers() {
    local infrastructure=$1

    local gateway_ip=$(echo "$infrastructure" | jq -r '.gateway.ip')

    log_info "Deploying Highper Gateway to ${gateway_ip}..."

    # Copy gateway binary and config
    scp -o StrictHostKeyChecking=no \
        "${GATEWAY_BINARY}" \
        "root@${gateway_ip}:/usr/local/bin/highper-gateway"

    scp -o StrictHostKeyChecking=no \
        "${LOAD_TEST_DIR}/docker/configs/gateway.toml" \
        "root@${gateway_ip}:/etc/highper-gateway/config.toml"

    # Start gateway
    ssh -o StrictHostKeyChecking=no "root@${gateway_ip}" \
        "nohup /usr/local/bin/highper-gateway --config /etc/highper-gateway/config.toml > /var/log/highper-gateway.log 2>&1 &"

    # Wait for gateway to start
    sleep 5
    wait_for_port "${gateway_ip}" "${GATEWAY_PORT}" 60

    log_success "Gateway deployed and running"
}

# ========================================
# Load Test Execution
# ========================================

run_load_test() {
    log_info "Running TCP load test..."

    log_info "Test configuration:"
    log_info "  Gateway: ${GATEWAY_HOST}:${GATEWAY_PORT}"
    log_info "  Backend: ${BACKEND_HOST}:${BACKEND_PORT}"
    log_info "  Connections: ${TCP_CONNECTIONS}"
    log_info "  Duration: ${TCP_DURATION}s"
    log_info "  Warmup: ${TCP_WARMUP}s"

    # Warmup phase
    if [ ${TCP_WARMUP} -gt 0 ]; then
        log_info "Warmup phase: ${TCP_WARMUP}s..."
        run_tcp_benchmark ${TCP_WARMUP} $((TCP_CONNECTIONS / 10)) "${RESULT_DIR}/warmup.log"
        sleep 2
    fi

    # Main test
    log_info "Main load test starting..."

    run_tcp_benchmark \
        ${TCP_DURATION} \
        ${TCP_CONNECTIONS} \
        "${RESULT_DIR}/tcp-results.json"

    log_success "Load test complete"
}

run_tcp_benchmark() {
    local duration=$1
    local connections=$2
    local output_file=$3

    # Use iperf3 for TCP throughput testing
    ensure_tool iperf3

    log_info "Running iperf3 TCP benchmark..."
    log_info "  Duration: ${duration}s"
    log_info "  Parallel connections: ${connections}"

    # Run iperf3 client
    timeout $((duration + 10)) iperf3 \
        -c "${GATEWAY_HOST}" \
        -p "${GATEWAY_PORT}" \
        -t ${duration} \
        -P ${connections} \
        -J \
        > "${output_file}" 2>&1

    local exit_code=$?

    if [ $exit_code -eq 0 ]; then
        log_success "iperf3 benchmark completed"

        # Extract metrics
        local throughput_gbps=$(jq -r '.end.sum_received.bits_per_second / 1000000000' "${output_file}")
        local retransmits=$(jq -r '.end.sum_sent.retransmits // 0' "${output_file}")

        log_info "Results:"
        log_info "  Throughput: ${throughput_gbps} Gbps"
        log_info "  Retransmits: ${retransmits}"

        return 0
    else
        log_error "iperf3 benchmark failed with exit code: $exit_code"
        return 1
    fi
}

# ========================================
# Results Analysis
# ========================================

analyze_results() {
    log_info "Analyzing results..."

    local results_file="${RESULT_DIR}/tcp-results.json"

    if [ ! -f "$results_file" ]; then
        log_error "Results file not found: $results_file"
        return 1
    fi

    # Extract metrics
    local throughput_gbps=$(jq -r '.end.sum_received.bits_per_second / 1000000000' "$results_file")
    local connections=$(jq -r '.start.test_start.num_streams' "$results_file")
    local duration=$(jq -r '.start.test_start.duration' "$results_file")
    local retransmits=$(jq -r '.end.sum_sent.retransmits // 0' "$results_file")

    # Calculate connections/sec (approximate)
    local conn_per_sec=$(echo "$connections / $duration" | bc -l)

    log_info "Performance Summary:"
    log_info "  Throughput: ${throughput_gbps} Gbps"
    log_info "  Connections: ${connections}"
    log_info "  Connections/sec: ${conn_per_sec}"
    log_info "  Retransmits: ${retransmits}"

    # Validate against targets
    local pass=true

    # For TCP, we focus on throughput and retransmits
    local min_throughput=1.0  # At least 1 Gbps
    if (( $(echo "$throughput_gbps < $min_throughput" | bc -l) )); then
        log_error "Throughput too low: ${throughput_gbps} Gbps < ${min_throughput} Gbps"
        pass=false
    else
        log_success "Throughput: ${throughput_gbps} Gbps (target: >${min_throughput} Gbps) ✓"
    fi

    # Check retransmits (should be minimal)
    local max_retransmits=1000
    if [ "$retransmits" -gt "$max_retransmits" ]; then
        log_warn "High retransmit count: ${retransmits} > ${max_retransmits}"
    else
        log_success "Retransmits: ${retransmits} (target: <${max_retransmits}) ✓"
    fi

    # Generate summary
    cat > "${RESULT_DIR}/summary.txt" <<EOF
========================================
Scenario 01: TCP Proxy - Load Test Results
========================================

Date: $(date)
Mode: ${MODE}
Duration: ${duration}s

Infrastructure:
  Gateway: ${GATEWAY_HOST}:${GATEWAY_PORT}
  Backend: ${BACKEND_HOST}:${BACKEND_PORT}

Performance:
  Throughput: ${throughput_gbps} Gbps
  Connections: ${connections}
  Connections/sec: ${conn_per_sec}
  Retransmits: ${retransmits}

Status: $([ "$pass" = "true" ] && echo "PASS ✓" || echo "FAIL ✗")

========================================
EOF

    cat "${RESULT_DIR}/summary.txt"

    if [ "$pass" = "true" ]; then
        return 0
    else
        return 1
    fi
}

# ========================================
# Main Execution
# ========================================

main() {
    log_info "========================================="
    log_info "Scenario 01: TCP Proxy Load Test"
    log_info "========================================="

    # Create result directory
    RESULT_DIR=$(create_result_dir "${SCENARIO_NAME}" "${MODE}")
    export RESULT_DIR

    # Save metadata
    save_test_metadata "${RESULT_DIR}" "${SCENARIO_NAME}" "${MODE}" "${MODE}"

    # Setup infrastructure
    setup_infrastructure

    # Run load test
    run_load_test

    # Analyze results
    analyze_results

    local exit_code=$?

    if [ $exit_code -eq 0 ]; then
        log_success "Scenario 01 completed successfully!"
    else
        log_error "Scenario 01 failed!"
    fi

    exit $exit_code
}

main "$@"
