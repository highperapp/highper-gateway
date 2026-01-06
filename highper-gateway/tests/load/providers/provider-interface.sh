#!/bin/bash
# Cloud Provider Interface - Abstract API for all providers
# Each provider (vultr, phoenixnap, hetzner) must implement these functions

# ========================================
# Provider Interface Functions
# Each provider script must implement:
# ========================================

# provider_init()
# Initialize provider (check API key, dependencies)
# Returns: 0 on success, 1 on failure

# provider_list_regions()
# List available regions/datacenters
# Output: JSON array of regions
# Returns: 0 on success

# provider_list_plans()
# List available server plans/sizes
# Output: JSON array of plans with specs
# Returns: 0 on success

# provider_create_server()
# Create a new server instance
# Args: $1=name, $2=region, $3=plan, $4=ssh_key_id
# Output: JSON with server details (id, ip, etc.)
# Returns: 0 on success

# provider_delete_server()
# Delete a server instance
# Args: $1=server_id
# Returns: 0 on success

# provider_get_server()
# Get server details
# Args: $1=server_id
# Output: JSON with server details
# Returns: 0 on success

# provider_wait_for_server()
# Wait for server to be ready
# Args: $1=server_id, $2=timeout_seconds
# Returns: 0 when ready, 1 on timeout

# provider_get_server_ip()
# Get server public IP address
# Args: $1=server_id
# Output: IP address
# Returns: 0 on success

# ========================================
# Common Helper Functions
# ========================================

check_provider_api_key() {
    local provider=$1
    local var_name="${provider^^}_API_KEY"

    if [ -z "${!var_name}" ]; then
        log_error "API key not found: ${var_name}"
        log_info "Set it with: export ${var_name}=your_api_key"
        return 1
    fi

    log_debug "API key found for ${provider}"
    return 0
}

validate_provider_response() {
    local response=$1
    local expected_status=${2:-200}

    local status=$(echo "$response" | jq -r '.status // .http_status // 200')

    if [ "$status" != "$expected_status" ]; then
        log_error "Provider API returned status: $status"
        log_error "Response: $response"
        return 1
    fi

    return 0
}

provider_api_request() {
    local method=$1
    local url=$2
    local api_key=$3
    local data=${4:-""}

    local curl_opts=(
        -s
        -X "$method"
        -H "Authorization: Bearer $api_key"
        -H "Content-Type: application/json"
    )

    if [ -n "$data" ]; then
        curl_opts+=(-d "$data")
    fi

    local response=$(curl "${curl_opts[@]}" "$url")

    if [ $? -ne 0 ]; then
        log_error "API request failed: $url"
        return 1
    fi

    echo "$response"
    return 0
}

# ========================================
# Server Provisioning Workflow
# ========================================

provision_load_test_infrastructure() {
    local provider=$1
    local scenario=$2
    local region=${3:-"auto"}
    local plan=${4:-"auto"}

    log_info "Provisioning infrastructure on ${provider} for scenario ${scenario}"

    # Load provider-specific implementation
    local provider_script="./providers/${provider}.sh"
    if [ ! -f "$provider_script" ]; then
        log_error "Provider script not found: $provider_script"
        return 1
    fi

    source "$provider_script"

    # Initialize provider
    provider_init || return 1

    # Auto-select region if needed
    if [ "$region" = "auto" ]; then
        log_info "Auto-selecting optimal region..."
        region=$(provider_list_regions | jq -r '.[0].id')
        log_info "Selected region: $region"
    fi

    # Auto-select plan if needed
    if [ "$plan" = "auto" ]; then
        log_info "Auto-selecting server plan..."
        # Select plan based on scenario requirements
        case "$scenario" in
            01-tcp-proxy|02-http-loadbalancer)
                # High CPU, high network
                plan=$(provider_list_plans | jq -r '.[] | select(.vcpu >= 8 and .memory >= 16384) | .id' | head -1)
                ;;
            04-api-gateway|09-waf-mtls)
                # Balanced resources
                plan=$(provider_list_plans | jq -r '.[] | select(.vcpu >= 4 and .memory >= 8192) | .id' | head -1)
                ;;
            *)
                # Default: 4 CPU, 8GB RAM
                plan=$(provider_list_plans | jq -r '.[] | select(.vcpu >= 4 and .memory >= 8192) | .id' | head -1)
                ;;
        esac
        log_info "Selected plan: $plan"
    fi

    # Create SSH key if needed
    local ssh_key_id=$(provider_create_ssh_key "loadtest-$(date +%s)")

    # Create gateway server
    log_info "Creating gateway server..."
    local gateway_server=$(provider_create_server \
        "gateway-${scenario}-$(date +%s)" \
        "$region" \
        "$plan" \
        "$ssh_key_id")

    local gateway_id=$(echo "$gateway_server" | jq -r '.id')
    local gateway_ip=$(echo "$gateway_server" | jq -r '.main_ip')

    # Wait for server to be ready
    log_info "Waiting for gateway server to be ready..."
    provider_wait_for_server "$gateway_id" 300 || return 1

    # Create backend servers (3 for load balancing)
    local backend_ids=()
    for i in 1 2 3; do
        log_info "Creating backend server $i..."
        local backend_server=$(provider_create_server \
            "backend-${scenario}-${i}-$(date +%s)" \
            "$region" \
            "$plan" \
            "$ssh_key_id")

        local backend_id=$(echo "$backend_server" | jq -r '.id')
        backend_ids+=("$backend_id")

        provider_wait_for_server "$backend_id" 300 || return 1
    done

    # Return infrastructure details
    cat <<EOF
{
  "provider": "$provider",
  "scenario": "$scenario",
  "region": "$region",
  "plan": "$plan",
  "gateway": {
    "id": "$gateway_id",
    "ip": "$gateway_ip"
  },
  "backends": [
$(for id in "${backend_ids[@]}"; do
    local ip=$(provider_get_server_ip "$id")
    echo "    {\"id\": \"$id\", \"ip\": \"$ip\"}"
done | paste -sd ',' -)
  ]
}
EOF

    return 0
}

destroy_load_test_infrastructure() {
    local provider=$1
    local infrastructure_json=$2

    log_info "Destroying infrastructure on ${provider}"

    # Load provider-specific implementation
    source "./providers/${provider}.sh"
    provider_init || return 1

    # Delete gateway
    local gateway_id=$(echo "$infrastructure_json" | jq -r '.gateway.id')
    log_info "Deleting gateway server: $gateway_id"
    provider_delete_server "$gateway_id"

    # Delete backends
    local backend_ids=$(echo "$infrastructure_json" | jq -r '.backends[].id')
    for backend_id in $backend_ids; do
        log_info "Deleting backend server: $backend_id"
        provider_delete_server "$backend_id"
    done

    log_success "Infrastructure destroyed"
    return 0
}
