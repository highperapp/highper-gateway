#!/bin/bash
# Highper Gateway Watchdog Script
# Monitors health endpoint and restarts service if unresponsive
#
# This script provides FreeBSD-level reliability through:
# 1. Health check monitoring
# 2. Automatic restart on failure
# 3. Configurable failure thresholds
# 4. Detailed logging for debugging
#
# Usage: ./watchdog.sh
# Or run as systemd service: systemctl start highper-watchdog

set -euo pipefail

# Configuration (can be overridden by environment variables)
HEALTH_URL="${HEALTH_URL:-http://localhost:9090/api/health}"
MAX_FAILURES="${MAX_FAILURES:-3}"
CHECK_INTERVAL="${CHECK_INTERVAL:-10}"
RESTART_DELAY="${RESTART_DELAY:-10}"
LOG_FILE="${LOG_FILE:-/var/log/highper-gateway/watchdog.log}"
SERVICE_NAME="${SERVICE_NAME:-highper-gateway}"

# Counters
failures=0
total_checks=0
total_restarts=0
start_time=$(date +%s)

# Logging function
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Initialize log file
mkdir -p "$(dirname "$LOG_FILE")"
log "INFO" "Watchdog started - Monitoring $HEALTH_URL every ${CHECK_INTERVAL}s"
log "INFO" "Max failures before restart: $MAX_FAILURES"

# Signal handler for graceful shutdown
trap 'log "INFO" "Watchdog received termination signal, shutting down..."; exit 0' SIGTERM SIGINT

# Function to check service health
check_health() {
    local response_code
    local response_time_start
    local response_time_end
    local response_time_ms

    response_time_start=$(date +%s%3N)

    # Perform health check with timeout
    if response_code=$(curl -sf -o /dev/null -w "%{http_code}" --max-time 5 "$HEALTH_URL" 2>/dev/null); then
        response_time_end=$(date +%s%3N)
        response_time_ms=$((response_time_end - response_time_start))

        if [ "$response_code" = "200" ]; then
            log "DEBUG" "Health check OK (${response_time_ms}ms, HTTP $response_code)"
            return 0
        else
            log "WARN" "Health check failed with HTTP $response_code"
            return 1
        fi
    else
        response_time_end=$(date +%s%3N)
        response_time_ms=$((response_time_end - response_time_start))
        log "ERROR" "Health check failed - connection refused or timeout (${response_time_ms}ms)"
        return 1
    fi
}

# Function to restart service
restart_service() {
    log "WARN" "Attempting to restart $SERVICE_NAME (failure count: $failures/$MAX_FAILURES)"

    ((total_restarts++))

    # Check if running under systemd
    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null || ! systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        log "INFO" "Restarting service via systemd: $SERVICE_NAME"
        if systemctl restart "$SERVICE_NAME" 2>&1 | tee -a "$LOG_FILE"; then
            log "INFO" "Service restart successful"

            # Wait for service to start
            sleep "$RESTART_DELAY"

            # Verify service is running
            if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
                log "INFO" "Service is active after restart"
            else
                log "ERROR" "Service failed to start after restart"
            fi
        else
            log "ERROR" "Service restart failed"
        fi
    else
        log "ERROR" "systemd is not available, cannot restart service"
    fi

    # Reset failure counter
    failures=0
}

# Function to log statistics
log_stats() {
    local uptime=$(($(date +%s) - start_time))
    local uptime_hours=$((uptime / 3600))
    local success_rate=0

    if [ $total_checks -gt 0 ]; then
        success_rate=$(echo "scale=2; (($total_checks - $total_restarts * $MAX_FAILURES) / $total_checks) * 100" | bc)
    fi

    log "INFO" "Statistics: Uptime=${uptime_hours}h, Checks=$total_checks, Restarts=$total_restarts, Success=${success_rate}%"
}

# Main monitoring loop
while true; do
    ((total_checks++))

    if check_health; then
        # Health check passed
        failures=0

        # Log statistics every 100 checks
        if [ $((total_checks % 100)) -eq 0 ]; then
            log_stats
        fi
    else
        # Health check failed
        ((failures++))
        log "WARN" "Health check failed ($failures/$MAX_FAILURES)"

        # Check if we've exceeded max failures
        if [ $failures -ge $MAX_FAILURES ]; then
            log "ERROR" "Max failures reached, initiating restart..."
            restart_service
        fi
    fi

    # Sleep until next check
    sleep "$CHECK_INTERVAL"
done
