# Highper Gateway - Deployment Guide

**Version:** 1.0
**Date:** January 11, 2026
**License:** Apache 2.0
**Status:** Production Ready

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Zero-Config Philosophy](#zero-config-philosophy)
4. [Common Deployment Patterns](#common-deployment-patterns)
5. [Scenario Deployments](#scenario-deployments)
   - [Scenario 01: Layer 4 TCP](#scenario-01-layer-4-tcp---pure-tcp-proxying)
   - [Scenario 02: HTTP/1.1](#scenario-02-layer-7-http---http11-load-balancing)
   - [Scenario 03: HTTPS/TLS](#scenario-03-httpstls-termination)
   - [Scenario 04: API Gateway](#scenario-04-api-gateway-with-rate-limiting)
   - [Scenario 05: HTTP/3 QUIC](#scenario-05-http3-quic)
   - [Scenario 06: WebSocket](#scenario-06-websocket-load-balancer)
   - [Scenario 07: gRPC](#scenario-07-grpc-gateway)
   - [Scenario 08: Database LB](#scenario-08-database-load-balancer)
   - [Scenario 09: WAF + mTLS](#scenario-09-waf--mtls)
   - [Scenario 10: Hybrid Multi-Protocol](#scenario-10-hybrid-multi-protocol)
   - [Scenario 11: CDN Caching](#scenario-11-cdn-edge-caching)
   - [Scenario 12: Service Discovery](#scenario-12-microservices-discovery)
   - [Scenario 13: GraphQL](#scenario-13-graphql-gateway)
   - [Scenario 14: PHP-FPM](#scenario-14-static--php-fpm)
   - [Scenario 15: Geo Load Balancing](#scenario-15-geographic-load-balancing)
6. [Daemon Management](#daemon-management)
7. [Monitoring & Health Checks](#monitoring--health-checks)
8. [Production Best Practices](#production-best-practices)
9. [Troubleshooting](#troubleshooting)

---

## Overview

This guide provides comprehensive deployment instructions for all 15 Highper Gateway use case scenarios. Each scenario includes:

- **Configuration Files**: Production-ready TOML/DSL configs
- **Daemon Setup**: systemd services, Docker Compose, or manual daemon management
- **Zero-Config Defaults**: Sensible defaults that work out-of-the-box
- **Best Practices**: Security, performance, and operational conventions
- **Health Checks**: Validation steps to ensure correct deployment

### Document Conventions

```bash
# Commands to run as root or with sudo
$ sudo systemctl start highper-gateway

# Commands to run as regular user
$ highper-gateway --config /etc/highper-gateway/config.toml

# Configuration file examples (TOML)
[server]
bind = "0.0.0.0:8080"

# DSL configuration examples
Route "/api/*" {
    Backend "http://backend1:8080"
}
```

---

## Prerequisites

### System Requirements

**Minimum (Development/Testing):**
- CPU: 2 cores
- RAM: 2GB
- Disk: 1GB
- OS: Linux (kernel 5.10+), macOS, or Windows with WSL2

**Recommended (Production):**
- CPU: 4-8 cores
- RAM: 8GB+
- Disk: 10GB+ (SSD recommended)
- OS: Linux (Ubuntu 22.04 LTS, Rocky Linux 9, or newer)
- Kernel: 5.15+ (for io_uring support)

### Software Dependencies

**Required:**
- Highper Gateway binary (latest release)
- systemd (for service management) or Docker

**Optional (scenario-specific):**
- Redis 7+ (for caching, session storage)
- Consul or etcd (for service discovery)
- MaxMind GeoLite2 database (for geographic load balancing)
- TLS certificates (Let's Encrypt recommended)

### Installation

```bash
# Option 1: Download pre-built binary
curl -L https://github.com/highperapp/highper-gateway/releases/latest/download/highper-gateway-linux-x86_64.tar.gz | tar xz
sudo mv highper-gateway /usr/local/bin/
sudo chmod +x /usr/local/bin/highper-gateway

# Option 2: Build from source
git clone https://github.com/highperapp/highper-gateway.git
cd highper-gateway/highper-gateway
cargo build --release
sudo cp target/release/highper-gateway /usr/local/bin/

# Verify installation
highper-gateway --version
# Output: highper-gateway 0.1.0

# Create standard directories
sudo mkdir -p /etc/highper-gateway
sudo mkdir -p /var/log/highper-gateway
sudo mkdir -p /var/lib/highper-gateway
```

---

## Zero-Config Philosophy

Highper Gateway follows a **zero-config** approach where sensible defaults allow immediate deployment with minimal configuration.

### Default Behavior

**Without any configuration:**
```bash
# This works out-of-the-box!
$ highper-gateway

# Default behavior:
# - Listen on 0.0.0.0:8080 (HTTP)
# - Proxy to localhost:9000 (single backend)
# - Enable access logging to stdout
# - Enable health checks
# - Use in-memory connection pooling
```

### Configuration Hierarchy

Highper Gateway uses a layered configuration approach:

1. **Built-in defaults** (compiled into binary)
2. **System config** (`/etc/highper-gateway/config.toml`)
3. **User config** (`~/.config/highper-gateway/config.toml`)
4. **Environment variables** (`HIGHPER_*`)
5. **Command-line flags** (highest priority)

```bash
# Examples of configuration hierarchy
# Priority: CLI > ENV > User Config > System Config > Defaults

# Using system config
$ highper-gateway --config /etc/highper-gateway/scenario-02.toml

# Using environment variables
$ HIGHPER_PORT=8080 HIGHPER_BACKEND=http://localhost:9000 highper-gateway

# Using command-line flags (override all)
$ highper-gateway --port 8080 --backend http://localhost:9000
```

### Best Practice: Convention over Configuration

**Conventional file locations (auto-detected):**
```
/etc/highper-gateway/
├── config.toml                 # Main configuration (auto-loaded)
├── tls/
│   ├── cert.pem               # Default TLS certificate
│   └── key.pem                # Default TLS private key
├── waf/
│   └── rules.conf             # Default WAF rules
└── geoip/
    └── GeoLite2-City.mmdb     # Default GeoIP database
```

When files exist in these conventional locations, they're automatically used without explicit configuration.

---

## Common Deployment Patterns

### Pattern 1: Single Binary Deployment

**Use case:** Simple deployments, development, testing

```bash
# 1. Copy binary
sudo cp highper-gateway /usr/local/bin/

# 2. Create config
sudo tee /etc/highper-gateway/config.toml <<EOF
[server]
bind = "0.0.0.0:8080"

[[upstreams]]
name = "backend"
servers = ["http://localhost:9000"]
EOF

# 3. Run as daemon
nohup highper-gateway --config /etc/highper-gateway/config.toml > /var/log/highper-gateway/output.log 2>&1 &

# 4. Save PID
echo $! | sudo tee /var/run/highper-gateway.pid
```

### Pattern 2: systemd Service

**Use case:** Production Linux deployments (recommended)

```bash
# 1. Create systemd service unit
sudo tee /etc/systemd/system/highper-gateway.service <<EOF
[Unit]
Description=Highper Gateway - High-performance reverse proxy
Documentation=https://github.com/highperapp/highper-gateway
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=highper
Group=highper
ExecStartPre=/usr/local/bin/highper-gateway --config /etc/highper-gateway/config.toml --validate
ExecStart=/usr/local/bin/highper-gateway --config /etc/highper-gateway/config.toml
ExecReload=/bin/kill -HUP \$MAINPID
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536
StandardOutput=journal
StandardError=journal
SyslogIdentifier=highper-gateway

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/highper-gateway /var/lib/highper-gateway

[Install]
WantedBy=multi-user.target
EOF

# 2. Create dedicated user
sudo useradd -r -s /bin/false -d /var/lib/highper-gateway highper

# 3. Set permissions
sudo chown -R highper:highper /etc/highper-gateway
sudo chown -R highper:highper /var/log/highper-gateway
sudo chown -R highper:highper /var/lib/highper-gateway

# 4. Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable highper-gateway
sudo systemctl start highper-gateway

# 5. Verify
sudo systemctl status highper-gateway
sudo journalctl -u highper-gateway -f
```

### Pattern 3: Docker Deployment

**Use case:** Containerized environments, Kubernetes

```bash
# 1. Create Dockerfile (if not using official image)
cat > Dockerfile <<EOF
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY highper-gateway /usr/local/bin/
RUN chmod +x /usr/local/bin/highper-gateway
RUN useradd -r -s /bin/false highper
USER highper
EXPOSE 8080 8443
ENTRYPOINT ["/usr/local/bin/highper-gateway"]
CMD ["--config", "/etc/highper-gateway/config.toml"]
EOF

# 2. Build image
docker build -t highper-gateway:latest .

# 3. Run container
docker run -d \
  --name highper-gateway \
  --restart unless-stopped \
  -p 8080:8080 \
  -p 8443:8443 \
  -v /etc/highper-gateway:/etc/highper-gateway:ro \
  -v /var/log/highper-gateway:/var/log/highper-gateway \
  highper-gateway:latest

# 4. View logs
docker logs -f highper-gateway
```

### Pattern 4: Docker Compose

**Use case:** Multi-container deployments with backends

```yaml
# docker-compose.yml
version: '3.8'

services:
  highper-gateway:
    image: highper-gateway:latest
    container_name: highper-gateway
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "8443:8443"
    volumes:
      - ./config:/etc/highper-gateway:ro
      - ./logs:/var/log/highper-gateway
    environment:
      - RUST_LOG=info
    depends_on:
      - backend1
      - backend2
    networks:
      - gateway-net

  backend1:
    image: nginx:alpine
    container_name: backend1
    networks:
      - gateway-net

  backend2:
    image: nginx:alpine
    container_name: backend2
    networks:
      - gateway-net

networks:
  gateway-net:
    driver: bridge
```

```bash
# Deploy with Docker Compose
docker-compose up -d

# View logs
docker-compose logs -f highper-gateway

# Reload configuration
docker-compose exec highper-gateway kill -HUP 1
```

---

## Scenario Deployments

## Scenario 01: Layer 4 TCP - Pure TCP Proxying

### Overview

Pure Layer 4 TCP proxying with connection pooling. Proxies TCP connections without inspecting application protocol.

**Use Cases:**
- Database proxying (MySQL, PostgreSQL, Redis)
- TCP-based application proxying
- Load balancing TCP services
- Connection pooling for backend databases

### Zero-Config Quick Start

```bash
# Start with minimal config (proxies localhost:9000 on port 8080)
$ highper-gateway --mode tcp --port 8080 --backend tcp://localhost:9000

# That's it! TCP proxy is running
```

### Configuration Files

#### Option A: TOML Configuration

**File:** `/etc/highper-gateway/scenario-01-tcp.toml`

```toml
# Scenario 01: Layer 4 TCP Proxying
# Zero-config defaults with best practices

[server]
# Listen address (0.0.0.0 = all interfaces)
bind = "0.0.0.0:8080"
mode = "tcp"  # Layer 4 mode
workers = 4   # Auto: num_cpus, override for tuning

[upstream]
# Backend TCP servers
name = "tcp-backend"
protocol = "tcp"
load_balancer = "round_robin"  # round_robin, least_conn, random
health_check_interval = "10s"

# Backend servers
servers = [
    "tcp://192.168.1.101:9000",
    "tcp://192.168.1.102:9000",
    "tcp://192.168.1.103:9000",
]

# Connection pooling (Layer 4)
[upstream.connection_pool]
enabled = true
max_idle_per_backend = 100
idle_timeout = "5m"
connect_timeout = "10s"

# Health checks (TCP connect probe)
[upstream.health_check]
enabled = true
interval = "10s"
timeout = "3s"
unhealthy_threshold = 3
healthy_threshold = 2

# TCP-specific options
[tcp]
# SO_KEEPALIVE
keepalive = true
keepalive_idle = "60s"
keepalive_interval = "10s"
keepalive_count = 3

# TCP buffer sizes
send_buffer_size = "64KB"
recv_buffer_size = "64KB"

# Timeout settings
read_timeout = "30s"
write_timeout = "30s"

# Logging
[logging]
level = "info"  # trace, debug, info, warn, error
access_log = "/var/log/highper-gateway/access.log"
error_log = "/var/log/highper-gateway/error.log"
format = "json"  # json or text

# Metrics
[metrics]
enabled = true
bind = "127.0.0.1:9091"  # Prometheus metrics endpoint
```

#### Option B: DSL Configuration

**File:** `/etc/highper-gateway/scenario-01-tcp.dsl`

```dsl
# Scenario 01: Layer 4 TCP Proxying - DSL Configuration

Server {
    Bind "0.0.0.0:8080"
    Mode "tcp"
    Workers 4
}

Upstream "tcp-backend" {
    Protocol "tcp"
    LoadBalancer "round_robin"

    Servers [
        "tcp://192.168.1.101:9000",
        "tcp://192.168.1.102:9000",
        "tcp://192.168.1.103:9000"
    ]

    ConnectionPool {
        MaxIdlePerBackend 100
        IdleTimeout "5m"
        ConnectTimeout "10s"
    }

    HealthCheck {
        Interval "10s"
        Timeout "3s"
        UnhealthyThreshold 3
        HealthyThreshold 2
    }
}

TCP {
    Keepalive true
    KeepaliveIdle "60s"
    KeepaliveInterval "10s"
    KeepaliveCount 3

    SendBufferSize "64KB"
    RecvBufferSize "64KB"

    ReadTimeout "30s"
    WriteTimeout "30s"
}

Logging {
    Level "info"
    AccessLog "/var/log/highper-gateway/access.log"
    ErrorLog "/var/log/highper-gateway/error.log"
    Format "json"
}

Metrics {
    Enabled true
    Bind "127.0.0.1:9091"
}
```

### Deployment Steps

#### Step 1: Prepare Configuration

```bash
# Create configuration directory
sudo mkdir -p /etc/highper-gateway

# Copy configuration (TOML or DSL)
sudo cp scenario-01-tcp.toml /etc/highper-gateway/config.toml

# Validate configuration
highper-gateway --config /etc/highper-gateway/config.toml --validate
```

#### Step 2: Create systemd Service

```bash
# Use the common systemd service from Pattern 2 above
sudo tee /etc/systemd/system/highper-gateway.service <<'EOF'
[Unit]
Description=Highper Gateway - Layer 4 TCP Proxy
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=highper
Group=highper
ExecStart=/usr/local/bin/highper-gateway --config /etc/highper-gateway/config.toml
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

# Create user if needed
sudo useradd -r -s /bin/false -d /var/lib/highper-gateway highper

# Set permissions
sudo chown -R highper:highper /etc/highper-gateway
sudo mkdir -p /var/log/highper-gateway
sudo chown -R highper:highper /var/log/highper-gateway

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable highper-gateway
sudo systemctl start highper-gateway
```

#### Step 3: Verify Deployment

```bash
# Check service status
sudo systemctl status highper-gateway

# Check if port is listening
sudo ss -tlnp | grep 8080

# Test TCP connection
nc -zv localhost 8080

# Check logs
sudo journalctl -u highper-gateway -f

# Check metrics
curl http://localhost:9091/metrics
```

### Backend Setup

**Example: Simple TCP Echo Server (for testing)**

```bash
# Python TCP echo server (backend on port 9000)
python3 <<'EOF'
import socket
server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.bind(('0.0.0.0', 9000))
server.listen(5)
print("TCP Echo Server on port 9000")
while True:
    client, addr = server.accept()
    print(f"Connection from {addr}")
    data = client.recv(1024)
    if data:
        client.sendall(data)  # Echo back
    client.close()
EOF
```

### Health Check Validation

```bash
# TCP health check script
cat > /usr/local/bin/check-tcp-backend.sh <<'EOF'
#!/bin/bash
HOST=$1
PORT=$2
timeout 3 bash -c "cat < /dev/null > /dev/tcp/$HOST/$PORT" 2>/dev/null
if [ $? -eq 0 ]; then
    echo "✓ Backend $HOST:$PORT is healthy"
    exit 0
else
    echo "✗ Backend $HOST:$PORT is down"
    exit 1
fi
EOF

chmod +x /usr/local/bin/check-tcp-backend.sh

# Test backends
/usr/local/bin/check-tcp-backend.sh 192.168.1.101 9000
/usr/local/bin/check-tcp-backend.sh 192.168.1.102 9000
/usr/local/bin/check-tcp-backend.sh 192.168.1.103 9000
```

### Production Best Practices

1. **Connection Pooling**: Enable for database backends to reduce connection overhead
2. **Health Checks**: Use TCP connect probes (fast and reliable)
3. **Timeouts**: Set appropriate read/write timeouts for your workload
4. **Keepalive**: Enable TCP keepalive to detect dead connections
5. **Buffer Sizes**: Tune based on your traffic patterns (default 64KB is good for most)
6. **File Descriptors**: Increase ulimit (see kernel tuning section)

### Monitoring

```bash
# Key metrics to monitor
curl -s http://localhost:9091/metrics | grep -E "tcp_|connection_pool_"

# Expected metrics:
# tcp_connections_active{backend="tcp-backend"} 42
# tcp_connections_total{backend="tcp-backend"} 1523
# connection_pool_idle{backend="tcp-backend"} 98
# connection_pool_active{backend="tcp-backend"} 2
# tcp_bytes_sent_total{backend="tcp-backend"} 15234567
# tcp_bytes_received_total{backend="tcp-backend"} 8765432
```

---

## Scenario 02: Layer 7 HTTP - HTTP/1.1 Load Balancing

### Overview

HTTP/1.1 load balancing with round-robin distribution, health checks, and connection pooling at the application layer.

**Use Cases:**
- Web application load balancing
- API gateway
- Microservices routing
- HTTP reverse proxy

### Zero-Config Quick Start

```bash
# Start with minimal config (HTTP proxy on port 8080)
$ highper-gateway --port 8080 --backend http://backend1:9000,http://backend2:9000

# Or even simpler (single backend)
$ highper-gateway --backend http://localhost:9000
# Defaults: listen on :8080, proxy to single backend
```

### Configuration Files

#### TOML Configuration

**File:** `/etc/highper-gateway/scenario-02-http.toml`

```toml
# Scenario 02: HTTP/1.1 Load Balancing
# Production-ready configuration with best practices

[server]
bind = "0.0.0.0:8080"
mode = "http"  # Layer 7 HTTP mode
workers = 0    # 0 = auto-detect CPU cores

# HTTP server settings
[http]
http1 = true   # Enable HTTP/1.1 (default: true)
http2 = false  # Disable HTTP/2 for this scenario
keepalive_timeout = "75s"
max_header_size = "8KB"
max_body_size = "10MB"

# Request timeouts
request_timeout = "30s"
header_timeout = "10s"

# Upstream backend configuration
[[upstreams]]
name = "http-backend"
protocol = "http"
load_balancer = "round_robin"  # round_robin, least_conn, ip_hash, random

# Backend servers
servers = [
    "http://backend1.local:9000",
    "http://backend2.local:9000",
    "http://backend3.local:9000",
]

# HTTP connection pooling
[upstreams.connection_pool]
enabled = true
max_idle_per_host = 32
idle_timeout = "90s"
max_lifetime = "10m"
connect_timeout = "5s"

# HTTP health checks
[upstreams.health_check]
enabled = true
type = "http"
path = "/health"
method = "GET"
interval = "10s"
timeout = "3s"
expected_status = 200
unhealthy_threshold = 3
healthy_threshold = 2

# Retry policy
[upstreams.retry]
enabled = true
max_attempts = 3
backoff = "exponential"  # linear, exponential
initial_backoff = "100ms"
max_backoff = "5s"
retry_on = ["5xx", "timeout", "connection_error"]

# Load balancer behavior
[upstreams.load_balancer_config]
# Round-robin with session affinity (optional)
session_affinity = false  # Enable for sticky sessions
session_cookie_name = "BACKEND_ID"
session_ttl = "1h"

# Circuit breaker (prevent cascading failures)
[upstreams.circuit_breaker]
enabled = true
failure_threshold = 5
success_threshold = 2
timeout = "30s"
half_open_requests = 3

# Request/Response modifications
[request]
# Add/modify request headers
add_headers = [
    ["X-Forwarded-Proto", "http"],
    ["X-Real-IP", "$remote_addr"],
    ["X-Forwarded-For", "$proxy_add_x_forwarded_for"],
]

# Preserve host header
preserve_host = true

# Remove sensitive headers
remove_headers = ["X-Powered-By", "Server"]

[response]
# Add response headers
add_headers = [
    ["X-Gateway", "Highper Gateway"],
    ["X-Cache-Status", "$cache_status"],
]

# Logging
[logging]
level = "info"
access_log = "/var/log/highper-gateway/access.log"
error_log = "/var/log/highper-gateway/error.log"
format = "combined"  # combined, json, custom

# Access log format
access_log_format = '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" $request_time'

# Metrics
[metrics]
enabled = true
bind = "127.0.0.1:9091"
path = "/metrics"

# Admin API
[admin]
enabled = true
bind = "127.0.0.1:9090"
# Endpoints: /health, /stats, /config, /reload
```

#### DSL Configuration

**File:** `/etc/highper-gateway/scenario-02-http.dsl`

```dsl
# Scenario 02: HTTP/1.1 Load Balancing - DSL

Server {
    Bind "0.0.0.0:8080"
    Mode "http"
    Workers 0  # Auto-detect
}

HTTP {
    HTTP1 true
    HTTP2 false
    KeepaliveTimeout "75s"
    MaxHeaderSize "8KB"
    MaxBodySize "10MB"
    RequestTimeout "30s"
}

Upstream "http-backend" {
    Protocol "http"
    LoadBalancer "round_robin"

    Servers [
        "http://backend1.local:9000",
        "http://backend2.local:9000",
        "http://backend3.local:9000"
    ]

    ConnectionPool {
        MaxIdlePerHost 32
        IdleTimeout "90s"
        ConnectTimeout "5s"
    }

    HealthCheck {
        Type "http"
        Path "/health"
        Method "GET"
        Interval "10s"
        Timeout "3s"
        ExpectedStatus 200
        UnhealthyThreshold 3
        HealthyThreshold 2
    }

    Retry {
        MaxAttempts 3
        Backoff "exponential"
        RetryOn ["5xx", "timeout"]
    }

    CircuitBreaker {
        FailureThreshold 5
        Timeout "30s"
    }
}

Request {
    AddHeaders [
        ["X-Real-IP", "$remote_addr"],
        ["X-Forwarded-For", "$proxy_add_x_forwarded_for"]
    ]
    PreserveHost true
}

Response {
    AddHeaders [
        ["X-Gateway", "Highper Gateway"]
    ]
}

Logging {
    Level "info"
    AccessLog "/var/log/highper-gateway/access.log"
    ErrorLog "/var/log/highper-gateway/error.log"
}

Metrics {
    Enabled true
    Bind "127.0.0.1:9091"
}

Admin {
    Enabled true
    Bind "127.0.0.1:9090"
}
```

### Deployment Steps

#### Step 1: Deploy Backend Services

**Option A: Simple HTTP Backends (Python)**

```bash
# Backend 1 (port 9000)
python3 -m http.server 9000 --directory /var/www/backend1 &

# Backend 2 (port 9001)
python3 -m http.server 9001 --directory /var/www/backend2 &

# Backend 3 (port 9002)
python3 -m http.server 9002 --directory /var/www/backend3 &
```

**Option B: Docker Backend Services**

```yaml
# docker-compose-backends.yml
version: '3.8'

services:
  backend1:
    image: nginx:alpine
    container_name: backend1
    ports:
      - "9000:80"
    volumes:
      - ./backend1:/usr/share/nginx/html:ro
    environment:
      - BACKEND_ID=backend1

  backend2:
    image: nginx:alpine
    container_name: backend2
    ports:
      - "9001:80"
    volumes:
      - ./backend2:/usr/share/nginx/html:ro
    environment:
      - BACKEND_ID=backend2

  backend3:
    image: nginx:alpine
    container_name: backend3
    ports:
      - "9002:80"
    volumes:
      - ./backend3:/usr/share/nginx/html:ro
    environment:
      - BACKEND_ID=backend3
```

```bash
# Create backend content
mkdir -p backend{1,2,3}
echo "<h1>Backend 1</h1>" > backend1/index.html
echo "<h1>Backend 2</h1>" > backend2/index.html
echo "<h1>Backend 3</h1>" > backend3/index.html

# Start backends
docker-compose -f docker-compose-backends.yml up -d
```

#### Step 2: Deploy Highper Gateway

```bash
# Copy configuration
sudo cp scenario-02-http.toml /etc/highper-gateway/config.toml

# Validate
highper-gateway --config /etc/highper-gateway/config.toml --validate

# Start with systemd
sudo systemctl restart highper-gateway

# Or run directly
highper-gateway --config /etc/highper-gateway/config.toml
```

#### Step 3: Verify Deployment

```bash
# Test load balancing (should round-robin across backends)
for i in {1..9}; do
    curl http://localhost:8080/
    echo ""
done

# Expected output (cycling through):
# <h1>Backend 1</h1>
# <h1>Backend 2</h1>
# <h1>Backend 3</h1>
# <h1>Backend 1</h1>
# ...

# Check health status
curl http://localhost:9090/health

# Check backend stats
curl http://localhost:9090/stats

# Check metrics
curl http://localhost:9091/metrics | grep http_requests
```

### Health Check Configuration

```bash
# Add health check endpoints to backends
# Backend health check endpoint (returns 200 OK)
cat > /var/www/backend1/health <<'EOF'
OK
EOF

# Test health check
curl http://backend1.local:9000/health
```

### Load Testing

```bash
# Simple load test with vegeta
echo "GET http://localhost:8080/" | vegeta attack -rate=1000/s -duration=60s | vegeta report

# Check distribution across backends
curl http://localhost:9090/stats | jq '.upstreams["http-backend"].servers'

# Expected output (roughly equal distribution):
# {
#   "backend1.local:9000": { "requests": 20003, "failures": 0 },
#   "backend2.local:9000": { "requests": 19998, "failures": 0 },
#   "backend3.local:9000": { "requests": 19999, "failures": 0 }
# }
```

### Production Best Practices

1. **Health Checks**: Always enable HTTP health checks with realistic endpoints
2. **Connection Pooling**: Enable for persistent connections (reduces latency)
3. **Circuit Breaker**: Protect against cascading failures
4. **Retry Logic**: Use exponential backoff for transient failures
5. **Session Affinity**: Enable sticky sessions if needed for stateful apps
6. **Monitoring**: Monitor request distribution and backend health

---

## Scenario 03: HTTPS/TLS Termination

### Overview

HTTPS/TLS 1.3 termination with automatic certificate management (ACME/Let's Encrypt), mutual TLS (mTLS), and OCSP stapling.

**Use Cases:**
- Production HTTPS websites
- API endpoints requiring TLS
- mTLS for service-to-service authentication
- Certificate management with automatic renewal

### Zero-Config Quick Start

```bash
# With automatic Let's Encrypt certificates
$ highper-gateway \
    --port 443 \
    --domain example.com \
    --email admin@example.com \
    --auto-tls \
    --backend http://localhost:9000

# With existing certificates
$ highper-gateway \
    --port 443 \
    --tls-cert /etc/letsencrypt/live/example.com/fullchain.pem \
    --tls-key /etc/letsencrypt/live/example.com/privkey.pem \
    --backend http://localhost:9000
```

### Configuration Files

#### TOML Configuration

**File:** `/etc/highper-gateway/scenario-03-https.toml`

```toml
# Scenario 03: HTTPS/TLS Termination
# Production-ready TLS configuration with ACME, mTLS, OCSP

[server]
bind = "0.0.0.0:443"
mode = "http"
workers = 0

# HTTP to HTTPS redirect
[server.redirect]
http_port = 80
redirect_to_https = true

# TLS Configuration
[tls]
enabled = true
protocols = ["TLSv1.2", "TLSv1.3"]  # Minimum TLS 1.2
ciphers = "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384"
prefer_server_ciphers = true

# Certificate configuration
[tls.certificates]
# Option 1: Manual certificate (conventional path - auto-detected)
cert = "/etc/highper-gateway/tls/cert.pem"
key = "/etc/highper-gateway/tls/key.pem"

# Option 2: ACME / Let's Encrypt (automatic)
[tls.acme]
enabled = true
directory = "https://acme-v02.api.letsencrypt.org/directory"  # Production
# directory = "https://acme-staging-v02.api.letsencrypt.org/directory"  # Staging for testing
email = "admin@example.com"
domains = ["example.com", "www.example.com", "api.example.com"]
challenge_type = "http-01"  # http-01, dns-01, tls-alpn-01
cache_dir = "/var/lib/highper-gateway/acme"
renew_before = "30d"  # Renew 30 days before expiry
agree_tos = true

# HTTP-01 challenge (requires port 80)
[tls.acme.http_challenge]
bind = "0.0.0.0:80"
webroot = "/var/www/acme-challenge"

# OCSP Stapling
[tls.ocsp]
enabled = true
cache_dir = "/var/lib/highper-gateway/ocsp"
refresh_interval = "1h"
timeout = "10s"

# Mutual TLS (mTLS) - Client Certificate Authentication
[tls.mtls]
enabled = true
mode = "require"  # optional, require, verify_if_given
client_ca = "/etc/highper-gateway/tls/client-ca.pem"
verify_depth = 3
verify_client_cert = true

# Client certificate revocation list (CRL)
crl = "/etc/highper-gateway/tls/crl.pem"

# HTTP Strict Transport Security (HSTS)
[tls.hsts]
enabled = true
max_age = 31536000  # 1 year
include_subdomains = true
preload = true

# Certificate pinning (optional, advanced)
[tls.pinning]
enabled = false
pins = [
    "sha256/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
]

# Session cache for TLS resumption
[tls.session_cache]
enabled = true
cache_type = "builtin"  # builtin, redis
size = "10MB"
timeout = "5m"

# Upstream (backend servers - HTTP)
[[upstreams]]
name = "http-backend"
protocol = "http"  # TLS termination: HTTPS → HTTP
servers = [
    "http://backend1:9000",
    "http://backend2:9000",
    "http://backend3:9000",
]

[upstreams.connection_pool]
max_idle_per_host = 32
idle_timeout = "90s"

[upstreams.health_check]
type = "http"
path = "/health"
interval = "10s"
timeout = "3s"

# Security headers
[response.headers]
add = [
    ["Strict-Transport-Security", "max-age=31536000; includeSubDomains; preload"],
    ["X-Content-Type-Options", "nosniff"],
    ["X-Frame-Options", "DENY"],
    ["X-XSS-Protection", "1; mode=block"],
    ["Referrer-Policy", "strict-origin-when-cross-origin"],
]

# Logging
[logging]
level = "info"
access_log = "/var/log/highper-gateway/access.log"
error_log = "/var/log/highper-gateway/error.log"

# TLS-specific logging
log_tls_version = true
log_cipher_suite = true
log_client_cert_dn = true

# Metrics
[metrics]
enabled = true
bind = "127.0.0.1:9091"
```

### Deployment Steps

#### Step 1: Certificate Preparation

**Option A: Let's Encrypt (ACME) - Automatic**

```bash
# No manual steps needed - certificates are obtained automatically
# Ensure port 80 is accessible for HTTP-01 challenge

# Create ACME cache directory
sudo mkdir -p /var/lib/highper-gateway/acme
sudo chown highper:highper /var/lib/highper-gateway/acme

# The gateway will automatically:
# 1. Request certificates from Let's Encrypt
# 2. Complete HTTP-01 challenge
# 3. Store certificates in cache_dir
# 4. Auto-renew 30 days before expiry
```

**Option B: Self-Signed Certificates (Testing)**

```bash
# Generate self-signed certificate for testing
sudo mkdir -p /etc/highper-gateway/tls

# Generate CA
openssl req -x509 -newkey rsa:4096 -days 365 -nodes \
    -keyout /tmp/ca-key.pem \
    -out /etc/highper-gateway/tls/ca-cert.pem \
    -subj "/CN=Test CA/O=Highper Gateway/C=US"

# Generate server certificate
openssl req -newkey rsa:4096 -nodes \
    -keyout /etc/highper-gateway/tls/key.pem \
    -out /tmp/server-req.pem \
    -subj "/CN=localhost/O=Highper Gateway/C=US"

# Sign server certificate with CA
openssl x509 -req -in /tmp/server-req.pem \
    -days 365 \
    -CA /etc/highper-gateway/tls/ca-cert.pem \
    -CAkey /tmp/ca-key.pem \
    -CAcreateserial \
    -out /etc/highper-gateway/tls/cert.pem

# Generate client certificate (for mTLS testing)
openssl req -newkey rsa:4096 -nodes \
    -keyout /tmp/client-key.pem \
    -out /tmp/client-req.pem \
    -subj "/CN=Test Client/O=Highper Gateway/C=US"

openssl x509 -req -in /tmp/client-req.pem \
    -days 365 \
    -CA /etc/highper-gateway/tls/ca-cert.pem \
    -CAkey /tmp/ca-key.pem \
    -CAcreateserial \
    -out /tmp/client-cert.pem

# Set permissions
sudo chown -R highper:highper /etc/highper-gateway/tls
sudo chmod 600 /etc/highper-gateway/tls/key.pem
```

**Option C: Existing Certificates (Let's Encrypt via certbot)**

```bash
# If you already have Let's Encrypt certificates via certbot
sudo certbot certonly --standalone -d example.com -d www.example.com

# Link or copy certificates
sudo mkdir -p /etc/highper-gateway/tls
sudo ln -s /etc/letsencrypt/live/example.com/fullchain.pem /etc/highper-gateway/tls/cert.pem
sudo ln -s /etc/letsencrypt/live/example.com/privkey.pem /etc/highper-gateway/tls/key.pem

# Or copy (if gateway user needs read access)
sudo cp /etc/letsencrypt/live/example.com/fullchain.pem /etc/highper-gateway/tls/cert.pem
sudo cp /etc/letsencrypt/live/example.com/privkey.pem /etc/highper-gateway/tls/key.pem
sudo chown highper:highper /etc/highper-gateway/tls/*
sudo chmod 600 /etc/highper-gateway/tls/key.pem
```

#### Step 2: Deploy Gateway

```bash
# Copy configuration
sudo cp scenario-03-https.toml /etc/highper-gateway/config.toml

# Validate configuration
highper-gateway --config /etc/highper-gateway/config.toml --validate

# Start service
sudo systemctl restart highper-gateway

# Check logs for TLS initialization
sudo journalctl -u highper-gateway -f | grep -i tls
```

#### Step 3: Verify TLS Configuration

```bash
# Test HTTPS connection
curl -v https://localhost:443/

# Test TLS version and cipher
openssl s_client -connect localhost:443 -tls1_3 -showcerts

# Test mTLS with client certificate
curl -v \
    --cacert /etc/highper-gateway/tls/ca-cert.pem \
    --cert /tmp/client-cert.pem \
    --key /tmp/client-key.pem \
    https://localhost:443/

# Test OCSP stapling
openssl s_client -connect localhost:443 -status | grep "OCSP Response"

# Test HTTP to HTTPS redirect
curl -I http://localhost:80/
# Should return: HTTP/1.1 301 Moved Permanently
# Location: https://localhost/

# SSL Labs test (for production domains)
# Visit: https://www.ssllabs.com/ssltest/analyze.html?d=example.com
```

### Certificate Renewal

**Automatic (ACME):**

```bash
# Certificates are automatically renewed by the gateway
# Monitor renewal logs
sudo journalctl -u highper-gateway | grep -i "certificate renew"

# Manual renewal trigger (if needed)
sudo systemctl reload highper-gateway  # Reload config including certs
```

**Manual (certbot):**

```bash
# Renew with certbot
sudo certbot renew

# Reload gateway to pick up new certificates
sudo systemctl reload highper-gateway
# Or send SIGHUP
sudo killall -HUP highper-gateway
```

### mTLS Client Certificate Validation

```bash
# Extract client certificate DN from requests
curl -v \
    --cacert /etc/highper-gateway/tls/ca-cert.pem \
    --cert /tmp/client-cert.pem \
    --key /tmp/client-key.pem \
    https://localhost:443/

# Check logs for client cert info
sudo journalctl -u highper-gateway | grep "client_cert_dn"
# Output: client_cert_dn="CN=Test Client,O=Highper Gateway,C=US"
```

### Security Best Practices

1. **TLS 1.3 Only (if possible)**: Disable TLS 1.2 if all clients support 1.3
2. **Strong Ciphers**: Use ECDHE for forward secrecy
3. **HSTS**: Always enable with long max-age (1 year)
4. **OCSP Stapling**: Reduces client-side OCSP lookups
5. **Certificate Pinning**: Only for advanced use cases (can break clients)
6. **Session Resumption**: Enable for performance
7. **Key Rotation**: Rotate TLS certificates regularly (auto with ACME)
8. **mTLS for APIs**: Use mutual TLS for service-to-service communication

### Monitoring

```bash
# TLS-specific metrics
curl -s http://localhost:9091/metrics | grep tls_

# Expected metrics:
# tls_handshakes_total{protocol="TLSv1.3"} 1523
# tls_handshakes_failed_total 3
# tls_session_cache_hits_total 8756
# tls_session_cache_misses_total 234
# tls_client_cert_verified_total 1200
# tls_client_cert_rejected_total 5
# tls_ocsp_staple_success_total 450
```

---

## Scenario 04: API Gateway with Rate Limiting

### Overview

API Gateway with advanced rate limiting using Token Bucket and Sliding Window algorithms. Supports per-client, per-route, and global rate limits.

**Use Cases:**
- Public API rate limiting
- Preventing abuse and DDoS
- Fair resource allocation
- API monetization (tiered limits)

### Zero-Config Quick Start

```bash
# Simple rate limit: 100 requests per minute
$ highper-gateway \
    --port 8080 \
    --backend http://localhost:9000 \
    --rate-limit "100/m"

# Per-IP rate limiting
$ highper-gateway \
    --port 8080 \
    --backend http://localhost:9000 \
    --rate-limit "1000/m" \
    --rate-limit-by ip
```

### Configuration Files

#### TOML Configuration

**File:** `/etc/highper-gateway/scenario-04-rate-limit.toml`

```toml
# Scenario 04: API Gateway with Rate Limiting
# Token Bucket + Sliding Window algorithms

[server]
bind = "0.0.0.0:8080"
mode = "http"
workers = 0

# Global rate limiting
[rate_limit]
enabled = true
algorithm = "token_bucket"  # token_bucket, sliding_window, leaky_bucket

# Default global limit (applies if no route-specific limit)
default_limit = "1000/m"  # 1000 requests per minute
burst = 100  # Allow burst of 100 requests

# Rate limit storage backend
storage = "memory"  # memory, redis
redis_url = "redis://localhost:6379/0"  # If storage = "redis"

# Rate limit key (what to limit by)
limit_by = "ip"  # ip, header:<name>, cookie:<name>, jwt:sub

# Response when rate limit exceeded
[rate_limit.exceeded]
status_code = 429  # HTTP 429 Too Many Requests
message = "Rate limit exceeded. Try again in {retry_after} seconds."
headers = [
    ["X-RateLimit-Limit", "{limit}"],
    ["X-RateLimit-Remaining", "{remaining}"],
    ["X-RateLimit-Reset", "{reset}"],
    ["Retry-After", "{retry_after}"],
]

# Upstream configuration
[[upstreams]]
name = "api-backend"
protocol = "http"
servers = ["http://api-server:9000"]

# Route-specific rate limits
[[routes]]
path = "/api/public/*"
upstream = "api-backend"

# Public API: 100 req/min per IP
[routes.rate_limit]
enabled = true
algorithm = "sliding_window"
limit = "100/m"
limit_by = "ip"

[[routes]]
path = "/api/premium/*"
upstream = "api-backend"

# Premium API: 1000 req/min per API key
[routes.rate_limit]
enabled = true
limit = "1000/m"
limit_by = "header:X-API-Key"
burst = 200

[[routes]]
path = "/api/unlimited/*"
upstream = "api-backend"

# No rate limit for unlimited tier
[routes.rate_limit]
enabled = false

# Authentication (for API key validation)
[auth]
enabled = true
type = "api_key"  # api_key, jwt, oauth2

[auth.api_key]
header_name = "X-API-Key"
query_param = "api_key"  # Alternative: ?api_key=xxx

# API key database (file-based for simplicity)
keys_file = "/etc/highper-gateway/api-keys.json"
# Format: { "key-abc123": { "tier": "premium", "rate_limit": "1000/m" } }

# Or use Redis for distributed API key storage
keys_redis = "redis://localhost:6379/1"

# JWT authentication (alternative)
[auth.jwt]
enabled = false
secret = "your-256-bit-secret"
algorithm = "HS256"  # HS256, RS256
issuer = "https://auth.example.com"
audience = "api.example.com"

# Extract rate limit from JWT claim
rate_limit_claim = "rate_limit"  # JWT claim: { "rate_limit": "1000/m" }

# Metrics
[metrics]
enabled = true
bind = "127.0.0.1:9091"

# Track rate limit metrics
[metrics.rate_limit]
track_per_key = true  # Track metrics per API key/IP
max_keys = 10000  # Limit tracked keys to prevent memory explosion

# Logging
[logging]
level = "info"
access_log = "/var/log/highper-gateway/access.log"
error_log = "/var/log/highper-gateway/error.log"

# Log rate limit events
log_rate_limit_exceeded = true
log_rate_limit_format = '$remote_addr - [$time_local] "RATE_LIMIT_EXCEEDED" path=$request_path limit=$rate_limit_name remaining=$rate_limit_remaining'
```

#### API Keys Configuration

**File:** `/etc/highper-gateway/api-keys.json`

```json
{
  "key-public-abc123": {
    "name": "Public API Key",
    "tier": "public",
    "rate_limit": "100/m",
    "burst": 10,
    "created_at": "2026-01-01T00:00:00Z",
    "expires_at": null
  },
  "key-premium-xyz789": {
    "name": "Premium API Key",
    "tier": "premium",
    "rate_limit": "1000/m",
    "burst": 200,
    "created_at": "2026-01-01T00:00:00Z",
    "expires_at": "2027-01-01T00:00:00Z"
  },
  "key-unlimited-def456": {
    "name": "Unlimited API Key",
    "tier": "unlimited",
    "rate_limit": null,
    "created_at": "2026-01-01T00:00:00Z",
    "expires_at": null
  }
}
```

### Deployment Steps

#### Step 1: Configure Rate Limiting

```bash
# Copy main configuration
sudo cp scenario-04-rate-limit.toml /etc/highper-gateway/config.toml

# Copy API keys database
sudo cp api-keys.json /etc/highper-gateway/

# Set permissions
sudo chown highper:highper /etc/highper-gateway/api-keys.json
sudo chmod 600 /etc/highper-gateway/api-keys.json
```

#### Step 2: Deploy Redis (Optional, for distributed rate limiting)

```bash
# Install Redis
sudo apt-get install redis-server

# Or use Docker
docker run -d --name redis -p 6379:6379 redis:7-alpine

# Test Redis connection
redis-cli ping
# Should return: PONG
```

#### Step 3: Start Gateway

```bash
# Validate configuration
highper-gateway --config /etc/highper-gateway/config.toml --validate

# Start service
sudo systemctl restart highper-gateway

# Check logs
sudo journalctl -u highper-gateway -f | grep rate_limit
```

#### Step 4: Test Rate Limiting

```bash
# Test public API (100 req/min limit)
for i in {1..120}; do
    curl -w "\nStatus: %{http_code}\n" \
         http://localhost:8080/api/public/users
    sleep 0.5
done

# After 100 requests, you should see:
# Status: 429
# {"error":"Rate limit exceeded. Try again in 42 seconds."}

# Check rate limit headers
curl -I http://localhost:8080/api/public/users
# X-RateLimit-Limit: 100
# X-RateLimit-Remaining: 87
# X-RateLimit-Reset: 1704844920
# Retry-After: 35

# Test with API key (premium: 1000 req/min)
curl -H "X-API-Key: key-premium-xyz789" \
     http://localhost:8080/api/premium/users

# Test unlimited tier
curl -H "X-API-Key: key-unlimited-def456" \
     http://localhost:8080/api/unlimited/users
# No rate limit applied
```

### Rate Limiting Algorithms

#### Token Bucket (default)

```toml
[rate_limit]
algorithm = "token_bucket"
limit = "100/m"  # 100 tokens per minute
burst = 20       # Bucket size: 20 tokens

# Behavior:
# - Bucket starts with 20 tokens
# - Each request consumes 1 token
# - Tokens refill at 100/60 = 1.67 per second
# - Allows burst of 20 requests instantly
# - Then throttles to 1.67 req/s
```

#### Sliding Window

```toml
[rate_limit]
algorithm = "sliding_window"
limit = "100/m"  # 100 requests per 60-second window

# Behavior:
# - Tracks timestamps of last 100 requests
# - Allows request if < 100 in last 60 seconds
# - More accurate than fixed window
# - Higher memory usage (tracks all timestamps)
```

#### Leaky Bucket

```toml
[rate_limit]
algorithm = "leaky_bucket"
limit = "100/m"  # Constant output rate
capacity = 200   # Queue size

# Behavior:
# - Requests queue in bucket
# - Leak at constant rate (100/60 = 1.67 req/s)
# - Smooth traffic to backend
# - Queue size = 200 requests
```

### Monitoring

```bash
# Rate limit metrics
curl -s http://localhost:9091/metrics | grep rate_limit

# Expected metrics:
# rate_limit_requests_total{route="/api/public/*",status="allowed"} 8534
# rate_limit_requests_total{route="/api/public/*",status="exceeded"} 234
# rate_limit_active_limits{algorithm="token_bucket"} 1523
# rate_limit_tokens_remaining{route="/api/public/*",key="1.2.3.4"} 87
```

### Production Best Practices

1. **Use Redis for distributed environments**: Memory-based rate limiting doesn't work across multiple instances
2. **Set appropriate burst values**: Allow legitimate traffic spikes
3. **Use sliding window for accuracy**: Token bucket is faster but less accurate
4. **Monitor rate limit metrics**: Track exceeded rate limits to tune limits
5. **Return helpful error messages**: Include Retry-After header
6. **Implement tiered limits**: Different limits for different API tiers
7. **Rate limit by API key**: More accurate than IP (handles NAT, proxies)

---

_Due to length constraints, I'll continue with the remaining scenarios (05-15) in a structured but condensed format. Each will follow the same pattern: Overview, Quick Start, Configuration, Deployment, and Best Practices._

---

## Scenario 05: HTTP/3 QUIC

### Overview

HTTP/3 over QUIC using Cloudflare's quiche library. Provides improved performance over lossy networks.

### Zero-Config Quick Start

```bash
# Enable HTTP/3 on port 443 (UDP)
$ highper-gateway \
    --port 443 \
    --protocol http3 \
    --tls-cert cert.pem \
    --tls-key key.pem \
    --backend http://localhost:9000
```

### Configuration

```toml
# /etc/highper-gateway/scenario-05-http3.toml

[server]
bind = "0.0.0.0:443"
protocol = "http3"  # HTTP/3 over QUIC

[http3]
enabled = true
max_concurrent_streams = 100
initial_max_data = "10MB"
initial_max_stream_data_bidi_local = "1MB"
max_idle_timeout = "30s"

# Alt-Svc header for HTTP/3 discovery
[http3.alt_svc]
enabled = true
max_age = 86400  # 24 hours

[tls]
cert = "/etc/highper-gateway/tls/cert.pem"
key = "/etc/highper-gateway/tls/key.pem"
protocols = ["TLSv1.3"]  # HTTP/3 requires TLS 1.3

[[upstreams]]
name = "backend"
protocol = "http"
servers = ["http://backend:9000"]
```

### Deployment

```bash
# HTTP/3 requires UDP port 443
sudo ufw allow 443/udp

# Deploy gateway
sudo cp scenario-05-http3.toml /etc/highper-gateway/config.toml
sudo systemctl restart highper-gateway

# Test with HTTP/3-capable client
curl --http3 https://localhost:443/
```

### Best Practices

- Requires TLS 1.3
- UDP port 443 must be open
- Fallback to HTTP/2 if HTTP/3 fails (Alt-Svc)
- Monitor QUIC connection quality metrics

---

## Scenario 06: WebSocket Load Balancer

### Overview

WebSocket load balancing with connection persistence and message routing.

### Quick Start

```bash
$ highper-gateway \
    --port 8080 \
    --websocket \
    --backend ws://backend1:9000,ws://backend2:9000
```

### Configuration

```toml
# /etc/highper-gateway/scenario-06-websocket.toml

[server]
bind = "0.0.0.0:8080"
mode = "http"

[websocket]
enabled = true
ping_interval = "30s"
pong_timeout = "10s"
max_message_size = "1MB"
compression = true  # Per-message deflate

[[upstreams]]
name = "ws-backend"
protocol = "websocket"
load_balancer = "least_conn"  # Sticky connections important
servers = [
    "ws://ws-server1:9000",
    "ws://ws-server2:9000",
    "ws://ws-server3:9000",
]

# Connection persistence (sticky sessions)
[upstreams.session]
enabled = true
cookie_name = "WS_BACKEND"
ttl = "24h"

[upstreams.health_check]
type = "websocket"
interval = "30s"
message = '{"type":"ping"}'
```

### Deployment

```bash
# Deploy WebSocket backends (example: Node.js)
cat > ws-server.js <<'EOF'
const WebSocket = require('ws');
const wss = new WebSocket.Server({ port: 9000 });
wss.on('connection', ws => {
    ws.on('message', msg => {
        ws.send(`Echo: ${msg}`);
    });
});
EOF

node ws-server.js &

# Deploy gateway
sudo cp scenario-06-websocket.toml /etc/highper-gateway/config.toml
sudo systemctl restart highper-gateway

# Test
wscat -c ws://localhost:8080/ws
# > Hello
# < Echo: Hello
```

---

## Scenario 07: gRPC Gateway

### Overview

gRPC load balancing with HTTP/2 streaming support.

### Quick Start

```bash
$ highper-gateway \
    --port 8080 \
    --grpc \
    --backend grpc://localhost:50051
```

### Configuration

```toml
# /etc/highper-gateway/scenario-07-grpc.toml

[server]
bind = "0.0.0.0:8080"
mode = "grpc"

[grpc]
enabled = true
max_concurrent_streams = 100
initial_window_size = "64KB"
max_frame_size = "16KB"

[[upstreams]]
name = "grpc-backend"
protocol = "grpc"
load_balancer = "round_robin"
servers = [
    "grpc://grpc-server1:50051",
    "grpc://grpc-server2:50051",
]

[upstreams.health_check]
type = "grpc"
service = "grpc.health.v1.Health"
interval = "10s"
```

### Deployment

```bash
# Deploy gRPC backend (example: Go)
# See: examples/grpc-server/main.go

# Deploy gateway
sudo cp scenario-07-grpc.toml /etc/highper-gateway/config.toml
sudo systemctl restart highper-gateway

# Test with grpcurl
grpcurl -plaintext localhost:8080 helloworld.Greeter/SayHello
```

---

## Scenario 08: Database Load Balancer

### Overview

Load balancing for MySQL, PostgreSQL, and Redis databases.

### Configuration

```toml
# /etc/highper-gateway/scenario-08-database.toml

[server]
bind = "0.0.0.0:3306"  # MySQL port
mode = "tcp"

# MySQL proxy
[[upstreams]]
name = "mysql-cluster"
protocol = "mysql"
load_balancer = "least_conn"
servers = [
    "mysql://mysql-primary:3306",
    "mysql://mysql-replica1:3306",
    "mysql://mysql-replica2:3306",
]

[upstreams.mysql]
read_write_split = true  # Route writes to primary, reads to replicas
max_connections_per_backend = 100
```

---

## Scenario 09: WAF + mTLS

### Overview

Web Application Firewall with 4 engines (ModSecurity, Coraza, AWS WAF, Custom) and mutual TLS.

### Configuration

```toml
# /etc/highper-gateway/scenario-09-waf-mtls.toml

[server]
bind = "0.0.0.0:443"
mode = "http"

[tls]
enabled = true
cert = "/etc/highper-gateway/tls/cert.pem"
key = "/etc/highper-gateway/tls/key.pem"

[tls.mtls]
enabled = true
mode = "require"
client_ca = "/etc/highper-gateway/tls/client-ca.pem"

[waf]
enabled = true
engine = "modsecurity"  # modsecurity, coraza, aws_waf, custom
rules_file = "/etc/highper-gateway/waf/owasp-crs.conf"
mode = "block"  # detect, block

[waf.modsecurity]
rules = [
    "/etc/highper-gateway/waf/rules/*.conf",
]
audit_log = "/var/log/highper-gateway/waf-audit.log"
```

---

## Scenario 10: Hybrid Multi-Protocol

### Overview

HTTP, WebSocket, and gRPC on the same gateway instance.

### Configuration

```toml
# /etc/highper-gateway/scenario-10-hybrid.toml

[[routes]]
path = "/api/*"
protocol = "http"
upstream = "http-backend"

[[routes]]
path = "/ws"
protocol = "websocket"
upstream = "ws-backend"

[[routes]]
path = "/grpc/*"
protocol = "grpc"
upstream = "grpc-backend"

[[upstreams]]
name = "http-backend"
protocol = "http"
servers = ["http://api-server:9000"]

[[upstreams]]
name = "ws-backend"
protocol = "websocket"
servers = ["ws://ws-server:9001"]

[[upstreams]]
name = "grpc-backend"
protocol = "grpc"
servers = ["grpc://grpc-server:50051"]
```

---

## Scenario 11: CDN Edge Caching

### Overview

Multi-tier caching with in-memory L1 and Redis L2 cache.

### Configuration

```toml
# /etc/highper-gateway/scenario-11-cdn-cache.toml

[cache]
enabled = true
mode = "multi_tier"

# L1: In-memory cache
[cache.l1]
enabled = true
max_size = "256MB"
max_entries = 10000
ttl = "5m"

# L2: Redis cache
[cache.l2]
enabled = true
backend = "redis"
redis_url = "redis://localhost:6379/0"
ttl = "1h"

# Cache rules
[[cache.rules]]
path = "/static/*"
enabled = true
ttl = "24h"
cache_key = "$uri"

[[cache.rules]]
path = "/api/*"
enabled = false  # Don't cache API responses
```

---

## Scenario 12: Microservices Discovery

### Overview

Service discovery with Consul/etcd and circuit breaker.

### Configuration

```toml
# /etc/highper-gateway/scenario-12-discovery.toml

[service_discovery]
enabled = true
backend = "consul"  # consul, etcd
address = "localhost:8500"
refresh_interval = "30s"

[[upstreams]]
name = "user-service"
protocol = "http"
discovery_service = "user-service"  # Consul service name

[upstreams.circuit_breaker]
enabled = true
failure_threshold = 5
timeout = "30s"
```

---

## Scenario 13: GraphQL Gateway

### Overview

GraphQL schema stitching and federation.

### Configuration

```toml
# /etc/highper-gateway/scenario-13-graphql.toml

[graphql]
enabled = true
federation = true

[[graphql.services]]
name = "users"
url = "http://users-graphql:4001/graphql"
schema_file = "/etc/highper-gateway/schemas/users.graphql"

[[graphql.services]]
name = "posts"
url = "http://posts-graphql:4002/graphql"
schema_file = "/etc/highper-gateway/schemas/posts.graphql"
```

---

## Scenario 14: Static + PHP-FPM

### Overview

FastCGI protocol implementation for PHP-FPM.

### Configuration

```toml
# /etc/highper-gateway/scenario-14-php-fpm.toml

[[routes]]
path = "*.php"
protocol = "fastcgi"
upstream = "php-fpm"

[[routes]]
path = "/static/*"
protocol = "http"
upstream = "static-files"

[[upstreams]]
name = "php-fpm"
protocol = "fastcgi"
servers = ["unix:///var/run/php/php8.2-fpm.sock"]

[upstreams.fastcgi]
script_filename = "/var/www/html$uri"
document_root = "/var/www/html"
```

---

## Scenario 15: Geographic Load Balancing

### Overview

GeoIP-based routing using MaxMind or IP2Location.

### Configuration

```toml
# /etc/highper-gateway/scenario-15-geo-lb.toml

[geoip]
enabled = true
database = "maxmind"  # maxmind, ip2location
database_path = "/etc/highper-gateway/geoip/GeoLite2-City.mmdb"

[[upstreams]]
name = "us-east"
region = "us-east-1"
servers = ["http://us-east-1.example.com"]

[[upstreams]]
name = "eu-west"
region = "eu-west-1"
servers = ["http://eu-west-1.example.com"]

[load_balancer]
strategy = "geo_proximity"  # Route to nearest region
fallback = "us-east"
```

---

## Daemon Management

### systemd Service Template

```bash
# /etc/systemd/system/highper-gateway.service
[Unit]
Description=Highper Gateway - Scenario %i
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=highper
Group=highper
ExecStartPre=/usr/local/bin/highper-gateway --config /etc/highper-gateway/scenario-%i.toml --validate
ExecStart=/usr/local/bin/highper-gateway --config /etc/highper-gateway/scenario-%i.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

```bash
# Usage:
sudo systemctl start highper-gateway@01   # Scenario 01
sudo systemctl start highper-gateway@02   # Scenario 02
```

---

## Monitoring & Health Checks

### Health Check Endpoints

```bash
# Gateway health
curl http://localhost:9090/health

# Backend health status
curl http://localhost:9090/backends

# Metrics
curl http://localhost:9091/metrics
```

---

## Production Best Practices

1. **Always enable TLS** in production
2. **Use systemd** for service management
3. **Enable health checks** for all backends
4. **Monitor metrics** via Prometheus
5. **Set appropriate timeouts** for your workload
6. **Enable rate limiting** to prevent abuse
7. **Use Redis** for distributed state (cache, rate limits, sessions)
8. **Log to files** (not stdout) in production
9. **Enable SIGHUP reload** for zero-downtime config updates
10. **Use connection pooling** for better performance

---

## Troubleshooting

### Common Issues

**Issue:** Gateway won't start
```bash
# Check logs
sudo journalctl -u highper-gateway -n 100

# Validate config
highper-gateway --config /etc/highper-gateway/config.toml --validate
```

**Issue:** Backend health checks failing
```bash
# Test backend directly
curl http://backend:9000/health

# Check DNS resolution
dig backend.local
```

**Issue:** Rate limiting not working
```bash
# Check Redis connection
redis-cli -h localhost -p 6379 ping

# Check rate limit metrics
curl http://localhost:9091/metrics | grep rate_limit
```

---

**Document Complete**
**Version:** 1.0
**Last Updated:** January 11, 2026
**License:** Apache 2.0
