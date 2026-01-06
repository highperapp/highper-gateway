#!/bin/bash
# Scenario 03 - HTTPS/TLS Termination
# Tests TLS performance with different cipher suites

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Go to load test directory
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 03: HTTPS/TLS Termination"
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

# Generate self-signed certificate for testing
echo "Generating self-signed TLS certificate..."
mkdir -p /tmp/gateway-certs

if [ ! -f /tmp/gateway-certs/server.key ]; then
    openssl req -x509 -newkey rsa:2048 -nodes \
        -keyout /tmp/gateway-certs/server.key \
        -out /tmp/gateway-certs/server.crt \
        -days 365 \
        -subj "/C=US/ST=Test/L=Test/O=Highper/CN=localhost" \
        2>/dev/null
    echo "✓ Certificate generated"
else
    echo "✓ Certificate already exists"
fi

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

# Create gateway config with TLS
echo "Creating gateway TLS configuration..."
cat > /tmp/gateway-tls-test.toml <<'EOF'
[server]
bind = ["127.0.0.1:8443"]
workers = "auto"
protocols = ["http1", "http2"]

[server.performance]
max_connections = 500000
read_buffer_size = 32768
write_buffer_size = 32768

# TLS Configuration
[tls]
enabled = true
cert_path = "/tmp/gateway-certs/server.crt"
key_path = "/tmp/gateway-certs/server.key"
min_version = "1.2"
max_version = "1.3"
alpn_protocols = ["h2", "http/1.1"]

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
name = "default"
upstream = "backends"

[routes.match]
paths = ["/*"]

[observability.metrics]
enabled = false

[observability.logging]
level = "warn"
format = "json"
EOF

# Start gateway
echo "Starting Highper Gateway (TLS mode)..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

if [ ! -f "$GATEWAY_BINARY" ]; then
    echo "ERROR: Gateway binary not found at $GATEWAY_BINARY"
    exit 1
fi

$GATEWAY_BINARY start --config /tmp/gateway-tls-test.toml > /tmp/gateway-tls.log 2>&1 &
GATEWAY_PID=$!

echo "Gateway started (PID: $GATEWAY_PID)"

# Wait for gateway
sleep 5

# Check if gateway is running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    echo "Log tail:"
    tail -20 /tmp/gateway-tls.log
    exit 1
fi

echo "✓ Gateway is running"

# Test HTTPS connectivity
echo "Testing HTTPS connectivity..."
if curl -k -s https://localhost:8443/api/ping > /dev/null 2>&1; then
    echo "✓ Gateway HTTPS port 8443 is responding"
else
    echo "✗ Gateway HTTPS port 8443 is NOT responding"
fi

# Test TLS version negotiation
echo ""
echo "Testing TLS protocol versions..."
echo -n "  TLS 1.2: "
if curl -k -s --tlsv1.2 --tls-max 1.2 https://localhost:8443/api/ping > /dev/null 2>&1; then
    echo "✓ Supported"
else
    echo "✗ Not supported"
fi

echo -n "  TLS 1.3: "
if curl -k -s --tlsv1.3 https://localhost:8443/api/ping > /dev/null 2>&1; then
    echo "✓ Supported"
else
    echo "✗ Not supported (may require newer curl)"
fi

# Create results directory
RESULT_DIR="results/local/03-tls/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo ""
echo "Results will be saved to: $RESULT_DIR"

# Run HTTPS throughput tests
echo ""
echo "Running HTTPS/TLS load tests with vegeta..."
echo "Target: https://localhost:8443/api/ping"
echo "Rates: 1K, 2K, 3K, 4K, 5K req/s"
echo "Duration: 10s per rate"
echo ""

# Test with different request rates
for rate in 1000 2000 3000 4000 5000; do
    echo "Testing at ${rate} req/s..."

    echo "GET https://localhost:8443/api/ping" | vegeta attack \
        -rate=${rate} \
        -duration=10s \
        -timeout=5s \
        -workers=8 \
        -keepalive=true \
        -insecure \
        > "${RESULT_DIR}/vegeta-${rate}rps.bin" 2>&1

    # Generate reports
    cat "${RESULT_DIR}/vegeta-${rate}rps.bin" | vegeta report -type=json > "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || true
    cat "${RESULT_DIR}/vegeta-${rate}rps.bin" | vegeta report -type=text > "${RESULT_DIR}/vegeta-${rate}rps.txt" 2>/dev/null || true

    # Extract key metrics
    if [ -f "${RESULT_DIR}/vegeta-${rate}rps.json" ]; then
        actual_rate=$(jq -r '.rate // 0' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/vegeta-${rate}rps.json" 2>/dev/null || echo "0")
        echo "  Results: ${actual_rate} req/s, P99: ${p99}ms, Success: ${success}%"
    fi
done

# Test TLS handshake performance
echo ""
echo "Testing TLS handshake performance (new connections, no keepalive)..."
echo "GET https://localhost:8443/api/ping" | vegeta attack \
    -rate=500 \
    -duration=10s \
    -timeout=5s \
    -workers=8 \
    -keepalive=false \
    -insecure \
    > "${RESULT_DIR}/vegeta-handshake.bin" 2>&1

cat "${RESULT_DIR}/vegeta-handshake.bin" | vegeta report -type=json > "${RESULT_DIR}/vegeta-handshake.json" 2>/dev/null || true
cat "${RESULT_DIR}/vegeta-handshake.bin" | vegeta report -type=text > "${RESULT_DIR}/vegeta-handshake.txt" 2>/dev/null || true

if [ -f "${RESULT_DIR}/vegeta-handshake.json" ]; then
    handshake_rate=$(jq -r '.rate // 0' "${RESULT_DIR}/vegeta-handshake.json" 2>/dev/null || echo "0")
    handshake_p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/vegeta-handshake.json" 2>/dev/null || echo "0")
    handshake_success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/vegeta-handshake.json" 2>/dev/null || echo "0")
    echo "  Handshake test: ${handshake_rate} req/s, P99: ${handshake_p99}ms, Success: ${handshake_success}%"
fi

# Generate summary
echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "TLS Performance Summary:"
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
echo "TLS Handshake Performance (no keepalive):"
if [ -f "${RESULT_DIR}/vegeta-handshake.json" ]; then
    handshake_p50=$(jq -r '.latencies."50th" // 0 | tonumber / 1000000' "${RESULT_DIR}/vegeta-handshake.json" 2>/dev/null || echo "0")
    echo "  500 req/s: P50=${handshake_p50}ms, P99=${handshake_p99}ms, Success=${handshake_success}%"
fi

echo ""
echo "========================================="
