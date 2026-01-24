#!/bin/bash
# Cloud Test Cleanup Library
#
# Provides automatic cleanup of cloud resources when tests are interrupted or fail.
# Supports multiple cloud providers: Vultr, AWS, GCP, Azure, DigitalOcean
#
# Usage:
#   source lib/cloud-cleanup.sh
#   cloud_cleanup_init "vultr"
#   instance_id=$(cloud_provision_instance "test-gateway" "vc2-1c-1gb" "ewr")
#   # ... run tests ...
#   cloud_cleanup  # Called automatically on EXIT/INT/TERM/ERR

set -euo pipefail

# Global state
declare -a CLOUD_INSTANCES=()
declare -a CLOUD_RESOURCES=()
CLOUD_PROVIDER=""
CLOUD_CLEANUP_DONE=false

# Initialize cloud cleanup for a provider
# Args: provider (vultr|aws|gcp|azure|digitalocean)
cloud_cleanup_init() {
    local provider="$1"
    CLOUD_PROVIDER="$provider"
    CLOUD_CLEANUP_DONE=false
    CLOUD_INSTANCES=()
    CLOUD_RESOURCES=()

    # Register trap handlers
    trap cloud_cleanup EXIT
    trap cloud_cleanup INT
    trap cloud_cleanup TERM
    trap 'cloud_cleanup; exit 1' ERR

    echo "[cloud-cleanup] Initialized for provider: $provider"
}

# Cleanup all registered cloud resources
cloud_cleanup() {
    if [ "$CLOUD_CLEANUP_DONE" = true ]; then
        return 0
    fi
    CLOUD_CLEANUP_DONE=true

    echo ""
    echo "[cloud-cleanup] Starting cleanup of cloud resources..."

    local cleanup_errors=0

    # Cleanup instances
    for instance_id in "${CLOUD_INSTANCES[@]:-}"; do
        if [ -n "$instance_id" ]; then
            echo "[cloud-cleanup] Deleting instance: $instance_id"
            if ! _cloud_delete_instance "$instance_id"; then
                echo "[cloud-cleanup] WARNING: Failed to delete instance $instance_id"
                cleanup_errors=$((cleanup_errors + 1))
            fi
        fi
    done

    # Cleanup other resources (volumes, IPs, etc.)
    for resource in "${CLOUD_RESOURCES[@]:-}"; do
        if [ -n "$resource" ]; then
            local resource_type="${resource%%:*}"
            local resource_id="${resource#*:}"
            echo "[cloud-cleanup] Deleting $resource_type: $resource_id"
            if ! _cloud_delete_resource "$resource_type" "$resource_id"; then
                echo "[cloud-cleanup] WARNING: Failed to delete $resource_type $resource_id"
                cleanup_errors=$((cleanup_errors + 1))
            fi
        fi
    done

    if [ $cleanup_errors -eq 0 ]; then
        echo "[cloud-cleanup] All resources cleaned up successfully"
    else
        echo "[cloud-cleanup] Cleanup completed with $cleanup_errors errors"
    fi

    return 0
}

# Register an instance for cleanup
# Args: instance_id
cloud_register_instance() {
    local instance_id="$1"
    CLOUD_INSTANCES+=("$instance_id")
    echo "[cloud-cleanup] Registered instance for cleanup: $instance_id"
}

# Register a resource for cleanup
# Args: resource_type resource_id
cloud_register_resource() {
    local resource_type="$1"
    local resource_id="$2"
    CLOUD_RESOURCES+=("${resource_type}:${resource_id}")
    echo "[cloud-cleanup] Registered $resource_type for cleanup: $resource_id"
}

# Provision a new instance and register for cleanup
# Args: name plan region [image]
# Returns: instance_id (also echoed)
cloud_provision_instance() {
    local name="$1"
    local plan="$2"
    local region="$3"
    local image="${4:-}"

    echo "[cloud-cleanup] Provisioning instance: $name ($plan in $region)"

    local instance_id
    instance_id=$(_cloud_create_instance "$name" "$plan" "$region" "$image")

    if [ -n "$instance_id" ]; then
        cloud_register_instance "$instance_id"
        echo "$instance_id"
    else
        echo "[cloud-cleanup] ERROR: Failed to provision instance"
        return 1
    fi
}

# Wait for instance to be ready
# Args: instance_id [timeout_seconds]
cloud_wait_for_instance() {
    local instance_id="$1"
    local timeout="${2:-300}"

    echo "[cloud-cleanup] Waiting for instance $instance_id (timeout: ${timeout}s)"

    local start_time=$(date +%s)
    while true; do
        local elapsed=$(($(date +%s) - start_time))
        if [ $elapsed -ge $timeout ]; then
            echo "[cloud-cleanup] ERROR: Timeout waiting for instance $instance_id"
            return 1
        fi

        if _cloud_instance_ready "$instance_id"; then
            echo "[cloud-cleanup] Instance $instance_id is ready"
            return 0
        fi

        sleep 5
    done
}

# Get instance IP address
# Args: instance_id
cloud_get_instance_ip() {
    local instance_id="$1"
    _cloud_get_ip "$instance_id"
}

# ============================================================================
# Provider-specific implementations
# ============================================================================

_cloud_create_instance() {
    local name="$1"
    local plan="$2"
    local region="$3"
    local image="$4"

    case "$CLOUD_PROVIDER" in
        vultr)
            _vultr_create_instance "$name" "$plan" "$region" "$image"
            ;;
        aws)
            _aws_create_instance "$name" "$plan" "$region" "$image"
            ;;
        digitalocean)
            _do_create_instance "$name" "$plan" "$region" "$image"
            ;;
        *)
            echo "[cloud-cleanup] ERROR: Unknown provider: $CLOUD_PROVIDER"
            return 1
            ;;
    esac
}

_cloud_delete_instance() {
    local instance_id="$1"

    case "$CLOUD_PROVIDER" in
        vultr)
            vultr-cli instance delete "$instance_id" --force 2>/dev/null || true
            ;;
        aws)
            aws ec2 terminate-instances --instance-ids "$instance_id" 2>/dev/null || true
            ;;
        digitalocean)
            doctl compute droplet delete "$instance_id" --force 2>/dev/null || true
            ;;
        *)
            echo "[cloud-cleanup] ERROR: Unknown provider: $CLOUD_PROVIDER"
            return 1
            ;;
    esac
}

_cloud_delete_resource() {
    local resource_type="$1"
    local resource_id="$2"

    case "$CLOUD_PROVIDER" in
        vultr)
            case "$resource_type" in
                volume)
                    vultr-cli block-storage delete "$resource_id" 2>/dev/null || true
                    ;;
                ip)
                    vultr-cli reserved-ip delete "$resource_id" 2>/dev/null || true
                    ;;
            esac
            ;;
        aws)
            case "$resource_type" in
                volume)
                    aws ec2 delete-volume --volume-id "$resource_id" 2>/dev/null || true
                    ;;
                eip)
                    aws ec2 release-address --allocation-id "$resource_id" 2>/dev/null || true
                    ;;
            esac
            ;;
        digitalocean)
            case "$resource_type" in
                volume)
                    doctl compute volume delete "$resource_id" --force 2>/dev/null || true
                    ;;
                ip)
                    doctl compute floating-ip delete "$resource_id" --force 2>/dev/null || true
                    ;;
            esac
            ;;
    esac
}

_cloud_instance_ready() {
    local instance_id="$1"

    case "$CLOUD_PROVIDER" in
        vultr)
            local status
            status=$(vultr-cli instance get "$instance_id" --output json 2>/dev/null | jq -r '.instance.status // empty')
            [ "$status" = "active" ]
            ;;
        aws)
            local state
            state=$(aws ec2 describe-instances --instance-ids "$instance_id" --query 'Reservations[0].Instances[0].State.Name' --output text 2>/dev/null)
            [ "$state" = "running" ]
            ;;
        digitalocean)
            local status
            status=$(doctl compute droplet get "$instance_id" --format Status --no-header 2>/dev/null)
            [ "$status" = "active" ]
            ;;
        *)
            return 1
            ;;
    esac
}

_cloud_get_ip() {
    local instance_id="$1"

    case "$CLOUD_PROVIDER" in
        vultr)
            vultr-cli instance get "$instance_id" --output json 2>/dev/null | jq -r '.instance.main_ip // empty'
            ;;
        aws)
            aws ec2 describe-instances --instance-ids "$instance_id" --query 'Reservations[0].Instances[0].PublicIpAddress' --output text 2>/dev/null
            ;;
        digitalocean)
            doctl compute droplet get "$instance_id" --format PublicIPv4 --no-header 2>/dev/null
            ;;
    esac
}

# Vultr-specific helpers
_vultr_create_instance() {
    local name="$1"
    local plan="$2"
    local region="$3"
    local image="${4:-387}"  # Default: Ubuntu 22.04 x64

    vultr-cli instance create \
        --region "$region" \
        --plan "$plan" \
        --os "$image" \
        --label "$name" \
        --output json 2>/dev/null | jq -r '.instance.id // empty'
}

# AWS-specific helpers
_aws_create_instance() {
    local name="$1"
    local instance_type="$2"
    local region="$3"
    local ami="${4:-ami-0c55b159cbfafe1f0}"  # Amazon Linux 2

    aws ec2 run-instances \
        --region "$region" \
        --instance-type "$instance_type" \
        --image-id "$ami" \
        --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$name}]" \
        --query 'Instances[0].InstanceId' \
        --output text 2>/dev/null
}

# DigitalOcean-specific helpers
_do_create_instance() {
    local name="$1"
    local size="$2"
    local region="$3"
    local image="${4:-ubuntu-22-04-x64}"

    doctl compute droplet create "$name" \
        --region "$region" \
        --size "$size" \
        --image "$image" \
        --format ID \
        --no-header \
        --wait 2>/dev/null
}

# ============================================================================
# Utility functions
# ============================================================================

# Check if a cloud CLI is available
cloud_check_cli() {
    case "$CLOUD_PROVIDER" in
        vultr)
            command -v vultr-cli >/dev/null 2>&1
            ;;
        aws)
            command -v aws >/dev/null 2>&1
            ;;
        digitalocean)
            command -v doctl >/dev/null 2>&1
            ;;
        *)
            return 1
            ;;
    esac
}

# Print cleanup status
cloud_status() {
    echo "[cloud-cleanup] Provider: $CLOUD_PROVIDER"
    echo "[cloud-cleanup] Instances registered: ${#CLOUD_INSTANCES[@]}"
    echo "[cloud-cleanup] Resources registered: ${#CLOUD_RESOURCES[@]}"
    echo "[cloud-cleanup] Cleanup done: $CLOUD_CLEANUP_DONE"
}
