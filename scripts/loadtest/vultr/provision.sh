#!/bin/bash
# Vultr server provisioning script for load testing

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/../common/utils.sh"

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Check API credentials
check_api_credentials "vultr"

# Check dependencies
check_dependencies

# Configuration
VULTR_API_URL="https://api.vultr.com/v2"
VULTR_OS_ID=387  # Ubuntu 22.04 LTS x64
TAG="load-test-${TEST_ID:-$(generate_test_id)}"

# Server tracking file
STATE_FILE="${PROJECT_ROOT}/results/${TAG}/vultr-servers.json"
mkdir -p "$(dirname "$STATE_FILE")"

log_info "============================================"
log_info "Vultr Load Test Provisioning"
log_info "============================================"
log_info "Test ID: ${TAG}"
log_info "Region: ${VULTR_REGION}"
log_info "Proxy Plan: ${VULTR_PROXY_PLAN}"
log_info "Backend Plan: ${VULTR_BACKEND_PLAN}"
log_info "Generator Plan: ${VULTR_GENERATOR_PLAN}"
log_info "============================================"

# Create proxy server
provision_proxy() {
    log_info "Creating proxy server..."

    local response
    response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        -H "Content-Type: application/json" \
        -d "{
            \"region\": \"${VULTR_REGION}\",
            \"plan\": \"${VULTR_PROXY_PLAN}\",
            \"os_id\": ${VULTR_OS_ID},
            \"label\": \"proxy-${TAG}\",
            \"hostname\": \"proxy\",
            \"tag\": \"${TAG}\"
        }")

    local proxy_id
    proxy_id=$(echo "$response" | jq -r '.instance.id // empty')

    if [ -z "$proxy_id" ]; then
        log_error "Failed to create proxy server"
        log_error "API Response: $response"
        return 1
    fi

    log_success "Proxy server created: ${proxy_id}"
    echo "$proxy_id"
}

# Create backend servers
provision_backends() {
    local count=${1:-3}

    log_info "Creating ${count} backend servers..."

    local backend_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating backend server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
            -H "Authorization: Bearer ${VULTR_API_KEY}" \
            -H "Content-Type: application/json" \
            -d "{
                \"region\": \"${VULTR_REGION}\",
                \"plan\": \"${VULTR_BACKEND_PLAN}\",
                \"os_id\": ${VULTR_OS_ID},
                \"label\": \"backend-${i}-${TAG}\",
                \"hostname\": \"backend-${i}\",
                \"tag\": \"${TAG}\"
            }")

        local backend_id
        backend_id=$(echo "$response" | jq -r '.instance.id // empty')

        if [ -z "$backend_id" ]; then
            log_error "Failed to create backend server ${i}"
            log_error "API Response: $response"
            return 1
        fi

        backend_ids+=("$backend_id")
        log_success "Backend server ${i} created: ${backend_id}"
    done

    echo "${backend_ids[@]}"
}

# Create load generator servers
provision_generators() {
    local count=${1:-3}

    log_info "Creating ${count} load generator servers..."

    local generator_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating generator server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
            -H "Authorization: Bearer ${VULTR_API_KEY}" \
            -H "Content-Type: application/json" \
            -d "{
                \"region\": \"${VULTR_REGION}\",
                \"plan\": \"${VULTR_GENERATOR_PLAN}\",
                \"os_id\": ${VULTR_OS_ID},
                \"label\": \"generator-${i}-${TAG}\",
                \"hostname\": \"generator-${i}\",
                \"tag\": \"${TAG}\"
            }")

        local generator_id
        generator_id=$(echo "$response" | jq -r '.instance.id // empty')

        if [ -z "$generator_id" ]; then
            log_error "Failed to create generator server ${i}"
            log_error "API Response: $response"
            return 1
        fi

        generator_ids+=("$generator_id")
        log_success "Generator server ${i} created: ${generator_id}"
    done

    echo "${generator_ids[@]}"
}

# Get server IP address
get_server_ip() {
    local server_id=$1

    local response
    response=$(curl -s "${VULTR_API_URL}/bare-metals/${server_id}" \
        -H "Authorization: Bearer ${VULTR_API_KEY}")

    echo "$response" | jq -r '.bare_metal.main_ip // empty'
}

# Wait for servers to be active
wait_for_servers() {
    log_info "Waiting for servers to be active (this may take 5-10 minutes)..."

    local all_server_ids=("$@")
    local max_wait=900  # 15 minutes
    local elapsed=0
    local check_interval=30

    while [ $elapsed -lt $max_wait ]; do
        local all_active=true

        for server_id in "${all_server_ids[@]}"; do
            local response
            response=$(curl -s "${VULTR_API_URL}/bare-metals/${server_id}" \
                -H "Authorization: Bearer ${VULTR_API_KEY}")

            local status
            status=$(echo "$response" | jq -r '.bare_metal.status // empty')

            if [ "$status" != "active" ]; then
                all_active=false
                break
            fi
        done

        if $all_active; then
            log_success "All servers are active!"
            return 0
        fi

        sleep $check_interval
        elapsed=$((elapsed + check_interval))
        echo -ne "${CYAN}[⏳]${NC} Waiting for servers... ${elapsed}/${max_wait}s\r"
    done

    echo -ne "\033[2K\r"  # Clear line
    log_error "Timeout waiting for servers to become active"
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
  "provider": "vultr",
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
    log_info "Starting provisioning process..."

    # Provision servers
    local proxy_id
    proxy_id=$(provision_proxy)

    local backend_ids
    backend_ids=($(provision_backends "${LOAD_TEST_BACKEND_COUNT:-3}"))

    local generator_ids
    generator_ids=($(provision_generators "${LOAD_TEST_GENERATOR_COUNT:-3}"))

    # Collect all server IDs
    local all_server_ids=("$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}")

    # Wait for all servers to be active
    wait_for_servers "${all_server_ids[@]}"

    # Wait an additional 60 seconds for SSH to be ready
    show_progress 60 "Waiting for SSH initialization"

    # Save state
    save_state "$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}"

    log_success "Provisioning complete! State saved to: ${STATE_FILE}"

    exit 0
}

# Run main function
main "$@"
