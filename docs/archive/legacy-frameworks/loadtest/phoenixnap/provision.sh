#!/bin/bash
# PhoenixNAP Bare Metal Cloud provisioning script

set -euo pipefail

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Load common utilities
source "${SCRIPT_DIR}/../common/utils.sh"

# Load environment
load_env "${PROJECT_ROOT}/.env"

# Check API credentials
check_api_credentials "phoenixnap"

# Check dependencies
check_dependencies

# Configuration
PNAP_AUTH_URL="https://auth.phoenixnap.com/auth/realms/BMC/protocol/openid-connect/token"
PNAP_API_URL="https://api.phoenixnap.com/bmc/v1"
TAG="load-test-${TEST_ID:-$(generate_test_id)}"

# Server tracking file
STATE_FILE="${PROJECT_ROOT}/results/${TAG}/phoenixnap-servers.json"
mkdir -p "$(dirname "$STATE_FILE")"

log_info "============================================"
log_info "PhoenixNAP Bare Metal Cloud Provisioning"
log_info "============================================"
log_info "Test ID: ${TAG}"
log_info "Location: ${PNAP_LOCATION}"
log_info "============================================"

# Get OAuth2 access token
get_access_token() {
    log_debug "Obtaining OAuth2 access token..."

    local response
    response=$(curl -s -X POST "${PNAP_AUTH_URL}" \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "grant_type=client_credentials" \
        -d "client_id=${PNAP_CLIENT_ID}" \
        -d "client_secret=${PNAP_CLIENT_SECRET}")

    local token
    token=$(echo "$response" | jq -r '.access_token // empty')

    if [ -z "$token" ]; then
        log_error "Failed to obtain access token"
        log_debug "Response: $response"
        return 1
    fi

    log_debug "Access token obtained successfully"
    echo "$token"
}

# Create proxy server
provision_proxy() {
    local access_token=$1

    log_info "Creating proxy server (s3.c3.large)..."

    local response
    response=$(curl -s -X POST "${PNAP_API_URL}/servers" \
        -H "Authorization: Bearer ${access_token}" \
        -H "Content-Type: application/json" \
        -d "{
            \"hostname\": \"proxy-${TAG}\",
            \"description\": \"Highper Gateway Proxy\",
            \"os\": \"ubuntu/bionic\",
            \"type\": \"s3.c3.large\",
            \"location\": \"${PNAP_LOCATION}\",
            \"installDefaultSshKeys\": true,
            \"tags\": [{\"name\": \"load-test\", \"value\": \"${TAG}\"}]
        }")

    local server_id
    server_id=$(echo "$response" | jq -r '.id // empty')

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
    local access_token=$1
    local count=${2:-3}

    log_info "Creating ${count} backend servers (s3.c1.medium)..."

    local backend_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating backend server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${PNAP_API_URL}/servers" \
            -H "Authorization: Bearer ${access_token}" \
            -H "Content-Type: application/json" \
            -d "{
                \"hostname\": \"backend-${i}-${TAG}\",
                \"description\": \"Backend Server ${i}\",
                \"os\": \"ubuntu/bionic\",
                \"type\": \"s3.c1.medium\",
                \"location\": \"${PNAP_LOCATION}\",
                \"installDefaultSshKeys\": true,
                \"tags\": [{\"name\": \"load-test\", \"value\": \"${TAG}\"}]
            }")

        local server_id
        server_id=$(echo "$response" | jq -r '.id // empty')

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
    local access_token=$1
    local count=${2:-3}

    log_info "Creating ${count} generator servers (s3.c2.medium)..."

    local generator_ids=()

    for i in $(seq 1 "$count"); do
        log_info "Creating generator server ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${PNAP_API_URL}/servers" \
            -H "Authorization: Bearer ${access_token}" \
            -H "Content-Type: application/json" \
            -d "{
                \"hostname\": \"generator-${i}-${TAG}\",
                \"description\": \"Load Generator ${i}\",
                \"os\": \"ubuntu/bionic\",
                \"type\": \"s3.c2.medium\",
                \"location\": \"${PNAP_LOCATION}\",
                \"installDefaultSshKeys\": true,
                \"tags\": [{\"name\": \"load-test\", \"value\": \"${TAG}\"}]
            }")

        local server_id
        server_id=$(echo "$response" | jq -r '.id // empty')

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

# Get server IP address
get_server_ip() {
    local access_token=$1
    local server_id=$2

    local response
    response=$(curl -s "${PNAP_API_URL}/servers/${server_id}" \
        -H "Authorization: Bearer ${access_token}")

    echo "$response" | jq -r '.publicIpAddresses[0] // empty'
}

# Wait for servers to be powered on
wait_for_servers() {
    local access_token=$1
    shift
    local all_server_ids=("$@")

    log_info "Waiting for servers to be powered on (this may take 10-15 minutes)..."

    local max_wait=1200  # 20 minutes
    local elapsed=0
    local check_interval=30

    while [ $elapsed -lt $max_wait ]; do
        local all_powered_on=true

        for server_id in "${all_server_ids[@]}"; do
            local response
            response=$(curl -s "${PNAP_API_URL}/servers/${server_id}" \
                -H "Authorization: Bearer ${access_token}")

            local status
            status=$(echo "$response" | jq -r '.status // empty')

            if [ "$status" != "powered-on" ]; then
                all_powered_on=false
                break
            fi
        done

        if $all_powered_on; then
            log_success "All servers are powered on!"
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
    local access_token=$1
    local proxy_id=$2
    shift 2
    local backend_ids=("$@")
    local backend_count=${LOAD_TEST_BACKEND_COUNT:-3}
    local generator_ids=("${backend_ids[@]:backend_count}")
    backend_ids=("${backend_ids[@]:0:backend_count}")

    log_info "Getting server IP addresses..."

    local proxy_ip
    proxy_ip=$(get_server_ip "$access_token" "$proxy_id")

    local backend_ips=()
    for backend_id in "${backend_ids[@]}"; do
        backend_ips+=($(get_server_ip "$access_token" "$backend_id"))
    done

    local generator_ips=()
    for generator_id in "${generator_ids[@]}"; do
        generator_ips+=($(get_server_ip "$access_token" "$generator_id"))
    done

    # Create JSON state
    cat > "$STATE_FILE" <<EOF
{
  "tag": "${TAG}",
  "provider": "phoenixnap",
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

    # Get OAuth2 token
    local access_token
    access_token=$(get_access_token)

    # Provision servers
    local proxy_id
    proxy_id=$(provision_proxy "$access_token")

    local backend_ids
    backend_ids=($(provision_backends "$access_token" "${LOAD_TEST_BACKEND_COUNT:-3}"))

    local generator_ids
    generator_ids=($(provision_generators "$access_token" "${LOAD_TEST_GENERATOR_COUNT:-3}"))

    # Collect all server IDs
    local all_server_ids=("$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}")

    # Wait for all servers to be powered on
    wait_for_servers "$access_token" "${all_server_ids[@]}"

    # Wait for SSH to be ready
    show_progress 90 "Waiting for SSH initialization"

    # Save state
    save_state "$access_token" "$proxy_id" "${backend_ids[@]}" "${generator_ids[@]}"

    log_success "Provisioning complete! State saved to: ${STATE_FILE}"

    exit 0
}

# Run main function
main "$@"
