#!/bin/bash
# Highper Gateway - Health Check Script
# Use for monitoring, alerting, and load balancer health probes

set -euo pipefail

# Configuration
HEALTH_PORT="${HEALTH_PORT:-8081}"
HEALTH_ENDPOINT="${HEALTH_ENDPOINT:-/health}"
METRICS_PORT="${METRICS_PORT:-9090}"
METRICS_ENDPOINT="${METRICS_ENDPOINT:-/metrics}"
TIMEOUT="${TIMEOUT:-5}"
VERBOSE="${VERBOSE:-false}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Health check script for Highper Gateway

OPTIONS:
    -h, --help          Show this help message
    -v, --verbose       Enable verbose output
    -a, --all           Run all checks
    -H, --health        Check health endpoint only
    -m, --metrics       Check metrics endpoint only
    -s, --service       Check systemd service status
    -c, --connections   Check active connections
    -p, --port PORT     Health check port (default: 8081)
    -t, --timeout SEC   Request timeout (default: 5)
    --json              Output in JSON format

EXAMPLES:
    $0 --health         # Quick health check
    $0 --all --verbose  # Full diagnostic
    $0 --json           # JSON output for monitoring
EOF
    exit 0
}

log_info() {
    [[ "$VERBOSE" == "true" ]] && echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

# Check health endpoint
check_health() {
    local url="http://localhost:${HEALTH_PORT}${HEALTH_ENDPOINT}"
    local response
    local http_code

    if response=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout "$TIMEOUT" "$url" 2>/dev/null); then
        if [[ "$response" == "200" ]]; then
            log_info "Health endpoint: OK"
            return 0
        else
            log_error "Health endpoint returned HTTP $response"
            return 1
        fi
    else
        log_error "Health endpoint unreachable"
        return 1
    fi
}

# Check metrics endpoint
check_metrics() {
    local url="http://localhost:${METRICS_PORT}${METRICS_ENDPOINT}"
    local response

    if response=$(curl -s --connect-timeout "$TIMEOUT" "$url" 2>/dev/null | head -5); then
        if [[ -n "$response" ]]; then
            log_info "Metrics endpoint: OK"
            return 0
        else
            log_error "Metrics endpoint returned empty response"
            return 1
        fi
    else
        log_error "Metrics endpoint unreachable"
        return 1
    fi
}

# Check systemd service
check_service() {
    if systemctl is-active --quiet highper-gateway 2>/dev/null; then
        log_info "Service status: running"
        return 0
    else
        log_error "Service status: not running"
        return 1
    fi
}

# Check active connections
check_connections() {
    local count
    count=$(ss -tnp 2>/dev/null | grep -c highper-gateway || echo "0")
    log_info "Active connections: $count"
    echo "$count"
}

# Get process info
get_process_info() {
    local pid
    pid=$(pgrep -f highper-gateway 2>/dev/null | head -1 || echo "")

    if [[ -n "$pid" ]]; then
        local mem rss cpu
        read -r mem rss cpu <<< $(ps -p "$pid" -o %mem,rss,%cpu --no-headers 2>/dev/null || echo "0 0 0")
        echo "PID: $pid, Memory: ${mem}%, RSS: ${rss}KB, CPU: ${cpu}%"
        return 0
    else
        log_error "Process not found"
        return 1
    fi
}

# Check io_uring status
check_iouring() {
    if [[ -f /proc/sys/kernel/io_uring_disabled ]]; then
        local disabled
        disabled=$(cat /proc/sys/kernel/io_uring_disabled)
        if [[ "$disabled" == "0" ]]; then
            log_info "io_uring: enabled"
            return 0
        else
            log_warn "io_uring: disabled"
            return 1
        fi
    else
        log_info "io_uring: available (legacy kernel)"
        return 0
    fi
}

# JSON output
output_json() {
    local health_ok=false
    local metrics_ok=false
    local service_ok=false
    local connections=0

    check_health &>/dev/null && health_ok=true
    check_metrics &>/dev/null && metrics_ok=true
    check_service &>/dev/null && service_ok=true
    connections=$(check_connections 2>/dev/null | tail -1)

    cat << EOF
{
  "timestamp": "$(date -Iseconds)",
  "status": $([ "$health_ok" = true ] && [ "$service_ok" = true ] && echo '"healthy"' || echo '"unhealthy"'),
  "checks": {
    "health_endpoint": $health_ok,
    "metrics_endpoint": $metrics_ok,
    "service_running": $service_ok
  },
  "metrics": {
    "active_connections": $connections
  }
}
EOF
}

# Run all checks
run_all_checks() {
    local exit_code=0

    echo "Highper Gateway Health Check"
    echo "============================"
    echo ""

    echo "Service Status:"
    check_service || exit_code=1
    get_process_info || true
    echo ""

    echo "Endpoints:"
    check_health || exit_code=1
    check_metrics || exit_code=1
    echo ""

    echo "System:"
    check_iouring || true
    check_connections || true
    echo ""

    return $exit_code
}

# Parse arguments
COMMAND="health"
JSON_OUTPUT=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -a|--all)
            COMMAND="all"
            shift
            ;;
        -H|--health)
            COMMAND="health"
            shift
            ;;
        -m|--metrics)
            COMMAND="metrics"
            shift
            ;;
        -s|--service)
            COMMAND="service"
            shift
            ;;
        -c|--connections)
            COMMAND="connections"
            shift
            ;;
        -p|--port)
            HEALTH_PORT="$2"
            shift 2
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        --json)
            JSON_OUTPUT=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Run command
if [[ "$JSON_OUTPUT" == "true" ]]; then
    output_json
    exit 0
fi

case $COMMAND in
    health)
        check_health
        ;;
    metrics)
        check_metrics
        ;;
    service)
        check_service
        ;;
    connections)
        check_connections
        ;;
    all)
        VERBOSE=true
        run_all_checks
        ;;
esac
