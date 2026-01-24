# Highper Gateway - Local Testing Guide

This guide covers testing Highper Gateway on local development machines, Docker Compose, and Rancher Desktop.

## Prerequisites

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| Linux Kernel | 5.10+ (io_uring support) | 6.1+ |
| CPU | 2 cores | 4+ cores |
| Memory | 1 GB | 4+ GB |
| Docker | 20.10+ | 24.0+ |

### Platform Compatibility

| Platform | io_uring Support | Notes |
|----------|------------------|-------|
| Linux (native) | Full | Best performance |
| WSL2 (Windows) | Full | Kernel 5.10+ required |
| Docker Desktop (Mac) | Limited | Runs in Linux VM |
| Docker Desktop (Windows) | Full (WSL2) | Requires WSL2 backend |
| Rancher Desktop | Full | With WSL2/Lima |

---

## Option 1: Local Development Machine (Linux/WSL2)

### Step 1: Build from Source

```bash
# Clone repository
git clone https://github.com/highperapp/highper-gateway.git
cd highper-gateway

# Build release binary
cargo build --release

# Binary is at ./target/release/highper-gateway
```

### Step 2: Create Configuration

```bash
# Create config directory
mkdir -p ~/.config/highper-gateway

# Copy a use case config (e.g., HTTP Load Balancer)
cp deploy/configs/yaml/uc02-http-lb.yaml ~/.config/highper-gateway/config.yaml
```

### Step 3: Start with Test Backends

```bash
# Terminal 1: Start test backend (using Python)
python3 -m http.server 8081

# Terminal 2: Start another backend
python3 -m http.server 8082

# Terminal 3: Start gateway
./target/release/highper-gateway --config ~/.config/highper-gateway/config.yaml
```

### Step 4: Test

```bash
# Test HTTP load balancing
curl -v http://localhost:8080/

# Check health
curl http://localhost:8081/health

# Check metrics
curl http://localhost:9090/metrics
```

---

## Option 2: Docker Compose

### Step 1: Prepare Environment

```bash
cd deploy/docker

# Create config directory
mkdir -p config

# Copy use case configuration
cp ../configs/yaml/uc02-http-lb.yaml config/config.yaml
```

### Step 2: Modify Configuration for Docker Networking

Edit `config/config.yaml` to use Docker service names:

```yaml
backends:
  - name: web-servers
    strategy: round_robin
    servers:
      # Use Docker service names instead of localhost
      - address: "backend-1:80"
        weight: 1
      - address: "backend-2:80"
        weight: 1
```

### Step 3: Start Services

```bash
# Start gateway only
docker-compose up -d highper-gateway

# Start with test backends
docker-compose --profile with-backends up -d

# Start with monitoring stack
docker-compose --profile monitoring up -d

# Start everything
docker-compose --profile with-backends --profile monitoring up -d
```

### Step 4: Verify

```bash
# Check containers
docker-compose ps

# View logs
docker-compose logs -f highper-gateway

# Test gateway
curl http://localhost:8080/

# Access Grafana (if monitoring enabled)
# http://localhost:3000 (admin/admin)
```

### Step 5: Test Different Use Cases

```bash
# Stop and switch to API Gateway use case
docker-compose down
cp ../configs/yaml/uc04-api-gateway.yaml config/config.yaml
docker-compose up -d
```

### Docker Compose Commands Reference

```bash
# Start
docker-compose up -d

# Stop
docker-compose down

# View logs
docker-compose logs -f highper-gateway

# Restart gateway (config change)
docker-compose restart highper-gateway

# Full rebuild
docker-compose up -d --build --force-recreate
```

---

## Option 3: Rancher Desktop

Rancher Desktop provides Docker and Kubernetes on local machines.

### Step 1: Configure Rancher Desktop

1. Install Rancher Desktop from https://rancherdesktop.io/
2. Choose **dockerd (moby)** as container runtime
3. Enable **WSL integration** (Windows) or **Lima VM** (macOS)
4. Allocate sufficient resources (4 CPU, 4GB RAM recommended)

### Step 2: Using Docker Mode

Same as Docker Compose (Option 2):

```bash
cd deploy/docker
docker-compose up -d
```

### Step 3: Using Kubernetes Mode

```bash
# Install with Helm
cd deploy/helm

# Create namespace
kubectl create namespace highper-gateway

# Install chart
helm install highper-gateway ./highper-gateway \
  --namespace highper-gateway \
  --set useCase="02" \
  --set replicaCount=1

# Check pods
kubectl get pods -n highper-gateway

# Port forward to test
kubectl port-forward -n highper-gateway svc/highper-gateway 8080:8080

# Test
curl http://localhost:8080/
```

### Step 4: View in Rancher Dashboard

1. Open Rancher Desktop dashboard
2. Navigate to **Workloads** > **Pods**
3. View logs and resource usage

---

## Testing Each Use Case

### Quick Test Matrix

| Use Case | Port(s) | Test Command |
|----------|---------|--------------|
| UC01 TCP Proxy | 3306, 5432 | `nc -zv localhost 3306` |
| UC02 HTTP LB | 8080 | `curl http://localhost:8080/` |
| UC03 HTTPS/TLS | 443 | `curl -k https://localhost/` |
| UC04 API Gateway | 443 | `curl -k https://localhost/v1/health` |
| UC05 HTTP/3 | 443/UDP | `curl --http3 https://localhost/` |
| UC06 WebSocket | 8080 | `websocat ws://localhost:8080/ws` |
| UC07 gRPC | 9090 | `grpcurl localhost:9090 list` |
| UC08 Database LB | 3306 | `mysql -h localhost -P 3306` |
| UC09 WAF+mTLS | 443 | `curl --cert client.crt https://localhost/` |
| UC10 Hybrid | Multiple | See config for ports |
| UC11 CDN Edge | 80 | `curl -I http://localhost/static/` |
| UC12 Discovery | 8080 | `curl http://localhost:8080/health` |
| UC13 GraphQL | 4000 | `curl -X POST http://localhost:4000/graphql` |
| UC14 Static+FastCGI | 80 | `curl http://localhost/test.php` |
| UC15 Geo LB | 80 | `curl -H "X-Forwarded-For: 8.8.8.8" http://localhost/` |

### Example: Testing UC04 API Gateway

```bash
# 1. Generate test certificates
cd deploy/docker
mkdir -p certs
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout certs/server.key \
  -out certs/server.crt \
  -subj "/CN=localhost"

# 2. Copy API gateway config
cp ../configs/yaml/uc04-api-gateway.yaml config/config.yaml

# 3. Start services
docker-compose up -d

# 4. Test rate limiting
for i in {1..20}; do
  curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/v1/users
  sleep 0.1
done
# Should see 429 when rate limited

# 5. Test with JWT (mock)
curl -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..." \
  http://localhost:8080/v1/orders
```

---

## Debugging

### View Logs

```bash
# Docker Compose
docker-compose logs -f highper-gateway

# Kubernetes
kubectl logs -f deployment/highper-gateway -n highper-gateway

# Local binary
RUST_LOG=debug ./highper-gateway --config config.yaml
```

### Check Health

```bash
# Health endpoint
curl http://localhost:8081/health

# Detailed health
curl http://localhost:8081/health/detailed
```

### Metrics

```bash
# Prometheus metrics
curl http://localhost:9090/metrics

# Key metrics to check
curl -s http://localhost:9090/metrics | grep -E "^highper_gateway"
```

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| `io_uring not available` | Old kernel | Use kernel 5.10+ or disable io_uring |
| `Permission denied` | Missing capabilities | Add `CAP_NET_BIND_SERVICE`, `CAP_IPC_LOCK` |
| `Connection refused` | Backend not running | Start backend services first |
| `Too many open files` | ulimit too low | Increase with `ulimit -n 65535` |

---

## Performance Testing

### Quick Benchmark

```bash
# Using wrk (install: apt install wrk)
wrk -t4 -c100 -d30s http://localhost:8080/

# Using hey (install: go install github.com/rakyll/hey@latest)
hey -n 10000 -c 100 http://localhost:8080/

# Using ab (Apache Bench)
ab -n 10000 -c 100 http://localhost:8080/
```

### Expected Results (Local Docker)

| Metric | Expected |
|--------|----------|
| Requests/sec | 10,000+ |
| Latency P50 | < 1ms |
| Latency P99 | < 10ms |

---

## Cleanup

```bash
# Docker Compose
docker-compose down -v  # -v removes volumes

# Kubernetes (Rancher)
helm uninstall highper-gateway -n highper-gateway
kubectl delete namespace highper-gateway

# Local
rm -rf ~/.config/highper-gateway
```

---

## Next Steps

After local testing:

1. **Deploy to staging** - Use Ansible playbooks in `deploy/ansible/`
2. **Run load tests** - See `tests/load/README.md`
3. **Production deployment** - Follow `deploy/DEPLOYMENT_STRATEGY.md`

## Support

- **Documentation:** https://github.com/highperapp/highper-gateway
- **Issues:** https://github.com/highperapp/highper-gateway/issues
