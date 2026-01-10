#!/bin/bash
# Debug DSL to YAML conversion

# Create simple HTTPS config
cat > /tmp/debug-https.proxy <<'EOF'
https://localhost:8443 {
    tls "/tmp/highper-certs/loadtest.crt" "/tmp/highper-certs/loadtest.key"
    proxy http://127.0.0.1:8081
}

log info
metrics prometheus port=9090
EOF

# Patch the converter to not delete temp file
echo "Testing DSL conversion..."

# Run validation which will create temp YAML
./target/release/highper-gateway validate -c /tmp/debug-https.proxy 2>&1 || true

# Check if temp YAML files exist
echo
echo "Looking for temp YAML files..."
ls -la /tmp/dsl_temp_*.yaml 2>/dev/null | head -5 || echo "No temp files found"

# If found, show content
for f in /tmp/dsl_temp_*.yaml; do
    if [ -f "$f" ]; then
        echo
        echo "=== Content of $f ==="
        cat "$f"
        echo "=== End ==="
        break
    fi
done
