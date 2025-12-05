#!/bin/bash
# Test DSL to YAML conversion by examining temp files

# Create simple test config
cat > /tmp/test-dsl.proxy <<'EOF'
https://localhost:8443 {
    tls "/tmp/highper-certs/loadtest.crt" "/tmp/highper-certs/loadtest.key"
    proxy http://127.0.0.1:8081
    lb round_robin
}

log info
EOF

# Create certificates
mkdir -p /tmp/highper-certs
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /tmp/highper-certs/loadtest.key \
  -out /tmp/highper-certs/loadtest.crt \
  -days 365 \
  -subj "/CN=localhost" \
  2>/dev/null

echo "Testing DSL to YAML conversion..."
echo

# Modify the converter temporarily to not delete the temp file
# We'll use strace to see what's being written
timeout 5 strace -e write ./target/release/highper-gateway validate -c /tmp/test-dsl.proxy 2>&1 | grep -A 20 "dsl_temp" | head -30 || true

echo
echo "Checking /tmp for temp files..."
ls -la /tmp/dsl_temp_* 2>/dev/null | head -5 || echo "No temp YAML files found (they are deleted quickly)"
