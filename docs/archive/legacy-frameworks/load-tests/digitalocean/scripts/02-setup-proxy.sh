#!/bin/bash
# Setup Proxy Droplet with OS tuning and highper-gateway
# Usage: ./02-setup-proxy.sh <PROXY_IP>

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

echo "=== Setting up Proxy Droplet: $PROXY_IP ==="

# Create setup script to run on droplet
cat > /tmp/proxy-setup.sh << 'SETUP_SCRIPT'
#!/bin/bash
set -e

echo "=== OS Tuning for 600k+ RPS ==="

# 1. System limits
cat >> /etc/security/limits.conf << 'EOF'
* soft nofile 2097152
* hard nofile 2097152
* soft nproc 65535
* hard nproc 65535
root soft nofile 2097152
root hard nofile 2097152
EOF

# 2. Sysctl tuning
cat > /etc/sysctl.d/99-loadtest.conf << 'EOF'
# File descriptors
fs.file-max = 2097152
fs.nr_open = 2097152

# Network core
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65535
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.core.rmem_default = 1048576
net.core.wmem_default = 1048576
net.core.optmem_max = 65535

# TCP tuning
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.tcp_max_tw_buckets = 2000000
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 10
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_keepalive_probes = 6
net.ipv4.tcp_rmem = 4096 1048576 16777216
net.ipv4.tcp_wmem = 4096 1048576 16777216
net.ipv4.tcp_mem = 786432 1048576 1572864
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_syncookies = 1

# Port range
net.ipv4.ip_local_port_range = 1024 65535

# Connection tracking (disable if not using NAT)
net.netfilter.nf_conntrack_max = 2097152
net.nf_conntrack_max = 2097152

# ARP cache
net.ipv4.neigh.default.gc_thresh1 = 8192
net.ipv4.neigh.default.gc_thresh2 = 32768
net.ipv4.neigh.default.gc_thresh3 = 65536

# Disable IPv6 if not needed
net.ipv6.conf.all.disable_ipv6 = 1
net.ipv6.conf.default.disable_ipv6 = 1
EOF

sysctl -p /etc/sysctl.d/99-loadtest.conf

# 3. PAM limits
echo "session required pam_limits.so" >> /etc/pam.d/common-session

# 4. Systemd limits
mkdir -p /etc/systemd/system.conf.d
cat > /etc/systemd/system.conf.d/limits.conf << 'EOF'
[Manager]
DefaultLimitNOFILE=2097152
DefaultLimitNPROC=65535
EOF

# 5. Install dependencies
apt-get update
apt-get install -y build-essential pkg-config libssl-dev curl htop iotop sysstat

# 6. Install Rust
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

echo ""
echo "=== OS Tuning Complete ==="
echo "Please reboot the droplet for all changes to take effect."
echo "After reboot, run the proxy binary deployment."

SETUP_SCRIPT

# Copy and run setup script
scp -o StrictHostKeyChecking=no /tmp/proxy-setup.sh root@$PROXY_IP:/root/
ssh root@$PROXY_IP "chmod +x /root/proxy-setup.sh && /root/proxy-setup.sh"

echo ""
echo "=== Setup complete. Rebooting droplet... ==="
ssh root@$PROXY_IP "reboot" || true

echo "Waiting 30 seconds for reboot..."
sleep 30

# Wait for SSH to come back
for i in {1..30}; do
    if ssh -o ConnectTimeout=5 root@$PROXY_IP "echo 'SSH is back'" 2>/dev/null; then
        break
    fi
    echo "Waiting for SSH... ($i/30)"
    sleep 5
done

echo ""
echo "=== Droplet rebooted and ready ==="
echo "Next: Deploy highper-gateway binary with ./02b-deploy-proxy-binary.sh"
