#!/bin/bash
# Common utilities for load testing automation

# Color codes
if [ "${ENABLE_COLOR:-true}" = "true" ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    MAGENTA='\033[0;35m'
    CYAN='\033[0;36m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    MAGENTA=''
    CYAN=''
    NC=''
fi

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
    [ "${SAVE_LOGS:-true}" = "true" ] && echo "[$(date +'%Y-%m-%d %H:%M:%S')] [INFO] $1" >> "${LOG_FILE:-loadtest.log}"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
    [ "${SAVE_LOGS:-true}" = "true" ] && echo "[$(date +'%Y-%m-%d %H:%M:%S')] [SUCCESS] $1" >> "${LOG_FILE:-loadtest.log}"
}

log_warn() {
    echo -e "${YELLOW}[⚠]${NC} $1"
    [ "${SAVE_LOGS:-true}" = "true" ] && echo "[$(date +'%Y-%m-%d %H:%M:%S')] [WARN] $1" >> "${LOG_FILE:-loadtest.log}"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1" >&2
    [ "${SAVE_LOGS:-true}" = "true" ] && echo "[$(date +'%Y-%m-%d %H:%M:%S')] [ERROR] $1" >> "${LOG_FILE:-loadtest.log}"
}

log_debug() {
    if [ "${LOG_LEVEL:-INFO}" = "DEBUG" ]; then
        echo -e "${MAGENTA}[DEBUG]${NC} $1"
        [ "${SAVE_LOGS:-true}" = "true" ] && echo "[$(date +'%Y-%m-%d %H:%M:%S')] [DEBUG] $1" >> "${LOG_FILE:-loadtest.log}"
    fi
}

# Progress indicator
show_progress() {
    local duration=$1
    local message=$2

    for ((i=0; i<duration; i++)); do
        echo -ne "${CYAN}[⏳]${NC} ${message}... $((duration - i))s remaining\r"
        sleep 1
    done
    echo -ne "\033[2K\r"  # Clear line
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check required commands
check_dependencies() {
    local missing_deps=()

    for cmd in curl jq ssh; do
        if ! command_exists "$cmd"; then
            missing_deps+=("$cmd")
        fi
    done

    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_error "Missing required dependencies: ${missing_deps[*]}"
        log_info "Install them with: apt-get install -y ${missing_deps[*]}"
        return 1
    fi

    return 0
}

# Load environment variables
load_env() {
    local env_file="${1:-.env}"

    if [ ! -f "$env_file" ]; then
        log_error "Environment file not found: $env_file"
        log_info "Copy .env.template to .env and fill in your credentials"
        return 1
    fi

    log_debug "Loading environment from: $env_file"
    set -a
    source "$env_file"
    set +a

    log_success "Environment loaded successfully"
    return 0
}

# Wait for SSH to be ready
wait_for_ssh() {
    local host=$1
    local user=${2:-root}
    local timeout=${3:-300}  # 5 minutes
    local key=${4:-${SSH_KEY_PATH}}

    log_info "Waiting for SSH on ${user}@${host}..."

    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        if ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 ${key:+-i $key} "${user}@${host}" "exit" 2>/dev/null; then
            log_success "SSH is ready on ${host}"
            return 0
        fi

        sleep 5
        elapsed=$((elapsed + 5))
        echo -ne "${CYAN}[⏳]${NC} Waiting for SSH... ${elapsed}/${timeout}s\r"
    done

    echo -ne "\033[2K\r"  # Clear line
    log_error "SSH timeout on ${host} after ${timeout}s"
    return 1
}

# Execute remote command via SSH
ssh_exec() {
    local host=$1
    local command=$2
    local user=${3:-root}
    local key=${4:-${SSH_KEY_PATH}}

    log_debug "Executing on ${host}: ${command}"

    ssh -o StrictHostKeyChecking=no ${key:+-i $key} "${user}@${host}" "$command"
}

# Copy file to remote server
ssh_copy() {
    local local_file=$1
    local host=$2
    local remote_path=$3
    local user=${4:-root}
    local key=${5:-${SSH_KEY_PATH}}

    log_info "Copying ${local_file} to ${host}:${remote_path}..."

    scp -o StrictHostKeyChecking=no ${key:+-i $key} "$local_file" "${user}@${host}:${remote_path}"
}

# Generate unique test ID
generate_test_id() {
    echo "test-$(date +%Y%m%d-%H%M%S)-$RANDOM"
}

# Save test metadata
save_test_metadata() {
    local test_id=$1
    local provider=$2
    local scenario=$3
    local results_dir=${4:-./results}

    local metadata_file="${results_dir}/${test_id}/metadata.json"

    mkdir -p "$(dirname "$metadata_file")"

    cat > "$metadata_file" <<EOF
{
  "test_id": "${test_id}",
  "provider": "${provider}",
  "scenario": "${scenario}",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "duration": ${LOAD_TEST_DURATION:-180},
  "target_rps": ${LOAD_TEST_TARGET_RPS:-600000},
  "target_concurrency": ${LOAD_TEST_TARGET_CONCURRENCY:-3000000},
  "backend_count": ${LOAD_TEST_BACKEND_COUNT:-3},
  "generator_count": ${LOAD_TEST_GENERATOR_COUNT:-3}
}
EOF

    log_success "Test metadata saved to: $metadata_file"
}

# Cleanup on error
cleanup_on_error() {
    log_error "An error occurred. Running cleanup..."

    if [ "${AUTO_CLEANUP_ON_FAILURE:-true}" = "true" ] && [ "${KEEP_SERVERS_FOR_DEBUG:-false}" != "true" ]; then
        log_warn "Auto-cleanup is enabled. Decommissioning servers..."
        # This will be called by the provider-specific decommission script
        return 0
    else
        log_warn "Auto-cleanup is disabled. Servers are still running."
        log_info "Run the decommission script manually when done debugging."
        return 1
    fi
}

# Setup error handling
setup_error_handling() {
    set -euo pipefail
    trap cleanup_on_error ERR
}

# Create results directory structure
create_results_dir() {
    local test_id=$1
    local results_dir=${2:-./results}

    local test_results_dir="${results_dir}/${test_id}"

    mkdir -p "${test_results_dir}"/{metrics,logs,configs}

    log_success "Results directory created: ${test_results_dir}"
    echo "${test_results_dir}"
}

# Check API credentials
check_api_credentials() {
    local provider=$1

    case "$provider" in
        vultr)
            if [ -z "${VULTR_API_KEY:-}" ]; then
                log_error "VULTR_API_KEY is not set"
                return 1
            fi
            ;;
        hetzner)
            if [ -z "${HETZNER_CLOUD_TOKEN:-}" ] && [ -z "${HETZNER_ROBOT_USER:-}" ]; then
                log_error "Neither HETZNER_CLOUD_TOKEN nor HETZNER_ROBOT_USER is set"
                return 1
            fi
            ;;
        hetzner-cloud)
            if [ -z "${HETZNER_CLOUD_TOKEN:-}" ]; then
                log_error "HETZNER_CLOUD_TOKEN is not set"
                return 1
            fi
            ;;
        hetzner-dedicated)
            if [ -z "${HETZNER_ROBOT_USER:-}" ] || [ -z "${HETZNER_ROBOT_PASSWORD:-}" ]; then
                log_error "HETZNER_ROBOT_USER or HETZNER_ROBOT_PASSWORD is not set"
                return 1
            fi
            ;;
        phoenixnap)
            if [ -z "${PNAP_CLIENT_ID:-}" ] || [ -z "${PNAP_CLIENT_SECRET:-}" ]; then
                log_error "PNAP_CLIENT_ID or PNAP_CLIENT_SECRET is not set"
                return 1
            fi
            ;;
        *)
            log_error "Unknown provider: $provider"
            return 1
            ;;
    esac

    log_success "API credentials verified for: $provider"
    return 0
}

# Export functions
export -f log_info log_success log_warn log_error log_debug
export -f show_progress command_exists check_dependencies
export -f load_env wait_for_ssh ssh_exec ssh_copy
export -f generate_test_id save_test_metadata
export -f cleanup_on_error setup_error_handling
export -f create_results_dir check_api_credentials
