#!/bin/bash
# Scenario 02: Layer 7 HTTP - HTTP/1.1 Load Balancing
# Protocol: HTTP/1.1
# Target: 200K+ req/s (based on DigitalOcean baseline: 207K req/s achieved)
# Focus: Round-robin, least connections, health checks

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

SCENARIO_NAME="02-http-loadbalancer"
SCENARIO_DESCRIPTION="Layer 7 HTTP - HTTP/1.1 Load Balancing"

# Performance targets (from VALIDATION_REPORT.md and historical data)
TARGET_RPS=200000  # 200K req/s based on DigitalOcean achievement
TARGET_P99_LATENCY_MS=5
TARGET_SUCCESS_RATE=99.9

# Test parameters
HTTP_RATE_START=${LOAD_TEST_RATE_START:-10000}
HTTP_RATE_STEP=${LOAD_TEST_RATE_STEP:-20000}
HTTP_RATE_MAX=${LOAD_TEST_RATE_MAX:-${TARGET_RPS}}
HTTP_DURATION=${LOAD_TEST_DURATION:-60}
HTTP_WARMUP=${LOAD_TEST_WARMUP:-10}

# Infrastructure
MODE="${LOAD_TEST_MODE:-local}"  # local, vultr, phoenixnap, hetzner
GATEWAY_HOST="${LOAD_TEST_GATEWAY_HOST:-localhost}"
GATEWAY_PORT="${LOAD_TEST_GATEWAY_PORT:-8080}"

# Test endpoints
TEST_ENDPOINT="${LOAD_TEST_ENDPOINT:-/api/ping}"

# ========================================
# Cleanup Handler
# ========================================

cleanup() {
    log_info "Cleaning up scenario 02..."

    if [ "${MODE}" = "local" ]; then
        # Stop docker-compose
        docker-compose \
            -f "${LOAD_TEST_DIR}/docker/docker-compose.base.yml" \
            -f "${LOAD_TEST_DIR}/docker/docker-compose.scenario-02-http.yml" \
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
        -f "${LOAD_TEST_DIR}/docker/docker-compose.scenario-02-http.yml" \
        up -d --build

    # Wait for services to be ready
    log_info "Waiting for services to be ready..."

    # Wait for gateway HTTP port
    wait_for_ready "http://${GATEWAY_HOST}:${GATEWAY_PORT}/health" 60

    # Verify backend health through gateway
    log_info "Verifying backend connectivity..."
    local response=$(curl -s "http://${GATEWAY_HOST}:${GATEWAY_PORT}${TEST_ENDPOINT}" || echo "")

    if [ -z "$response" ]; then
        log_error "Failed to reach backends through gateway"
        return 1
    fi

    log_success "Local infrastructure ready"
    log_debug "Test response: $response"
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

    # Extract gateway IP
    export GATEWAY_HOST=$(echo "$infrastructure" | jq -r '.gateway.ip')

    log_info "Gateway IP: ${GATEWAY_HOST}"

    # Deploy to cloud servers
    deploy_to_cloud_servers "$infrastructure"

    # Wait for gateway to be ready
    wait_for_ready "http://${GATEWAY_HOST}:${GATEWAY_PORT}/health" 120

    log_success "Cloud infrastructure ready"
}

deploy_to_cloud_servers() {
    local infrastructure=$1

    local gateway_ip=$(echo "$infrastructure" | jq -r '.gateway.ip')
    local backend_ips=$(echo "$infrastructure" | jq -r '.backends[].ip')

    # Deploy backends first
    for backend_ip in $backend_ips; do
        log_info "Deploying backend to ${backend_ip}..."

        # Upload and start Rust backend
        scp -o StrictHostKeyChecking=no \
            -r "${LOAD_TEST_DIR}/docker/backends/rust-http" \
            "root@${backend_ip}:/root/backend"

        ssh -o StrictHostKeyChecking=no "root@${backend_ip}" \
            "cd /root/backend && cargo build --release && nohup ./target/release/load-test-backend > /var/log/backend.log 2>&1 &"

        wait_for_ready "http://${backend_ip}:8000/health" 60
    done

    # Deploy gateway
    log_info "Deploying Highper Gateway to ${gateway_ip}..."

    scp -o StrictHostKeyChecking=no \
        "${GATEWAY_BINARY}" \
        "root@${gateway_ip}:/usr/local/bin/highper-gateway"

    # Generate config with backend IPs
    local config_file="${RESULT_DIR}/gateway-cloud.toml"
    generate_cloud_config "$infrastructure" > "$config_file"

    scp -o StrictHostKeyChecking=no \
        "$config_file" \
        "root@${gateway_ip}:/etc/highper-gateway/config.toml"

    # Start gateway
    ssh -o StrictHostKeyChecking=no "root@${gateway_ip}" \
        "nohup /usr/local/bin/highper-gateway --config /etc/highper-gateway/config.toml > /var/log/highper-gateway.log 2>&1 &"

    log_success "Deployment complete"
}

generate_cloud_config() {
    local infrastructure=$1

    # Create dynamic config based on infrastructure
    # This would generate a TOML config with actual backend IPs
    # For now, output a placeholder
    cat "${LOAD_TEST_DIR}/docker/configs/gateway.toml"
}

# ========================================
# Load Test Execution
# ========================================

run_load_test() {
    log_info "Running HTTP load test with vegeta..."

    # Ensure vegeta is installed
    ensure_tool vegeta

    log_info "Test configuration:"
    log_info "  Target: http://${GATEWAY_HOST}:${GATEWAY_PORT}${TEST_ENDPOINT}"
    log_info "  Rate range: ${HTTP_RATE_START} - ${HTTP_RATE_MAX} req/s"
    log_info "  Step: ${HTTP_RATE_STEP} req/s"
    log_info "  Duration: ${HTTP_DURATION}s per rate"
    log_info "  Warmup: ${HTTP_WARMUP}s"

    # Warmup phase
    if [ ${HTTP_WARMUP} -gt 0 ]; then
        log_info "Warmup phase: ${HTTP_WARMUP}s at ${HTTP_RATE_START} req/s..."
        run_vegeta_test ${HTTP_RATE_START} ${HTTP_WARMUP} "${RESULT_DIR}/warmup"
        sleep 2
    fi

    # Progressive load test
    local current_rate=${HTTP_RATE_START}
    while [ $current_rate -le ${HTTP_RATE_MAX} ]; do
        log_info "Testing at ${current_rate} req/s..."

        run_vegeta_test \
            $current_rate \
            ${HTTP_DURATION} \
            "${RESULT_DIR}/vegeta-${current_rate}rps"

        # Check if we should continue (stop if errors)
        local success_rate=$(jq -r '.success * 100' "${RESULT_DIR}/vegeta-${current_rate}rps.json")
        if (( $(echo "$success_rate < 95.0" | bc -l) )); then
            log_warn "Success rate dropped to ${success_rate}%, stopping load test"
            break
        fi

        current_rate=$((current_rate + HTTP_RATE_STEP))
    done

    log_success "Load test complete"
}

run_vegeta_test() {
    local rate=$1
    local duration=$2
    local output_prefix=$3

    log_info "Running vegeta at ${rate} req/s for ${duration}s..."

    # Generate vegeta target
    local target_url="http://${GATEWAY_HOST}:${GATEWAY_PORT}${TEST_ENDPOINT}"

    # Run vegeta attack
    echo "GET ${target_url}" | vegeta attack \
        -rate=${rate} \
        -duration=${duration}s \
        -timeout=30s \
        -keepalive=true \
        -max-workers=1000 \
        -header="User-Agent: vegeta-loadtest" \
        -header="X-Load-Test: scenario-02" \
        > "${output_prefix}.bin"

    # Generate reports
    cat "${output_prefix}.bin" | vegeta report -type=json > "${output_prefix}.json"
    cat "${output_prefix}.bin" | vegeta report -type=text > "${output_prefix}.txt"

    # Log results
    local actual_rate=$(jq -r '.rate' "${output_prefix}.json")
    local p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "${output_prefix}.json")
    local success=$(jq -r '.success * 100' "${output_prefix}.json")

    log_info "Results: ${actual_rate} req/s, P99: ${p99}ms, Success: ${success}%"

    return 0
}

# ========================================
# Results Analysis
# ========================================

analyze_results() {
    log_info "Analyzing results..."

    local best_rps=0
    local best_file=""

    # Find best successful run
    for result_file in "${RESULT_DIR}"/vegeta-*rps.json; do
        if [ ! -f "$result_file" ]; then
            continue
        fi

        local rate=$(jq -r '.rate' "$result_file")
        local success=$(jq -r '.success * 100' "$result_file")
        local p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "$result_file")

        # Consider it successful if success rate > 99% and P99 < 10ms
        if (( $(echo "$success > 99.0" | bc -l) )) && (( $(echo "$p99 < 10.0" | bc -l) )); then
            if (( $(echo "$rate > $best_rps" | bc -l) )); then
                best_rps=$rate
                best_file=$result_file
            fi
        fi
    done

    if [ -z "$best_file" ]; then
        log_error "No successful test runs found"
        return 1
    fi

    log_info "Best performance: ${best_rps} req/s"

    # Extract detailed metrics from best run
    local p50=$(jq -r '.latencies."50th" | tonumber / 1000000' "$best_file")
    local p95=$(jq -r '.latencies."95th" | tonumber / 1000000' "$best_file")
    local p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "$best_file")
    local success=$(jq -r '.success * 100' "$best_file")
    local throughput=$(jq -r '.throughput' "$best_file")

    # Validate against targets
    local pass=true

    # Check RPS (allow 90% of target)
    local min_rps=$(echo "$TARGET_RPS * 0.9" | bc)
    if (( $(echo "$best_rps < $min_rps" | bc -l) )); then
        log_warn "RPS below target: ${best_rps} < ${TARGET_RPS} (allowing 90%: ${min_rps})"
        # Only fail if less than 50% of target
        if (( $(echo "$best_rps < $TARGET_RPS * 0.5" | bc -l) )); then
            pass=false
        fi
    else
        log_success "RPS: ${best_rps} (target: ${TARGET_RPS}) ✓"
    fi

    # Check P99 latency
    if (( $(echo "$p99 > $TARGET_P99_LATENCY_MS" | bc -l) )); then
        log_warn "P99 latency: ${p99}ms > ${TARGET_P99_LATENCY_MS}ms"
    else
        log_success "P99 latency: ${p99}ms (target: <${TARGET_P99_LATENCY_MS}ms) ✓"
    fi

    # Check success rate
    if (( $(echo "$success < $TARGET_SUCCESS_RATE" | bc -l) )); then
        log_error "Success rate: ${success}% < ${TARGET_SUCCESS_RATE}%"
        pass=false
    else
        log_success "Success rate: ${success}% (target: >${TARGET_SUCCESS_RATE}%) ✓"
    fi

    # Generate summary
    cat > "${RESULT_DIR}/summary.txt" <<EOF
========================================
Scenario 02: HTTP Load Balancer - Results
========================================

Date: $(date)
Mode: ${MODE}
Duration: ${HTTP_DURATION}s per rate

Infrastructure:
  Gateway: ${GATEWAY_HOST}:${GATEWAY_PORT}
  Endpoint: ${TEST_ENDPOINT}

Performance (Best Run):
  Requests/sec: ${best_rps}
  P50 Latency: ${p50}ms
  P95 Latency: ${p95}ms
  P99 Latency: ${p99}ms
  Success Rate: ${success}%
  Throughput: ${throughput} MB/s

Targets:
  RPS: ${TARGET_RPS}
  P99: <${TARGET_P99_LATENCY_MS}ms
  Success: >${TARGET_SUCCESS_RATE}%

Historical Baseline:
  DigitalOcean: 207K req/s achieved

Status: $([ "$pass" = "true" ] && echo "PASS ✓" || echo "PARTIAL ⚠")

========================================
EOF

    cat "${RESULT_DIR}/summary.txt"

    # Generate detailed report
    generate_summary_report "${RESULT_DIR}" "${SCENARIO_NAME}"

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
    log_info "Scenario 02: HTTP Load Balancer Test"
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
        log_success "Scenario 02 completed successfully!"
        log_info "Beat the DigitalOcean baseline? Check the results above!"
    else
        log_warn "Scenario 02 completed with warnings"
    fi

    exit $exit_code
}

main "$@"
