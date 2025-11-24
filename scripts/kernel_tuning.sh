#!/bin/bash
# Kernel tuning script for Highper Gateway - High-performance reverse proxy
#
# This script applies production-grade kernel optimizations for:
# - High connection throughput (500K+ RPS target)
# - Low latency (<0.5ms p50)
# - Reduced TIME_WAIT socket issues
# - TCP Fast Open support
# - BBR congestion control (if available)
#
# Run as root: sudo ./scripts/kernel_tuning.sh

set -e

echo "========================================"
echo "Highper Gateway Kernel Optimization Script"
echo "========================================"
echo ""

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}ERROR: This script must be run as root${NC}"
    echo "Usage: sudo $0"
    exit 1
fi

echo -e "${GREEN}✓${NC} Running as root"
echo ""

# Backup current sysctl settings
BACKUP_FILE="/tmp/sysctl_backup_$(date +%Y%m%d_%H%M%S).conf"
sysctl -a > "$BACKUP_FILE" 2>/dev/null
echo -e "${GREEN}✓${NC} Current settings backed up to: $BACKUP_FILE"
echo ""

echo "Applying kernel optimizations..."
echo "--------------------------------"

# ========================================
# TCP TIME_WAIT Optimization
# ========================================
echo -e "${YELLOW}[1/12]${NC} TCP TIME_WAIT optimization..."

# Reuse TIME_WAIT sockets for new connections (safe in most cases)
sysctl -w net.ipv4.tcp_tw_reuse=1
echo "  ✓ net.ipv4.tcp_tw_reuse = 1"

# Reduce FIN_WAIT timeout from 60s to 10s
sysctl -w net.ipv4.tcp_fin_timeout=10
echo "  ✓ net.ipv4.tcp_fin_timeout = 10s"

# Increase TIME_WAIT buckets to handle more connections
sysctl -w net.ipv4.tcp_max_tw_buckets=400000
echo "  ✓ net.ipv4.tcp_max_tw_buckets = 400000"

# ========================================
# Port Range Expansion
# ========================================
echo -e "${YELLOW}[2/12]${NC} Expanding ephemeral port range..."

# Expand ephemeral port range (default is often 32768-60999)
sysctl -w net.ipv4.ip_local_port_range="10000 65535"
echo "  ✓ net.ipv4.ip_local_port_range = 10000 65535"

# ========================================
# Socket Backlog
# ========================================
echo -e "${YELLOW}[3/12]${NC} Increasing socket backlog..."

# Maximum number of queued connections
sysctl -w net.core.somaxconn=65535
echo "  ✓ net.core.somaxconn = 65535"

# SYN backlog queue size
sysctl -w net.ipv4.tcp_max_syn_backlog=8192
echo "  ✓ net.ipv4.tcp_max_syn_backlog = 8192"

# ========================================
# File Descriptors
# ========================================
echo -e "${YELLOW}[4/12]${NC} Increasing file descriptor limits..."

# Maximum number of file handles
sysctl -w fs.file-max=2097152
echo "  ✓ fs.file-max = 2097152"

# Maximum number of open files per process
sysctl -w fs.nr_open=2097152
echo "  ✓ fs.nr_open = 2097152"

# ========================================
# Connection Tracking
# ========================================
echo -e "${YELLOW}[5/12]${NC} Tuning connection tracking..."

# Increase conntrack table size (if netfilter is used)
if [ -f /proc/sys/net/netfilter/nf_conntrack_max ]; then
    sysctl -w net.netfilter.nf_conntrack_max=1048576
    echo "  ✓ net.netfilter.nf_conntrack_max = 1048576"
else
    echo "  ⚠ Netfilter not available (skipping)"
fi

# ========================================
# TCP Buffer Sizes
# ========================================
echo -e "${YELLOW}[6/12]${NC} Optimizing TCP buffer sizes..."

# Maximum receive buffer size (128MB)
sysctl -w net.core.rmem_max=134217728
echo "  ✓ net.core.rmem_max = 128MB"

# Maximum send buffer size (128MB)
sysctl -w net.core.wmem_max=134217728
echo "  ✓ net.core.wmem_max = 128MB"

# TCP receive buffer: min, default, max (in bytes)
sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
echo "  ✓ net.ipv4.tcp_rmem = 4KB 87KB 128MB"

# TCP send buffer: min, default, max (in bytes)
sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"
echo "  ✓ net.ipv4.tcp_wmem = 4KB 64KB 128MB"

# ========================================
# TCP Fast Open
# ========================================
echo -e "${YELLOW}[7/12]${NC} Enabling TCP Fast Open..."

# TCP Fast Open: 1=client, 2=server, 3=both
# Reduces connection setup by 1 RTT
if sysctl -w net.ipv4.tcp_fastopen=3 2>/dev/null; then
    echo "  ✓ net.ipv4.tcp_fastopen = 3 (client + server)"
else
    echo "  ⚠ TCP Fast Open not supported (requires Linux 3.7+)"
fi

# ========================================
# BBR Congestion Control
# ========================================
echo -e "${YELLOW}[8/12]${NC} Enabling BBR congestion control..."

# Check if BBR is available
if sysctl net.ipv4.tcp_available_congestion_control 2>/dev/null | grep -q bbr; then
    sysctl -w net.ipv4.tcp_congestion_control=bbr
    sysctl -w net.core.default_qdisc=fq
    echo "  ✓ net.ipv4.tcp_congestion_control = bbr"
    echo "  ✓ net.core.default_qdisc = fq (Fair Queue)"
else
    echo "  ⚠ BBR not available (requires Linux 4.9+, check kernel modules)"
    echo "    To enable: modprobe tcp_bbr && echo 'tcp_bbr' >> /etc/modules-load.d/modules.conf"
fi

# ========================================
# TCP Keep-Alive
# ========================================
echo -e "${YELLOW}[9/12]${NC} Tuning TCP keep-alive..."

# Time before sending keep-alive probes (default: 7200s)
sysctl -w net.ipv4.tcp_keepalive_time=60
echo "  ✓ net.ipv4.tcp_keepalive_time = 60s"

# Interval between keep-alive probes (default: 75s)
sysctl -w net.ipv4.tcp_keepalive_intvl=10
echo "  ✓ net.ipv4.tcp_keepalive_intvl = 10s"

# Number of keep-alive probes (default: 9)
sysctl -w net.ipv4.tcp_keepalive_probes=3
echo "  ✓ net.ipv4.tcp_keepalive_probes = 3"

# ========================================
# Network Device Queue
# ========================================
echo -e "${YELLOW}[10/12]${NC} Tuning network device queues..."

# Increase network device backlog
sysctl -w net.core.netdev_max_backlog=16384
echo "  ✓ net.core.netdev_max_backlog = 16384"

# ========================================
# TCP Performance Tuning
# ========================================
echo -e "${YELLOW}[11/12]${NC} Additional TCP performance tuning..."

# Enable TCP window scaling (for high-bandwidth networks)
sysctl -w net.ipv4.tcp_window_scaling=1
echo "  ✓ net.ipv4.tcp_window_scaling = 1"

# Enable TCP timestamps (for RTT measurement)
sysctl -w net.ipv4.tcp_timestamps=1
echo "  ✓ net.ipv4.tcp_timestamps = 1"

# Enable selective acknowledgments
sysctl -w net.ipv4.tcp_sack=1
echo "  ✓ net.ipv4.tcp_sack = 1"

# Disable SYN cookies (we have large backlog)
# SYN cookies can reduce performance
sysctl -w net.ipv4.tcp_syncookies=0
echo "  ✓ net.ipv4.tcp_syncookies = 0 (disabled for performance)"

# ========================================
# Memory Pressure
# ========================================
echo -e "${YELLOW}[12/12]${NC} Tuning memory pressure settings..."

# TCP memory limits (in pages, 4KB each)
# Low threshold, pressure threshold, max
sysctl -w net.ipv4.tcp_mem="786432 1048576 26777216"
echo "  ✓ net.ipv4.tcp_mem = 3GB 4GB 100GB"

echo ""
echo "========================================"
echo -e "${GREEN}✓ All optimizations applied!${NC}"
echo "========================================"
echo ""

# ========================================
# Persistence Instructions
# ========================================
echo "To make these settings permanent:"
echo ""
echo "1. Add to /etc/sysctl.conf or /etc/sysctl.d/99-highper-gateway.conf:"
echo ""
echo "cat > /etc/sysctl.d/99-highper-gateway.conf << 'EOF'"
echo "# Highper Gateway Production Optimizations"
echo ""
echo "# TCP TIME_WAIT optimization"
echo "net.ipv4.tcp_tw_reuse = 1"
echo "net.ipv4.tcp_fin_timeout = 10"
echo "net.ipv4.tcp_max_tw_buckets = 400000"
echo ""
echo "# Port range"
echo "net.ipv4.ip_local_port_range = 10000 65535"
echo ""
echo "# Socket backlog"
echo "net.core.somaxconn = 65535"
echo "net.ipv4.tcp_max_syn_backlog = 8192"
echo ""
echo "# File descriptors"
echo "fs.file-max = 2097152"
echo "fs.nr_open = 2097152"
echo ""
echo "# Connection tracking"
echo "net.netfilter.nf_conntrack_max = 1048576"
echo ""
echo "# TCP buffers"
echo "net.core.rmem_max = 134217728"
echo "net.core.wmem_max = 134217728"
echo "net.ipv4.tcp_rmem = 4096 87380 134217728"
echo "net.ipv4.tcp_wmem = 4096 65536 134217728"
echo ""
echo "# TCP Fast Open"
echo "net.ipv4.tcp_fastopen = 3"
echo ""
echo "# BBR congestion control"
echo "net.ipv4.tcp_congestion_control = bbr"
echo "net.core.default_qdisc = fq"
echo ""
echo "# TCP keep-alive"
echo "net.ipv4.tcp_keepalive_time = 60"
echo "net.ipv4.tcp_keepalive_intvl = 10"
echo "net.ipv4.tcp_keepalive_probes = 3"
echo ""
echo "# Network device queue"
echo "net.core.netdev_max_backlog = 16384"
echo ""
echo "# TCP performance"
echo "net.ipv4.tcp_window_scaling = 1"
echo "net.ipv4.tcp_timestamps = 1"
echo "net.ipv4.tcp_sack = 1"
echo "net.ipv4.tcp_syncookies = 0"
echo ""
echo "# Memory pressure"
echo "net.ipv4.tcp_mem = 786432 1048576 26777216"
echo "EOF"
echo ""
echo "2. Apply on boot:"
echo "   sysctl -p /etc/sysctl.d/99-highper-gateway.conf"
echo ""

# ========================================
# User Limits
# ========================================
echo "Don't forget to set user limits in /etc/security/limits.conf:"
echo ""
echo "*  soft  nofile  1048576"
echo "*  hard  nofile  1048576"
echo "*  soft  nproc   unlimited"
echo "*  hard  nproc   unlimited"
echo ""

echo "========================================"
echo -e "${GREEN}Setup Complete!${NC}"
echo "========================================"
echo ""
echo "Current settings have been applied temporarily."
echo "Reboot or follow the instructions above to make them permanent."
echo ""
echo "To verify settings:"
echo "  sysctl net.ipv4.tcp_tw_reuse"
echo "  sysctl net.ipv4.tcp_fastopen"
echo "  sysctl net.ipv4.tcp_congestion_control"
echo ""

exit 0
