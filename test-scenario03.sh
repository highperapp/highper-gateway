#!/bin/bash
# Test Script for Scenario 03: HTTPS/TLS Termination

set -e

echo "=== Scenario 03: HTTPS/TLS Termination Test ==="
echo

# Create certs directory
echo "1. Creating certs directory..."
mkdir -p /tmp/highper-certs

# Generate self-signed certificate for testing
echo "2. Generating self-signed TLS certificate..."
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /tmp/highper-certs/loadtest.key \
  -out /tmp/highper-certs/loadtest.crt \
  -days 365 \
  -subj "/CN=localhost" \
  2>/dev/null

echo "   ✓ Certificate generated"

# Update config to use /tmp path
echo "3. Creating test config with correct cert paths..."
cat > /tmp/scenario-03-test.proxy <<'EOF'
# Scenario 03: Layer 7 HTTPS/TLS Termination (Test)
https://localhost:8443 {
    tls "/tmp/highper-certs/loadtest.crt" "/tmp/highper-certs/loadtest.key"
    proxy http://127.0.0.1:8081 http://127.0.0.1:8082 http://127.0.0.1:8083
    lb least_conn
    health interval=10s path="/health" timeout=5s
    keepalive 120s
    max_conns 2000000
    compress gzip br
    rate_limit 550000 burst=75000
}

log info
metrics prometheus port=9090
buffer_pool enabled size=16384 pool_size=16777216
EOF

echo "   ✓ Test config created"

# Validate config
echo "4. Validating configuration..."
if ./target/release/highper-gateway validate -c /tmp/scenario-03-test.proxy; then
    echo "   ✓ Configuration valid"
else
    echo "   ✗ Configuration validation failed"
    exit 1
fi

# Check if backend servers are running
echo
echo "5. Checking backend servers..."
for port in 8081 8082 8083; do
    if curl -s --max-time 1 http://127.0.0.1:$port/ > /dev/null 2>&1; then
        echo "   ✓ Backend on port $port is running"
    else
        echo "   ✗ Backend on port $port not running"
        echo "     Start with: python3 load-tests/simple-backend-local.py $port &"
    fi
done

echo
echo "=== Configuration validated. Ready to test. ==="
echo
echo "To start the gateway:"
echo "  ./target/release/highper-gateway start -c /tmp/scenario-03-test.proxy"
echo
echo "To test HTTPS (in another terminal):"
echo "  curl -k https://127.0.0.1:8443/"
echo "  for i in {1..9}; do curl -sk https://127.0.0.1:8443/ | grep -o backend-[0-9]*; done"
echo
echo "To check metrics:"
echo "  curl -s http://127.0.0.1:9090/metrics | grep http_requests_total"
