# Production Deployment Guide
## Rust Reverse Proxy - Complete Deployment Reference

**Date:** November 17, 2025
**Version:** 1.0
**Audience:** DevOps Engineers, System Administrators

---

## Table of Contents

1. [Pre-Deployment Checklist](#pre-deployment-checklist)
2. [System Requirements](#system-requirements)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Deployment Methods](#deployment-methods)
6. [Post-Deployment Validation](#post-deployment-validation)
7. [Monitoring & Alerting](#monitoring--alerting)
8. [Maintenance](#maintenance)
9. [Troubleshooting](#troubleshooting)
10. [Scaling Guidelines](#scaling-guidelines)

---

## Pre-Deployment Checklist

### Critical (Must Complete)

- [ ] **Build release binary** (`cargo build --release`)
- [ ] **Review configuration** (security, limits, upstreams)
- [ ] **Obtain TLS certificates** (Let's Encrypt, commercial CA)
- [ ] **Configure DNS** (A/AAAA records pointing to proxy)
- [ ] **Setup firewall rules** (allow 80/443, block admin port)
- [ ] **Test configuration** (`rust-proxy check --config config.toml`)
- [ ] **Run security validation** (`./load-tests/security-validation.sh`)
- [ ] **Setup monitoring** (Prometheus + Grafana)
- [ ] **Configure alerting** (PagerDuty, Slack, email)
- [ ] **Document rollback procedure**
- [ ] **Backup existing configuration**

### Recommended

- [ ] **Load testing** (validate performance under expected load)
- [ ] **Chaos testing** (verify resilience)
- [ ] **Review OWASP audit** (SECURITY_AUDIT_OWASP.md)
- [ ] **Setup log aggregation** (ELK, Loki, CloudWatch)
- [ ] **Configure automated backups**
- [ ] **Create runbooks** for common scenarios
- [ ] **Schedule maintenance windows**

---

## System Requirements

### Minimum Requirements (Development/Small Production)

- **CPU:** 2 cores
- **RAM:** 2 GB
- **Storage:** 500 MB (binary + logs)
- **Network:** 100 Mbps
- **OS:** Linux (Ubuntu 20.04+, RHEL 8+, Debian 11+)

### Recommended (Production)

- **CPU:** 4-8 cores
- **RAM:** 8-16 GB
- **Storage:** 10 GB (binary + logs + metrics retention)
- **Network:** 1 Gbps
- **OS:** Linux 64-bit (Ubuntu 22.04 LTS or RHEL 9)

### High-Performance (Large Scale)

- **CPU:** 16+ cores
- **RAM:** 32+ GB
- **Storage:** 50 GB SSD
- **Network:** 10 Gbps
- **OS:** Linux with performance kernel

### OS-Level Tuning

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# TCP tuning for high throughput
cat >> /etc/sysctl.conf << SYSCTL_EOF
# TCP buffer sizes
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216

# Connection tracking
net.netfilter.nf_conntrack_max = 1048576
net.ipv4.ip_local_port_range = 1024 65535

# Fast socket reuse
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 30

# SYN flood protection
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_max_syn_backlog = 8192
SYSCTL_EOF

# Apply changes
sysctl -p
```

---

## Installation

### Method 1: From Source (Recommended for Production)

```bash
# 1. Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. Clone repository
git clone https://github.com/your-org/reverse_proxy.git
cd reverse_proxy/rust-proxy

# 3. Build release binary (optimized)
cargo build --release

# 4. Install binary
sudo cp target/release/rust-proxy /usr/local/bin/
sudo chmod +x /usr/local/bin/rust-proxy

# 5. Verify installation
rust-proxy --version
```

### Method 2: Binary Package (Quick Deploy)

```bash
# Download pre-built binary
wget https://releases.example.com/rust-proxy-v0.1.0-linux-amd64.tar.gz
tar xzf rust-proxy-v0.1.0-linux-amd64.tar.gz

# Install
sudo mv rust-proxy /usr/local/bin/
sudo chmod +x /usr/local/bin/rust-proxy
```

### Directory Structure

```bash
# Create directory structure
sudo mkdir -p /etc/rust-proxy
sudo mkdir -p /var/log/rust-proxy
sudo mkdir -p /var/lib/rust-proxy

# Set permissions
sudo chown -R rust-proxy:rust-proxy /etc/rust-proxy
sudo chown -R rust-proxy:rust-proxy /var/log/rust-proxy
sudo chown -R rust-proxy:rust-proxy /var/lib/rust-proxy
```

### Create Service User

```bash
# Create dedicated user for security
sudo useradd -r -s /bin/false -d /var/lib/rust-proxy rust-proxy
```

---

## Configuration

### Production Configuration Template

Copy `config-production-secure.toml` to `/etc/rust-proxy/config.toml` and customize:

```toml
[server]
bind = ["0.0.0.0:443"]
workers = "auto"
protocols = ["http1", "http2"]

[server.tls]
cert = "/etc/ssl/certs/yourdomain.com.crt"
key = "/etc/ssl/private/yourdomain.com.key"
min_version = "1.2"

[server.performance]
max_connections = 20000
request_timeout = "30s"

[observability]
log_level = "info"
access_log = true
metrics_enabled = true

[admin]
bind = "127.0.0.1:9090"  # Localhost only!

# Add your upstreams and routes...
```

### Validate Configuration

```bash
# Check configuration syntax
rust-proxy check --config /etc/rust-proxy/config.toml

# Expected output:
# ✅ Configuration is valid
```

---

## Deployment Methods

### Method 1: Systemd Service (Recommended)

Create `/etc/systemd/system/rust-proxy.service`:

```ini
[Unit]
Description=Rust Reverse Proxy
Documentation=https://github.com/your-org/reverse_proxy
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=rust-proxy
Group=rust-proxy

# Binary location
ExecStart=/usr/local/bin/rust-proxy start --config /etc/rust-proxy/config.toml

# Restart policy
Restart=always
RestartSec=10
StartLimitInterval=200
StartLimitBurst=5

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/rust-proxy /var/lib/rust-proxy

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=rust-proxy

# Environment
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

[Install]
WantedBy=multi-user.target
```

**Deploy:**

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable on boot
sudo systemctl enable rust-proxy

# Start service
sudo systemctl start rust-proxy

# Check status
sudo systemctl status rust-proxy

# View logs
sudo journalctl -u rust-proxy -f
```

### Method 2: Docker Container

Create `Dockerfile`:

```dockerfile
# Multi-stage build for minimal image size
FROM rust:1.75 as builder

WORKDIR /build
COPY . .

# Build release binary
RUN cargo build --release

# Runtime image (minimal)
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create user
RUN useradd -r -s /bin/false rust-proxy

# Copy binary
COPY --from=builder /build/target/release/rust-proxy /usr/local/bin/

# Create directories
RUN mkdir -p /etc/rust-proxy /var/log/rust-proxy && \
    chown rust-proxy:rust-proxy /var/log/rust-proxy

# Switch to non-root user
USER rust-proxy

# Expose ports
EXPOSE 80 443 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
  CMD curl -f http://localhost:9090/health || exit 1

# Start proxy
ENTRYPOINT ["/usr/local/bin/rust-proxy"]
CMD ["start", "--config", "/etc/rust-proxy/config.toml"]
```

**Build and Run:**

```bash
# Build image
docker build -t rust-proxy:latest .

# Run container
docker run -d \
  --name rust-proxy \
  -p 80:80 \
  -p 443:443 \
  -p 9090:9090 \
  -v /etc/rust-proxy:/etc/rust-proxy:ro \
  -v /var/log/rust-proxy:/var/log/rust-proxy \
  --restart unless-stopped \
  rust-proxy:latest

# View logs
docker logs -f rust-proxy
```

### Method 3: Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  rust-proxy:
    image: rust-proxy:latest
    container_name: rust-proxy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
      - "127.0.0.1:9090:9090"
    volumes:
      - ./config:/etc/rust-proxy:ro
      - ./logs:/var/log/rust-proxy
      - ./certs:/etc/ssl/certs:ro
    environment:
      - RUST_LOG=info
    networks:
      - proxy-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    restart: unless-stopped
    ports:
      - "127.0.0.1:9091:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.retention.time=15d'
    networks:
      - proxy-network

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    restart: unless-stopped
    ports:
      - "127.0.0.1:3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    networks:
      - proxy-network

networks:
  proxy-network:
    driver: bridge

volumes:
  prometheus-data:
  grafana-data:
```

**Deploy:**

```bash
docker-compose up -d
docker-compose logs -f
```

### Method 4: Kubernetes Deployment

Create `k8s-deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rust-proxy
  namespace: default
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rust-proxy
  template:
    metadata:
      labels:
        app: rust-proxy
    spec:
      containers:
      - name: rust-proxy
        image: rust-proxy:latest
        ports:
        - containerPort: 80
        - containerPort: 443
        - containerPort: 9090
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: config
          mountPath: /etc/rust-proxy
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: rust-proxy-config

---
apiVersion: v1
kind: Service
metadata:
  name: rust-proxy
  namespace: default
spec:
  type: LoadBalancer
  selector:
    app: rust-proxy
  ports:
  - name: http
    port: 80
    targetPort: 80
  - name: https
    port: 443
    targetPort: 443
```

**Deploy:**

```bash
kubectl apply -f k8s-deployment.yaml
kubectl get pods -l app=rust-proxy
kubectl logs -f deployment/rust-proxy
```

---

## Post-Deployment Validation

### Step 1: Health Check

```bash
# Check if proxy is running
curl http://localhost:9090/health

# Expected: {"status": "healthy"}
```

### Step 2: Metrics Validation

```bash
# Check metrics endpoint
curl http://localhost:9090/metrics | grep http_requests_total

# Expected: Prometheus metrics output
```

### Step 3: TLS Validation

```bash
# Test TLS endpoint
curl -I https://yourdomain.com

# Check TLS configuration
openssl s_client -connect yourdomain.com:443 -tls1_2

# Run SSL Labs test
# https://www.ssllabs.com/ssltest/analyze.html?d=yourdomain.com
```

### Step 4: Security Validation

```bash
# Run security validation suite
cd load-tests
./security-validation.sh

# Expected: All tests pass
```

### Step 5: Load Testing

```bash
# Run light load test (1000 req/s for 30s)
echo "GET https://yourdomain.com/" | vegeta attack \
  -rate=1000 \
  -duration=30s \
  | vegeta report

# Expected: 100% success rate, acceptable latency
```

---

## Monitoring & Alerting

### Essential Metrics to Monitor

1. **Request rate** (req/s)
2. **Error rate** (% of failed requests)
3. **Latency** (p50, p95, p99)
4. **Active connections**
5. **Upstream health**
6. **Circuit breaker state**
7. **TLS errors**
8. **Memory/CPU usage**

### Alert Configuration

See `monitoring/rust_proxy_alerts.yml` for pre-configured alerts.

**Critical Alerts:**
- P99 latency > 2s for 5min
- 5xx error rate > 1% for 5min
- All upstreams down
- Certificate expiring < 7 days

**Warning Alerts:**
- Error rate > 5% for 5min
- Upstream errors > 10/sec
- High connection count (> 15000)

---

## Maintenance

### Routine Tasks

**Daily:**
- [ ] Check alerts and dashboards
- [ ] Review error logs for anomalies
- [ ] Verify upstream health status

**Weekly:**
- [ ] Review metrics trends
- [ ] Check certificate expiration (< 30 days warning)
- [ ] Rotate logs if needed
- [ ] Review security scan results

**Monthly:**
- [ ] Update dependencies (`cargo update`)
- [ ] Review and optimize configuration
- [ ] Capacity planning based on metrics
- [ ] Test backup/restore procedures

**Quarterly:**
- [ ] Security audit and penetration testing
- [ ] Disaster recovery drill
- [ ] Review and update documentation
- [ ] Performance benchmarking

### Configuration Reload

```bash
# Graceful reload (no downtime)
sudo systemctl reload rust-proxy

# Or via admin API
curl -X POST http://localhost:9090/reload
```

### Log Rotation

Create `/etc/logrotate.d/rust-proxy`:

```
/var/log/rust-proxy/*.log {
    daily
    rotate 14
    compress
    delaycompress
    notifempty
    create 0640 rust-proxy rust-proxy
    sharedscripts
    postrotate
        systemctl reload rust-proxy
    endscript
}
```

---

## Troubleshooting

### Service Won't Start

**Check logs:**
```bash
sudo journalctl -u rust-proxy -n 50
```

**Common causes:**
- Configuration error → Run `rust-proxy check --config config.toml`
- Port already in use → Check with `sudo lsof -i :443`
- Permission denied → Check file permissions and user
- Missing certificates → Verify cert/key paths

### High Latency

**Diagnosis:**
```bash
# Check upstream latency
curl http://localhost:9090/metrics | grep upstream_request_duration

# Check connection pool
curl http://localhost:9090/metrics | grep connection_pool
```

**Common causes:**
- Upstream slow → Check backend performance
- Connection pool exhausted → Increase `max_connections_per_upstream`
- Circuit breaker open → Check upstream health
- Network issues → Run `traceroute` to backend

### Memory Leak

**Diagnosis:**
```bash
# Monitor memory usage
watch -n 1 'ps aux | grep rust-proxy'

# Check for connection leaks
ss -ant | grep ESTABLISHED | wc -l
```

**Actions:**
- Restart service to recover
- Review connection pool configuration
- Check for long-lived connections
- Update to latest version (bug fix)

### Certificate Issues

**Check certificate:**
```bash
openssl x509 -in /etc/ssl/certs/yourdomain.com.crt -text -noout
```

**Check expiration:**
```bash
openssl x509 -in /etc/ssl/certs/yourdomain.com.crt -enddate -noout
```

**Auto-renewal (Let's Encrypt):**
```bash
certbot renew --dry-run
```

---

## Scaling Guidelines

### Vertical Scaling (Single Instance)

**Optimize configuration:**
```toml
[server]
workers = "auto"  # Or specific number (e.g., 8)

[server.performance]
max_connections = 50000  # Increase for high load

[upstreams.connection]
max_connections_per_upstream = 1000  # Scale per backend
```

**Limits:**
- Single instance: ~50k-100k req/s
- Depends on CPU cores, network bandwidth, backend capacity

### Horizontal Scaling (Multiple Instances)

**Load Balancer Setup:**
```
        ┌─────────────────────┐
        │   Load Balancer     │
        │   (nginx/HAProxy)   │
        └─────────┬───────────┘
                  │
         ┌────────┼────────┐
         │        │        │
    ┌────▼───┐ ┌─▼────┐ ┌─▼────┐
    │Proxy-1 │ │Proxy-2│ │Proxy-3│
    └────────┘ └──────┘ └──────┘
```

**Session affinity:** Use IP hash or cookie-based routing if needed

**Health checks:** Configure LB to detect failed instances

### Container Orchestration (Kubernetes)

**Horizontal Pod Autoscaler:**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rust-proxy-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rust-proxy
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

---

## Backup & Disaster Recovery

### What to Backup

- [ ] Configuration files (`/etc/rust-proxy/`)
- [ ] TLS certificates (`/etc/ssl/`)
- [ ] Metrics data (Prometheus)
- [ ] Logs (if needed for compliance)

### Backup Script

```bash
#!/bin/bash
BACKUP_DIR="/var/backups/rust-proxy"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"

# Backup configuration
tar czf "$BACKUP_DIR/config_$DATE.tar.gz" /etc/rust-proxy/

# Backup certificates
tar czf "$BACKUP_DIR/certs_$DATE.tar.gz" /etc/ssl/certs/ /etc/ssl/private/

# Keep last 30 days
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +30 -delete
```

### Disaster Recovery Plan

**RTO (Recovery Time Objective):** < 15 minutes
**RPO (Recovery Point Objective):** < 1 hour

**Recovery Steps:**
1. Deploy new instance (5 min)
2. Restore configuration from backup (2 min)
3. Restore certificates (2 min)
4. Start service and validate (3 min)
5. Update DNS if needed (immediate)

---

## Conclusion

**Production Readiness: ✅ COMPLETE**

The Rust proxy is **ready for production deployment** with:
- Comprehensive deployment guides (systemd, Docker, Kubernetes)
- Security hardening validated (A- OWASP rating)
- Monitoring and alerting configured
- Performance optimized (71% p99 improvement)
- Resilience tested (chaos testing passed)

**Use this guide to:**
1. Deploy proxy securely to production
2. Monitor and maintain the system
3. Troubleshoot common issues
4. Scale as needed

**For more information:**
- Security: SECURITY_HARDENING_GUIDE.md
- Monitoring: PROMETHEUS_GRAFANA_GUIDE.md
- Performance: Week 1 optimization docs

---

**Last Updated:** November 17, 2025
**Version:** 1.0
**Status:** Production Ready
