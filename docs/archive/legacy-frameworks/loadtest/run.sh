#!/bin/bash
# Main load test orchestration script
# Usage: ./run.sh <provider> <scenario> [action]

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/common/utils.sh"

# Parse arguments
PROVIDER="${1:-}"
SCENARIO="${2:-}"
ACTION="${3:-all}"  # all, provision, deploy, test, decommission

if [ -z "$PROVIDER" ] || [ -z "$SCENARIO" ]; then
    cat <<EOF
${GREEN}Highper Gateway - Load Test Orchestrator${NC}

${CYAN}Usage:${NC}
  $0 <provider> <scenario> [action]

${CYAN}Providers:${NC}
  vultr              Vultr Bare Metal
  hetzner           Hetzner Dedicated
  hetzner-cloud      Hetzner Cloud
  phoenixnap         PhoenixNAP Bare Metal Cloud

${CYAN}Scenarios:${NC}
  scenario-01        Layer 4 TCP Load Balancer
  scenario-02        Layer 7 HTTP + TLS Termination
  scenario-03        Layer 7 HTTP + TLS Passthrough
  scenario-04        API Gateway
  scenario-05        HTTP/3 (QUIC) Multi-Protocol
  scenario-06        WebSocket Load Balancer
  scenario-07        gRPC Gateway
  scenario-08        Database Load Balancer
  scenario-09        Secure API Gateway (WAF + mTLS)
  scenario-10        Hybrid Multi-Protocol
  scenario-11        CDN Edge Proxy (Caching)
  scenario-12        Microservices Gateway
  scenario-13        GraphQL Gateway
  scenario-14        Static Web Server + PHP-FPM
  scenario-15        Geographic Load Balancer

${CYAN}Actions:${NC}
  all                Run full test (provision → deploy → test → decommission)
  provision          Only provision servers
  deploy             Only deploy gateway and backends
  test               Only run load test
  decommission       Only cleanup servers

${CYAN}Examples:${NC}
  # Run full test on Vultr with Layer 7 TLS Termination
  $0 vultr scenario-02

  # Only provision servers on Hetzner
  $0 hetzner scenario-02 provision

  # Run test and cleanup (assumes servers already provisioned)
  $0 vultr scenario-02 test
  $0 vultr scenario-02 decommission

${YELLOW}Before running:${NC}
  1. Copy .env.template to .env
  2. Fill in your API credentials
  3. Review scenario config in configs/scenarios/

EOF
    exit 1
fi

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Check dependencies
check_dependencies

# Generate unique test ID
export TEST_ID=$(generate_test_id)
export TAG="load-test-${TEST_ID}"

log_info "============================================"
log_info "Highper Gateway Load Test Orchestrator"
log_info "============================================"
log_info "Provider:    ${PROVIDER}"
log_info "Scenario:    ${SCENARIO}"
log_info "Action:      ${ACTION}"
log_info "Test ID:     ${TEST_ID}"
log_info "============================================"

# Create results directory
RESULTS_DIR=$(create_results_dir "$TEST_ID")
export RESULTS_DIR

# Save test metadata
save_test_metadata "$TEST_ID" "$PROVIDER" "$SCENARIO" "$RESULTS_DIR"

# Provider script paths
PROVIDER_DIR="${SCRIPT_DIR}/${PROVIDER}"

if [ ! -d "$PROVIDER_DIR" ]; then
    log_error "Provider not supported: ${PROVIDER}"
    log_info "Supported providers: vultr, hetzner, hetzner-cloud, phoenixnap"
    exit 1
fi

PROVISION_SCRIPT="${PROVIDER_DIR}/provision.sh"
DEPLOY_SCRIPT="${PROVIDER_DIR}/deploy.sh"
TEST_SCRIPT="${PROVIDER_DIR}/test.sh"
DECOMMISSION_SCRIPT="${PROVIDER_DIR}/decommission.sh"

# State file
STATE_FILE="${RESULTS_DIR}/${PROVIDER}-servers.json"

# Cleanup function
cleanup() {
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        log_error "Test failed with exit code: $exit_code"

        if [ "${AUTO_CLEANUP_ON_FAILURE:-true}" = "true" ] && [ "${KEEP_SERVERS_FOR_DEBUG:-false}" != "true" ]; then
            log_warn "Running automatic cleanup..."
            if [ -f "$DECOMMISSION_SCRIPT" ] && [ -f "$STATE_FILE" ]; then
                "$DECOMMISSION_SCRIPT" "$TAG" "$STATE_FILE" || true
            fi
        else
            log_warn "Servers are still running. Cleanup manually with:"
            log_warn "  $0 ${PROVIDER} ${SCENARIO} decommission"
        fi
    fi

    exit $exit_code
}

trap cleanup EXIT

# Execute actions
case "$ACTION" in
    all)
        log_info "Running full test lifecycle..."

        # Provision
        if [ -f "$PROVISION_SCRIPT" ]; then
            log_info "Step 1/4: Provisioning servers..."
            "$PROVISION_SCRIPT"
        else
            log_error "Provision script not found: ${PROVISION_SCRIPT}"
            exit 1
        fi

        # Deploy
        if [ -f "$DEPLOY_SCRIPT" ]; then
            log_info "Step 2/4: Deploying gateway and backends..."
            "$DEPLOY_SCRIPT" "$STATE_FILE" "$SCENARIO"
        else
            log_warn "Deploy script not found, skipping deployment"
        fi

        # Test
        if [ -f "$TEST_SCRIPT" ]; then
            log_info "Step 3/4: Running load test..."
            "$TEST_SCRIPT" "$STATE_FILE" "$SCENARIO"
        else
            log_warn "Test script not found, skipping test"
        fi

        # Decommission
        if [ -f "$DECOMMISSION_SCRIPT" ]; then
            if [ "${AUTO_CLEANUP_ON_SUCCESS:-true}" = "true" ]; then
                log_info "Step 4/4: Decommissioning servers..."
                "$DECOMMISSION_SCRIPT" "$TAG" "$STATE_FILE"
            else
                log_warn "Auto-cleanup disabled. Decommission manually with:"
                log_warn "  $0 ${PROVIDER} ${SCENARIO} decommission"
            fi
        else
            log_warn "Decommission script not found"
        fi
        ;;

    provision)
        if [ -f "$PROVISION_SCRIPT" ]; then
            log_info "Provisioning servers..."
            "$PROVISION_SCRIPT"
        else
            log_error "Provision script not found: ${PROVISION_SCRIPT}"
            exit 1
        fi
        ;;

    deploy)
        if [ ! -f "$STATE_FILE" ]; then
            log_error "State file not found: ${STATE_FILE}"
            log_info "Run provision first: $0 ${PROVIDER} ${SCENARIO} provision"
            exit 1
        fi

        if [ -f "$DEPLOY_SCRIPT" ]; then
            log_info "Deploying gateway and backends..."
            "$DEPLOY_SCRIPT" "$STATE_FILE" "$SCENARIO"
        else
            log_error "Deploy script not found: ${DEPLOY_SCRIPT}"
            exit 1
        fi
        ;;

    test)
        if [ ! -f "$STATE_FILE" ]; then
            log_error "State file not found: ${STATE_FILE}"
            log_info "Run provision first: $0 ${PROVIDER} ${SCENARIO} provision"
            exit 1
        fi

        if [ -f "$TEST_SCRIPT" ]; then
            log_info "Running load test..."
            "$TEST_SCRIPT" "$STATE_FILE" "$SCENARIO"
        else
            log_error "Test script not found: ${TEST_SCRIPT}"
            exit 1
        fi
        ;;

    decommission)
        if [ ! -f "$STATE_FILE" ]; then
            log_error "State file not found: ${STATE_FILE}"
            log_info "Nothing to decommission"
            exit 1
        fi

        if [ -f "$DECOMMISSION_SCRIPT" ]; then
            log_info "Decommissioning servers..."
            "$DECOMMISSION_SCRIPT" "$TAG" "$STATE_FILE"
        else
            log_error "Decommission script not found: ${DECOMMISSION_SCRIPT}"
            exit 1
        fi
        ;;

    *)
        log_error "Unknown action: ${ACTION}"
        log_info "Valid actions: all, provision, deploy, test, decommission"
        exit 1
        ;;
esac

log_success "Action '${ACTION}' completed successfully!"
log_info "Results saved to: ${RESULTS_DIR}"

exit 0
