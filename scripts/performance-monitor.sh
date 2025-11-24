#!/bin/bash
#
# Performance Monitoring Script for Rust Reverse Proxy
# Monitors key performance metrics in real-time
#
# Usage: ./performance-monitor.sh [interval]
#

set -euo pipefail

INTERVAL=${1:-5}  # Default 5 seconds
PROXY_PORT=${PROXY_PORT:-9090}
METRICS_URL="http://localhost:${PROXY_PORT}/metrics"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check if proxy is running
check_proxy() {
    if ! pgrep -x "highper-gateway" > /dev/null; then
        echo -e "${RED}ERROR: highper-gateway is not running${NC}"
        exit 1
    fi
}

# Get proxy PID
get_pid() {
    pgrep -x "highper-gateway" | head -n1
}

# Get metrics from Prometheus endpoint
get_metric() {
    local metric=$1
    curl -s "$METRICS_URL" 2>/dev/null | grep "^${metric}" | grep -v "^#" | awk '{print $2}' | tail -n1
}

# Get CPU usage
get_cpu_usage() {
    local pid=$(get_pid)
    ps -p "$pid" -o %cpu --no-headers 2>/dev/null | awk '{print $1}' || echo "0"
}

# Get memory usage (RSS in MB)
get_memory_usage() {
    local pid=$(get_pid)
    ps -p "$pid" -o rss --no-headers 2>/dev/null | awk '{print $1/1024}' || echo "0"
}

# Get file descriptor count
get_fd_count() {
    local pid=$(get_pid)
    ls /proc/"$pid"/fd 2>/dev/null | wc -l || echo "0"
}

# Get connection count
get_connection_count() {
    local pid=$(get_pid)
    ss -tan | grep -c "$(echo $pid | xargs -I{} ss -tanp | grep {} | awk '{print $4}' | head -n1 | cut -d: -f2)" 2>/dev/null || echo "0"
}

# Get network bandwidth
get_network_stats() {
    local iface=$(ip route | grep default | awk '{print $5}' | head -n1)
    if [ -n "$iface" ]; then
        cat /sys/class/net/"$iface"/statistics/rx_bytes
        cat /sys/class/net/"$iface"/statistics/tx_bytes
    else
        echo "0"
        echo "0"
    fi
}

# Format bytes to human readable
format_bytes() {
    local bytes=$1
    if [ "$bytes" -lt 1024 ]; then
        echo "${bytes} B"
    elif [ "$bytes" -lt 1048576 ]; then
        echo "$(awk "BEGIN {printf \"%.2f\", $bytes/1024}") KB"
    elif [ "$bytes" -lt 1073741824 ]; then
        echo "$(awk "BEGIN {printf \"%.2f\", $bytes/1048576}") MB"
    else
        echo "$(awk "BEGIN {printf \"%.2f\", $bytes/1073741824}") GB"
    fi
}

# Clear screen and print header
print_header() {
    clear
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║                                                                ║"
    echo "║       Rust Reverse Proxy - Performance Monitor                ║"
    echo "║       Refresh: ${INTERVAL}s | Press Ctrl+C to exit                     ║"
    echo "║                                                                ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    echo ""
}

# Print metric with color coding
print_metric() {
    local label=$1
    local value=$2
    local threshold_warn=${3:-}
    local threshold_crit=${4:-}

    local color=$GREEN
    if [ -n "$threshold_crit" ] && (( $(echo "$value > $threshold_crit" | bc -l) )); then
        color=$RED
    elif [ -n "$threshold_warn" ] && (( $(echo "$value > $threshold_warn" | bc -l) )); then
        color=$YELLOW
    fi

    printf "%-30s ${color}%s${NC}\n" "$label:" "$value"
}

# Calculate rate (per second)
calculate_rate() {
    local current=$1
    local previous=$2
    local interval=$3

    if [ -z "$previous" ] || [ "$previous" = "0" ]; then
        echo "0"
    else
        echo "$(awk "BEGIN {printf \"%.2f\", ($current - $previous) / $interval}")"
    fi
}

# Main monitoring loop
monitor() {
    check_proxy

    local prev_rx_bytes=""
    local prev_tx_bytes=""
    local prev_requests=""

    while true; do
        print_header

        # System metrics
        echo -e "${BLUE}=== System Metrics ===${NC}"
        local cpu=$(get_cpu_usage)
        local mem=$(get_memory_usage)
        local fds=$(get_fd_count)

        print_metric "CPU Usage" "${cpu}%" 70 85
        print_metric "Memory (RSS)" "$(printf '%.2f' $mem) MB" 3072 4096
        print_metric "File Descriptors" "$fds" 50000 60000

        echo ""

        # Network metrics
        echo -e "${BLUE}=== Network Metrics ===${NC}"
        read rx_bytes tx_bytes < <(get_network_stats)

        if [ -n "$prev_rx_bytes" ]; then
            local rx_rate=$(calculate_rate "$rx_bytes" "$prev_rx_bytes" "$INTERVAL")
            local tx_rate=$(calculate_rate "$tx_bytes" "$prev_tx_bytes" "$INTERVAL")

            print_metric "RX Rate" "$(format_bytes ${rx_rate%.*})/s"
            print_metric "TX Rate" "$(format_bytes ${tx_rate%.*})/s"
        else
            print_metric "RX Rate" "Collecting..."
            print_metric "TX Rate" "Collecting..."
        fi

        prev_rx_bytes=$rx_bytes
        prev_tx_bytes=$tx_bytes

        echo ""

        # Proxy metrics (from Prometheus)
        echo -e "${BLUE}=== Proxy Metrics ===${NC}"

        if curl -s "$METRICS_URL" >/dev/null 2>&1; then
            local total_requests=$(get_metric "http_requests_total")

            if [ -n "$prev_requests" ] && [ "$total_requests" != "$prev_requests" ]; then
                local req_rate=$(calculate_rate "${total_requests:-0}" "${prev_requests:-0}" "$INTERVAL")
                print_metric "Request Rate" "$(printf '%.0f' $req_rate) req/s" 8000 9500
            else
                print_metric "Request Rate" "0 req/s"
            fi

            prev_requests=$total_requests

            # Connection pool metrics (if available)
            local pool_active=$(get_metric "connection_pool_active")
            local pool_idle=$(get_metric "connection_pool_idle")

            if [ -n "$pool_active" ]; then
                print_metric "Active Connections" "$pool_active" 400 450
            fi

            if [ -n "$pool_idle" ]; then
                print_metric "Idle Connections" "$pool_idle"
            fi
        else
            print_metric "Metrics Endpoint" "${RED}Unavailable${NC}"
        fi

        echo ""

        # Connection statistics
        echo -e "${BLUE}=== Connection States ===${NC}"
        if command -v ss >/dev/null 2>&1; then
            local established=$(ss -tan state established | grep -c ":8080\|:8443" || echo "0")
            local time_wait=$(ss -tan state time-wait | grep -c ":8080\|:8443" || echo "0")
            local close_wait=$(ss -tan state close-wait | grep -c ":8080\|:8443" || echo "0")

            print_metric "ESTABLISHED" "$established"
            print_metric "TIME_WAIT" "$time_wait" 5000 10000
            print_metric "CLOSE_WAIT" "$close_wait" 100 500
        else
            print_metric "Connection Stats" "ss command not available"
        fi

        echo ""
        echo "Last update: $(date '+%Y-%m-%d %H:%M:%S')"

        sleep "$INTERVAL"
    done
}

# Handle Ctrl+C
trap 'echo ""; echo "Monitoring stopped."; exit 0' INT TERM

# Start monitoring
monitor
