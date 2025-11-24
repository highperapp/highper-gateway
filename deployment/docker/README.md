# Docker Deployment

This directory contains Docker and Docker Compose configurations for deploying the Rust reverse proxy.

## Files

- **Dockerfile** - Alpine-based minimal image (~20MB)
- **Dockerfile.ubuntu** - Ubuntu-based image for better compatibility
- **docker-compose.yml** - Development/testing compose setup
- **docker-compose.production.yml** - Production-ready compose with monitoring
- **.dockerignore** - Files to exclude from Docker build context

## Quick Start

### 1. Build the Image

```bash
# From the repository root
cd /home/infy/reverse_proxy

# Build using Alpine (recommended - smaller)
docker build -f deployment/docker/Dockerfile -t highper-gateway:latest .

# Or build using Ubuntu (better compatibility)
docker build -f deployment/docker/Dockerfile.ubuntu -t highper-gateway:latest .
```

### 2. Run with Docker

```bash
# Create config directory
mkdir -p /opt/highper-gateway

# Copy your configuration
cp config-production-secure.toml /opt/highper-gateway/config.toml

# Run the container
docker run -d \
  --name highper-gateway \
  --restart unless-stopped \
  -p 80:8080 \
  -p 443:8443 \
  -p 9090:9090 \
  -v /opt/highper-gateway/config.toml:/etc/highper-gateway/config.toml:ro \
  -v /opt/highper-gateway/certs:/etc/highper-gateway/certs:ro \
  --ulimit nofile=65536:65536 \
  highper-gateway:latest
```

### 3. Run with Docker Compose

```bash
# Development setup
cd deployment/docker
docker-compose up -d

# Production setup
docker-compose -f docker-compose.production.yml up -d
```

## Image Variants

### Alpine-based (Dockerfile)
- **Size**: ~20-30 MB
- **Pros**: Minimal attack surface, smaller image
- **Cons**: May have compatibility issues with some libraries
- **Use when**: Production deployment, security is priority

### Ubuntu-based (Dockerfile.ubuntu)
- **Size**: ~100-150 MB
- **Pros**: Better compatibility, easier debugging
- **Cons**: Larger image, more packages
- **Use when**: Development, debugging, compatibility issues

## Configuration

### Environment Variables

```bash
# Logging level (error, warn, info, debug, trace)
RUST_LOG=info

# Backtrace on panic
RUST_BACKTRACE=1

# Custom config path
CONFIG_PATH=/etc/highper-gateway/config.toml
```

### Volume Mounts

**Required:**
- `/etc/highper-gateway/config.toml` - Configuration file (read-only)

**Optional:**
- `/etc/highper-gateway/certs` - TLS certificates (read-only)
- `/var/log/highper-gateway` - Log files
- `/var/lib/highper-gateway` - Persistent data

### Port Mapping

- `8080` - HTTP traffic
- `8443` - HTTPS traffic (if TLS enabled)
- `9090` - Admin API and metrics

## Docker Compose Setup

### Development (docker-compose.yml)

Includes:
- Rust proxy
- Prometheus
- Grafana

```bash
docker-compose up -d

# Access services
# Proxy: http://localhost:80
# Metrics: http://localhost:9090/metrics
# Prometheus: http://localhost:9091
# Grafana: http://localhost:3000 (admin/changeme)
```

### Production (docker-compose.production.yml)

Includes:
- Rust proxy with hardened security
- Prometheus with 90-day retention
- Grafana
- Alertmanager

**Setup:**

```bash
# 1. Create directories
sudo mkdir -p /opt/highper-gateway /opt/monitoring
sudo mkdir -p /var/log/highper-gateway /var/lib/highper-gateway

# 2. Copy configurations
sudo cp config-production-secure.toml /opt/highper-gateway/config.toml
sudo cp monitoring/*.yml /opt/monitoring/
sudo cp monitoring/*.json /opt/monitoring/

# 3. Set Grafana password
export GF_ADMIN_PASSWORD="your-secure-password"

# 4. Start services
cd deployment/docker
docker-compose -f docker-compose.production.yml up -d

# 5. Verify
docker-compose -f docker-compose.production.yml ps
docker-compose -f docker-compose.production.yml logs -f highper-gateway
```

## Health Checks

The Docker images include health checks that run every 30 seconds:

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/highper-gateway", "health"]
```

Check health status:
```bash
docker inspect --format='{{.State.Health.Status}}' highper-gateway
```

## Security Hardening

### Container Security

The production compose file includes:

```yaml
security_opt:
  - no-new-privileges:true  # Prevent privilege escalation
cap_drop:
  - ALL                      # Drop all capabilities
cap_add:
  - NET_BIND_SERVICE         # Only add required capabilities
```

### Read-Only Root Filesystem (Optional)

For maximum security, enable read-only root:

```yaml
read_only: true
tmpfs:
  - /tmp:noexec,nosuid,size=100m
```

### Resource Limits

Production limits:

```yaml
deploy:
  resources:
    limits:
      cpus: '8'
      memory: 8G
    reservations:
      cpus: '4'
      memory: 4G
```

## Monitoring

### Prometheus Metrics

Access metrics:
```bash
curl http://localhost:9090/metrics
```

### Grafana Dashboards

1. Open http://localhost:3000
2. Login with admin/changeme (development) or your password (production)
3. Add Prometheus data source: http://prometheus:9090
4. Import dashboard from `/monitoring/grafana-dashboard.json`

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs highper-gateway

# Check configuration
docker exec highper-gateway /usr/local/bin/highper-gateway validate --config /etc/highper-gateway/config.toml
```

### Permission Errors

```bash
# Ensure volumes are writable
sudo chown -R 1000:1000 /var/log/highper-gateway
sudo chown -R 1000:1000 /var/lib/highper-gateway
```

### High Memory Usage

```bash
# Check resource usage
docker stats highper-gateway

# Adjust memory limits in docker-compose.yml
```

### Network Issues

```bash
# Check port bindings
docker port highper-gateway

# Check network connectivity
docker exec highper-gateway nc -zv backend-server 8080
```

## Performance Tuning

### Ulimits

Set file descriptor limits:

```yaml
ulimits:
  nofile:
    soft: 65536
    hard: 65536
  nproc:
    soft: 4096
    hard: 4096
```

### CPU Affinity

Pin to specific cores:

```yaml
cpuset: "0-3"  # Use cores 0-3
```

### Network Performance

For high-throughput scenarios:

```yaml
sysctls:
  - net.core.somaxconn=4096
  - net.ipv4.tcp_max_syn_backlog=4096
  - net.ipv4.tcp_tw_reuse=1
```

## Updating

### Rolling Update

```bash
# Build new image
docker build -f deployment/docker/Dockerfile -t highper-gateway:v2 .

# Update compose file to use v2
# Then:
docker-compose up -d --no-deps highper-gateway
```

### Zero-Downtime Update

```bash
# Scale up
docker-compose up -d --scale highper-gateway=2

# Wait for new container to be healthy
# Then scale down old container
```

## Backup and Restore

### Backup Configuration

```bash
docker run --rm -v highper-gateway_proxy-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/proxy-data-backup.tar.gz -C /data .
```

### Restore Configuration

```bash
docker run --rm -v highper-gateway_proxy-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/proxy-data-backup.tar.gz -C /data
```

## Production Checklist

- [ ] Build image with specific version tag
- [ ] Use production compose file
- [ ] Set strong Grafana password
- [ ] Configure TLS certificates
- [ ] Set resource limits appropriate for your load
- [ ] Configure log rotation
- [ ] Set up external log aggregation
- [ ] Configure alerting
- [ ] Test health checks
- [ ] Test container restart behavior
- [ ] Document your specific configuration
