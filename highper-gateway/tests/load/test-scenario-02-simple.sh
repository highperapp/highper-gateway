#!/bin/bash
# Simplified Scenario 02 Test - HTTP Load Balancer
# Uses minimal docker-compose to avoid dependency issues

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Go to load test directory
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 02: HTTP Load Balancer (Simplified)"
echo "========================================="

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    docker-compose -f docker/docker-compose.minimal.yml down 2>/dev/null || true
    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Start infrastructure
echo "Starting Docker infrastructure..."
docker-compose -f docker/docker-compose.minimal.yml up -d --build

# Wait for services
echo "Waiting for services to be ready..."
sleep 30

# Check if gateway is responding
echo "Checking gateway health..."
for i in {1..30}; do
    if curl -s -f http://localhost:9090/health > /dev/null 2>&1; then
        echo "✓ Gateway is ready!"
        break
    fi
    echo "Waiting for gateway... ($i/30)"
    sleep 2
done

# Test backend connectivity
echo "Testing backend connectivity through gateway..."
curl -s http://localhost:8080/api/ping | jq '.' || echo "Failed to reach backend"

# Create results directory
RESULT_DIR="results/local/02-http-simple/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo "Results will be saved to: $RESULT_DIR"

# Run vegeta load test
echo ""
echo "Running load test with vegeta..."
echo "Target: http://localhost:8080/api/ping"
echo "Rate: 1000 -> 5000 req/s"
echo "Duration: 15s per rate"
echo ""

for rate in 1000 2000 3000 4000 5000; do
    echo "Testing at ${rate} req/s..."

    echo "GET http://localhost:8080/api/ping" | vegeta attack \
        -rate=${rate} \
        -duration=15s \
        -timeout=10s \
        -keepalive=true \
        > "${RESULT_DIR}/vegeta-${rate}rps.bin"

    # Generate report
    cat "${RESULT_DIR}/vegeta-${rate}rps.bin" | vegeta report -type=json > "${RESULT_DIR}/vegeta-${rate}rps.json"
    cat "${RESULT_DIR}/vegeta-${rate}rps.bin" | vegeta report -type=text > "${RESULT_DIR}/vegeta-${rate}rps.txt"

    # Show results
    actual_rate=$(jq -r '.rate' "${RESULT_DIR}/vegeta-${rate}rps.json")
    p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "${RESULT_DIR}/vegeta-${rate}rps.json")
    success=$(jq -r '.success * 100' "${RESULT_DIR}/vegeta-${rate}rps.json")

    echo "  Results: ${actual_rate} req/s, P99: ${p99}ms, Success: ${success}%"

    # Stop if success rate drops
    if (( $(echo "$success < 95.0" | bc -l) )); then
        echo "Success rate dropped below 95%, stopping test"
        break
    fi
done

# Generate summary
echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "Performance Summary:"
for json_file in "$RESULT_DIR"/vegeta-*rps.json; do
    if [ -f "$json_file" ]; then
        rate=$(jq -r '.rate' "$json_file")
        p50=$(jq -r '.latencies."50th" | tonumber / 1000000' "$json_file")
        p99=$(jq -r '.latencies."99th" | tonumber / 1000000' "$json_file")
        success=$(jq -r '.success * 100' "$json_file")
        echo "  ${rate} req/s: P50=${p50}ms, P99=${p99}ms, Success=${success}%"
    fi
done

echo ""
echo "Compared to historical baseline: 207K req/s on DigitalOcean"
echo "========================================="
