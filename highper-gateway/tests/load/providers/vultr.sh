#!/bin/bash
# Vultr Cloud Provider Implementation
# API Documentation: https://www.vultr.com/api/

set -euo pipefail

# Load common helpers
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../helpers/common.sh"
source "${SCRIPT_DIR}/provider-interface.sh"

# ========================================
# Vultr API Configuration
# ========================================

VULTR_API_BASE="https://api.vultr.com/v2"
VULTR_API_KEY="${VULTR_API_KEY:-}"

# ========================================
# Provider Interface Implementation
# ========================================

provider_init() {
    log_info "Initializing Vultr provider"

    check_provider_api_key "vultr" || return 1

    # Check required tools
    ensure_tool jq || return 1

    # Validate API key by fetching account info
    local response=$(curl -s \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        "${VULTR_API_BASE}/account")

    if [ $? -ne 0 ] || [ -z "$response" ]; then
        log_error "Failed to connect to Vultr API"
        return 1
    fi

    log_success "Vultr provider initialized"
    return 0
}

provider_list_regions() {
    log_debug "Fetching Vultr regions..."

    local response=$(curl -s \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        "${VULTR_API_BASE}/regions")

    echo "$response" | jq -c '.regions'
    return 0
}

provider_list_plans() {
    log_debug "Fetching Vultr plans..."

    local response=$(curl -s \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        "${VULTR_API_BASE}/plans")

    echo "$response" | jq -c '.plans | map({
        id: .id,
        vcpu: .vcpu_count,
        memory: .ram,
        disk: .disk,
        bandwidth: .bandwidth,
        price_monthly: .monthly_cost
    })'
    return 0
}

provider_create_ssh_key() {
    local name=$1

    # Check if SSH key exists
    if [ ! -f ~/.ssh/id_rsa.pub ]; then
        log_warn "No SSH key found, generating one..."
        ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa -N "" -q
    fi

    local ssh_key=$(cat ~/.ssh/id_rsa.pub)

    log_info "Creating SSH key: $name"

    local response=$(curl -s \
        -X POST \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        -H "Content-Type: application/json" \
        -d "{\"name\":\"${name}\",\"ssh_key\":\"${ssh_key}\"}" \
        "${VULTR_API_BASE}/ssh-keys")

    local key_id=$(echo "$response" | jq -r '.ssh_key.id')

    if [ -z "$key_id" ] || [ "$key_id" = "null" ]; then
        log_warn "Failed to create SSH key, using existing keys"
        # Get first existing key
        local keys_response=$(curl -s \
            -H "Authorization: Bearer ${VULTR_API_KEY}" \
            "${VULTR_API_BASE}/ssh-keys")
        key_id=$(echo "$keys_response" | jq -r '.ssh_keys[0].id')
    fi

    echo "$key_id"
    return 0
}

provider_create_server() {
    local name=$1
    local region=$2
    local plan=$3
    local ssh_key_id=$4

    log_info "Creating Vultr instance: $name"

    # Use Ubuntu 22.04 LTS
    local os_id=1743  # Ubuntu 22.04 LTS

    local request_data=$(cat <<EOF
{
    "region": "${region}",
    "plan": "${plan}",
    "os_id": ${os_id},
    "label": "${name}",
    "hostname": "${name}",
    "enable_ipv6": false,
    "backups": "disabled",
    "ddos_protection": false,
    "activation_email": false,
    "sshkey_id": ["${ssh_key_id}"],
    "user_data": "$(echo '#!/bin/bash
# Install Docker
curl -fsSL https://get.docker.com | sh
systemctl enable docker
systemctl start docker

# Install docker-compose
curl -L "https://github.com/docker/compose/releases/download/v2.24.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
chmod +x /usr/local/bin/docker-compose

# Install tools
apt-get update
apt-get install -y wget curl jq htop iperf3 git

echo "Server provisioned at $(date)" > /root/provisioned.txt
' | base64 -w0)"
}
EOF
)

    local response=$(curl -s \
        -X POST \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        -H "Content-Type: application/json" \
        -d "$request_data" \
        "${VULTR_API_BASE}/instances")

    local instance=$(echo "$response" | jq -c '.instance // .')

    if [ -z "$instance" ] || [ "$instance" = "null" ]; then
        log_error "Failed to create instance"
        log_error "Response: $response"
        return 1
    fi

    echo "$instance"
    return 0
}

provider_delete_server() {
    local server_id=$1

    log_info "Deleting Vultr instance: $server_id"

    curl -s -X DELETE \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        "${VULTR_API_BASE}/instances/${server_id}"

    return 0
}

provider_get_server() {
    local server_id=$1

    local response=$(curl -s \
        -H "Authorization: Bearer ${VULTR_API_KEY}" \
        "${VULTR_API_BASE}/instances/${server_id}")

    echo "$response" | jq -c '.instance'
    return 0
}

provider_wait_for_server() {
    local server_id=$1
    local timeout=${2:-300}

    log_info "Waiting for Vultr instance to be ready: $server_id"

    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        local server=$(provider_get_server "$server_id")
        local status=$(echo "$server" | jq -r '.status')
        local server_state=$(echo "$server" | jq -r '.server_state')

        if [ "$status" = "active" ] && [ "$server_state" = "ok" ]; then
            log_success "Instance is ready"
            return 0
        fi

        log_debug "Instance status: $status, server_state: $server_state (${elapsed}/${timeout}s)"
        sleep 10
        elapsed=$((elapsed + 10))
    done

    log_error "Instance did not become ready within ${timeout}s"
    return 1
}

provider_get_server_ip() {
    local server_id=$1

    local server=$(provider_get_server "$server_id")
    echo "$server" | jq -r '.main_ip'
    return 0
}

# ========================================
# Vultr-Specific Helpers
# ========================================

vultr_get_optimal_region() {
    local target_continent=${1:-"na"}  # na, eu, asia

    log_info "Finding optimal Vultr region for: $target_continent"

    local regions=$(provider_list_regions)

    case "$target_continent" in
        na|north-america)
            echo "$regions" | jq -r '.[] | select(.continent == "North America") | .id' | head -1
            ;;
        eu|europe)
            echo "$regions" | jq -r '.[] | select(.continent == "Europe") | .id' | head -1
            ;;
        asia)
            echo "$regions" | jq -r '.[] | select(.continent == "Asia") | .id' | head -1
            ;;
        *)
            echo "$regions" | jq -r '.[0].id'
            ;;
    esac

    return 0
}

vultr_get_high_performance_plan() {
    local min_vcpu=${1:-8}
    local min_memory_mb=${2:-16384}

    log_info "Finding high-performance Vultr plan (${min_vcpu} vCPU, ${min_memory_mb}MB RAM)"

    local plans=$(provider_list_plans)

    echo "$plans" | jq -r \
        ".[] | select(.vcpu >= ${min_vcpu} and .memory >= ${min_memory_mb}) | .id" | \
        head -1

    return 0
}

log_debug "Vultr provider loaded"
