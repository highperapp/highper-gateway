#!/bin/bash
# Deploy Prometheus + Grafana observability stack on proxy server
# Usage: ./deploy-observability.sh <proxy-ip> <backend-ip-1> <backend-ip-2> <backend-ip-3>

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1" >&2; }

# Arguments
PROXY_IP="${1:-}"
BACKEND_1="${2:-}"
BACKEND_2="${3:-}"
BACKEND_3="${4:-}"

if [ -z "$PROXY_IP" ] || [ -z "$BACKEND_1" ] || [ -z "$BACKEND_2" ] || [ -z "$BACKEND_3" ]; then
    log_error "Usage: $0 <proxy-ip> <backend-1-ip> <backend-2-ip> <backend-3-ip>"
    exit 1
fi

log_info "============================================"
log_info "Deploying Observability Stack"
log_info "============================================"
log_info "Proxy: $PROXY_IP"
log_info "Backends: $BACKEND_1, $BACKEND_2, $BACKEND_3"
log_info "============================================"

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Create temporary directory for modified configs
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

log_info "Preparing configurations..."

# Copy all configs to temp directory
cp -r "$SCRIPT_DIR"/* "$TMP_DIR/"

# Replace placeholders in Prometheus config
log_info "Configuring Prometheus targets..."
sed -i "s/PROXY_IP/localhost/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/BACKEND_1/${BACKEND_1}/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/BACKEND_2/${BACKEND_2}/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/BACKEND_3/${BACKEND_3}/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/GENERATOR_1/generator-1/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/GENERATOR_2/generator-2/g" "$TMP_DIR/prometheus/prometheus.yml"
sed -i "s/GENERATOR_3/generator-3/g" "$TMP_DIR/prometheus/prometheus.yml"

log_success "Configurations prepared"

# Copy to proxy server
log_info "Copying observability stack to proxy server..."
ssh root@"$PROXY_IP" "mkdir -p /opt/highper/observability"
scp -r "$TMP_DIR"/* root@"$PROXY_IP":/opt/highper/observability/

log_success "Files copied to proxy server"

# Install Docker and Docker Compose if needed
log_info "Checking Docker installation..."
if ! ssh root@"$PROXY_IP" "command -v docker" >/dev/null 2>&1; then
    log_warn "Docker not found, installing..."
    ssh root@"$PROXY_IP" "curl -fsSL https://get.docker.com | sh"
    log_success "Docker installed"
else
    log_success "Docker already installed"
fi

if ! ssh root@"$PROXY_IP" "command -v docker-compose" >/dev/null 2>&1; then
    log_warn "Docker Compose not found, installing..."
    ssh root@"$PROXY_IP" "curl -L 'https://github.com/docker/compose/releases/download/v2.23.0/docker-compose-linux-x86_64' -o /usr/local/bin/docker-compose && chmod +x /usr/local/bin/docker-compose"
    log_success "Docker Compose installed"
else
    log_success "Docker Compose already installed"
fi

# Start the observability stack
log_info "Starting observability stack..."
ssh root@"$PROXY_IP" "cd /opt/highper/observability && docker-compose up -d"

# Wait for services to be healthy
log_info "Waiting for services to start..."
sleep 10

# Check service status
log_info "Checking service status..."
ssh root@"$PROXY_IP" "cd /opt/highper/observability && docker-compose ps"

# Test Prometheus
log_info "Testing Prometheus..."
if ssh root@"$PROXY_IP" "curl -sf http://localhost:9091/-/healthy" >/dev/null 2>&1; then
    log_success "Prometheus is healthy"
else
    log_error "Prometheus health check failed"
    exit 1
fi

# Test Grafana
log_info "Testing Grafana..."
sleep 5  # Give Grafana more time to start
if ssh root@"$PROXY_IP" "curl -sf http://localhost:3000/api/health" >/dev/null 2>&1; then
    log_success "Grafana is healthy"
else
    log_warn "Grafana health check failed (may need more time)"
fi

log_success "============================================"
log_success "Observability Stack Deployed Successfully"
log_success "============================================"
log_info "Access URLs:"
log_info "  Grafana:    http://${PROXY_IP}:3000 (admin/highper2025)"
log_info "  Prometheus: http://${PROXY_IP}:9091"
log_info "  Node Exporter: http://${PROXY_IP}:9100/metrics"
log_info "  cAdvisor:   http://${PROXY_IP}:8080"
log_success "============================================"

# Display Prometheus targets
log_info "Prometheus targets:"
ssh root@"$PROXY_IP" "curl -s http://localhost:9091/api/v1/targets | jq -r '.data.activeTargets[] | \"  \\(.job): \\(.health) (\\(.scrapeUrl))\"'" || log_warn "Could not fetch Prometheus targets"

log_success "Deployment complete!"
