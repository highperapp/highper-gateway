#!/bin/bash
# Build and deploy binaries to all droplets
set -e

cd "$(dirname "$0")/../../.."

if [ -f load-tests/digitalocean/droplet-ips.env ]; then
    source load-tests/digitalocean/droplet-ips.env
else
    echo "Error: droplet-ips.env not found"
    exit 1
fi

echo "=== Building Binaries ==="

# Build highper-gateway
echo "Building highper-gateway..."
cd highper-gateway
cargo build --release
cd ..

# Build rust-backend
echo "Building rust-backend..."
cd load-tests/simple-backend-rust
cargo build --release
cd ../..

echo ""
echo "=== Deploying Binaries ==="

# Deploy to proxy
echo "Deploying highper-gateway to $PROXY_PUBLIC..."
scp -o StrictHostKeyChecking=no highper-gateway/target/release/highper-gateway root@$PROXY_PUBLIC:/usr/local/bin/
ssh root@$PROXY_PUBLIC "chmod +x /usr/local/bin/highper-gateway"

# Deploy backend to all backend droplets
for i in 1 2 3; do
    eval "IP=\$BACKEND_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo "Deploying rust-backend to backend-$i ($IP)..."
        scp load-tests/simple-backend-rust/target/release/rust-backend root@$IP:/usr/local/bin/
        ssh root@$IP "chmod +x /usr/local/bin/rust-backend"
    fi
done

echo ""
echo "=== Creating Proxy Config ==="

# Create proxy config with backend private IPs
BACKEND_SERVERS=""
for i in 1 2 3; do
    eval "IP=\$BACKEND_${i}_PRIVATE"
    if [ -n "$IP" ]; then
        if [ -n "$BACKEND_SERVERS" ]; then
            BACKEND_SERVERS="$BACKEND_SERVERS,"
        fi
        BACKEND_SERVERS="$BACKEND_SERVERS
    { url = \"http://$IP:80\", weight = 1, max_conns = 100000 }"
    fi
done

ssh root@$PROXY_PUBLIC "cat > /root/config.toml" << EOF
[server]
bind = ["0.0.0.0:8080"]
workers = "auto"

[server.performance]
max_connections = 500000
backlog = 65535
tcp_nodelay = true
tcp_keepalive = true

[[upstreams]]
name = "backend"
servers = [$BACKEND_SERVERS
]

[upstreams.health_check]
enabled = false

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
max_connections_per_upstream = 300000
min_idle_connections = 10000
connection_timeout = "30s"
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

echo ""
echo "=== Creating Systemd Services ==="

# Backend service
for i in 1 2 3; do
    eval "IP=\$BACKEND_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo "Creating backend service on backend-$i..."
        ssh root@$IP "cat > /etc/systemd/system/rust-backend.service" << 'EOF'
[Unit]
Description=Rust Backend Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/rust-backend
Restart=always
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF
        ssh root@$IP "systemctl daemon-reload && systemctl enable rust-backend && systemctl start rust-backend"
    fi
done

# Proxy service (if not exists)
ssh root@$PROXY_PUBLIC "cat > /etc/systemd/system/highper-gateway.service" << 'EOF'
[Unit]
Description=Highper Gateway Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/highper-gateway start --config /root/config.toml
Restart=always
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF
ssh root@$PROXY_PUBLIC "systemctl daemon-reload && systemctl enable highper-gateway && systemctl restart highper-gateway"

echo ""
echo "=== Verifying Services ==="

# Check backends
for i in 1 2 3; do
    eval "IP=\$BACKEND_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo -n "backend-$i: "
        ssh root@$IP "systemctl is-active rust-backend" || echo "FAILED"
    fi
done

# Check proxy
echo -n "proxy: "
ssh root@$PROXY_PUBLIC "systemctl is-active highper-gateway" || echo "FAILED"

echo ""
echo "=== Deployment Complete ==="
