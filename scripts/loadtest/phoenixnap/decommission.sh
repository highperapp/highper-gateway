#!/bin/bash
# PhoenixNAP Bare Metal Cloud decommissioning script

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

# Configuration
PNAP_AUTH_URL="https://auth.phoenixnap.com/auth/realms/BMC/protocol/openid-connect/token"
PNAP_API_URL="https://api.phoenixnap.com/bmc/v1"

# Parse arguments
TAG="${1:-}"
STATE_FILE="${2:-}"

if [ -z "$STATE_FILE" ] && [ -n "$TAG" ]; then
    STATE_FILE="${PROJECT_ROOT}/results/${TAG}/phoenixnap-servers.json"
fi

if [ -z "$STATE_FILE" ] || [ ! -f "$STATE_FILE" ]; then
    log_error "State file not found: ${STATE_FILE}"
    log_info "Usage: $0 <tag> [state_file]"
    log_info "   or: $0 <state_file>"
    exit 1
fi

log_info "============================================"
log_info "PhoenixNAP Bare Metal Cloud Decommissioning"
log_info "============================================"
log_info "State file: ${STATE_FILE}"
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

# Delete server by ID
delete_server() {
    local access_token=$1
    local server_id=$2
    local server_label=$3

    log_info "Deleting server: ${server_label} (${server_id})..."

    local response
    response=$(curl -s -X DELETE "${PNAP_API_URL}/servers/${server_id}" \
        -H "Authorization: Bearer ${access_token}" \
        -w "\n%{http_code}")

    local http_code
    http_code=$(echo "$response" | tail -n1)

    if [ "$http_code" = "204" ] || [ "$http_code" = "200" ]; then
        log_success "Server deleted: ${server_label}"
        return 0
    elif [ "$http_code" = "404" ]; then
        log_warn "Server not found (may already be deleted): ${server_label}"
        return 0
    else
        log_warn "Failed to delete server ${server_label} (HTTP ${http_code})"
        log_debug "Response: $(echo "$response" | head -n-1)"
        return 1
    fi
}

# Main execution
main() {
    log_info "Loading server state from: ${STATE_FILE}"

    # Get OAuth2 token
    local access_token
    access_token=$(get_access_token)

    # Parse state file
    local proxy_id
    proxy_id=$(jq -r '.proxy.id' "$STATE_FILE")

    local backend_ids
    backend_ids=$(jq -r '.backends[].id' "$STATE_FILE" | tr '\n' ' ')

    local generator_ids
    generator_ids=$(jq -r '.generators[].id' "$STATE_FILE" | tr '\n' ' ')

    # Delete proxy
    if [ -n "$proxy_id" ] && [ "$proxy_id" != "null" ]; then
        delete_server "$access_token" "$proxy_id" "proxy"
    fi

    # Delete backends
    local i=1
    for backend_id in $backend_ids; do
        if [ -n "$backend_id" ] && [ "$backend_id" != "null" ]; then
            delete_server "$access_token" "$backend_id" "backend-${i}"
        fi
        i=$((i + 1))
    done

    # Delete generators
    i=1
    for generator_id in $generator_ids; do
        if [ -n "$generator_id" ] && [ "$generator_id" != "null" ]; then
            delete_server "$access_token" "$generator_id" "generator-${i}"
        fi
        i=$((i + 1))
    done

    log_info ""
    log_success "============================================"
    log_success "All servers decommissioned successfully!"
    log_success "============================================"

    # Optionally delete state file
    if [ "${DELETE_STATE_FILE:-false}" = "true" ]; then
        rm -f "$STATE_FILE"
        log_info "State file deleted: ${STATE_FILE}"
    fi

    exit 0
}

# Run main function
main "$@"
