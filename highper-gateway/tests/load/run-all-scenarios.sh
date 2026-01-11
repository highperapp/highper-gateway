#!/bin/bash
# Comprehensive test runner for all 15 scenarios

set -euo pipefail
cd "$(dirname "$0")"

echo "========================================="
echo "Highper Gateway - Full Validation Suite"
echo "Testing all 15 Production Scenarios"
echo "========================================="

# Track results
TOTAL=15
PASSED=0
FAILED=0
declare -a FAILED_SCENARIOS

# Test runner function
run_test() {
    local num="$1"
    local name="$2"
    local script="$3"
    local timeout="${4:-120}"

    echo ""
    echo "========================================="
    echo "Scenario $num: $name"
    echo "========================================="

    if [ ! -f "$script" ]; then
        echo "⚠ Test script not found: $script"
        FAILED=$((FAILED + 1))
        FAILED_SCENARIOS+=("$num: $name (script not found)")
        return 1
    fi

    # Run test with timeout
    local start_time=$(date +%s)
    if timeout "$timeout" bash "$script" > "/tmp/scenario-${num}.log" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo "✅ PASS (${duration}s)"
        PASSED=$((PASSED + 1))
        return 0
    else
        local exit_code=$?
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        if [ $exit_code -eq 124 ]; then
            echo "✗ TIMEOUT (>${timeout}s)"
        else
            echo "✗ FAIL (exit code: $exit_code, ${duration}s)"
        fi

        # Show last 15 lines of output
        echo "Last 15 lines of output:"
        tail -15 "/tmp/scenario-${num}.log" | sed 's/^/  /'

        FAILED=$((FAILED + 1))
        FAILED_SCENARIOS+=("$num: $name")
        return 1
    fi
}

# Run all 15 scenarios with increased timeouts
run_test 1 "Layer 4 TCP - Pure TCP Proxying" "test-scenario-01-tcp-native.sh" 200 || true
run_test 2 "Layer 7 HTTP - HTTP/1.1 Load Balancing" "test-scenario-02-native.sh" 150 || true
run_test 3 "HTTPS/TLS Termination" "test-scenario-03-tls.sh" 180 || true
run_test 4 "API Gateway with Rate Limiting" "test-scenario-04-rate-limit.sh" 150 || true
run_test 5 "HTTP/3 QUIC" "test-scenario-05-http3.sh" 180 || true
run_test 6 "WebSocket Load Balancer" "test-scenario-06-websocket.sh" 150 || true
run_test 7 "gRPC Gateway" "test-scenario-07-grpc.sh" 180 || true
run_test 8 "Database Load Balancer" "test-scenario-08-database.sh" 180 || true
run_test 9 "WAF + mTLS" "test-scenario-09-waf.sh" 180 || true
run_test 10 "Hybrid Multi-Protocol" "test-scenario-10-multi.sh" 180 || true
run_test 11 "CDN Edge Caching" "test-scenario-11-cache.sh" 150 || true
run_test 12 "Microservices Discovery" "test-scenario-12-discovery.sh" 180 || true
run_test 13 "GraphQL Gateway" "test-scenario-13-graphql.sh" 180 || true
run_test 14 "Static + PHP-FPM" "test-scenario-14-php.sh" 180 || true
run_test 15 "Geographic Load Balancing" "test-scenario-15-geo.sh" 150 || true

# Final summary
echo ""
echo "========================================="
echo "Final Results"
echo "========================================="
echo "Total Scenarios: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "🎉 ALL SCENARIOS PASSED! 100% Production Ready"
    exit 0
else
    echo "Failed Scenarios:"
    for scenario in "${FAILED_SCENARIOS[@]}"; do
        echo "  ✗ $scenario"
    done
    echo ""
    echo "Success Rate: $(echo "scale=1; $PASSED * 100 / $TOTAL" | bc)%"
    exit 1
fi
