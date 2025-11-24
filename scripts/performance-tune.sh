#!/bin/bash
#
# Performance Tuning Script for Rust Reverse Proxy
# Applies OS-level optimizations for high-performance operation
#
# Usage: sudo ./performance-tune.sh [apply|check|revert]
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSCTL_FILE="/etc/sysctl.d/99-highper-gateway.conf"
LIMITS_FILE="/etc/security/limits.d/99-highper-gateway.conf"
BACKUP_DIR="/var/backups/highper-gateway-tuning"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

# Backup existing configuration
backup_config() {
    log_info "Backing up existing configuration..."
    mkdir -p "$BACKUP_DIR"

    if [ -f "$SYSCTL_FILE" ]; then
        cp "$SYSCTL_FILE" "$BACKUP_DIR/sysctl.conf.$(date +%Y%m%d-%H%M%S)"
    fi

    if [ -f "$LIMITS_FILE" ]; then
        cp "$LIMITS_FILE" "$BACKUP_DIR/limits.conf.$(date +%Y%m%d-%H%M%S)"
    fi

    log_info "Backup saved to $BACKUP_DIR"
}

# Apply sysctl tuning
apply_sysctl() {
    log_info "Applying kernel parameters..."

    cat > "$SYSCTL_FILE" << 'EOF'
# Rust Reverse Proxy - Performance Tuning
# Applied: $(date)

# File descriptor limits
fs.file-max = 2097152
fs.nr_open = 2097152

# Network tuning
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65536
net.ipv4.tcp_max_syn_backlog = 65536

# TCP connection handling
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 300
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.tcp_keepalive_intvl = 15

# TCP buffer sizes (bytes)
net.core.rmem_default = 262144
net.core.rmem_max = 16777216
net.core.wmem_default = 262144
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216

# TCP optimization
net.ipv4.tcp_max_tw_buckets = 1440000
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_slow_start_after_idle = 0

# Congestion control (BBR)
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# Local port range
net.ipv4.ip_local_port_range = 10000 65535

# Connection tracking (if using iptables/netfilter)
net.netfilter.nf_conntrack_max = 1048576
net.nf_conntrack_max = 1048576

# Disable IPv6 (optional - uncomment if not using IPv6)
# net.ipv6.conf.all.disable_ipv6 = 1
# net.ipv6.conf.default.disable_ipv6 = 1
EOF

    # Apply sysctl settings
    sysctl -p "$SYSCTL_FILE"

    log_info "Kernel parameters applied"
}

# Apply user limits
apply_limits() {
    log_info "Applying user limits..."

    cat > "$LIMITS_FILE" << 'EOF'
# Rust Reverse Proxy - User Limits
# Applied: $(date)

highper-gateway soft nofile 65536
highper-gateway hard nofile 65536
highper-gateway soft nproc 4096
highper-gateway hard nproc 4096
highper-gateway soft memlock unlimited
highper-gateway hard memlock unlimited
EOF

    log_info "User limits configured"
    log_warn "User limits will take effect on next login"
}

# Configure CPU governor
configure_cpu() {
    log_info "Configuring CPU governor..."

    if [ -d /sys/devices/system/cpu/cpu0/cpufreq ]; then
        echo "performance" | tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor > /dev/null
        log_info "CPU governor set to 'performance'"
    else
        log_warn "CPU frequency scaling not available"
    fi
}

# Configure transparent huge pages
configure_thp() {
    log_info "Configuring Transparent Huge Pages..."

    if [ -f /sys/kernel/mm/transparent_hugepage/enabled ]; then
        echo madvise | tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null
        echo madvise | tee /sys/kernel/mm/transparent_hugepage/defrag > /dev/null
        log_info "THP set to 'madvise'"
    else
        log_warn "Transparent Huge Pages not available"
    fi
}

# Disable unnecessary services
disable_services() {
    log_info "Checking for unnecessary services..."

    # List of services that may impact performance
    SERVICES=(
        "bluetooth"
        "cups"
        "avahi-daemon"
    )

    for service in "${SERVICES[@]}"; do
        if systemctl is-active --quiet "$service" 2>/dev/null; then
            log_warn "Service '$service' is running (consider disabling)"
        fi
    done
}

# Configure network interface
configure_network() {
    log_info "Checking network interface configuration..."

    # Detect primary network interface
    IFACE=$(ip route | grep default | awk '{print $5}' | head -n1)

    if [ -z "$IFACE" ]; then
        log_warn "Could not detect primary network interface"
        return
    fi

    log_info "Primary interface: $IFACE"

    # Check if ethtool is available
    if ! command -v ethtool &> /dev/null; then
        log_warn "ethtool not installed (recommended for network tuning)"
        return
    fi

    # Increase ring buffer sizes
    log_info "Optimizing ring buffers..."
    MAX_RX=$(ethtool -g "$IFACE" 2>/dev/null | grep -A4 "Pre-set" | grep RX: | awk '{print $2}')
    MAX_TX=$(ethtool -g "$IFACE" 2>/dev/null | grep -A4 "Pre-set" | grep TX: | awk '{print $2}' | head -n1)

    if [ -n "$MAX_RX" ] && [ -n "$MAX_TX" ]; then
        ethtool -G "$IFACE" rx "$MAX_RX" tx "$MAX_TX" 2>/dev/null || log_warn "Could not set ring buffers"
        log_info "Ring buffers set to max: RX=$MAX_RX, TX=$MAX_TX"
    fi

    # Enable offloading features
    log_info "Enabling offloading features..."
    ethtool -K "$IFACE" tso on gso on gro on 2>/dev/null || log_warn "Could not enable offloading"
}

# Check current configuration
check_config() {
    log_info "Checking current configuration..."

    echo ""
    echo "=== File Descriptors ==="
    echo "System max: $(cat /proc/sys/fs/file-max)"
    echo "Currently open: $(cat /proc/sys/fs/file-nr | awk '{print $1}')"

    echo ""
    echo "=== Network Configuration ==="
    echo "somaxconn: $(cat /proc/sys/net/core/somaxconn)"
    echo "tcp_max_syn_backlog: $(cat /proc/sys/net/ipv4/tcp_max_syn_backlog)"
    echo "tcp_tw_reuse: $(cat /proc/sys/net/ipv4/tcp_tw_reuse)"
    echo "tcp_congestion_control: $(cat /proc/sys/net/ipv4/tcp_congestion_control)"

    echo ""
    echo "=== TCP Buffers ==="
    echo "tcp_rmem: $(cat /proc/sys/net/ipv4/tcp_rmem)"
    echo "tcp_wmem: $(cat /proc/sys/net/ipv4/tcp_wmem)"

    echo ""
    echo "=== CPU Governor ==="
    if [ -d /sys/devices/system/cpu/cpu0/cpufreq ]; then
        cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor | head -n1
    else
        echo "Not available"
    fi

    echo ""
    echo "=== Transparent Huge Pages ==="
    if [ -f /sys/kernel/mm/transparent_hugepage/enabled ]; then
        cat /sys/kernel/mm/transparent_hugepage/enabled
    else
        echo "Not available"
    fi
}

# Revert to defaults
revert_config() {
    log_warn "Reverting to default configuration..."

    if [ -f "$SYSCTL_FILE" ]; then
        rm "$SYSCTL_FILE"
        log_info "Removed $SYSCTL_FILE"
    fi

    if [ -f "$LIMITS_FILE" ]; then
        rm "$LIMITS_FILE"
        log_info "Removed $LIMITS_FILE"
    fi

    # Reload defaults
    sysctl --system

    log_info "Configuration reverted (reboot recommended)"
}

# Apply all optimizations
apply_all() {
    log_info "Starting performance tuning..."

    backup_config
    apply_sysctl
    apply_limits
    configure_cpu
    configure_thp
    disable_services
    configure_network

    echo ""
    log_info "Performance tuning complete!"
    log_info "Some changes require a reboot to take full effect"
    log_warn "Test thoroughly before deploying to production"
}

# Main script
main() {
    check_root

    case "${1:-}" in
        apply)
            apply_all
            ;;
        check)
            check_config
            ;;
        revert)
            revert_config
            ;;
        *)
            echo "Usage: $0 [apply|check|revert]"
            echo ""
            echo "Commands:"
            echo "  apply   - Apply performance optimizations"
            echo "  check   - Check current configuration"
            echo "  revert  - Revert to default configuration"
            exit 1
            ;;
    esac
}

main "$@"
