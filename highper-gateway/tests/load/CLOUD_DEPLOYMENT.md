# Cloud Deployment Requirements

## Changes Required for Cloud Testing

### 1. Network Binding Changes

**Local (Current):**
```toml
[server]
bind = ["127.0.0.1:8080"]  # Localhost only
```

**Cloud (Required):**
```toml
[server]
bind = ["0.0.0.0:8080"]  # All interfaces
```

### 2. Backend URL Configuration

**Local (Current):**
```toml
servers = [
    { url = "http://localhost:8001" },
    { url = "http://localhost:8002" },
    { url = "http://localhost:8003" },
]
```

**Cloud (Required - Option A: Container Names):**
```toml
servers = [
    { url = "http://backend-http-1:8000" },
    { url = "http://backend-http-2:8000" },
    { url = "http://backend-http-3:8000" },
]
```

**Cloud (Required - Option B: Host Network):**
```toml
servers = [
    { url = "http://10.0.0.10:8001" },
    { url = "http://10.0.0.11:8002" },
    { url = "http://10.0.0.12:8003" },
]
```

### 3. Health Check URLs

**Local (Current):**
```bash
curl -s -f http://localhost:8001/health
```

**Cloud (Required):**
```bash
# Use public IP or DNS
curl -s -f http://${SERVER_IP}:8001/health
```

### 4. Load Generator Configuration

**Local (Current):**
- Single vegeta instance on WSL2
- Limited to ~2K req/s

**Cloud (Required):**
- Distributed load generation
- Multiple vegeta instances across different VMs
- Or use k6/Locust for higher throughput

**Example Distributed Setup:**
```bash
# Load generator 1
echo "GET http://${GATEWAY_IP}:8080/api/ping" | vegeta attack -rate=50000 -duration=30s > results1.bin

# Load generator 2 (different VM)
echo "GET http://${GATEWAY_IP}:8080/api/ping" | vegeta attack -rate=50000 -duration=30s > results2.bin

# Combine results
cat results1.bin results2.bin | vegeta report
```

### 5. File Paths and Permissions

**Changes:**
- Gateway binary location: `/usr/local/bin/highper-gateway` or `/opt/gateway/bin/`
- Config files: `/etc/highper-gateway/` or `/opt/gateway/config/`
- Logs: `/var/log/highper-gateway/` (with proper permissions)
- Results: `/var/lib/highper-gateway/results/` or cloud storage (S3, etc.)

### 6. Environment Variables

**Cloud Environment:**
```bash
# Gateway configuration
export GATEWAY_MODE=cloud
export GATEWAY_BIND_IP=0.0.0.0
export GATEWAY_PORT=8080

# Backend configuration
export BACKEND_NETWORK_MODE=bridge  # or host
export BACKEND_IPS="10.0.0.10,10.0.0.11,10.0.0.12"

# Cloud provider
export CLOUD_PROVIDER=vultr  # or phoenixnap, hetzner
export CLOUD_REGION=us-east
```

### 7. Docker Compose Changes

**Local (Current):**
```yaml
services:
  backend-http-1:
    ports:
      - "8001:8000"  # Host:Container
```

**Cloud (Option A: Bridge Network):**
```yaml
services:
  backend-http-1:
    networks:
      - gateway_network
    expose:
      - "8000"

  gateway:
    networks:
      - gateway_network
    ports:
      - "8080:8080"
      - "8443:8443"

networks:
  gateway_network:
    driver: bridge
```

**Cloud (Option B: Host Network - Best Performance):**
```yaml
services:
  backend-http-1:
    network_mode: host
    environment:
      - PORT=8001
```

### 8. Firewall and Security

**Required Rules:**
```bash
# Allow gateway ports
ufw allow 8080/tcp
ufw allow 8443/tcp
ufw allow 9443/tcp  # HTTP/3 QUIC

# Allow monitoring
ufw allow 9090/tcp  # Prometheus
ufw allow 3000/tcp  # Grafana

# Allow SSH
ufw allow 22/tcp
```

### 9. TLS Certificates (Scenario 03+)

**Local (Current):**
- Self-signed certificates
- No ACME

**Cloud (Required):**
```toml
[tls]
enabled = true

[tls.acme]
enabled = true
provider = "letsencrypt"
email = "admin@example.com"
domains = ["gateway.example.com"]
```

Or manual certificates:
```toml
[tls.certificates]
cert_path = "/etc/ssl/certs/gateway.crt"
key_path = "/etc/ssl/private/gateway.key"
```

### 10. Monitoring and Observability

**Cloud Setup:**
```toml
[observability.metrics]
enabled = true
bind = "0.0.0.0:9090"  # Accessible externally
path = "/metrics"

[observability.logging]
level = "info"
format = "json"
output = "/var/log/highper-gateway/gateway.log"

[observability.tracing]
enabled = true
endpoint = "http://jaeger:14268/api/traces"
```

## Cloud Deployment Script Template

```bash
#!/bin/bash
# deploy-to-cloud.sh

set -euo pipefail

CLOUD_PROVIDER=${1:-vultr}
GATEWAY_IP=${2:-}

if [ -z "$GATEWAY_IP" ]; then
    echo "Usage: $0 <cloud-provider> <gateway-ip>"
    exit 1
fi

# 1. Upload files to cloud server
rsync -avz --exclude 'target' \
    /mnt/e/my-opensource/highper-gateway/ \
    root@${GATEWAY_IP}:/opt/highper-gateway/

# 2. SSH and setup
ssh root@${GATEWAY_IP} << 'EOF'
cd /opt/highper-gateway/highper-gateway

# Install dependencies
apt-get update
apt-get install -y docker.io docker-compose build-essential

# Build gateway
cargo build --release

# Copy binary
cp target/release/highper-gateway /usr/local/bin/

# Setup systemd service
cat > /etc/systemd/system/highper-gateway.service <<'SERVICE'
[Unit]
Description=Highper Gateway
After=network.target

[Service]
Type=simple
User=gateway
ExecStart=/usr/local/bin/highper-gateway start --config /etc/highper-gateway/gateway.toml
Restart=always

[Install]
WantedBy=multi-user.target
SERVICE

# Start backends
cd tests/load/docker
docker-compose -f docker-compose-prebuilt.yml up -d

# Enable and start gateway
systemctl enable highper-gateway
systemctl start highper-gateway
EOF

echo "Deployment complete! Gateway available at http://${GATEWAY_IP}:8080"
```

## Testing from Local Machine to Cloud Gateway

```bash
#!/bin/bash
# test-cloud-gateway.sh

GATEWAY_IP=$1

echo "Testing cloud gateway at ${GATEWAY_IP}..."

# Run vegeta from local machine targeting cloud gateway
echo "GET http://${GATEWAY_IP}:8080/api/ping" | vegeta attack \
    -rate=50000 \
    -duration=30s \
    -timeout=5s \
    -workers=12 \
    -keepalive=true \
    > results-cloud.bin

# Generate report
vegeta report -type=json results-cloud.bin > results-cloud.json
vegeta report -type=text results-cloud.bin
```

## Summary of Changes

| Component | Local | Cloud |
|-----------|-------|-------|
| Gateway bind | 127.0.0.1 | 0.0.0.0 |
| Backend URLs | localhost:800X | container-name:8000 or IP |
| Health checks | localhost | server IP |
| Load generation | Single vegeta | Distributed |
| TLS | Self-signed | ACME/LetsEncrypt |
| Monitoring | Disabled | Enabled |
| Network | Bridge | Bridge or Host |
| Firewall | None | UFW rules |
