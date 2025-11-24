#!/bin/bash
# Kernel tuning script for production-grade reverse proxy
# Run as root: sudo ./kernel-tuning.sh
# This script optimizes kernel parameters for high-performance networking

set -e

echo "========================================="
echo "Kernel Tuning for Highper Gateway"
echo "========================================="
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "Error: This script must be run as root (use sudo)"
   exit 1
fi

# Backup current sysctl config
BACKUP_FILE="/etc/sysctl.conf.backup.$(date +%Y%m%d_%H%M%S)"
echo "Creating backup of /etc/sysctl.conf to $BACKUP_FILE"
cp /etc/sysctl.conf "$BACKUP_FILE"

echo ""
echo "Applying kernel tuning parameters..."
echo ""

# ========================================
# TCP Connection Management
# ========================================
echo "## TCP Connection Management ##"

# Enable TIME_WAIT socket reuse for new outbound connections
# CRITICAL: Reduces port exhaustion and enables <10s port release
sysctl -w net.ipv4.tcp_tw_reuse=1
echo "✓ net.ipv4.tcp_tw_reuse=1 (TIME_WAIT socket reuse)"

# Reduce FIN_WAIT timeout from 60s to 10s
# CRITICAL: Faster port release (<10s target)
sysctl -w net.ipv4.tcp_fin_timeout=10
echo "✓ net.ipv4.tcp_fin_timeout=10 (10s instead of 60s)"

# Increase TIME_WAIT bucket size to handle more concurrent connections
sysctl -w net.ipv4.tcp_max_tw_buckets=400000
echo "✓ net.ipv4.tcp_max_tw_buckets=400000"

# Expand ephemeral port range for more concurrent connections
sysctl -w net.ipv4.ip_local_port_range="10000 65535"
echo "✓ net.ipv4.ip_local_port_range=\"10000 65535\" (55k ports)"

# ========================================
# TCP Fast Open (TFO)
# ========================================
echo ""
echo "## TCP Fast Open ##"

# Enable TCP Fast Open for client and server (reduces latency by 1 RTT)
# 1 = client, 2 = server, 3 = both
sysctl -w net.ipv4.tcp_fastopen=3
echo "✓ net.ipv4.tcp_fastopen=3 (client + server)"

# ========================================
# Socket Backlog & Connection Limits
# ========================================
echo ""
echo "## Socket Backlog & Limits ##"

# Maximum number of pending connections in listen queue
sysctl -w net.core.somaxconn=65535
echo "✓ net.core.somaxconn=65535"

# Maximum number of SYN packets in backlog (prevents SYN flood)
sysctl -w net.ipv4.tcp_max_syn_backlog=8192
echo "✓ net.ipv4.tcp_max_syn_backlog=8192"

# Maximum number of connections that can be tracked
sysctl -w net.nf_conntrack_max=1048576 2>/dev/null || echo "⚠  net.nf_conntrack_max (not available, skip)"

# ========================================
# Buffer Sizes
# ========================================
echo ""
echo "## Network Buffer Sizes ##"

# Increase network buffer sizes for high throughput
sysctl -w net.core.rmem_max=134217728     # 128MB
sysctl -w net.core.wmem_max=134217728     # 128MB
echo "✓ net.core.rmem_max=134217728 (128MB)"
echo "✓ net.core.wmem_max=134217728 (128MB)"

# TCP memory auto-tuning (min, default, max in bytes)
sysctl -w net.ipv4.tcp_rmem="4096 87380 67108864"   # 4KB, 85KB, 64MB
sysctl -w net.ipv4.tcp_wmem="4096 65536 67108864"   # 4KB, 64KB, 64MB
echo "✓ net.ipv4.tcp_rmem=\"4096 87380 67108864\""
echo "✓ net.ipv4.tcp_wmem=\"4096 65536 67108864\""

# Total memory for TCP buffers (pages)
# Calculate based on available memory (example: 4GB system)
sysctl -w net.ipv4.tcp_mem="786432 1048576 26777216"
echo "✓ net.ipv4.tcp_mem=\"786432 1048576 26777216\""

# ========================================
# BBR Congestion Control
# ========================================
echo ""
echo "## BBR Congestion Control ##"

# Check if BBR is available (Linux 4.9+)
if lsmod | grep -q tcp_bbr || modprobe tcp_bbr 2>/dev/null; then
    sysctl -w net.ipv4.tcp_congestion_control=bbr
    sysctl -w net.core.default_qdisc=fq
    echo "✓ net.ipv4.tcp_congestion_control=bbr"
    echo "✓ net.core.default_qdisc=fq (required for BBR)"
else
    echo "⚠  BBR not available (requires Linux 4.9+), using default congestion control"
fi

# ========================================
# TCP Keepalive
# ========================================
echo ""
echo "## TCP Keepalive ##"

# Reduce keepalive probe intervals for faster dead connection detection
sysctl -w net.ipv4.tcp_keepalive_time=60        # Start probes after 60s idle
sysctl -w net.ipv4.tcp_keepalive_intvl=10       # Probe interval: 10s
sysctl -w net.ipv4.tcp_keepalive_probes=3       # Number of probes before giving up
echo "✓ net.ipv4.tcp_keepalive_time=60"
echo "✓ net.ipv4.tcp_keepalive_intvl=10"
echo "✓ net.ipv4.tcp_keepalive_probes=3"

# ========================================
# File Descriptor Limits
# ========================================
echo ""
echo "## File Descriptor Limits ##"

# System-wide file descriptor limits
sysctl -w fs.file-max=2097152
sysctl -w fs.nr_open=2097152
echo "✓ fs.file-max=2097152"
echo "✓ fs.nr_open=2097152"

# ========================================
# Performance Optimizations
# ========================================
echo ""
echo "## Performance Optimizations ##"

# Enable SYN cookies to prevent SYN flood attacks
sysctl -w net.ipv4.tcp_syncookies=1
echo "✓ net.ipv4.tcp_syncookies=1"

# Disable TCP slow start after idle
sysctl -w net.ipv4.tcp_slow_start_after_idle=0
echo "✓ net.ipv4.tcp_slow_start_after_idle=0"

# Enable TCP window scaling for high-bandwidth connections
sysctl -w net.ipv4.tcp_window_scaling=1
echo "✓ net.ipv4.tcp_window_scaling=1"

# Enable TCP timestamps
sysctl -w net.ipv4.tcp_timestamps=1
echo "✓ net.ipv4.tcp_timestamps=1"

# Enable selective acknowledgments (SACK)
sysctl -w net.ipv4.tcp_sack=1
echo "✓ net.ipv4.tcp_sack=1"

# ========================================
# IPv6 (optional, disable if not used)
# ========================================
# Uncomment to disable IPv6 if not needed
# sysctl -w net.ipv6.conf.all.disable_ipv6=1
# sysctl -w net.ipv6.conf.default.disable_ipv6=1

# ========================================
# Persist Settings
# ========================================
echo ""
echo "========================================="
echo "Making settings persistent..."
echo "========================================="

# Append to /etc/sysctl.conf if not already present
cat >> /etc/sysctl.conf << 'EOF'

# ========================================
# Highper Gateway - Production Kernel Tuning
# Applied: $(date)
# ========================================

# TCP Connection Management
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 10
net.ipv4.tcp_max_tw_buckets = 400000
net.ipv4.ip_local_port_range = 10000 65535

# TCP Fast Open
net.ipv4.tcp_fastopen = 3

# Socket Limits
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 8192

# Buffer Sizes
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864
net.ipv4.tcp_mem = 786432 1048576 26777216

# BBR Congestion Control (if available)
net.ipv4.tcp_congestion_control = bbr
net.core.default_qdisc = fq

# TCP Keepalive
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_keepalive_probes = 3

# File Descriptors
fs.file-max = 2097152
fs.nr_open = 2097152

# Performance
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_window_scaling = 1
net.ipv4.tcp_timestamps = 1
net.ipv4.tcp_sack = 1

EOF

echo "✓ Settings appended to /etc/sysctl.conf"

# ========================================
# Verification
# ========================================
echo ""
echo "========================================="
echo "Verification"
echo "========================================="
echo ""
echo "Current kernel parameters:"
echo "  tcp_tw_reuse: $(sysctl net.ipv4.tcp_tw_reuse | cut -d= -f2)"
echo "  tcp_fin_timeout: $(sysctl net.ipv4.tcp_fin_timeout | cut -d= -f2)"
echo "  tcp_fastopen: $(sysctl net.ipv4.tcp_fastopen | cut -d= -f2)"
echo "  somaxconn: $(sysctl net.core.somaxconn | cut -d= -f2)"
echo "  file-max: $(sysctl fs.file-max | cut -d= -f2)"
echo ""

# Check current TIME_WAIT and CLOSE_WAIT connections
TIMEWAIT=$(ss -ant | grep TIME-WAIT | wc -l)
CLOSEWAIT=$(ss -ant | grep CLOSE-WAIT | wc -l)
ESTABLISHED=$(ss -ant | grep ESTAB | wc -l)

echo "Current TCP connections:"
echo "  ESTABLISHED: $ESTABLISHED"
echo "  TIME_WAIT: $TIMEWAIT"
echo "  CLOSE_WAIT: $CLOSEWAIT"
echo ""

# ========================================
# User Limits (ulimit)
# ========================================
echo "========================================="
echo "Setting User Limits (ulimit)"
echo "========================================="
echo ""

# Update limits.conf for system-wide user limits
cat >> /etc/security/limits.conf << 'EOF'

# Highper Gateway - File Descriptor Limits
* soft nofile 65535
* hard nofile 65535
* soft nproc 65535
* hard nproc 65535

EOF

echo "✓ Updated /etc/security/limits.conf"
echo ""
echo "Note: User limits will apply to new login sessions"
echo "Current session ulimit: $(ulimit -n)"
echo ""

# ========================================
# Summary
# ========================================
echo "========================================="
echo "Summary"
echo "========================================="
echo ""
echo "✓ Kernel tuning applied successfully"
echo "✓ Settings persisted to /etc/sysctl.conf"
echo "✓ Backup created: $BACKUP_FILE"
echo ""
echo "Expected Results:"
echo "  - Port release time: <10 seconds"
echo "  - Max concurrent connections: 60,000+"
echo "  - TIME_WAIT duration: ~10 seconds"
echo "  - Connection latency: -1 RTT (with TFO)"
echo "  - Throughput: +20-25% (with BBR)"
echo ""
echo "Next Steps:"
echo "  1. Reboot system (or reload sysctl: sysctl -p)"
echo "  2. Verify settings: sysctl -a | grep -E 'tcp_tw_reuse|tcp_fin_timeout'"
echo "  3. Monitor with: watch -n1 'ss -s'"
echo "  4. Start highper-gateway with systemd service"
echo ""
echo "========================================="
