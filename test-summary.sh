#!/bin/bash
# Quick test summary for all 15 scenarios
cd "$(dirname "$0")"

echo "========================================="
echo "Highper Gateway - Quick Validation"
echo "Testing all 15 Scenarios"
echo "========================================="

# Cleanup function
cleanup() {
    pkill -f highper-gateway 2>/dev/null || true
    docker stop $(docker ps -aq --filter "name=backend") 2>/dev/null || true
    docker stop $(docker ps -aq --filter "name=grpc-server") 2>/dev/null || true
    docker stop $(docker ps -aq --filter "name=graphql-server") 2>/dev/null || true
    for port in 8080 8443 9000 9090; do lsof -ti:$port | xargs kill -9 2>/dev/null || true; done
    sleep 2
}

run_quick_test() {
    local num="$1"
    local name="$2"
    local script="$3"

    echo ""
    printf "Scenario %02d: %s ... " "$num" "$name"
    
    cleanup >/dev/null 2>&1
    
    if timeout 60 bash "$script" >/tmp/quick-test-$num.log 2>&1; then
        echo "✅ PASS"
        return 0
    else
        echo "✗ FAIL"
        echo "  Error: $(tail -5 /tmp/quick-test-$num.log | grep -E 'ERROR|Error|FAIL' | head -1)"
        return 1
    fi
}

PASSED=0
FAILED=0

# Run tests
run_quick_test 1 "Layer 4 TCP" "test-scenario-01-tcp-native.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 2 "Layer 7 HTTP" "test-scenario-02-native.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 3 "HTTPS/TLS" "test-scenario-03-tls.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 4 "Rate Limiting" "test-scenario-04-rate-limit.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 5 "HTTP/3 QUIC" "test-scenario-05-http3.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 6 "WebSocket" "test-scenario-06-websocket.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 7 "gRPC Gateway" "test-scenario-07-grpc.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 8 "Database LB" "test-scenario-08-database.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 9 "WAF + mTLS" "test-scenario-09-waf.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 10 "Multi-Protocol" "test-scenario-10-multi.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 11 "CDN Caching" "test-scenario-11-cache.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 12 "Service Discovery" "test-scenario-12-discovery.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 13 "GraphQL Gateway" "test-scenario-13-graphql.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 14 "PHP-FPM" "test-scenario-14-php.sh" && ((PASSED++)) || ((FAILED++))
run_quick_test 15 "Geographic LB" "test-scenario-15-geo.sh" && ((PASSED++)) || ((FAILED++))

cleanup >/dev/null 2>&1

echo ""
echo "========================================="
echo "Results: $PASSED/15 passed, $FAILED/15 failed"
if [ $FAILED -eq 0 ]; then
    echo "🎉 ALL TESTS PASSED!"
else
    echo "⚠️  Some tests failed. Check logs in /tmp/quick-test-*.log"
fi
echo "========================================="
