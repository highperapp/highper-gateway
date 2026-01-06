#!/bin/bash
# Batch Test All Scenarios - Quick Results Collection
set -u

cd "$(dirname "$0")"

RESULTS_DIR="test-results-$(date +%Y%m%d)"
SUMMARY_FILE="$RESULTS_DIR/test-summary.txt"

mkdir -p "$RESULTS_DIR"

echo "=========================================" | tee "$SUMMARY_FILE"
echo "Batch Testing All 15 Scenarios" | tee -a "$SUMMARY_FILE"
echo "Started: $(date)" | tee -a "$SUMMARY_FILE"
echo "=========================================" | tee -a "$SUMMARY_FILE"
echo "" | tee -a "$SUMMARY_FILE"

# Array of scenarios to test
scenarios=(
    "04:test-scenario-04-rate-limit.sh:Rate Limiting"
    "05:test-scenario-05-http3.sh:HTTP/3 QUIC"
    "06:test-scenario-06-websocket.sh:WebSocket"
    "07:test-scenario-07-grpc.sh:gRPC Gateway"
    "08:test-scenario-08-database.sh:Database LB"
    "09:test-scenario-09-waf.sh:WAF + mTLS"
    "10:test-scenario-10-multi.sh:Multi-Protocol"
    "11:test-scenario-11-cache.sh:CDN Caching"
    "12:test-scenario-12-discovery.sh:Service Discovery"
    "13:test-scenario-13-graphql.sh:GraphQL Gateway"
    "14:test-scenario-14-php.sh:PHP-FPM"
    "15:test-scenario-15-geo.sh:Geographic LB"
)

# Test each scenario
for scenario_info in "${scenarios[@]}"; do
    IFS=: read -r num script name <<< "$scenario_info"

    echo "=========================================" | tee -a "$SUMMARY_FILE"
    echo "Testing Scenario $num: $name" | tee -a "$SUMMARY_FILE"
    echo "Script: $script" | tee -a "$SUMMARY_FILE"
    echo "=========================================" | tee -a "$SUMMARY_FILE"

    LOG_FILE="$RESULTS_DIR/scenario-$(printf "%02d" "$num")-output.log"

    # Run scenario with timeout
    timeout 300 bash "$script" > "$LOG_FILE" 2>&1
    EXIT_CODE=$?

    if [ $EXIT_CODE -eq 0 ]; then
        echo "✅ PASSED" | tee -a "$SUMMARY_FILE"
    elif [ $EXIT_CODE -eq 124 ]; then
        echo "⏱️  TIMEOUT (300s)" | tee -a "$SUMMARY_FILE"
    else
        echo "❌ FAILED (exit code: $EXIT_CODE)" | tee -a "$SUMMARY_FILE"
    fi

    # Extract key metrics from log
    if grep -q "Test Complete" "$LOG_FILE" 2>/dev/null; then
        echo "  Test completed successfully" | tee -a "$SUMMARY_FILE"

        # Try to extract performance summary
        if grep -A5 "Performance Summary" "$LOG_FILE" 2>/dev/null | head -3 | tail -1 | tee -a "$SUMMARY_FILE"; then
            :
        fi
    else
        echo "  Test did not complete" | tee -a "$SUMMARY_FILE"
        echo "  Last 5 lines:" | tee -a "$SUMMARY_FILE"
        tail -5 "$LOG_FILE" 2>/dev/null | sed 's/^/    /' | tee -a "$SUMMARY_FILE"
    fi

    echo "" | tee -a "$SUMMARY_FILE"

    # Brief cleanup between scenarios
    docker-compose -f docker/docker-compose-prebuilt.yml down 2>/dev/null || true
    sleep 2
done

echo "=========================================" | tee -a "$SUMMARY_FILE"
echo "Batch Testing Complete!" | tee -a "$SUMMARY_FILE"
echo "Completed: $(date)" | tee -a "$SUMMARY_FILE"
echo "=========================================" | tee -a "$SUMMARY_FILE"
echo "" | tee -a "$SUMMARY_FILE"
echo "Full results in: $RESULTS_DIR/" | tee -a "$SUMMARY_FILE"
echo "Summary: $SUMMARY_FILE" | tee -a "$SUMMARY_FILE"
