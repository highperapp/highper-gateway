#!/bin/bash
# Production deployment script for Highper Gateway
# Run as root: sudo ./deploy.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "========================================="
echo "Highper Gateway - Production Deployment"
echo "========================================="
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "Error: This script must be run as root (use sudo)"
   exit 1
fi

# ========================================
# 1. Create User & Directories
# ========================================
echo "Step 1: Creating user and directories..."

# Create highper-gateway user if it doesn't exist
if ! id -u highper-gateway >/dev/null 2>&1; then
    useradd --system --no-create-home --shell /bin/false highper-gateway
    echo "✓ Created user: highper-gateway"
else
    echo "✓ User already exists: highper-gateway"
fi

# Create directories
mkdir -p /etc/highper-gateway
mkdir -p /var/lib/highper-gateway/{certs,cache,data}
mkdir -p /var/log/highper-gateway
mkdir -p /usr/local/bin

echo "✓ Directories created"

# ========================================
# 2. Build Release Binary
# ========================================
echo ""
echo "Step 2: Building release binary..."

cd "$PROJECT_ROOT/highper-gateway"

if command -v cargo &> /dev/null; then
    echo "Building with cargo..."
    cargo build --release
    BINARY_PATH="$PROJECT_ROOT/target/release/highper-gateway"
else
    echo "Error: cargo not found. Please install Rust."
    exit 1
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo "Error: Binary not found at $BINARY_PATH"
    exit 1
fi

echo "✓ Binary built successfully"

# ========================================
# 3. Install Binary
# ========================================
echo ""
echo "Step 3: Installing binary..."

cp "$BINARY_PATH" /usr/local/bin/highper-gateway
chmod +x /usr/local/bin/highper-gateway
chown root:root /usr/local/bin/highper-gateway

# Give capability to bind to privileged ports
setcap 'cap_net_bind_service=+ep' /usr/local/bin/highper-gateway

echo "✓ Binary installed to /usr/local/bin/highper-gateway"

# ========================================
# 4. Install Configuration
# ========================================
echo ""
echo "Step 4: Installing configuration..."

if [[ -f "$PROJECT_ROOT/config.example.yaml" ]]; then
    if [[ ! -f /etc/highper-gateway/config.yaml ]]; then
        cp "$PROJECT_ROOT/config.example.yaml" /etc/highper-gateway/config.yaml
        echo "✓ Configuration template copied"
        echo "  Please edit /etc/highper-gateway/config.yaml before starting"
    else
        echo "⚠  Configuration already exists, skipping"
    fi
fi

# Set permissions
chown -R highper-gateway:highper-gateway /etc/highper-gateway
chown -R highper-gateway:highper-gateway /var/lib/highper-gateway
chown -R highper-gateway:highper-gateway /var/log/highper-gateway

echo "✓ Permissions set"

# ========================================
# 5. Apply Kernel Tuning
# ========================================
echo ""
echo "Step 5: Applying kernel tuning..."

if [[ -f "$SCRIPT_DIR/kernel-tuning.sh" ]]; then
    bash "$SCRIPT_DIR/kernel-tuning.sh"
else
    echo "⚠  Kernel tuning script not found, skipping"
fi

# ========================================
# 6. Install Systemd Service
# ========================================
echo ""
echo "Step 6: Installing systemd service..."

if [[ -f "$SCRIPT_DIR/highper-gateway.service" ]]; then
    cp "$SCRIPT_DIR/highper-gateway.service" /etc/systemd/system/
    systemctl daemon-reload
    echo "✓ Systemd service installed"
else
    echo "⚠  Service file not found at $SCRIPT_DIR/highper-gateway.service"
fi

# ========================================
# 7. Configure Firewall (optional)
# ========================================
echo ""
echo "Step 7: Configuring firewall..."

if command -v ufw &> /dev/null; then
    echo "UFW detected, opening ports..."
    ufw allow 80/tcp comment 'Highper Gateway HTTP'
    ufw allow 443/tcp comment 'Highper Gateway HTTPS'
    echo "✓ Firewall rules added (ufw)"
elif command -v firewall-cmd &> /dev/null; then
    echo "firewalld detected, opening ports..."
    firewall-cmd --permanent --add-service=http
    firewall-cmd --permanent --add-service=https
    firewall-cmd --reload
    echo "✓ Firewall rules added (firewalld)"
else
    echo "⚠  No firewall detected, skipping"
fi

# ========================================
# 8. Enable and Start Service
# ========================================
echo ""
echo "Step 8: Enabling service..."

systemctl enable highper-gateway
echo "✓ Service enabled (will start on boot)"

# ========================================
# Summary
# ========================================
echo ""
echo "========================================="
echo "Deployment Complete!"
echo "========================================="
echo ""
echo "Installation Summary:"
echo "  Binary: /usr/local/bin/highper-gateway"
echo "  Config: /etc/highper-gateway/config.yaml"
echo "  Data:   /var/lib/highper-gateway/"
echo "  Logs:   /var/log/highper-gateway/"
echo "  Service: highper-gateway.service"
echo ""
echo "Next Steps:"
echo "  1. Edit configuration:"
echo "     sudo nano /etc/highper-gateway/config.yaml"
echo ""
echo "  2. Validate configuration:"
echo "     sudo -u highper-gateway /usr/local/bin/highper-gateway --validate-config"
echo ""
echo "  3. Start the service:"
echo "     sudo systemctl start highper-gateway"
echo ""
echo "  4. Check status:"
echo "     sudo systemctl status highper-gateway"
echo ""
echo "  5. View logs:"
echo "     sudo journalctl -u highper-gateway -f"
echo ""
echo "  6. Monitor connections:"
echo "     watch -n1 'ss -s'"
echo ""
echo "Useful Commands:"
echo "  Start:   systemctl start highper-gateway"
echo "  Stop:    systemctl stop highper-gateway"
echo "  Restart: systemctl restart highper-gateway"
echo "  Reload:  systemctl reload highper-gateway  (SIGHUP - hot reload config)"
echo "  Status:  systemctl status highper-gateway"
echo "  Logs:    journalctl -u highper-gateway -f"
echo ""
echo "Performance Monitoring:"
echo "  systemctl status highper-gateway  # Shows FD usage, memory, CPU"
echo "  ss -s                        # Socket statistics"
echo "  ss -ant | grep TIME-WAIT | wc -l  # TIME_WAIT count"
echo ""
echo "========================================="
