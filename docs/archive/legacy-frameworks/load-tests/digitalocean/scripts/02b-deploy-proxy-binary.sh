#!/bin/bash
# Deploy highper-gateway binary to the droplet
# Usage: ./02b-deploy-proxy-binary.sh <PROXY_IP>

set -e

PROXY_IP="${1:-}"
if [ -z "$PROXY_IP" ]; then
    if [ -f ../config.env ]; then
        source ../config.env
        PROXY_IP="$PROXY_PUBLIC_IP"
    fi
fi

if [ -z "$PROXY_IP" ]; then
    echo "Usage: $0 <PROXY_IP>"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "=== Building highper-gateway for Linux ==="
cd "$PROJECT_ROOT/highper-gateway"

# Build release binary (cross-compile if needed)
if [[ "$(uname)" == "Linux" ]]; then
    cargo build --release
    BINARY_PATH="$PROJECT_ROOT/highper-gateway/target/release/highper-gateway"
else
    echo "Cross-compiling for Linux..."
    # For macOS/Windows, need to cross-compile
    rustup target add x86_64-unknown-linux-gnu
    cargo build --release --target x86_64-unknown-linux-gnu
    BINARY_PATH="$PROJECT_ROOT/highper-gateway/target/x86_64-unknown-linux-gnu/release/highper-gateway"
fi

echo "=== Deploying to $PROXY_IP ==="

# Create proxy config optimized for high throughput
cat > /tmp/proxy-loadtest-config.toml << 'EOF'
[server]
bind = ["0.0.0.0:8080"]
workers = "auto"

[server.performance]
max_connections = 100000
backlog = 65535
tcp_nodelay = true
tcp_keepalive = true

[[upstreams]]
name = "backend"
# Will be updated with actual backend service IP
servers = [
    { url = "http://BACKEND_IP:80", weight = 1, max_conns = 10000 }
]

[upstreams.health_check]
enabled = false

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
max_connections_per_upstream = 50000
min_idle_connections = 1000
connection_timeout = "10s"
tcp_nodelay = true
tcp_keepalive = true
circuit_breaker_enabled = false
retry_max_attempts = 0

[[routes]]
name = "default"
upstream = "backend"

[routes.match]
paths = ["/"]

[observability]
log_level = "warn"
access_log = false
EOF

# Copy binary and config
scp "$BINARY_PATH" root@$PROXY_IP:/usr/local/bin/highper-gateway
scp /tmp/proxy-loadtest-config.toml root@$PROXY_IP:/root/config.toml
ssh root@$PROXY_IP "chmod +x /usr/local/bin/highper-gateway"

# Create systemd service
ssh root@$PROXY_IP << 'EOF'
cat > /etc/systemd/system/highper-gateway.service << 'SERVICE'
[Unit]
Description=Rust Reverse Proxy
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/highper-gateway start --config /root/config.toml
Restart=on-failure
RestartSec=5
LimitNOFILE=2097152
LimitNPROC=65535

[Install]
WantedBy=multi-user.target
SERVICE

systemctl daemon-reload
systemctl enable highper-gateway
EOF

echo ""
echo "=== Proxy binary deployed ==="
echo "Config: /root/config.toml"
echo "Binary: /usr/local/bin/highper-gateway"
echo ""
echo "Note: Update BACKEND_IP in /root/config.toml after DOKS deployment"
echo "Then start with: systemctl start highper-gateway"
