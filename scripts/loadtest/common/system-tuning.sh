#!/bin/bash
# Comprehensive system tuning for high-performance load testing
# Applies OS, TCP, network, and resource limit optimizations
# Used on: Proxy (gateway), Backends, Load Generators

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1" >&2; }

# Server role: proxy, backend, or generator
ROLE="${1:-proxy}"

log_info "============================================"
log_info "System Tuning for High-Performance Load Testing"
log_info "Role: ${ROLE}"
log_info "============================================"

# ==============================================================================
# 1. KERNEL NETWORK TUNING
# ==============================================================================
tune_network() {
    log_info "Applying network tuning..."

    cat >> /etc/sysctl.conf <<'EOF'

# ==============================================================================
# High-Performance Network Tuning
# Applied: $(date)
# ==============================================================================

# Connection backlog settings
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.core.netdev_max_backlog = 65535

# TIME_WAIT reduction (critical for high RPS)
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_max_tw_buckets = 2000000

# Port range for outbound connections
net.ipv4.ip_local_port_range = 1024 65535

# TCP keepalive settings
net.ipv4.tcp_keepalive_time = 600
net.ipv4.tcp_keepalive_intvl = 30
net.ipv4.tcp_keepalive_probes = 5

# TCP window scaling
net.ipv4.tcp_window_scaling = 1
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864

# TCP congestion control (BBR for better throughput)
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# Reduce TCP slow start after idle
net.ipv4.tcp_slow_start_after_idle = 0

# SYN cookies for SYN flood protection
net.ipv4.tcp_syncookies = 1

# Increase number of incoming connections
net.core.netdev_budget = 50000
net.core.netdev_budget_usecs = 5000

# Optimize for low latency
net.ipv4.tcp_fastopen = 3

# Connection tracking (if firewall is used)
net.netfilter.nf_conntrack_max = 1000000
net.netfilter.nf_conntrack_tcp_timeout_established = 600

EOF

    sysctl -p >/dev/null 2>&1
    log_success "Network tuning applied"
}

# ==============================================================================
# 2. FILE DESCRIPTOR LIMITS
# ==============================================================================
tune_file_descriptors() {
    log_info "Configuring file descriptor limits..."

    # System-wide file descriptor limit
    cat >> /etc/sysctl.conf <<'EOF'

# File descriptor limits
fs.file-max = 10000000
fs.nr_open = 10000000

EOF

    # Per-process limits
    cat >> /etc/security/limits.conf <<'EOF'

# High-performance load testing limits
*                soft    nofile          10000000
*                hard    nofile          10000000
*                soft    nproc           10000000
*                hard    nproc           10000000
root             soft    nofile          10000000
root             hard    nofile          10000000
root             soft    nproc           10000000
root             hard    nproc           10000000

EOF

    # Session limits
    cat >> /etc/pam.d/common-session <<'EOF'
session required pam_limits.so
EOF

    sysctl -p >/dev/null 2>&1
    log_success "File descriptor limits configured (10M files)"
}

# ==============================================================================
# 3. MEMORY AND SWAP TUNING
# ==============================================================================
tune_memory() {
    log_info "Configuring memory and swap settings..."

    cat >> /etc/sysctl.conf <<'EOF'

# Memory tuning
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5
vm.overcommit_memory = 1

# Transparent Huge Pages (disable for better latency)
# Note: This is set via /sys/kernel/mm/transparent_hugepage/enabled

EOF

    # Disable transparent huge pages for lower latency
    echo never > /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || true
    echo never > /sys/kernel/mm/transparent_hugepage/defrag 2>/dev/null || true

    sysctl -p >/dev/null 2>&1
    log_success "Memory tuning applied"
}

# ==============================================================================
# 4. CPU TUNING
# ==============================================================================
tune_cpu() {
    log_info "Configuring CPU settings..."

    # Set CPU governor to performance mode
    if [ -d /sys/devices/system/cpu/cpu0/cpufreq ]; then
        for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            echo performance > "$cpu" 2>/dev/null || true
        done
        log_success "CPU governor set to performance mode"
    else
        log_warn "CPU frequency scaling not available"
    fi

    # Disable CPU power saving
    cat >> /etc/sysctl.conf <<'EOF'

# CPU tuning
kernel.sched_migration_cost_ns = 5000000

EOF

    sysctl -p >/dev/null 2>&1
}

# ==============================================================================
# 5. DISK I/O TUNING
# ==============================================================================
tune_disk_io() {
    log_info "Configuring disk I/O settings..."

    # Set I/O scheduler to deadline or none for SSDs
    for disk in /sys/block/sd*/queue/scheduler; do
        if [ -f "$disk" ]; then
            echo deadline > "$disk" 2>/dev/null || echo none > "$disk" 2>/dev/null || true
        fi
    done

    for disk in /sys/block/nvme*/queue/scheduler; do
        if [ -f "$disk" ]; then
            echo none > "$disk" 2>/dev/null || true
        fi
    done

    log_success "Disk I/O scheduler configured"
}

# ==============================================================================
# 6. ROLE-SPECIFIC TUNING
# ==============================================================================
tune_role_specific() {
    case "$ROLE" in
        proxy)
            log_info "Applying proxy-specific tuning..."

            cat >> /etc/sysctl.conf <<'EOF'

# Proxy-specific tuning
net.ipv4.tcp_max_orphans = 262144
net.ipv4.tcp_timestamps = 1
net.ipv4.tcp_sack = 1

# Connection tracking for proxy
net.netfilter.nf_conntrack_max = 2000000

EOF
            ;;

        backend)
            log_info "Applying backend-specific tuning..."

            cat >> /etc/sysctl.conf <<'EOF'

# Backend-specific tuning
net.ipv4.tcp_timestamps = 1
net.ipv4.tcp_sack = 1

EOF
            ;;

        generator)
            log_info "Applying load generator-specific tuning..."

            cat >> /etc/sysctl.conf <<'EOF'

# Load generator tuning (optimize for creating many connections)
net.ipv4.tcp_timestamps = 1
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_max_orphans = 524288

# Aggressive local port usage
net.ipv4.ip_local_port_range = 1024 65535

EOF
            ;;
    esac

    sysctl -p >/dev/null 2>&1
    log_success "Role-specific tuning applied for: ${ROLE}"
}

# ==============================================================================
# 7. IRQ AFFINITY (Pin interrupts to specific CPUs)
# ==============================================================================
tune_irq_affinity() {
    log_info "Configuring IRQ affinity..."

    # This is advanced tuning - distribute network IRQs across CPUs
    # Skip if irqbalance is handling this
    if systemctl is-active --quiet irqbalance; then
        log_info "irqbalance is running, skipping manual IRQ tuning"
    else
        log_warn "irqbalance not running - consider enabling for better distribution"
    fi
}

# ==============================================================================
# 8. SYSTEM RESOURCE LIMITS
# ==============================================================================
tune_system_limits() {
    log_info "Configuring system resource limits..."

    cat >> /etc/systemd/system.conf <<'EOF'

[Manager]
DefaultLimitNOFILE=10000000
DefaultLimitNPROC=10000000

EOF

    systemctl daemon-reexec 2>/dev/null || true
    log_success "System resource limits configured"
}

# ==============================================================================
# 9. VERIFICATION
# ==============================================================================
verify_tuning() {
    log_info "Verifying tuning settings..."

    local errors=0

    # Check key settings
    local file_max=$(sysctl -n fs.file-max)
    if [ "$file_max" -ge 10000000 ]; then
        log_success "fs.file-max: ${file_max}"
    else
        log_error "fs.file-max: ${file_max} (expected >= 10000000)"
        errors=$((errors + 1))
    fi

    local somaxconn=$(sysctl -n net.core.somaxconn)
    if [ "$somaxconn" -ge 65535 ]; then
        log_success "net.core.somaxconn: ${somaxconn}"
    else
        log_warn "net.core.somaxconn: ${somaxconn} (expected >= 65535)"
    fi

    local tw_reuse=$(sysctl -n net.ipv4.tcp_tw_reuse)
    if [ "$tw_reuse" = "1" ]; then
        log_success "net.ipv4.tcp_tw_reuse: enabled"
    else
        log_warn "net.ipv4.tcp_tw_reuse: disabled"
    fi

    local congestion=$(sysctl -n net.ipv4.tcp_congestion_control)
    log_info "TCP congestion control: ${congestion}"

    if [ $errors -gt 0 ]; then
        log_error "Some tuning verifications failed"
        return 1
    fi

    log_success "All tuning verified successfully"
}

# ==============================================================================
# 10. SUMMARY REPORT
# ==============================================================================
print_summary() {
    log_info ""
    log_info "============================================"
    log_info "System Tuning Summary"
    log_info "============================================"
    log_info "Role: ${ROLE}"
    log_info ""
    log_info "Key Settings:"
    log_info "  File descriptors: $(sysctl -n fs.file-max)"
    log_info "  Connection backlog: $(sysctl -n net.core.somaxconn)"
    log_info "  TIME_WAIT reuse: $(sysctl -n net.ipv4.tcp_tw_reuse)"
    log_info "  TIME_WAIT timeout: $(sysctl -n net.ipv4.tcp_fin_timeout)s"
    log_info "  Port range: $(sysctl -n net.ipv4.ip_local_port_range)"
    log_info "  TCP congestion: $(sysctl -n net.ipv4.tcp_congestion_control)"
    log_info "  Max TW buckets: $(sysctl -n net.ipv4.tcp_max_tw_buckets)"
    log_info ""
    log_success "System is optimized for high-performance load testing"
    log_info "Reboot recommended for all settings to take full effect"
    log_info "============================================"
}

# ==============================================================================
# MAIN EXECUTION
# ==============================================================================
main() {
    # Check if running as root
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root"
        exit 1
    fi

    # Backup original sysctl.conf
    if [ ! -f /etc/sysctl.conf.backup ]; then
        cp /etc/sysctl.conf /etc/sysctl.conf.backup
        log_info "Backed up original sysctl.conf"
    fi

    # Apply tuning
    tune_network
    tune_file_descriptors
    tune_memory
    tune_cpu
    tune_disk_io
    tune_role_specific
    tune_irq_affinity
    tune_system_limits

    # Verify
    verify_tuning

    # Summary
    print_summary

    log_success "System tuning complete!"
}

# Run main function
main "$@"
