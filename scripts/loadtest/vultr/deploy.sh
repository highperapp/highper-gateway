#!/bin/bash
# Vultr deployment script - Deploy Highper Gateway and backends

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/../common/utils.sh"

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Parse arguments
STATE_FILE="${1:-}"
SCENARIO="${2:-scenario-02}"

if [ -z "$STATE_FILE" ] || [ ! -f "$STATE_FILE" ]; then
    log_error "State file not found: ${STATE_FILE}"
    log_info "Usage: $0 <state_file> [scenario]"
    exit 1
fi

SCENARIO_CONFIG="${PROJECT_ROOT}/configs/scenarios/${SCENARIO}-layer7-tls-termination.yaml"

if [ ! -f "$SCENARIO_CONFIG" ]; then
    log_warn "Scenario config not found: ${SCENARIO_CONFIG}"
    log_info "Will use default configuration"
fi

log_info "============================================"
log_info "Vultr Deployment"
log_info "============================================"
log_info "State file: ${STATE_FILE}"
log_info "Scenario: ${SCENARIO}"
log_info "============================================"

# Parse state file
PROXY_IP=$(jq -r '.proxy.ip' "$STATE_FILE")
BACKEND_IPS=($(jq -r '.backends[].ip' "$STATE_FILE"))
GENERATOR_IPS=($(jq -r '.generators[].ip' "$STATE_FILE"))

log_info "Proxy IP: ${PROXY_IP}"
log_info "Backend IPs: ${BACKEND_IPS[*]}"
log_info "Generator IPs: ${GENERATOR_IPS[*]}"

# Wait for all SSH to be ready
wait_for_all_ssh() {
    log_info "Waiting for SSH to be ready on all servers..."

    wait_for_ssh "$PROXY_IP" "root" 300 &
    local proxy_pid=$!

    for ip in "${BACKEND_IPS[@]}"; do
        wait_for_ssh "$ip" "root" 300 &
    done

    for ip in "${GENERATOR_IPS[@]}"; do
        wait_for_ssh "$ip" "root" 300 &
    done

    wait
    log_success "All servers are SSH-ready!"
}

# Deploy backend servers (simple HTTP server for testing)
deploy_backends() {
    log_info "Deploying backend servers..."

    for i in "${!BACKEND_IPS[@]}"; do
        local ip="${BACKEND_IPS[$i]}"
        local backend_num=$((i + 1))

        log_info "Deploying backend ${backend_num} (${ip})..."

        ssh_exec "$ip" "apt-get update && apt-get install -y nginx" "root"

        # Create simple test page
        ssh_exec "$ip" "cat > /var/www/html/index.html <<'EOF'
<!DOCTYPE html>
<html>
<head><title>Backend ${backend_num}</title></head>
<body>
<h1>Backend Server ${backend_num}</h1>
<p>IP: ${ip}</p>
<p>Timestamp: \$(date)</p>
</body>
</html>
EOF" "root"

        # Create health check endpoint
        ssh_exec "$ip" "cat > /var/www/html/health <<'EOF'
OK
EOF" "root"

        # Start nginx
        ssh_exec "$ip" "systemctl start nginx && systemctl enable nginx" "root"

        log_success "Backend ${backend_num} deployed"
    done
}

# Deploy Highper Gateway
deploy_gateway() {
    log_info "Deploying Highper Gateway on proxy (${PROXY_IP})..."

    # Install dependencies
    log_info "Installing dependencies..."
    ssh_exec "$PROXY_IP" "apt-get update && apt-get install -y curl wget git build-essential" "root"

    # Install Rust (if needed for building)
    log_info "Checking for Rust installation..."
    if ! ssh_exec "$PROXY_IP" "command -v cargo" "root" 2>/dev/null; then
        log_info "Installing Rust..."
        ssh_exec "$PROXY_IP" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" "root"
        ssh_exec "$PROXY_IP" "source \$HOME/.cargo/env" "root"
    fi

    # Copy gateway binary (if we have a release build)
    if [ -f "${PROJECT_ROOT}/target/release/highper-gateway" ]; then
        log_info "Copying pre-built gateway binary..."
        ssh_copy "${PROJECT_ROOT}/target/release/highper-gateway" "$PROXY_IP" "/usr/local/bin/highper-gateway" "root"
        ssh_exec "$PROXY_IP" "chmod +x /usr/local/bin/highper-gateway" "root"
    else
        log_warn "No release binary found. Gateway must be built on server."
        log_info "Cloning and building gateway (this may take 10-15 minutes)..."

        ssh_exec "$PROXY_IP" "cd /opt && git clone https://github.com/YOUR_REPO/highper-gateway.git || true" "root"
        ssh_exec "$PROXY_IP" "cd /opt/highper-gateway && git pull" "root"
        ssh_exec "$PROXY_IP" "cd /opt/highper-gateway && source \$HOME/.cargo/env && cargo build --release" "root"
        ssh_exec "$PROXY_IP" "cp /opt/highper-gateway/target/release/highper-gateway /usr/local/bin/" "root"
    fi

    # Create configuration
    log_info "Creating gateway configuration..."

    # Generate backend addresses from IPs
    local backend_config=""
    for i in "${!BACKEND_IPS[@]}"; do
        backend_config="${backend_config}    - address: \"${BACKEND_IPS[$i]}:80\"\n      weight: 1\n      max_connections: 1000000\n"
    done

    # Create config file
    ssh_exec "$PROXY_IP" "mkdir -p /etc/highper" "root"

    ssh_exec "$PROXY_IP" "cat > /etc/highper/config.yaml <<'EOFCONFIG'
# Highper Gateway Configuration - Load Test
server:
  listen_address: \"0.0.0.0\"
  listen_port: 8080
  max_connections: 3000000
  connection_timeout: 60s
  keepalive_timeout: 90s

  buffer_pool:
    enabled: true
    buffer_size: 16384
    pool_size: 16777216

load_balancer:
  algorithm: \"round_robin\"
  backends:
${backend_config}
  health_check:
    enabled: true
    interval: 10s
    timeout: 5s
    path: \"/health\"
    healthy_threshold: 2
    unhealthy_threshold: 3

connection_pool:
  enabled: true
  min_idle: 100
  max_idle: 10000
  max_open: 1000000
  idle_timeout: 300s

rate_limit:
  enabled: true
  requests_per_second: 700000
  burst: 100000

observability:
  metrics:
    enabled: true
    prometheus:
      enabled: true
      port: 9090
  logging:
    level: \"info\"
    format: \"json\"

backpressure:
  enabled: true
  max_connections: 3000000
  memory_limit_mb: 49152
EOFCONFIG" "root"

    # Create systemd service
    ssh_exec "$PROXY_IP" "cat > /etc/systemd/system/highper-gateway.service <<'EOFSERVICE'
[Unit]
Description=Highper Gateway
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/highper-gateway --config /etc/highper/config.yaml
Restart=always
RestartSec=5
LimitNOFILE=10000000
LimitNPROC=10000000

[Install]
WantedBy=multi-user.target
EOFSERVICE" "root"

    # Apply kernel tuning
    log_info "Applying kernel tuning..."
    ssh_exec "$PROXY_IP" "cat >> /etc/sysctl.conf <<'EOFSYSCTL'
# High-performance networking
net.core.somaxconn=65535
net.ipv4.tcp_max_syn_backlog=65535
net.core.netdev_max_backlog=65535
net.ipv4.tcp_tw_reuse=1
net.ipv4.tcp_fin_timeout=15
net.ipv4.ip_local_port_range=1024 65535
net.ipv4.tcp_max_tw_buckets=2000000
fs.file-max=10000000
EOFSYSCTL" "root"

    ssh_exec "$PROXY_IP" "sysctl -p" "root"

    # Start gateway
    log_info "Starting Highper Gateway..."
    ssh_exec "$PROXY_IP" "systemctl daemon-reload" "root"
    ssh_exec "$PROXY_IP" "systemctl start highper-gateway" "root"
    ssh_exec "$PROXY_IP" "systemctl enable highper-gateway" "root"

    # Wait for gateway to be ready
    sleep 5

    # Check status
    if ssh_exec "$PROXY_IP" "systemctl is-active highper-gateway" "root" | grep -q "active"; then
        log_success "Highper Gateway is running!"
    else
        log_error "Gateway failed to start"
        ssh_exec "$PROXY_IP" "journalctl -u highper-gateway -n 50" "root"
        return 1
    fi
}

# Install load generators
deploy_generators() {
    log_info "Installing load generators..."

    for i in "${!GENERATOR_IPS[@]}"; do
        local ip="${GENERATOR_IPS[$i]}"
        local gen_num=$((i + 1))

        log_info "Setting up generator ${gen_num} (${ip})..."

        # Install wrk2 and hey
        ssh_exec "$ip" "apt-get update && apt-get install -y build-essential git libssl-dev golang-go" "root"

        # Install wrk2
        ssh_exec "$ip" "cd /opt && git clone https://github.com/giltene/wrk2.git || true" "root"
        ssh_exec "$ip" "cd /opt/wrk2 && make clean && make" "root"
        ssh_exec "$ip" "cp /opt/wrk2/wrk /usr/local/bin/wrk2" "root"

        # Install hey
        ssh_exec "$ip" "go install github.com/rakyll/hey@latest" "root"
        ssh_exec "$ip" "cp /root/go/bin/hey /usr/local/bin/" "root"

        log_success "Generator ${gen_num} ready"
    done
}

# Main execution
main() {
    log_info "Starting deployment process..."

    # Wait for SSH
    wait_for_all_ssh

    # Deploy backends
    deploy_backends

    # Deploy gateway
    deploy_gateway

    # Deploy generators
    deploy_generators

    log_success "============================================"
    log_success "Deployment Complete!"
    log_success "============================================"
    log_info "Gateway: http://${PROXY_IP}:8080"
    log_info "Metrics: http://${PROXY_IP}:9090/metrics"
    log_info ""
    log_info "Backend servers: ${#BACKEND_IPS[@]}"
    for i in "${!BACKEND_IPS[@]}"; do
        log_info "  Backend $((i+1)): http://${BACKEND_IPS[$i]}"
    done
    log_info ""
    log_info "Load generators: ${#GENERATOR_IPS[@]} ready"
    log_success "============================================"

    exit 0
}

# Run main function
main "$@"
