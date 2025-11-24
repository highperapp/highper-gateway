#!/bin/bash
# tune-droplet.sh - Apply optimal TCP/OS tuning for high-throughput load testing
# Usage: ./tune-droplet.sh [all|proxy|backend|loadgen]
# Run on each droplet as root

set -e

echo "=== Applying High-Throughput TCP/OS Tuning ==="

# Connection handling
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.core.netdev_max_backlog=65535

# Connection reuse (critical for high RPS)
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=5
sysctl -w net.ipv4.tcp_max_tw_buckets=500000

# Port range
sysctl -w net.ipv4.ip_local_port_range="1024 65535"

# Buffer sizes
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216

# File descriptors - must also set in /etc/security/limits.d/ for PAM
if ! grep -q "nofile 1000000" /etc/security/limits.conf 2>/dev/null; then
    echo "* soft nofile 1000000" >> /etc/security/limits.conf
    echo "* hard nofile 1000000" >> /etc/security/limits.conf
    echo "root soft nofile 1000000" >> /etc/security/limits.conf
    echo "root hard nofile 1000000" >> /etc/security/limits.conf
fi

# Also set in limits.d for SSH sessions
cat > /etc/security/limits.d/99-loadtest.conf << 'EOF'
* soft nofile 1000000
* hard nofile 1000000
root soft nofile 1000000
root hard nofile 1000000
EOF

# Enable PAM limits for SSH
if ! grep -q "session required pam_limits.so" /etc/pam.d/sshd 2>/dev/null; then
    echo "session required pam_limits.so" >> /etc/pam.d/sshd
fi

# Set DefaultLimitNOFILE for systemd
if ! grep -q "DefaultLimitNOFILE" /etc/systemd/system.conf 2>/dev/null; then
    echo "DefaultLimitNOFILE=1000000" >> /etc/systemd/system.conf
fi

echo ""
echo "NOTE: Reboot or restart SSH daemon for ulimit changes to take effect"
echo "      systemctl restart sshd && exit"

echo ""
echo "=== Tuning Applied ==="
echo "Verify with: sysctl net.core.somaxconn net.ipv4.tcp_tw_reuse"
echo "Note: Reboot or re-login required for ulimit changes to persist"
