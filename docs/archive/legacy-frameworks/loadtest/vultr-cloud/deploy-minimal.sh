#!/bin/bash
# Quick deployment for minimal test (1 proxy, 3 backends, 1 generator)

set -euo pipefail

PROXY_IP="140.82.13.133"
BACKEND_IPS=("107.191.43.194" "45.63.8.188" "207.246.92.132")
GENERATOR_IP="207.148.18.64"

SSH_KEY="${HOME}/.ssh/id_rsa"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10"

echo "============================================"
echo "Deploying Minimal Layer 7 Test"
echo "============================================"

# Wait for SSH
echo "[1/6] Waiting for SSH to be ready..."
for ip in "$PROXY_IP" "${BACKEND_IPS[@]}" "$GENERATOR_IP"; do
    echo "  Waiting for $ip..."
    for i in {1..30}; do
        if ssh $SSH_OPTS -i "$SSH_KEY" root@$ip "echo ready" 2>/dev/null; then
            echo "  ✓ $ip ready"
            break
        fi
        sleep 10
    done
done

# Build binaries
echo "[2/6] Building binaries..."
cd /home/infy/highper-gateway
cargo build --release --bin highper-gateway 2>&1 | tail -5
cd load-tests && cargo build --release 2>&1 | tail -5 && cd ..

# Deploy backends
echo "[3/6] Deploying fast-backend to 3 servers..."
for i in "${!BACKEND_IPS[@]}"; do
    ip="${BACKEND_IPS[$i]}"
    echo "  Deploying to backend-$((i+1)) ($ip)..."

    scp $SSH_OPTS -i "$SSH_KEY" load-tests/target/release/fast-backend root@$ip:/usr/local/bin/

    ssh $SSH_OPTS -i "$SSH_KEY" root@$ip "
        chmod +x /usr/local/bin/fast-backend
        killall fast-backend 2>/dev/null || true
        nohup /usr/local/bin/fast-backend > /var/log/fast-backend.log 2>&1 &
        sleep 2
        curl -s http://localhost:8080/health || echo 'Backend starting...'
    "
    echo "  ✓ Backend-$((i+1)) deployed"
done

# Create scenario-02 config
echo "[4/6] Deploying highper-gateway to proxy..."
cat > /tmp/scenario-02.conf <<EOF
localhost:8080 {
    proxy ${BACKEND_IPS[0]}:8080 ${BACKEND_IPS[1]}:8080 ${BACKEND_IPS[2]}:8080
    lb round_robin
    health interval=10s path="/health" timeout=5s
    keepalive 90s
    max_conns 1000000
    compress gzip br
    rate_limit 200000 burst=50000
    pool min_idle=100 max_idle=5000 max_open=500000
}
log info
metrics prometheus port=9090
buffer_pool enabled size=16384 pool_size=8388608
EOF

scp $SSH_OPTS -i "$SSH_KEY" /tmp/scenario-02.conf root@$PROXY_IP:/etc/highper-gateway.conf
scp $SSH_OPTS -i "$SSH_KEY" target/release/highper-gateway root@$PROXY_IP:/usr/local/bin/

ssh $SSH_OPTS -i "$SSH_KEY" root@$PROXY_IP "
    chmod +x /usr/local/bin/highper-gateway
    killall highper-gateway 2>/dev/null || true

    # System tuning
    sysctl -w net.ipv4.tcp_tw_reuse=1
    sysctl -w net.ipv4.tcp_fin_timeout=15
    sysctl -w net.core.somaxconn=65535
    sysctl -w fs.file-max=5000000
    ulimit -n 5000000

    nohup /usr/local/bin/highper-gateway --config /etc/highper-gateway.conf > /var/log/highper-gateway.log 2>&1 &
    sleep 3
    curl -s http://localhost:9090/metrics | head -5
"
echo "✓ Proxy deployed"

# Deploy vegeta
echo "[5/6] Deploying vegeta to generator..."
ssh $SSH_OPTS -i "$SSH_KEY" root@$GENERATOR_IP "
    wget -qO- https://github.com/tsenart/vegeta/releases/download/v12.11.1/vegeta_12.11.1_linux_amd64.tar.gz | tar xz -C /usr/local/bin
    chmod +x /usr/local/bin/vegeta
    /usr/local/bin/vegeta --version
"
echo "✓ Vegeta deployed"

# Run test
echo "[6/6] Running 60-second validation test..."
ssh $SSH_OPTS -i "$SSH_KEY" root@$GENERATOR_IP "
    echo 'GET http://$PROXY_IP:8080/' | /usr/local/bin/vegeta attack -rate=50000/s -duration=60s -workers=24 > /tmp/results.bin
    cat /tmp/results.bin | /usr/local/bin/vegeta report
"

echo "============================================"
echo "Test Complete!"
echo "============================================"
echo "Proxy: http://$PROXY_IP:8080"
echo "Metrics: http://$PROXY_IP:9090/metrics"
echo ""
echo "To cleanup: ./scripts/loadtest/vultr-cloud/cleanup-partial.sh"
