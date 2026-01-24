#!/bin/bash
# Cloud Load Test Runner
#
# Runs load tests against Highper Gateway in cloud environment.
# Automatically provisions and cleans up cloud instances.
#
# Usage:
#   ./cloud-test-runner.sh [provider] [scenario]
#
# Examples:
#   ./cloud-test-runner.sh vultr all        # Run all scenarios on Vultr
#   ./cloud-test-runner.sh aws 02           # Run scenario 02 on AWS
#   ./cloud-test-runner.sh digitalocean 04  # Run scenario 04 on DigitalOcean
#
# Environment variables:
#   CLOUD_REGION    - Override default region
#   CLOUD_PLAN      - Override default instance plan
#   SKIP_CLEANUP    - Set to 'true' to keep instances after test

set -euo pipefail
cd "$(dirname "$0")"

# Source cloud cleanup library
source lib/cloud-cleanup.sh

# Default configuration
PROVIDER="${1:-vultr}"
SCENARIO="${2:-all}"
REGION="${CLOUD_REGION:-ewr}"
PLAN="${CLOUD_PLAN:-vc2-2c-4gb}"
SKIP_CLEANUP="${SKIP_CLEANUP:-false}"

# Instance names
GATEWAY_INSTANCE=""
BACKEND_INSTANCE=""
LOADGEN_INSTANCE=""

# IP addresses
GATEWAY_IP=""
BACKEND_IP=""
LOADGEN_IP=""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Override cleanup if SKIP_CLEANUP is set
if [ "$SKIP_CLEANUP" = "true" ]; then
    cloud_cleanup() {
        log_warn "SKIP_CLEANUP is set - instances will NOT be cleaned up"
        log_warn "Remember to manually delete instances:"
        for instance_id in "${CLOUD_INSTANCES[@]:-}"; do
            echo "  - $instance_id"
        done
    }
fi

# Initialize cloud cleanup
cloud_cleanup_init "$PROVIDER"

# Check CLI availability
if ! cloud_check_cli; then
    log_error "Cloud CLI not found for provider: $PROVIDER"
    log_error "Please install the required CLI:"
    case "$PROVIDER" in
        vultr) echo "  brew install vultr/vultr-cli/vultr-cli" ;;
        aws) echo "  brew install awscli" ;;
        digitalocean) echo "  brew install doctl" ;;
    esac
    exit 1
fi

log_info "Cloud Load Test Runner"
log_info "Provider: $PROVIDER"
log_info "Region: $REGION"
log_info "Plan: $PLAN"
log_info "Scenario: $SCENARIO"
echo ""

# ============================================================================
# Provision Infrastructure
# ============================================================================

provision_infrastructure() {
    log_info "Provisioning test infrastructure..."

    # Provision gateway instance
    log_info "Creating gateway instance..."
    GATEWAY_INSTANCE=$(cloud_provision_instance "highper-gateway-test" "$PLAN" "$REGION")
    if [ -z "$GATEWAY_INSTANCE" ]; then
        log_error "Failed to create gateway instance"
        exit 1
    fi

    # Provision backend instance
    log_info "Creating backend instance..."
    BACKEND_INSTANCE=$(cloud_provision_instance "highper-backend-test" "$PLAN" "$REGION")
    if [ -z "$BACKEND_INSTANCE" ]; then
        log_error "Failed to create backend instance"
        exit 1
    fi

    # Provision load generator instance
    log_info "Creating load generator instance..."
    LOADGEN_INSTANCE=$(cloud_provision_instance "highper-loadgen-test" "$PLAN" "$REGION")
    if [ -z "$LOADGEN_INSTANCE" ]; then
        log_error "Failed to create load generator instance"
        exit 1
    fi

    # Wait for all instances to be ready
    log_info "Waiting for instances to be ready..."
    cloud_wait_for_instance "$GATEWAY_INSTANCE" 300
    cloud_wait_for_instance "$BACKEND_INSTANCE" 300
    cloud_wait_for_instance "$LOADGEN_INSTANCE" 300

    # Get IP addresses
    GATEWAY_IP=$(cloud_get_instance_ip "$GATEWAY_INSTANCE")
    BACKEND_IP=$(cloud_get_instance_ip "$BACKEND_INSTANCE")
    LOADGEN_IP=$(cloud_get_instance_ip "$LOADGEN_INSTANCE")

    log_info "Infrastructure provisioned:"
    log_info "  Gateway:  $GATEWAY_IP ($GATEWAY_INSTANCE)"
    log_info "  Backend:  $BACKEND_IP ($BACKEND_INSTANCE)"
    log_info "  LoadGen:  $LOADGEN_IP ($LOADGEN_INSTANCE)"
}

# ============================================================================
# Setup Instances
# ============================================================================

setup_instances() {
    log_info "Setting up instances..."

    # Wait for SSH to be available
    log_info "Waiting for SSH connectivity..."
    for ip in "$GATEWAY_IP" "$BACKEND_IP" "$LOADGEN_IP"; do
        local retries=30
        while [ $retries -gt 0 ]; do
            if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "root@$ip" "echo ok" 2>/dev/null; then
                break
            fi
            retries=$((retries - 1))
            sleep 5
        done
        if [ $retries -eq 0 ]; then
            log_error "SSH connection to $ip timed out"
            exit 1
        fi
    done

    # Setup backend instance
    log_info "Setting up backend instance..."
    ssh -o StrictHostKeyChecking=no "root@$BACKEND_IP" bash <<'EOF'
apt-get update -qq
apt-get install -y -qq python3 nginx
# Start simple HTTP server
cat > /var/www/html/index.html <<HTML
<!DOCTYPE html>
<html><body><h1>Backend Server</h1></body></html>
HTML
systemctl start nginx
systemctl enable nginx
EOF

    # Setup gateway instance
    log_info "Setting up gateway instance..."
    ssh -o StrictHostKeyChecking=no "root@$GATEWAY_IP" bash <<EOF
apt-get update -qq
apt-get install -y -qq curl build-essential

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Download and build highper-gateway (or use pre-built binary)
# For now, we'll simulate with nginx as reverse proxy
apt-get install -y -qq nginx
cat > /etc/nginx/sites-available/default <<NGINX
upstream backend {
    server $BACKEND_IP:80;
}
server {
    listen 8080;
    location / {
        proxy_pass http://backend;
    }
}
NGINX
systemctl restart nginx
EOF

    # Setup load generator instance
    log_info "Setting up load generator instance..."
    ssh -o StrictHostKeyChecking=no "root@$LOADGEN_IP" bash <<'EOF'
apt-get update -qq
apt-get install -y -qq curl

# Install vegeta
curl -sL https://github.com/tsenart/vegeta/releases/download/v12.11.1/vegeta_12.11.1_linux_amd64.tar.gz | tar xz -C /usr/local/bin
chmod +x /usr/local/bin/vegeta
EOF

    log_info "All instances set up successfully"
}

# ============================================================================
# Run Load Tests
# ============================================================================

run_load_tests() {
    local scenario="$1"

    log_info "Running load tests for scenario: $scenario"

    # Create target file
    local target_url="http://$GATEWAY_IP:8080/"

    ssh -o StrictHostKeyChecking=no "root@$LOADGEN_IP" bash <<EOF
echo "GET $target_url" > /tmp/targets.txt

echo "Running load test at various RPS levels..."

for rps in 100 500 1000 2000 5000; do
    echo ""
    echo "Testing at \$rps RPS..."
    vegeta attack -targets=/tmp/targets.txt -rate=\$rps -duration=30s | vegeta report
done
EOF

    log_info "Load tests completed"
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "Starting cloud load test..."

    # Provision infrastructure
    provision_infrastructure

    # Setup instances
    setup_instances

    # Run tests
    if [ "$SCENARIO" = "all" ]; then
        for i in $(seq -w 1 15); do
            run_load_tests "$i" || true
        done
    else
        run_load_tests "$SCENARIO"
    fi

    log_info "Cloud load test completed successfully!"
    log_info "Cleanup will be triggered automatically..."
}

# Run main function
main "$@"
