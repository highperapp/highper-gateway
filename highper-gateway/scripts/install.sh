#!/bin/bash
# Highper Gateway Installation Script
#
# One-liner installation:
#   curl -fsSL https://raw.githubusercontent.com/anthropics/highper-gateway/main/scripts/install.sh | bash
#
# With options:
#   curl ... | bash -s -- --port 8080 --backend localhost:3000 --tls auto
#
# Copyright 2024-2026 Highper Gateway Contributors
# Licensed under the Apache License, Version 2.0

set -e

# Configuration
REPO="anthropics/highper-gateway"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/highper-gateway"
DATA_DIR="/var/lib/highper-gateway"
LOG_DIR="/var/log/highper-gateway"
SERVICE_NAME="highper-gateway"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options
PORT=8080
BACKEND=""
TLS_MODE=""
DOMAIN=""
EMAIL=""
VERSION="latest"
SKIP_SERVICE=false
UNINSTALL=false

# Print colored output
info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Show usage
usage() {
    cat << EOF
Highper Gateway Installation Script

Usage: $0 [OPTIONS]

Options:
  -p, --port PORT         HTTP port to listen on (default: 8080)
  -b, --backend URL       Backend server URL (required for config generation)
  -t, --tls MODE          TLS mode: auto (Let's Encrypt) or manual
  -d, --domain DOMAIN     Domain for automatic TLS
  -e, --email EMAIL       Email for Let's Encrypt registration
  -v, --version VERSION   Version to install (default: latest)
  --skip-service          Don't create systemd service
  --uninstall             Uninstall Highper Gateway
  -h, --help              Show this help message

Examples:
  # Basic installation (latest version)
  $0

  # Install with quick-start config
  $0 --backend localhost:3000 --port 80

  # Install with automatic HTTPS
  $0 --backend localhost:3000 --tls auto --domain example.com --email admin@example.com

  # Install specific version
  $0 --version v0.1.0

EOF
    exit 0
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -p|--port)
                PORT="$2"
                shift 2
                ;;
            -b|--backend)
                BACKEND="$2"
                shift 2
                ;;
            -t|--tls)
                TLS_MODE="$2"
                shift 2
                ;;
            -d|--domain)
                DOMAIN="$2"
                shift 2
                ;;
            -e|--email)
                EMAIL="$2"
                shift 2
                ;;
            -v|--version)
                VERSION="$2"
                shift 2
                ;;
            --skip-service)
                SKIP_SERVICE=true
                shift
                ;;
            --uninstall)
                UNINSTALL=true
                shift
                ;;
            -h|--help)
                usage
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
}

# Detect OS and architecture
# Note: Highper Gateway uses io_uring which requires Linux kernel 5.1+
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    # io_uring requires Linux
    if [ "$OS" != "linux" ]; then
        error "Highper Gateway requires Linux (uses io_uring for high performance).\nDetected OS: $OS\n\nSupported: Linux with kernel 5.1+ (Ubuntu 20.04+, Debian 11+, RHEL 8+, Fedora 31+)"
    fi

    # Check kernel version for io_uring support (5.1+)
    KERNEL_VERSION=$(uname -r | cut -d. -f1-2)
    KERNEL_MAJOR=$(echo "$KERNEL_VERSION" | cut -d. -f1)
    KERNEL_MINOR=$(echo "$KERNEL_VERSION" | cut -d. -f2)

    if [ "$KERNEL_MAJOR" -lt 5 ] || ([ "$KERNEL_MAJOR" -eq 5 ] && [ "$KERNEL_MINOR" -lt 1 ]); then
        warn "Kernel $KERNEL_VERSION detected. io_uring requires kernel 5.1+"
        warn "Some features may not work optimally. Consider upgrading your kernel."
    else
        info "Kernel $KERNEL_VERSION - io_uring support confirmed"
    fi

    case $ARCH in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        armv7l|armv7)
            ARCH="armv7"
            ;;
        *)
            error "Unsupported architecture: $ARCH\nSupported: x86_64, aarch64, armv7"
            ;;
    esac

    PLATFORM="linux-${ARCH}"
    info "Detected platform: $PLATFORM"

    # Detect Linux distribution
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        info "Distribution: $NAME $VERSION_ID"
    fi
}

# Check for required tools
check_dependencies() {
    local missing=()

    for cmd in curl tar; do
        if ! command -v $cmd &> /dev/null; then
            missing+=($cmd)
        fi
    done

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing required tools: ${missing[*]}\nPlease install them and try again."
    fi
}

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        warn "Not running as root. Some operations may require sudo."
        SUDO="sudo"
    else
        SUDO=""
    fi
}

# Get the latest version from GitHub
get_latest_version() {
    if [ "$VERSION" = "latest" ]; then
        info "Fetching latest version..."
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
        if [ -z "$VERSION" ]; then
            error "Failed to fetch latest version"
        fi
        info "Latest version: $VERSION"
    fi
}

# Download and install the binary
install_binary() {
    local tmp_dir=$(mktemp -d)
    local archive_name="highper-gateway-${VERSION}-${PLATFORM}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${VERSION}/${archive_name}"

    info "Downloading Highper Gateway ${VERSION}..."

    if ! curl -fsSL -o "${tmp_dir}/${archive_name}" "$download_url"; then
        # Try without version prefix
        archive_name="highper-gateway-${PLATFORM}.tar.gz"
        download_url="https://github.com/${REPO}/releases/download/${VERSION}/${archive_name}"
        if ! curl -fsSL -o "${tmp_dir}/${archive_name}" "$download_url"; then
            error "Failed to download from: $download_url"
        fi
    fi

    info "Extracting archive..."
    tar -xzf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

    info "Installing binary to ${INSTALL_DIR}..."
    $SUDO install -m 755 "${tmp_dir}/highper-gateway" "${INSTALL_DIR}/highper-gateway"

    rm -rf "$tmp_dir"
    success "Binary installed successfully"
}

# Create necessary directories
create_directories() {
    info "Creating directories..."
    $SUDO mkdir -p "$CONFIG_DIR"
    $SUDO mkdir -p "$DATA_DIR/certs"
    $SUDO mkdir -p "$LOG_DIR"
    success "Directories created"
}

# Generate configuration file
generate_config() {
    if [ -z "$BACKEND" ]; then
        info "No backend specified, skipping config generation"
        info "You can start with: highper-gateway run --backend YOUR_BACKEND"
        return
    fi

    info "Generating configuration..."

    local config_file="${CONFIG_DIR}/config.yaml"

    # Build TLS config section
    local tls_config=""
    if [ "$TLS_MODE" = "auto" ]; then
        if [ -z "$DOMAIN" ]; then
            error "--domain is required when using --tls auto"
        fi
        tls_config="
tls:
  auto: true
  acme:
    provider: letsencrypt
    email: ${EMAIL:-admin@${DOMAIN}}
    staging: false
    domains:
      - ${DOMAIN}
    challenge_type: http-01
    storage:
      storage_type: file
      path: ${DATA_DIR}/certs
    renewal_days: 30"
    fi

    # Generate config
    $SUDO tee "$config_file" > /dev/null << EOF
# Highper Gateway Configuration
# Generated by install.sh on $(date)

server:
  bind:
    - "0.0.0.0:${PORT}"
  workers: auto
${tls_config}

upstreams:
  - name: default
    servers:
      - url: ${BACKEND}
        weight: 100
    load_balancing:
      algorithm: round_robin
    health_check:
      enabled: true
      interval: 10s
      timeout: 5s

routes:
  - name: default
    upstream: default
    match:
      paths:
        - "/*"

observability:
  metrics:
    enabled: true
    bind: "0.0.0.0:9090"
    endpoint: /metrics

admin:
  enabled: true
  bind: "127.0.0.1:9000"
  auth_enabled: false
EOF

    success "Configuration generated: $config_file"
}

# Create systemd service
create_systemd_service() {
    if [ "$SKIP_SERVICE" = true ]; then
        info "Skipping systemd service creation"
        return
    fi

    if [ "$OS" != "linux" ]; then
        info "Skipping systemd service (not Linux)"
        return
    fi

    if ! command -v systemctl &> /dev/null; then
        warn "systemctl not found, skipping service creation"
        return
    fi

    info "Creating systemd service..."

    $SUDO tee /etc/systemd/system/${SERVICE_NAME}.service > /dev/null << EOF
[Unit]
Description=Highper Gateway - High-performance reverse proxy and API gateway
Documentation=https://github.com/${REPO}
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Group=root
ExecStart=${INSTALL_DIR}/highper-gateway start --config ${CONFIG_DIR}/config.yaml
ExecReload=/bin/kill -HUP \$MAINPID
Restart=on-failure
RestartSec=5
LimitNOFILE=65535

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${DATA_DIR} ${LOG_DIR}
PrivateTmp=true

# Environment
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
EOF

    $SUDO systemctl daemon-reload
    success "Systemd service created: ${SERVICE_NAME}"

    info "To enable and start the service:"
    echo "  sudo systemctl enable ${SERVICE_NAME}"
    echo "  sudo systemctl start ${SERVICE_NAME}"
}

# Uninstall Highper Gateway
uninstall() {
    info "Uninstalling Highper Gateway..."

    # Stop and disable service
    if [ "$OS" = "linux" ] && command -v systemctl &> /dev/null; then
        $SUDO systemctl stop ${SERVICE_NAME} 2>/dev/null || true
        $SUDO systemctl disable ${SERVICE_NAME} 2>/dev/null || true
        $SUDO rm -f /etc/systemd/system/${SERVICE_NAME}.service
        $SUDO systemctl daemon-reload
    fi

    # Remove binary
    $SUDO rm -f "${INSTALL_DIR}/highper-gateway"

    # Ask about config and data
    read -p "Remove configuration directory ${CONFIG_DIR}? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        $SUDO rm -rf "$CONFIG_DIR"
    fi

    read -p "Remove data directory ${DATA_DIR}? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        $SUDO rm -rf "$DATA_DIR"
    fi

    success "Highper Gateway uninstalled"
    exit 0
}

# Print post-installation instructions
print_instructions() {
    echo
    echo "=========================================="
    echo -e "${GREEN}Highper Gateway installed successfully!${NC}"
    echo "=========================================="
    echo
    echo "Quick start:"
    if [ -n "$BACKEND" ]; then
        echo "  sudo systemctl start ${SERVICE_NAME}"
        echo "  sudo systemctl status ${SERVICE_NAME}"
    else
        echo "  highper-gateway run --backend localhost:3000"
    fi
    echo
    echo "Configuration file: ${CONFIG_DIR}/config.yaml"
    echo "Data directory: ${DATA_DIR}"
    echo "Log directory: ${LOG_DIR}"
    echo
    echo "Useful commands:"
    echo "  highper-gateway --help              # Show help"
    echo "  highper-gateway validate -c FILE    # Validate config"
    echo "  highper-gateway test -c FILE        # Test backends"
    echo "  highper-gateway health              # Check health"
    echo
    echo "Documentation: https://github.com/${REPO}"
    echo
}

# Main installation flow
main() {
    echo
    echo "=========================================="
    echo "  Highper Gateway Installer"
    echo "=========================================="
    echo

    parse_args "$@"

    if [ "$UNINSTALL" = true ]; then
        check_root
        uninstall
    fi

    detect_platform
    check_dependencies
    check_root
    get_latest_version
    install_binary
    create_directories
    generate_config
    create_systemd_service
    print_instructions
}

# Run main function
main "$@"
