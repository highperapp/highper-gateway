# Configuration Examples

This directory contains validated configuration examples for highper-gateway using the Caddy-like DSL syntax.

## Available Examples

### 1. `simple-load-balancer.proxy`
**Purpose**: Local testing and development
**Use case**: Test load balancing on your local machine before cloud deployment

**Features**:
- Listens on `localhost:8080`
- Distributes to 3 local backends (ports 8081-8083)
- Round-robin load balancing
- Health checks every 10 seconds

**How to test locally**:
```bash
# Terminal 1-3: Start simple backend servers
python3 -m http.server 8081  # Backend 1
python3 -m http.server 8082  # Backend 2
python3 -m http.server 8083  # Backend 3

# Terminal 4: Start the gateway
./target/release/highper-gateway start -c examples/simple-load-balancer.proxy

# Terminal 5: Test load balancing
for i in {1..10}; do curl http://localhost:8080/ && echo; done
```

### 2. `cloud-load-balancer.proxy`
**Purpose**: Cloud deployment on Vultr, AWS, PhoenixNAP, etc.
**Use case**: Production-ready load balancing configuration

**Features**:
- Listens on all interfaces (via domain-style address)
- Configurable backend IPs
- Health checks, timeouts, admin API
- Prometheus metrics enabled

**How to use**:
1. Replace `10.0.1.10`, `10.0.1.11`, `10.0.1.12` with your actual backend IPs
2. Optionally change `api.gateway:8080` to match your desired listen address
3. Deploy to your proxy server
4. Access admin API on port 9090, metrics on `/metrics`

**Cloud deployment example**:
```bash
# Copy to proxy server
scp examples/cloud-load-balancer.proxy root@proxy-server:/root/gateway.proxy

# Edit backend IPs
ssh root@proxy-server
nano /root/gateway.proxy  # Replace 10.0.1.x with actual IPs

# Start the gateway
./highper-gateway start -c gateway.proxy
```

## DSL Syntax Guide

### Site Address Formats

✅ **Correct formats**:
```
localhost:8080              # Local testing
api.gateway:8080            # Domain-style (works on any interface)
example.com:8080            # Actual domain
https://example.com         # HTTPS with auto-TLS
:3306                       # TCP-only (for databases)
```

❌ **Incorrect formats** (will cause "No matching route found"):
```
0.0.0.0:8080                # Raw IP - doesn't work for routing
:8080                       # Port-only for HTTP - ambiguous
```

### Load Balancing Algorithms

```
lb round_robin              # Default - even distribution
lb least_conn               # Send to server with fewest connections
lb ip_hash                  # Session persistence (same client → same backend)
lb random                   # Random selection
lb consistent_hash          # For caching scenarios
```

### Health Check Configuration

```
health interval=10s path="/health" timeout=5s

# Options:
# - interval: How often to check (default: 10s)
# - path: HTTP path to check (default: /)
# - timeout: Request timeout (default: 5s)
# - healthy: Checks before marking healthy (default: 2)
# - unhealthy: Checks before marking unhealthy (default: 3)
```

### Complete Example with All Features

```
# Production-ready configuration
api.example.com:8080 {
    # Backend servers
    proxy http://backend1:8080 http://backend2:8080 http://backend3:8080

    # Load balancing
    lb least_conn

    # Health monitoring
    health interval=10s path="/health" timeout=5s healthy=2 unhealthy=3

    # Timeouts
    timeout 30s

    # Optional: CORS (if serving API)
    cors

    # Optional: Compression
    compress gzip br

    # Optional: Rate limiting
    rate_limit 1000 per 1m
}

# Global settings
log info                    # Logging level: debug, info, warn, error
admin :9090                 # Admin API port
metrics on                  # Enable Prometheus metrics
```

## Validation Before Deployment

**CRITICAL**: Always validate configuration locally before cloud deployment to avoid wasted time and money.

### Step 1: Syntax Check
```bash
# Check if config parses correctly
./target/release/highper-gateway validate -c your-config.proxy
```

### Step 2: Local Testing
```bash
# Start backends
python3 -c "
from http.server import HTTPServer, BaseHTTPRequestHandler
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'OK from backend')
    def log_message(self, *args): pass
HTTPServer(('', 8081), Handler).serve_forever()
" &

# Repeat for ports 8082, 8083, etc.

# Start gateway
./target/release/highper-gateway start -c your-config.proxy

# Test routing
curl http://localhost:8080/
```

### Step 3: Load Testing
```bash
# Install vegeta
go install github.com/tsenart/vegeta@latest

# Run load test
echo "GET http://localhost:8080/" | vegeta attack -duration=10s -rate=100 | vegeta report
```

## Common Issues and Solutions

### Issue 1: "No matching route found"

**Symptom**: Gateway starts but returns 404 for all requests

**Causes**:
- Using `0.0.0.0:8080` or raw IP addresses as site address
- Using `:8080` for HTTP (this is for TCP-only)

**Solution**:
Use domain-style addresses: `localhost:8080` or `api.gateway:8080`

### Issue 2: "Connection refused" to backends

**Symptom**: Gateway starts but can't reach backends

**Causes**:
- Backends not running
- Firewall blocking ports
- Wrong backend addresses

**Solution**:
```bash
# Check backends are running
curl http://backend-ip:8080/

# Check firewall (Ubuntu/Debian)
sudo ufw status
sudo ufw allow 8080/tcp

# Verify backend IPs in config
```

### Issue 3: Health checks failing

**Symptom**: Backends marked as unhealthy

**Causes**:
- Health check path doesn't exist
- Backend doesn't respond quickly enough
- Network connectivity issues

**Solution**:
```bash
# Test health check path manually
curl http://backend-ip:8080/health

# Increase timeout if backends are slow
health interval=10s path="/health" timeout=10s

# Or remove health checks for testing:
# (comment out the health line)
```

## Migration from YAML

If you have existing YAML configurations:

**YAML** (20+ lines):
```yaml
server:
  bind: ["0.0.0.0:8080"]

upstreams:
  - name: "backend"
    servers:
      - url: "http://backend1:8080"
      - url: "http://backend2:8080"
    load_balancing:
      algorithm: "round_robin"
    health_check:
      enabled: true
      interval: 10s

routes:
  - match:
      paths: ["/"]
    upstream: "backend"
```

**DSL** (4 lines):
```
api.gateway:8080 {
    proxy http://backend1:8080 http://backend2:8080
    health interval=10s
}
```

**10x simpler!**

## Next Steps

1. ✅ Test configurations locally using `simple-load-balancer.proxy`
2. ✅ Verify load balancing works as expected
3. ✅ Validate health checks function correctly
4. ✅ Only then proceed to cloud deployment using `cloud-load-balancer.proxy`

**Remember**: Local validation is FREE. Cloud debugging costs money. Always validate locally first!

## Additional Resources

- `docs/dev-notes/DSL_USER_GUIDE.md` - Complete DSL reference
- `docs/dev-notes/DSL_DESIGN.md` - Design specifications
- `load-tests/LOAD_TEST_PREPARATION_CHECKLIST.md` - Pre-deployment checklist
- `load-tests/INFRASTRUCTURE_SELECTION_GUIDE.md` - Cloud provider selection guide
