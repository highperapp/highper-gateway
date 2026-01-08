#!/bin/bash
# Scenario 04 - API Gateway with Rate Limiting
# Tests rate limiting algorithms and effectiveness

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Go to load test directory
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 04: API Gateway Rate Limiting"
echo "========================================="

# Cleanup function
cleanup() {
    echo "Cleaning up..."

    # Stop gateway if running
    if [ ! -z "${GATEWAY_PID:-}" ]; then
        echo "Stopping gateway (PID: $GATEWAY_PID)..."
        kill $GATEWAY_PID 2>/dev/null || true
        wait $GATEWAY_PID 2>/dev/null || true
    fi

    # Stop Docker backends
    (cd docker 2>/dev/null && docker-compose -f docker-compose-prebuilt.yml down 2>/dev/null) || true

    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Start Docker backends
echo "Starting backend servers..."
(cd docker && docker-compose -f docker-compose-prebuilt.yml up -d --build)

# Wait for backends
echo "Waiting for backends to be ready..."
sleep 15

# Check backend health
for port in 8001 8002 8003; do
    if curl -s -f http://localhost:$port/health > /dev/null 2>&1; then
        echo "✓ Backend on port $port is ready"
    else
        echo "✗ Backend on port $port is NOT ready"
    fi
done

# Create gateway config with rate limiting
echo "Creating gateway configuration with rate limiting..."
cat > /tmp/gateway-ratelimit-test.toml <<'EOF'
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"
protocols = ["http1", "http2"]

[server.performance]
max_connections = 500000
read_buffer_size = 32768
write_buffer_size = 32768

# Global rate limiting configuration (applies to all requests)
[rate_limit]
enabled = true
capacity = 100
window = "10s"

[[upstreams]]
name = "backends"

servers = [
    { url = "http://localhost:8001", weight = 1 },
    { url = "http://localhost:8002", weight = 1 },
    { url = "http://localhost:8003", weight = 1 },
]

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
timeout = "2s"
keepalive = "60s"
pool_size = 500
tcp_nodelay = true

[[routes]]
name = "api-route"
upstream = "backends"

[routes.match]
paths = ["/api/*"]
methods = ["GET", "POST"]

[[routes]]
name = "default"
upstream = "backends"

[routes.match]
paths = ["/*"]

[observability.metrics]
enabled = false

[observability.logging]
level = "info"
format = "json"
EOF

# Start gateway
echo "Starting Highper Gateway with rate limiting..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

if [ ! -f "$GATEWAY_BINARY" ]; then
    echo "ERROR: Gateway binary not found at $GATEWAY_BINARY"
    exit 1
fi

$GATEWAY_BINARY start --config /tmp/gateway-ratelimit-test.toml > /tmp/gateway-ratelimit.log 2>&1 &
GATEWAY_PID=$!

echo "Gateway started (PID: $GATEWAY_PID)"

# Wait for gateway
sleep 5

# Check if gateway is running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    echo "Log tail:"
    tail -20 /tmp/gateway-ratelimit.log
    exit 1
fi

echo "✓ Gateway is running"

# Test basic connectivity
echo "Testing gateway connectivity..."
if curl -s http://localhost:8080/api/ping > /dev/null 2>&1; then
    echo "✓ Gateway is responding"
else
    echo "✗ Gateway is NOT responding"
fi

# Create results directory
RESULT_DIR="results/local/04-rate-limit/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo ""
echo "Results will be saved to: $RESULT_DIR"

# Test 1: Below rate limit (should succeed 100%)
echo ""
echo "========================================="
echo "Test 1: Below Rate Limit (500 req/s)"
echo "Limit: 1000 req/s, Testing at: 500 req/s"
echo "Expected: 100% success"
echo "========================================="

echo "GET http://localhost:8080/api/ping" | vegeta attack \
    -rate=500 \
    -duration=10s \
    -timeout=5s \
    -workers=4 \
    -keepalive=true \
    > "${RESULT_DIR}/below-limit.bin" 2>&1

cat "${RESULT_DIR}/below-limit.bin" | vegeta report -type=json > "${RESULT_DIR}/below-limit.json"
cat "${RESULT_DIR}/below-limit.bin" | vegeta report -type=text > "${RESULT_DIR}/below-limit.txt"

if [ -f "${RESULT_DIR}/below-limit.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/below-limit.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/below-limit.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/below-limit.json")
    status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/below-limit.json")
    status_429=$(jq -r '.status_codes."429" // 0' "${RESULT_DIR}/below-limit.json")

    echo "Results:"
    echo "  Rate: ${rate} req/s"
    echo "  P99 Latency: ${p99}ms"
    echo "  Success: ${success}%"
    echo "  200 OK: ${status_200}"
    echo "  429 Rate Limited: ${status_429}"
fi

# Test 2: At rate limit (should succeed ~100%)
echo ""
echo "========================================="
echo "Test 2: At Rate Limit (1000 req/s)"
echo "Limit: 1000 req/s, Testing at: 1000 req/s"
echo "Expected: ~100% success (with token bucket)"
echo "========================================="

echo "GET http://localhost:8080/api/ping" | vegeta attack \
    -rate=1000 \
    -duration=10s \
    -timeout=5s \
    -workers=4 \
    -keepalive=true \
    > "${RESULT_DIR}/at-limit.bin" 2>&1

cat "${RESULT_DIR}/at-limit.bin" | vegeta report -type=json > "${RESULT_DIR}/at-limit.json"
cat "${RESULT_DIR}/at-limit.bin" | vegeta report -type=text > "${RESULT_DIR}/at-limit.txt"

if [ -f "${RESULT_DIR}/at-limit.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/at-limit.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/at-limit.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/at-limit.json")
    status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/at-limit.json")
    status_429=$(jq -r '.status_codes."429" // 0' "${RESULT_DIR}/at-limit.json")

    echo "Results:"
    echo "  Rate: ${rate} req/s"
    echo "  P99 Latency: ${p99}ms"
    echo "  Success: ${success}%"
    echo "  200 OK: ${status_200}"
    echo "  429 Rate Limited: ${status_429}"
fi

# Test 3: Above rate limit (should see 429 responses)
echo ""
echo "========================================="
echo "Test 3: Above Rate Limit (2000 req/s)"
echo "Limit: 1000 req/s, Testing at: 2000 req/s"
echo "Expected: ~50% success, ~50% rate limited"
echo "========================================="

echo "GET http://localhost:8080/api/ping" | vegeta attack \
    -rate=2000 \
    -duration=10s \
    -timeout=5s \
    -workers=8 \
    -keepalive=true \
    > "${RESULT_DIR}/above-limit.bin" 2>&1

cat "${RESULT_DIR}/above-limit.bin" | vegeta report -type=json > "${RESULT_DIR}/above-limit.json"
cat "${RESULT_DIR}/above-limit.bin" | vegeta report -type=text > "${RESULT_DIR}/above-limit.txt"

if [ -f "${RESULT_DIR}/above-limit.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/above-limit.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/above-limit.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/above-limit.json")
    status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/above-limit.json")
    status_429=$(jq -r '.status_codes."429" // 0' "${RESULT_DIR}/above-limit.json")

    echo "Results:"
    echo "  Rate: ${rate} req/s"
    echo "  P99 Latency: ${p99}ms"
    echo "  Success: ${success}%"
    echo "  200 OK: ${status_200}"
    echo "  429 Rate Limited: ${status_429}"

    # Calculate percentage of rate-limited requests
    total=$((status_200 + status_429))
    if [ $total -gt 0 ]; then
        rate_limited_pct=$(echo "scale=2; $status_429 * 100 / $total" | bc)
        echo "  Rate Limited %: ${rate_limited_pct}%"
    fi
fi

# Test 4: Burst test (should handle burst within limits)
echo ""
echo "========================================="
echo "Test 4: Burst Test (3000 req/s for 3s)"
echo "Limit: 1000 req/s with 1500 burst"
echo "Expected: Burst absorbed initially, then rate limited"
echo "========================================="

echo "GET http://localhost:8080/api/ping" | vegeta attack \
    -rate=3000 \
    -duration=3s \
    -timeout=5s \
    -workers=8 \
    -keepalive=true \
    > "${RESULT_DIR}/burst-test.bin" 2>&1

cat "${RESULT_DIR}/burst-test.bin" | vegeta report -type=json > "${RESULT_DIR}/burst-test.json"
cat "${RESULT_DIR}/burst-test.bin" | vegeta report -type=text > "${RESULT_DIR}/burst-test.txt"

if [ -f "${RESULT_DIR}/burst-test.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/burst-test.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/burst-test.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/burst-test.json")
    status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/burst-test.json")
    status_429=$(jq -r '.status_codes."429" // 0' "${RESULT_DIR}/burst-test.json")

    echo "Results:"
    echo "  Rate: ${rate} req/s"
    echo "  P99 Latency: ${p99}ms"
    echo "  Success: ${success}%"
    echo "  200 OK: ${status_200}"
    echo "  429 Rate Limited: ${status_429}"
fi

# Test 5: Unlimited route (for comparison)
echo ""
echo "========================================="
echo "Test 5: Unlimited Route (2000 req/s)"
echo "No rate limit on /unlimited/* path"
echo "Expected: 100% success"
echo "========================================="

echo "GET http://localhost:8080/unlimited/ping" | vegeta attack \
    -rate=2000 \
    -duration=10s \
    -timeout=5s \
    -workers=8 \
    -keepalive=true \
    > "${RESULT_DIR}/unlimited.bin" 2>&1

cat "${RESULT_DIR}/unlimited.bin" | vegeta report -type=json > "${RESULT_DIR}/unlimited.json"
cat "${RESULT_DIR}/unlimited.bin" | vegeta report -type=text > "${RESULT_DIR}/unlimited.txt"

if [ -f "${RESULT_DIR}/unlimited.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/unlimited.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/unlimited.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/unlimited.json")
    status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/unlimited.json")

    echo "Results:"
    echo "  Rate: ${rate} req/s"
    echo "  P99 Latency: ${p99}ms"
    echo "  Success: ${success}%"
    echo "  200 OK: ${status_200}"
fi

# Generate summary
echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "Rate Limiting Effectiveness Summary:"
echo ""

printf "%-25s | %-12s | %-12s | %-12s | %-12s\n" "Test" "Target Rate" "Success %" "200 OK" "429 Limited"
printf "%-25s-+-%-12s-+-%-12s-+-%-12s-+-%-12s\n" "-------------------------" "------------" "------------" "------------" "------------"

for test in below-limit at-limit above-limit burst-test unlimited; do
    if [ -f "${RESULT_DIR}/${test}.json" ]; then
        test_name=$(echo $test | sed 's/-/ /g' | awk '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)}1')
        target_rate="N/A"
        case $test in
            below-limit) target_rate="500 req/s" ;;
            at-limit) target_rate="1000 req/s" ;;
            above-limit) target_rate="2000 req/s" ;;
            burst-test) target_rate="3000 req/s" ;;
            unlimited) target_rate="2000 req/s" ;;
        esac

        success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/${test}.json")
        status_200=$(jq -r '.status_codes."200" // 0' "${RESULT_DIR}/${test}.json")
        status_429=$(jq -r '.status_codes."429" // 0' "${RESULT_DIR}/${test}.json")

        printf "%-25s | %-12s | %11.2f%% | %12d | %12d\n" "$test_name" "$target_rate" "$success" "$status_200" "$status_429"
    fi
done

echo ""
echo "========================================="
