#!/bin/bash
# Scenario 01 - TCP Proxy (Native Gateway)
# Tests Layer 4 TCP proxying performance

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Go to load test directory
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 01: TCP Proxy (Native)"
echo "========================================="

# Cleanup function
cleanup() {
    echo "Cleaning up..."

    # Stop gateway if running
    if [ ! -z "${GATEWAY_PID:-}" ]; then
        echo "Stopping gateway (PID: $GATEWAY_PID)..."
        kill $GATEWAY_PID 2>/dev/null || true

        # Wait up to 5 seconds for graceful shutdown
        for i in {1..10}; do
            if ! kill -0 $GATEWAY_PID 2>/dev/null; then
                break
            fi
            sleep 0.5
        done

        # Force kill if still running
        if kill -0 $GATEWAY_PID 2>/dev/null; then
            echo "Force killing gateway (PID: $GATEWAY_PID)..."
            kill -9 $GATEWAY_PID 2>/dev/null || true
            sleep 1
        fi
    fi

    # Stop Docker backends
    (cd docker 2>/dev/null && docker-compose -f docker-compose-prebuilt.yml down 2>/dev/null) || true
    docker network rm tcp-loadtest 2>/dev/null || true

    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Create Docker network
echo "Creating Docker network..."
docker network create tcp-loadtest 2>/dev/null || true

# Start Rust HTTP server backends (high-performance)
echo "Starting Rust HTTP server backends for TCP proxy test..."
(cd docker && docker-compose -f docker-compose-prebuilt.yml up -d --build)

# Wait for backends
echo "Waiting for Rust backends to be ready..."
sleep 15

# Check backend health
for port in 8001 8002 8003; do
    if curl -s -f http://localhost:$port/health > /dev/null 2>&1; then
        echo "✓ Rust backend on port $port is ready"
    else
        echo "✗ Rust backend on port $port is NOT ready"
    fi
done

# Create gateway config for TCP proxy
echo "Creating gateway TCP proxy configuration..."
cat > /tmp/gateway-tcp-test.toml <<'EOF'
[server]
bind = ["127.0.0.1:9000"]
workers = "auto"

[server.performance]
max_connections = 500000
read_buffer_size = 65536
write_buffer_size = 65536

# TCP Upstream backends (using tcp:// scheme for Layer 4 proxying)
[[upstreams]]
name = "tcp-backends"

servers = [
    { url = "tcp://localhost:8001", weight = 1 },
    { url = "tcp://localhost:8002", weight = 1 },
    { url = "tcp://localhost:8003", weight = 1 },
]

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
timeout = "5s"
keepalive = "60s"
pool_size = 500
tcp_nodelay = true

[upstreams.health_check.active]
enabled = false  # Disable during load test

# Route all traffic through TCP upstreams
[[routes]]
name = "tcp-route"
upstream = "tcp-backends"

[routes.match]
paths = ["/*"]

[observability.metrics]
enabled = false

[observability.logging]
level = "warn"
format = "json"
EOF

# Start gateway
echo "Starting Highper Gateway (TCP mode)..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

if [ ! -f "$GATEWAY_BINARY" ]; then
    echo "ERROR: Gateway binary not found at $GATEWAY_BINARY"
    exit 1
fi

$GATEWAY_BINARY start --config /tmp/gateway-tcp-test.toml > /tmp/gateway-tcp.log 2>&1 &
GATEWAY_PID=$!

echo "Gateway started (PID: $GATEWAY_PID)"

# Wait for gateway
sleep 5

# Check if gateway is running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    echo "Log tail:"
    tail -20 /tmp/gateway-tcp.log
    exit 1
fi

echo "✓ Gateway is running"

# Test TCP connectivity through gateway
echo "Testing TCP connectivity through gateway..."
if curl -s http://localhost:9000/ > /dev/null 2>&1; then
    echo "✓ Gateway TCP port 9000 is accepting connections and proxying to backends"
else
    echo "✗ Gateway TCP port 9000 is NOT responding"
fi

# Test round-robin distribution
echo ""
echo "Testing round-robin load balancing (10 requests to /api/ping)..."
for i in {1..10}; do
    response=$(curl -s http://localhost:9000/api/ping 2>/dev/null)
    if echo "$response" | grep -q "backend"; then
        backend=$(echo "$response" | jq -r '.backend' 2>/dev/null || echo "unknown")
        echo "  Request $i: SUCCESS (routed to $backend)"
    else
        echo "  Request $i: FAILED"
    fi
done

# Create results directory
RESULT_DIR="results/local/01-tcp-native/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo ""
echo "Results will be saved to: $RESULT_DIR"

# Run HTTP throughput tests through TCP proxy
echo ""
echo "Running TCP proxy throughput tests with vegeta..."
echo "Gateway: localhost:9000 (TCP) -> HTTP backends on 8001-8003"
echo "Rates: 1K, 2K, 3K, 4K, 5K req/s"
echo "Duration: 10s per rate"
echo ""

# Test with different request rates
for rate in 1000 2000 3000 4000 5000; do
    echo "Testing at ${rate} req/s..."

    # Run vegeta with timeout wrapper to prevent hangs
    timeout 30s bash -c "echo 'GET http://localhost:9000/api/ping' | vegeta attack \
        -rate=${rate} \
        -duration=10s \
        -timeout=5s \
        -workers=8 \
        -keepalive=true \
        > '${RESULT_DIR}/vegeta-${rate}rps.bin' 2>&1" || {
        echo "  ⚠ Vegeta timed out or failed for ${rate} req/s (continuing...)"
        continue
    }

    # Generate reports with error handling
    if [ -f "${RESULT_DIR}/vegeta-${rate}rps.bin" ] && [ -s "${RESULT_DIR}/vegeta-${rate}rps.bin" ]; then
        timeout 10s vegeta report -type=json < "${RESULT_DIR}/vegeta-${rate}rps.bin" > "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || true
        timeout 10s vegeta report -type=text < "${RESULT_DIR}/vegeta-${rate}rps.bin" > "${RESULT_DIR}/vegeta-${rate}rps.txt" 2>/dev/null || true
    fi

    # Extract key metrics
    if [ -f "${RESULT_DIR}/vegeta-${rate}rps.json" ] && [ -s "${RESULT_DIR}/vegeta-${rate}rps.json" ]; then
        actual_rate=$(jq -r '.rate // 0' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        echo "  Results: ${actual_rate} req/s, P99: ${p99}ms, Success: ${success}%"
    else
        echo "  ⚠ No valid results for ${rate} req/s"
    fi

    # Brief pause between tests to allow connections to close
    sleep 2
done

# Generate summary
echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "TCP Proxy Performance Summary:"
for json_file in "$RESULT_DIR"/vegeta-*rps.json; do
    if [ -f "$json_file" ]; then
        rate=$(basename "$json_file" | sed 's/vegeta-\([0-9]*\)rps.json/\1/')
        actual_rate=$(jq -r '.rate // 0' "$json_file" 2>/dev/null || echo "0")
        p50=$(jq -r '.latencies."50th" // 0 | tonumber / 1000000' "$json_file" 2>/dev/null || echo "0")
        p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "$json_file" 2>/dev/null || echo "0")
        success=$(jq -r '.success // 0 | . * 100' "$json_file" 2>/dev/null || echo "0")
        echo "  ${rate} req/s target: ${actual_rate} req/s actual, P50=${p50}ms, P99=${p99}ms, Success=${success}%"
    fi
done

echo ""
echo "========================================="
