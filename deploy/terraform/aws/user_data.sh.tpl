#!/bin/bash
# Highper Gateway - EC2 Instance Bootstrap
# Use Case: ${use_case}
# Environment: ${environment}

set -euo pipefail

# Logging
exec > >(tee /var/log/highper-gateway-setup.log) 2>&1
echo "Starting Highper Gateway setup at $(date)"

# System info
echo "Kernel version: $(uname -r)"
KERNEL_VERSION=$(uname -r | cut -d. -f1,2)

# Check kernel version for io_uring support
MIN_KERNEL="5.11"
if [[ "$(printf '%s\n' "$MIN_KERNEL" "$KERNEL_VERSION" | sort -V | head -n1)" != "$MIN_KERNEL" ]]; then
    echo "WARNING: Kernel $KERNEL_VERSION may have limited io_uring support. Recommended: 5.11+"
fi

# Update system
apt-get update
apt-get upgrade -y

# Install dependencies
apt-get install -y \
    curl \
    wget \
    ca-certificates \
    gnupg \
    liburing2 \
    liburing-dev \
    jq

# Set up io_uring limits
cat > /etc/security/limits.d/highper-gateway.conf << 'EOF'
# Highper Gateway io_uring limits
*               soft    memlock         unlimited
*               hard    memlock         unlimited
*               soft    nofile          1048576
*               hard    nofile          1048576
*               soft    nproc           unlimited
*               hard    nproc           unlimited
EOF

# Kernel parameters for high-performance networking
cat > /etc/sysctl.d/99-highper-gateway.conf << 'EOF'
# Network performance tuning for Highper Gateway

# Increase socket buffer sizes
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.core.rmem_default = 16777216
net.core.wmem_default = 16777216
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728

# Connection handling
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65535
net.ipv4.tcp_max_syn_backlog = 65535

# TCP performance
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_no_metrics_save = 1
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_tw_reuse = 1

# Keepalive
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_keepalive_probes = 6

# Enable BBR congestion control
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# File handles
fs.file-max = 2097152
fs.nr_open = 2097152

# Virtual memory
vm.swappiness = 10
vm.dirty_ratio = 60
vm.dirty_background_ratio = 2
EOF

sysctl -p /etc/sysctl.d/99-highper-gateway.conf

# Create directories
mkdir -p /etc/highper-gateway/{certs,rules,backends}
mkdir -p /var/log/highper-gateway
mkdir -p /var/lib/highper-gateway

# Create service user
useradd --system --shell /usr/sbin/nologin --home /var/lib/highper-gateway highper-gateway || true

# Download and install Highper Gateway binary
# TODO: Replace with actual download URL
ARCH=$(uname -m)
case $ARCH in
    x86_64)  ARCH="amd64" ;;
    aarch64) ARCH="arm64" ;;
esac

# Placeholder for binary download
# curl -fsSL "https://releases.highper-gateway.io/latest/highper-gateway-linux-$ARCH" -o /usr/bin/highper-gateway
# chmod +x /usr/bin/highper-gateway

# For now, create a placeholder
echo "Binary download placeholder - replace with actual binary" > /usr/bin/highper-gateway.placeholder

# Set capabilities (when binary is available)
# setcap 'cap_net_bind_service,cap_ipc_lock=+ep' /usr/bin/highper-gateway

# Create default configuration for use case ${use_case}
cat > /etc/highper-gateway/config.yaml << 'EOF'
# Highper Gateway Configuration
# Use Case: ${use_case}
# Generated: $(date)

server:
  name: "highper-gateway-${environment}"

# io_uring configuration
io_uring:
  entries: 4096
  sq_poll: true
  sq_poll_cpu: 0

# Logging
logging:
  level: info
  format: json
  output: /var/log/highper-gateway/gateway.log

# Metrics
metrics:
  enabled: true
  endpoint: /metrics
  port: 9090

# Health check
health:
  enabled: true
  endpoint: /health
  port: 8081
EOF

# Create systemd service file
cat > /etc/systemd/system/highper-gateway.service << 'EOF'
[Unit]
Description=Highper Gateway - High Performance Reverse Proxy
Documentation=https://highper-gateway.io/docs
After=network-online.target
Wants=network-online.target

[Service]
Type=exec
User=highper-gateway
Group=highper-gateway
ExecStart=/usr/bin/highper-gateway --config /etc/highper-gateway/config.yaml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5

# Capabilities
CapabilityBoundingSet=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_IPC_LOCK

# Memory locking for io_uring
LimitMEMLOCK=infinity
LimitNOFILE=1048576
LimitNPROC=infinity

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes

# Logging
StandardOutput=append:/var/log/highper-gateway/gateway.log
StandardError=append:/var/log/highper-gateway/error.log

# Working directory
WorkingDirectory=/var/lib/highper-gateway

# Environment
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
EOF

# Set permissions
chown -R highper-gateway:highper-gateway /etc/highper-gateway
chown -R highper-gateway:highper-gateway /var/log/highper-gateway
chown -R highper-gateway:highper-gateway /var/lib/highper-gateway

chmod 750 /etc/highper-gateway
chmod 640 /etc/highper-gateway/config.yaml

# Reload systemd
systemctl daemon-reload

# Enable service (don't start yet - binary not available)
systemctl enable highper-gateway

# Log rotation
cat > /etc/logrotate.d/highper-gateway << 'EOF'
/var/log/highper-gateway/*.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    create 0640 highper-gateway highper-gateway
    sharedscripts
    postrotate
        systemctl reload highper-gateway > /dev/null 2>&1 || true
    endscript
}
EOF

echo "Highper Gateway setup completed at $(date)"
echo "Use case: ${use_case}"
echo "Environment: ${environment}"
echo "Note: Binary needs to be installed before starting the service"
