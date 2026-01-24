#!/bin/bash
# Highper Gateway - Installation Script
# Supports Debian/Ubuntu and RHEL/CentOS/Fedora

set -euo pipefail

# Configuration
VERSION="${HIGHPER_GATEWAY_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/bin}"
CONFIG_DIR="${CONFIG_DIR:-/etc/highper-gateway}"
LOG_DIR="${LOG_DIR:-/var/log/highper-gateway}"
DATA_DIR="${DATA_DIR:-/var/lib/highper-gateway}"
SERVICE_USER="highper-gateway"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

# Detect OS
detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS=$ID
        VERSION_ID=${VERSION_ID:-}
    else
        log_error "Cannot detect OS"
        exit 1
    fi
    log_info "Detected OS: $OS $VERSION_ID"
}

# Check kernel version
check_kernel() {
    KERNEL_VERSION=$(uname -r | cut -d. -f1,2)
    MIN_KERNEL="5.1"

    if [[ "$(printf '%s\n' "$MIN_KERNEL" "$KERNEL_VERSION" | sort -V | head -n1)" != "$MIN_KERNEL" ]]; then
        log_error "Kernel $KERNEL_VERSION is too old. Minimum required: 5.1"
        exit 1
    fi

    if [[ "$(printf '%s\n' "5.11" "$KERNEL_VERSION" | sort -V | head -n1)" != "5.11" ]]; then
        log_warn "Kernel $KERNEL_VERSION has limited io_uring support. Recommended: 5.11+"
    fi

    log_info "Kernel version: $KERNEL_VERSION"
}

# Install dependencies
install_deps() {
    log_info "Installing dependencies..."

    case $OS in
        ubuntu|debian)
            apt-get update
            apt-get install -y curl wget ca-certificates liburing2 jq
            ;;
        centos|rhel|rocky|almalinux|fedora)
            dnf install -y curl wget ca-certificates liburing jq || \
            yum install -y curl wget ca-certificates liburing jq
            ;;
        *)
            log_warn "Unsupported OS: $OS. Attempting to continue..."
            ;;
    esac
}

# Create user and directories
setup_system() {
    log_info "Setting up system..."

    # Create service user
    if ! id "$SERVICE_USER" &>/dev/null; then
        useradd --system --shell /usr/sbin/nologin --home "$DATA_DIR" "$SERVICE_USER"
        log_info "Created user: $SERVICE_USER"
    fi

    # Create directories
    mkdir -p "$CONFIG_DIR"/{certs,rules,backends}
    mkdir -p "$LOG_DIR"
    mkdir -p "$DATA_DIR"

    # Set permissions
    chown -R "$SERVICE_USER:$SERVICE_USER" "$CONFIG_DIR"
    chown -R "$SERVICE_USER:$SERVICE_USER" "$LOG_DIR"
    chown -R "$SERVICE_USER:$SERVICE_USER" "$DATA_DIR"
    chmod 750 "$CONFIG_DIR"
}

# Download binary
download_binary() {
    log_info "Downloading Highper Gateway $VERSION..."

    ARCH=$(uname -m)
    case $ARCH in
        x86_64)  ARCH="amd64" ;;
        aarch64) ARCH="arm64" ;;
        *)
            log_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac

    DOWNLOAD_URL="https://releases.highper-gateway.io/${VERSION}/highper-gateway-linux-${ARCH}"
    CHECKSUM_URL="https://releases.highper-gateway.io/${VERSION}/checksums.sha256"

    # Download binary
    if curl -fsSL "$DOWNLOAD_URL" -o /tmp/highper-gateway; then
        log_info "Downloaded binary"
    else
        log_error "Failed to download binary from $DOWNLOAD_URL"
        log_warn "Creating placeholder - replace with actual binary"
        echo "#!/bin/bash" > /tmp/highper-gateway
        echo "echo 'Placeholder binary - replace with actual Highper Gateway'" >> /tmp/highper-gateway
    fi

    # Install binary
    install -m 755 /tmp/highper-gateway "$INSTALL_DIR/highper-gateway"
    rm -f /tmp/highper-gateway

    # Set capabilities
    if command -v setcap &>/dev/null; then
        setcap 'cap_net_bind_service,cap_ipc_lock=+ep' "$INSTALL_DIR/highper-gateway" || true
        log_info "Set capabilities on binary"
    fi
}

# Configure system limits
configure_limits() {
    log_info "Configuring system limits..."

    cat > /etc/security/limits.d/highper-gateway.conf << EOF
# Highper Gateway limits
$SERVICE_USER soft memlock unlimited
$SERVICE_USER hard memlock unlimited
$SERVICE_USER soft nofile 1048576
$SERVICE_USER hard nofile 1048576
$SERVICE_USER soft nproc unlimited
$SERVICE_USER hard nproc unlimited
EOF
}

# Configure sysctl
configure_sysctl() {
    log_info "Configuring kernel parameters..."

    cat > /etc/sysctl.d/99-highper-gateway.conf << 'EOF'
# Highper Gateway network tuning
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.core.rmem_default = 16777216
net.core.wmem_default = 16777216
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_tw_reuse = 1
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
fs.file-max = 2097152
fs.nr_open = 2097152
vm.swappiness = 10
EOF

    sysctl -p /etc/sysctl.d/99-highper-gateway.conf || true
}

# Install default configuration
install_config() {
    log_info "Installing default configuration..."

    if [[ ! -f "$CONFIG_DIR/config.yaml" ]]; then
        cat > "$CONFIG_DIR/config.yaml" << 'EOF'
# Highper Gateway Configuration
server:
  name: "highper-gateway"

io_uring:
  entries: 4096
  sq_poll: true
  sq_poll_cpu: 0

logging:
  level: info
  format: json
  output: /var/log/highper-gateway/gateway.log

metrics:
  enabled: true
  endpoint: /metrics
  port: 9090

health:
  enabled: true
  endpoint: /health
  port: 8081
EOF
        chown "$SERVICE_USER:$SERVICE_USER" "$CONFIG_DIR/config.yaml"
        chmod 640 "$CONFIG_DIR/config.yaml"
    fi
}

# Install systemd service
install_service() {
    log_info "Installing systemd service..."

    cat > /etc/systemd/system/highper-gateway.service << EOF
[Unit]
Description=Highper Gateway - High Performance Reverse Proxy
After=network-online.target
Wants=network-online.target

[Service]
Type=exec
User=$SERVICE_USER
Group=$SERVICE_USER
ExecStart=$INSTALL_DIR/highper-gateway --config $CONFIG_DIR/config.yaml
ExecReload=/bin/kill -HUP \$MAINPID
Restart=on-failure
RestartSec=5
CapabilityBoundingSet=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
LimitMEMLOCK=infinity
LimitNOFILE=1048576
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=$LOG_DIR
ReadWritePaths=$DATA_DIR
StandardOutput=append:$LOG_DIR/gateway.log
StandardError=append:$LOG_DIR/error.log
WorkingDirectory=$DATA_DIR

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable highper-gateway
    log_info "Service installed and enabled"
}

# Install log rotation
install_logrotate() {
    log_info "Installing log rotation..."

    cat > /etc/logrotate.d/highper-gateway << EOF
$LOG_DIR/*.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    create 0640 $SERVICE_USER $SERVICE_USER
    sharedscripts
    postrotate
        systemctl reload highper-gateway > /dev/null 2>&1 || true
    endscript
}
EOF
}

# Main installation
main() {
    echo "=========================================="
    echo "  Highper Gateway Installation Script"
    echo "=========================================="

    check_root
    detect_os
    check_kernel
    install_deps
    setup_system
    download_binary
    configure_limits
    configure_sysctl
    install_config
    install_service
    install_logrotate

    echo ""
    echo "=========================================="
    log_info "Installation complete!"
    echo "=========================================="
    echo ""
    echo "Next steps:"
    echo "  1. Edit configuration: $CONFIG_DIR/config.yaml"
    echo "  2. Start the service:  systemctl start highper-gateway"
    echo "  3. Check status:       systemctl status highper-gateway"
    echo "  4. View logs:          journalctl -u highper-gateway -f"
    echo ""
}

# Run main
main "$@"
