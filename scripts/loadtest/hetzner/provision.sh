#!/bin/bash
# Hetzner Dedicated Server provisioning via Robot API

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/../common/utils.sh"

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Check API credentials
check_api_credentials "hetzner-dedicated"

# Check dependencies
check_dependencies

# Configuration
HETZNER_API_URL="https://robot-ws.your-server.de"
TAG="load-test-${TEST_ID:-$(generate_test_id)}"

# Server tracking file
STATE_FILE="${PROJECT_ROOT}/results/${TAG}/hetzner-servers.json"
mkdir -p "$(dirname "$STATE_FILE")"

log_info "============================================"
log_info "Hetzner Dedicated Server Provisioning"
log_info "============================================"
log_info "Test ID: ${TAG}"
log_info "Location: ${HETZNER_DEDICATED_LOCATION}"
log_info "============================================"

# Note: Hetzner dedicated servers need to be ordered through Server Auction
# or pre-existing servers. This script manages existing servers.

log_warn "============================================"
log_warn "HETZNER DEDICATED SERVER PROVISIONING"
log_warn "============================================"
log_warn "Hetzner dedicated servers cannot be provisioned via API."
log_warn "You need to either:"
log_warn "  1. Order servers via Server Auction (https://www.hetzner.com/sb)"
log_warn "  2. Use existing servers in your account"
log_warn "  3. Use Hetzner Cloud instead (fast API provisioning)"
log_warn ""
log_warn "For automated provisioning, use:"
log_warn "  ./scripts/loadtest/run.sh hetzner-cloud scenario-02"
log_warn "============================================"

# List available servers
list_servers() {
    log_info "Listing available servers in your account..."

    local response
    response=$(curl -s -u "${HETZNER_ROBOT_USER}:${HETZNER_ROBOT_PASSWORD}" \
        "${HETZNER_API_URL}/server")

    echo "$response" | jq -r '.[] | "\(.server.server_ip) - \(.server.product) - \(.server.dc)"'
}

# For now, use Hetzner Cloud which has instant API provisioning
log_info "Switching to Hetzner Cloud for instant provisioning..."
log_info "Hetzner Cloud has:"
log_info "  - Instant API provisioning (30-60 seconds)"
log_info "  - Hourly billing"
log_info "  - CCX instances (dedicated vCPU)"
log_info ""

# Use Hetzner Cloud API instead
HETZNER_CLOUD_API="https://api.hetzner.cloud/v1"

log_info "Creating servers via Hetzner Cloud API..."

# Create proxy server (CCX63: 48 vCPU, 192GB RAM)
provision_proxy() {
    log_info "Creating proxy server (CCX63)..."

    local response
    response=$(curl -s -X POST "${HETZNER_CLOUD_API}/servers" \
        -H "Authorization: Bearer ${HETZNER_CLOUD_TOKEN}" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"proxy-${TAG}\",
            \"server_type\": \"ccx63\",
            \"location\": \"${HETZNER_CLOUD_LOCATION}\",
            \"image\": \"ubuntu-22.04\",
            \"labels\": {
                \"purpose\": \"load-test\",
                \"tag\": \"${TAG}\"
            }
        }")

    local server_id
    server_id=$(echo "$response" | jq -r '.server.id // empty')

    if [ -z "$server_id" ]; then
        log_error "Failed to create proxy server"
        log_debug "Response: $response"
        return 1
    fi

    log_success "Proxy server created: ${server_id}"
    echo "$server_id"
}

# Create backend servers
provision_backends() {
    local count=${1:-3}

    log_info "Creating ${count} backend servers (CCX33)..."

    local backend_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating backend server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${HETZNER_CLOUD_API}/servers" \
            -H "Authorization: Bearer ${HETZNER_CLOUD_TOKEN}" \
            -H "Content-Type: application/json" \
            -d "{
                \"name\": \"backend-${i}-${TAG}\",
                \"server_type\": \"ccx33\",
                \"location\": \"${HETZNER_CLOUD_LOCATION}\",
                \"image\": \"ubuntu-22.04\",
                \"labels\": {
                    \"purpose\": \"load-test\",
                    \"tag\": \"${TAG}\"
                }
            }")

        local server_id
        server_id=$(echo "$response" | jq -r '.server.id // empty')

        if [ -z "$server_id" ]; then
            log_error "Failed to create backend server ${i}"
            log_debug "Response: $response"
            return 1
        fi

        backend_ids+=("$server_id")
        log_success "Backend server ${i} created: ${server_id}"
    done

    echo "${backend_ids[@]}"
}

# Create load generator servers
provision_generators() {
    local count=${1:-3}

    log_info "Creating ${count} generator servers (CCX43)..."

    local generator_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating generator server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${HETZNER_CLOUD_API}/servers" \
            -H "Authorization: Bearer ${HETZNER_CLOUD_TOKEN}" \
            -H "Content-Type: application/json" \
            -d "{
                \"name\": \"generator-${i}-${TAG}\",
                \"server_type\": \"ccx43\",
                \"location\": \"${HETZNER_CLOUD_LOCATION}\",
                \"image\": \"ubuntu-22.04\",
                \"labels\": {
                    \"purpose\": \"load-test\",
                    \"tag\": \"${TAG}\"
                }
            }")

        local server_id
        server_id=$(echo "$response" | jq -r '.server.id // empty')

        if [ -z "$server_id" ]; then
            log_error "Failed to create generator server ${i}"
            log_debug "Response: $response"
            return 1
        fi

        generator_ids+=("$server_id")
        log_success "Generator server ${i} created: ${server_id}"
    done

    echo "${generator_ids[@]}"
}

# Get server IP
get_server_ip() {
    local server_id=$1

    local response
    response=$(curl -s "${HETZNER_CLOUD_API}/servers/${server_id}" \
        -H "Authorization: Bearer ${HETZNER_CLOUD_TOKEN}")

    echo "$response" | jq -r '.server.public_net.ipv4.ip // empty'
}

# Wait for servers to be running
wait_for_servers() {
    log_info "Waiting for servers to be running (usually 30-60 seconds)..."

    local all_server_ids=("$@")
    local max_wait=300
    local elapsed=0
    local check_interval=10

    while [ $elapsed -lt $max_wait ]; do
        local all_running=true

        for server_id in "${all_server_ids[@]}"; do
            local response
            response=$(curl -s "${HETZNER_CLOUD_API}/servers/${server_id}" \
                -H "Authorization: Bearer ${HETZNER_CLOUD_TOKEN}")

            local status
            status=$(echo "$response" | jq -r '.server.status // empty')

            if [ "$status" != "running" ]; then
                all_running=false
                break
            fi
        done

        if $all_running; then
            log_success "All servers are running!"
            return 0
        fi

        sleep $check_interval
        elapsed=$((elapsed + check_interval))
        echo -ne "${CYAN}[⏳]${NC} Waiting for servers... ${elapsed}/${max_wait}s\r"
    done

    echo -ne "\033[2K\r"
    log_error "Timeout waiting for servers"
    return 1
}

# Save server state
save_state() {
    local proxy_id=$1
    shift
    local backend_ids=("$@")
    local backend_count=${LOAD_TEST_BACKEND_COUNT:-3}
    local generator_ids=("${backend_ids[@]:backend_count}")
    backend_ids=("${backend_ids[@]:0:backend_count}")

    log_info "Getting server IP addresses..."

    local proxy_ip
    proxy_ip=$(get_server_ip "$proxy_id")

    local backend_ips=()
    for backend_id in "${backend_ids[@]}"; do
        backend_ips+=($(get_server_ip "$backend_id"))
    done

    local generator_ips=()
    for generator_id in "${generator_ids[@]}"; do
        generator_ips+=($(get_server_ip "$generator_id"))
    done

    # Create JSON state
    cat > "$STATE_FILE" <<EOF
{
  "tag": "${TAG}",
  "provider": "hetzner",
  "proxy": {
    "id": "${proxy_id}",
    "ip": "${proxy_ip}"
  },
  "backends": [
$(for i in "${!backend_ids[@]}"; do
    echo "    {\"id\": \"${backend_ids[$i]}\", \"ip\": \"${backend_ips[$i]}\"}"
    [ $i -lt $((${#backend_ids[@]} - 1)) ] && echo ","
done)
  ],
  "generators": [
$(for i in "${!generator_ids[@]}"; do
    echo "    {\"id\": \"${generator_ids[$i]}\", \"ip\": \"${generator_ips[$i]}\"}"
    [ $i -lt $((${#generator_ids[@]} - 1)) ] && echo ","
done)
  ]
}
EOF

    log_success "Server state saved to: ${STATE_FILE}"

    # Print summary
    log_info ""
    log_info "============================================"
    log_info "Server Provisioning Complete!"
    log_info "============================================"
    log_info "Proxy:      ${proxy_ip} (${proxy_id})"
    log_info "Backends:   ${#backend_ids[@]} servers"
    for i in "${!backend_ips[@]}"; do
        log_info "  Backend $((i+1)): ${backend_ips[$i]}"
    done
    log_info "Generators: ${#generator_ids[@]} servers"
    for i in "${!generator_ips[@]}"; do
        log_info "  Generator $((i+1)): ${generator_ips[$i]}"
    done
    log_info "============================================"
}

# Main execution
main() {
    log_info "Starting Hetzner Cloud provisioning..."

    # Provision servers
    local proxy_id
    proxy_id=$(provision_proxy)

    local backend_ids
    backend_ids=($(provision_backends "${LOAD_TEST_BACKEND_COUNT:-3}"))

    local generator_ids
    generator_ids=($(provision_generators "${LOAD_TEST_GENERATOR_COUNT:-3}"))

    # Collect all server IDs
    local all_server_ids=("$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}")

    # Wait for all servers to be running
    wait_for_servers "${all_server_ids[@]}"

    # Wait for SSH to be ready
    show_progress 60 "Waiting for SSH initialization"

    # Save state
    save_state "$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}"

    log_success "Provisioning complete! State saved to: ${STATE_FILE}"

    exit 0
}

# Run main function
main "$@"
