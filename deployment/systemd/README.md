# Systemd Service Files

This directory contains systemd service files for deploying the Rust reverse proxy.

## Files

- **highper-gateway.service** - Basic service file (unprivileged ports 8080+)
- **highper-gateway-privileged.service** - Service file with CAP_NET_BIND_SERVICE (for ports 80/443)

## Installation Steps

### 1. Create User and Group

```bash
sudo useradd -r -s /bin/false highper-gateway
sudo usermod -aG highper-gateway highper-gateway
```

### 2. Create Directories

```bash
sudo mkdir -p /opt/highper-gateway
sudo mkdir -p /etc/highper-gateway
sudo mkdir -p /var/log/highper-gateway
sudo mkdir -p /var/lib/highper-gateway

sudo chown highper-gateway:highper-gateway /opt/highper-gateway
sudo chown highper-gateway:highper-gateway /var/log/highper-gateway
sudo chown highper-gateway:highper-gateway /var/lib/highper-gateway
```

### 3. Install Binary

```bash
# Build the release binary
cd highper-gateway
cargo build --release

# Install the binary
sudo cp target/release/highper-gateway /usr/local/bin/
sudo chown root:root /usr/local/bin/highper-gateway
sudo chmod 755 /usr/local/bin/highper-gateway
```

### 4. Install Configuration

```bash
# Copy your production configuration
sudo cp config-production-secure.toml /etc/highper-gateway/config.toml
sudo chown root:highper-gateway /etc/highper-gateway/config.toml
sudo chmod 640 /etc/highper-gateway/config.toml
```

### 5. Install Service File

**For unprivileged ports (8080, 8443):**
```bash
sudo cp deployment/systemd/highper-gateway.service /etc/systemd/system/
```

**For privileged ports (80, 443):**
```bash
sudo cp deployment/systemd/highper-gateway-privileged.service /etc/systemd/system/highper-gateway.service
```

### 6. Enable and Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable highper-gateway

# Start the service
sudo systemctl start highper-gateway

# Check status
sudo systemctl status highper-gateway
```

## Service Management

### View Logs

```bash
# Real-time logs
sudo journalctl -u highper-gateway -f

# Last 100 lines
sudo journalctl -u highper-gateway -n 100

# Logs since yesterday
sudo journalctl -u highper-gateway --since yesterday
```

### Restart Service

```bash
sudo systemctl restart highper-gateway
```

### Stop Service

```bash
sudo systemctl stop highper-gateway
```

### Reload Configuration

```bash
# If the proxy supports SIGHUP for config reload
sudo systemctl reload highper-gateway

# Otherwise, restart
sudo systemctl restart highper-gateway
```

## Troubleshooting

### Service Won't Start

```bash
# Check service status
sudo systemctl status highper-gateway

# Check detailed logs
sudo journalctl -u highper-gateway -n 50 --no-pager

# Verify configuration
/usr/local/bin/highper-gateway validate --config /etc/highper-gateway/config.toml
```

### Permission Issues

```bash
# Verify file ownership
ls -la /etc/highper-gateway/
ls -la /var/log/highper-gateway/
ls -la /var/lib/highper-gateway/

# Fix if needed
sudo chown -R highper-gateway:highper-gateway /var/log/highper-gateway
sudo chown -R highper-gateway:highper-gateway /var/lib/highper-gateway
```

### Port Binding Issues

```bash
# Check if port is already in use
sudo ss -tlnp | grep :80
sudo ss -tlnp | grep :443

# For privileged ports, ensure CAP_NET_BIND_SERVICE is set
sudo systemctl cat highper-gateway | grep -i capability
```

## Performance Tuning

Add these settings to the service file under `[Service]`:

```ini
# CPU affinity (bind to specific cores)
CPUAffinity=0-3

# Nice level (lower = higher priority, range -20 to 19)
Nice=-5

# I/O scheduling class
IOSchedulingClass=realtime
IOSchedulingPriority=0

# Memory limits (optional)
MemoryMax=4G
MemoryHigh=3G
```

## Security Hardening

The service files include security hardening directives:

- `NoNewPrivileges=true` - Prevents privilege escalation
- `PrivateTmp=true` - Isolates /tmp
- `ProtectSystem=strict` - Makes system directories read-only
- `ProtectHome=true` - Denies access to /home
- `ReadWritePaths=` - Explicitly allows write access only where needed

For additional hardening, see the systemd hardening guide:
https://www.freedesktop.org/software/systemd/man/systemd.exec.html
