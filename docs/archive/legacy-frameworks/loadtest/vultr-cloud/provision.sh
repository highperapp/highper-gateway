#!/bin/bash
# Vultr Cloud Instance provisioning for load testing
# Fast provisioning (1-2 min) vs bare metal (15-30 min)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

source "${SCRIPT_DIR}/../common/utils.sh"
load_env "${PROJECT_ROOT}/.env"
check_api_credentials "vultr"

VULTR_API_URL="https://api.vultr.com/v2"
VULTR_OS_ID=1743  # Ubuntu 22.04 LTS (cloud instances)
TAG="load-test-${TEST_ID:-$(generate_test_id)}"
STATE_FILE="${PROJECT_ROOT}/results/${TAG}/vultr-cloud-servers.json"
mkdir -p "$(dirname "$STATE_FILE")"

log_info "============================================"
log_info "Vultr Cloud Load Test Provisioning"
log_info "============================================"
log_info "Test ID: ${TAG}"
log_info "Region: ${VULTR_CLOUD_REGION}"
log_info "Proxy: ${VULTR_CLOUD_PROXY_PLAN}"
log_info "Backend: ${VULTR_CLOUD_BACKEND_PLAN}"
log_info "Generator: ${VULTR_CLOUD_GENERATOR_PLAN}"
log_info "============================================"

# Provision proxy server
provision_proxy() {
    log_info "Creating cloud proxy server..."

    local response
    response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        -H "Content-Type: application/json" \
        -d "{
            \"region\": \"${VULTR_CLOUD_REGION}\",
            \"plan\": \"${VULTR_CLOUD_PROXY_PLAN}\",
            \"os_id\": ${VULTR_OS_ID},
            \"label\": \"proxy-${TAG}\",
            \"hostname\": \"proxy\",
            \"tag\": \"${TAG}\",
            \"sshkey_id\": [\"${VULTR_SSH_KEY_ID}\"]
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

# Provision backends
provision_backends() {
    local count=${1:-3}
    log_info "Creating ${count} backend servers..."

    local backend_ids=()
    for i in $(seq 1 "$count"); do
        log_info "Creating backend ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
            -H "Authorization: Bearer ${VULTR_API_KEY}" \
            -H "Content-Type: application/json" \
            -d "{
                \"region\": \"${VULTR_CLOUD_REGION}\",
                \"plan\": \"${VULTR_CLOUD_BACKEND_PLAN}\",
                \"os_id\": ${VULTR_OS_ID},
                \"label\": \"backend-${i}-${TAG}\",
                \"hostname\": \"backend-${i}\",
                \"tag\": \"${TAG}\",
                \"sshkey_id\": [\"${VULTR_SSH_KEY_ID}\"]
            }")

        local backend_id
        backend_id=$(echo "$response" | jq -r '.instance.id // empty')

        if [ -z "$backend_id" ]; then
            log_error "Failed to create backend ${i}"
            log_error "API Response: $response"
            return 1
        fi

        backend_ids+=("$backend_id")
        log_success "Backend ${i} created: ${backend_id}"
    done

    echo "${backend_ids[@]}"
}

# Provision generators
provision_generators() {
    local count=${1:-3}
    log_info "Creating ${count} generator servers..."

    local generator_ids=()
    for i in $(seq 1 "$count"); do
        log_info "Creating generator ${i}/${count}..."

        local response
        response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
            -H "Authorization: Bearer ${VULTR_API_KEY}" \
            -H "Content-Type: application/json" \
            -d "{
                \"region\": \"${VULTR_CLOUD_REGION}\",
                \"plan\": \"${VULTR_CLOUD_GENERATOR_PLAN}\",
                \"os_id\": ${VULTR_OS_ID},
                \"label\": \"generator-${i}-${TAG}\",
                \"hostname\": \"generator-${i}\",
                \"tag\": \"${TAG}\",
                \"sshkey_id\": [\"${VULTR_SSH_KEY_ID}\"]
            }")

        local generator_id
        generator_id=$(echo "$response" | jq -r '.instance.id // empty')

        if [ -z "$generator_id" ]; then
            log_error "Failed to create generator ${i}"
            log_error "API Response: $response"
            return 1
        fi

        generator_ids+=("$generator_id")
        log_success "Generator ${i} created: ${generator_id}"
    done

    echo "${generator_ids[@]}"
}

# Wait for instances to be active
wait_for_instances() {
    local instance_ids=("$@")
    log_info "Waiting for ${#instance_ids[@]} instances to become active..."

    local max_wait=300  # 5 minutes max
    local elapsed=0

    while [ $elapsed -lt $max_wait ]; do
        local all_active=true

        for id in "${instance_ids[@]}"; do
            local status
            status=$(curl -s -H "Authorization: Bearer ${VULTR_API_KEY}" \
                "${VULTR_API_URL}/instances/${id}" | jq -r '.instance.status')

            if [ "$status" != "active" ]; then
                all_active=false
                break
            fi
        done

        if $all_active; then
            log_success "All instances are active!"
            return 0
        fi

        sleep 10
        elapsed=$((elapsed + 10))
        log_info "Still waiting... (${elapsed}s/${max_wait}s)"
    done

    log_error "Timeout waiting for instances to become active"
    return 1
}

# Get instance IPs
get_instance_ips() {
    local instance_ids=("$@")
    local ips=()

    for id in "${instance_ids[@]}"; do
        local ip
        ip=$(curl -s -H "Authorization: Bearer ${VULTR_API_KEY}" \
            "${VULTR_API_URL}/instances/${id}" | jq -r '.instance.main_ip')
        ips+=("$ip")
    done

    echo "${ips[@]}"
}

# Main provisioning
log_info "Starting provisioning process..."

PROXY_ID=$(provision_proxy)
BACKEND_IDS=($(provision_backends 3))
GENERATOR_IDS=($(provision_generators 3))

ALL_IDS=("$PROXY_ID" "${BACKEND_IDS[@]}" "${GENERATOR_IDS[@]}")
wait_for_instances "${ALL_IDS[@]}"

IPS=($(get_instance_ips "${ALL_IDS[@]}"))
PROXY_IP="${IPS[0]}"
BACKEND_IPS=("${IPS[1]}" "${IPS[2]}" "${IPS[3]}")
GENERATOR_IPS=("${IPS[4]}" "${IPS[5]}" "${IPS[6]}")

# Save state
cat > "$STATE_FILE" <<EOF
{
  "test_id": "${TAG}",
  "provider": "vultr-cloud",
  "proxy": {"id": "${PROXY_ID}", "ip": "${PROXY_IP}"},
  "backends": [
    {"id": "${BACKEND_IDS[0]}", "ip": "${BACKEND_IPS[0]}"},
    {"id": "${BACKEND_IDS[1]}", "ip": "${BACKEND_IPS[1]}"},
    {"id": "${BACKEND_IDS[2]}", "ip": "${BACKEND_IPS[2]}"}
  ],
  "generators": [
    {"id": "${GENERATOR_IDS[0]}", "ip": "${GENERATOR_IPS[0]}"},
    {"id": "${GENERATOR_IDS[1]}", "ip": "${GENERATOR_IPS[1]}"},
    {"id": "${GENERATOR_IDS[2]}", "ip": "${GENERATOR_IPS[2]}"}
  ]
}
EOF

log_success "============================================"
log_success "Cloud Provisioning Complete!"
log_success "============================================"
log_info "Proxy IP: ${PROXY_IP}"
log_info "Backend IPs: ${BACKEND_IPS[*]}"
log_info "Generator IPs: ${GENERATOR_IPS[*]}"
log_info "State file: ${STATE_FILE}"
log_success "============================================"

exit 0
