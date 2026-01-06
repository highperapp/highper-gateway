#!/bin/bash
# Quick Test Remaining Scenarios (04-15)
set -u

cd "$(dirname "$0")"

RESULTS_DIR="test-results-$(date +%Y%m%d)"
mkdir -p "$RESULTS_DIR"

echo "========================================="
echo "Testing Scenarios 04-15"
echo "Started: $(date)"
echo "========================================="

# Test each scenario
for num in 04 05 06 07 08 09 10 11 12 13 14 15; do
    case $num in
        04) script="test-scenario-04-rate-limit.sh"; name="Rate Limiting" ;;
        05) script="test-scenario-05-http3.sh"; name="HTTP/3 QUIC" ;;
        06) script="test-scenario-06-websocket.sh"; name="WebSocket" ;;
        07) script="test-scenario-07-grpc.sh"; name="gRPC" ;;
        08) script="test-scenario-08-database.sh"; name="Database" ;;
        09) script="test-scenario-09-waf.sh"; name="WAF+mTLS" ;;
        10) script="test-scenario-10-multi.sh"; name="Multi-Protocol" ;;
        11) script="test-scenario-11-cache.sh"; name="CDN Cache" ;;
        12) script="test-scenario-12-discovery.sh"; name="Service Discovery" ;;
        13) script="test-scenario-13-graphql.sh"; name="GraphQL" ;;
        14) script="test-scenario-14-php.sh"; name="PHP-FPM" ;;
        15) script="test-scenario-15-geo.sh"; name="Geographic LB" ;;
    esac

    echo ""
    echo "========================================="
    echo "Scenario $num: $name"
    echo "Script: $script"
    echo "========================================="

    LOG_FILE="$RESULTS_DIR/scenario-$num-test.log"

    # Run with timeout
    timeout 300 bash "$script" > "$LOG_FILE" 2>&1
    EXIT_CODE=$?

    if [ $EXIT_CODE -eq 0 ]; then
        echo "✅ PASSED"
    elif [ $EXIT_CODE -eq 124 ]; then
        echo "⏱️  TIMEOUT"
    else
        echo "❌ FAILED (exit: $EXIT_CODE)"
    fi

    # Show last few lines
    echo "Last 10 lines:"
    tail -10 "$LOG_FILE" | sed 's/^/  /'

    # Cleanup between tests
    docker-compose -f docker/docker-compose-prebuilt.yml down 2>/dev/null || true
    docker rm -f $(docker ps -aq --filter "name=geo-\|consul-\|graphql-\|grpc-\|ws-") 2>/dev/null || true
    sleep 3
done

echo ""
echo "========================================="
echo "All Tests Complete!"
echo "Completed: $(date)"
echo "========================================="
echo "Results in: $RESULTS_DIR/"
