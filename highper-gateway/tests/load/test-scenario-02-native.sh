#!/bin/bash
# Simplified Scenario 02 - Native Gateway (no Docker for gateway)
# Runs gateway natively, backends in Docker

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Go to load test directory
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 02: HTTP Load Balancer (Native)"
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
    docker-compose -f docker/docker-compose-prebuilt.yml down 2>/dev/null || true

    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Start Docker backends
echo "Starting backend servers..."
docker-compose -f docker/docker-compose-prebuilt.yml up -d --build

# Wait for backends
echo "Waiting for backends to be ready..."
sleep 20

# Check backend health
for port in 8001 8002 8003; do
    if curl -s -f http://localhost:$port/health > /dev/null 2>&1; then
        echo "✓ Backend on port $port is ready"
    else
        echo "✗ Backend on port $port is NOT ready"
    fi
done

# Create optimized gateway config for high throughput
echo "Creating optimized gateway configuration..."
cat > /tmp/gateway-test.toml <<'EOF'
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"  # Use all CPU cores (12)
protocols = ["http1", "http2"]  # Enable HTTP/2 for multiplexing

[server.performance]
max_connections = 500000  # Increased from 100K
max_requests_per_connection = 10000  # Higher reuse
read_buffer_size = 32768  # 32KB (increased from default 16KB)
write_buffer_size = 32768  # 32KB
connect_timeout = "2s"  # Faster failure detection
request_timeout = "30s"  # Reduced from 60s
idle_timeout = "60s"  # Reduced keepalive

[server.performance.connection_pool]
max_idle_per_host = 500  # Increased connection pooling
min_idle_per_host = 100  # Pre-warm connections
max_connection_lifetime = "300s"
idle_timeout = "60s"
prewarm = true  # Pre-create connections
metrics_enabled = false  # Disable for performance

[[upstreams]]
name = "backends"

servers = [
    { url = "http://localhost:8001", weight = 1, max_conns = 50000 },
    { url = "http://localhost:8002", weight = 1, max_conns = 50000 },
    { url = "http://localhost:8003", weight = 1, max_conns = 50000 },
]

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
timeout = "2s"
keepalive = "60s"
pool_size = 500  # Increased pool
tcp_nodelay = true  # Disable Nagle's algorithm for lower latency

[upstreams.health_check.active]
enabled = false  # Disable during load test to reduce overhead

[[routes]]
name = "default"
upstream = "backends"

[routes.match]
paths = ["/*"]
methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]

[observability.metrics]
enabled = false  # Disable metrics for max performance

[observability.logging]
level = "warn"  # Reduce logging overhead
format = "json"  # Faster than pretty
EOF

# Start gateway
echo "Starting Highper Gateway..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

if [ ! -f "$GATEWAY_BINARY" ]; then
    echo "ERROR: Gateway binary not found at $GATEWAY_BINARY"
    exit 1
fi

$GATEWAY_BINARY start --config /tmp/gateway-test.toml > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!

echo "Gateway started (PID: $GATEWAY_PID)"

# Wait for gateway
sleep 5

# Check if gateway is running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    echo "Log tail:"
    tail -20 /tmp/gateway.log
    exit 1
fi

echo "✓ Gateway is running"

# Test connectivity
echo "Testing gateway connectivity..."
if curl -s http://localhost:8080/api/ping | jq '.' 2>/dev/null; then
    echo "✓ Gateway is responding"
else
    echo "Gateway response test (without jq parsing):"
    curl -s http://localhost:8080/api/ping || echo "Failed"
fi

# Create results directory
RESULT_DIR="results/local/02-http-native/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo "Results will be saved to: $RESULT_DIR"

# Run vegeta load test with progressive rates (adjusted for WSL2)
echo ""
echo "Running optimized load test with vegeta..."
echo "Target: http://localhost:8080/api/ping"
echo "Rates: 500, 1K, 2K, 3K, 4K, 5K req/s (WSL2-optimized)"
echo "Duration: 10s per rate"
echo ""

for rate in 500 1000 2000 3000 4000 5000; do
    echo "Testing at ${rate} req/s..."

    # Run vegeta with timeout wrapper to prevent hangs
    timeout 30s bash -c "echo 'GET http://localhost:8080/api/ping' | vegeta attack \
        -rate=${rate} \
        -duration=10s \
        -timeout=5s \
        -workers=4 \
        -keepalive=true \
        -max-workers=8 \
        > '${RESULT_DIR}/vegeta-${rate}rps.bin' 2>&1" || {
        echo "  ⚠ Vegeta timed out or failed for ${rate} req/s (continuing...)"
        continue
    }

    # Generate reports with error handling
    if [ -f "${RESULT_DIR}/vegeta-${rate}rps.bin" ] && [ -s "${RESULT_DIR}/vegeta-${rate}rps.bin" ]; then
        timeout 10s vegeta report -type=json < "${RESULT_DIR}/vegeta-${rate}rps.bin" > "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || true
        timeout 10s vegeta report -type=text < "${RESULT_DIR}/vegeta-${rate}rps.bin" > "${RESULT_DIR}/vegeta-${rate}rps.txt" 2>/dev/null || true
    fi

    # Show results
    if [ -f "${RESULT_DIR}/vegeta-${rate}rps.json" ] && [ -s "${RESULT_DIR}/vegeta-${rate}rps.json" ]; then
        actual_rate=$(jq -r '.rate // 0' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")

        echo "  Results: ${actual_rate} req/s, P99: ${p99}ms, Success: ${success}%"

        # Stop if success rate drops
        if (( $(echo "$success < 95.0" | bc -l) )); then
            echo "Success rate dropped below 95%, stopping test"
            break
        fi
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
