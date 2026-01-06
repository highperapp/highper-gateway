#!/bin/bash
# Quick Sequential Test of Remaining Scenarios
set -u

cd "$(dirname "$0")"

RESULTS_DIR="test-results-$(date +%Y%m%d)"
mkdir -p "$RESULTS_DIR"

echo "========================================"
echo "Testing Remaining Scenarios (07-15)"
echo "Started: $(date)"
echo "========================================"

# Array of scenarios to test
scenarios=(
    "07:test-scenario-07-grpc.sh:gRPC"
    "08:test-scenario-08-database.sh:Database/Redis"
    "09:test-scenario-09-waf.sh:WAF+mTLS"
    "10:test-scenario-10-multi.sh:Multi-Protocol"
    "11:test-scenario-11-cache.sh:CDN Cache"
    "12:test-scenario-12-discovery.sh:Service Discovery"
    "14:test-scenario-14-php.sh:PHP-FPM"
    "15:test-scenario-15-geo.sh:Geographic LB"
)

for scenario_info in "${scenarios[@]}"; do
    IFS=: read -r num script name <<< "$scenario_info"

    echo ""
    echo "========================================"
    echo "Scenario $num: $name"
    echo "========================================"

    LOG_FILE="$RESULTS_DIR/scenario-$num-quick.log"

    # Clean up before each test
    docker-compose -f docker/docker-compose-prebuilt.yml down 2>/dev/null || true
    docker rm -f $(docker ps -aq --filter "name=geo-\|consul-\|graphql-\|grpc-\|ws-\|redis-\|php-") 2>/dev/null || true
    sleep 2

    # Run test with timeout
    timeout 180 bash "$script" > "$LOG_FILE" 2>&1
    EXIT_CODE=$?

    if [ $EXIT_CODE -eq 0 ]; then
        echo "✅ PASSED"
    elif [ $EXIT_CODE -eq 124 ]; then
        echo "⏱️  TIMEOUT (3 min)"
    else
        echo "❌ FAILED (exit: $EXIT_CODE)"
    fi

    # Show summary
    if grep -q "Test Complete" "$LOG_FILE" 2>/dev/null; then
        echo "  ✓ Test completed"
        tail -3 "$LOG_FILE" 2>/dev/null | grep -E "req/s|Success|✓" | head -2 | sed 's/^/  /'
    else
        echo "  Last error:"
        grep -iE "error|failed|ERROR" "$LOG_FILE" 2>/dev/null | tail -1 | sed 's/^/  /'
    fi
done

echo ""
echo "========================================"
echo "All Tests Complete!"
echo "Completed: $(date)"
echo "========================================"
