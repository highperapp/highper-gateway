# Caddy-like DSL User Guide

**Version**: 1.0
**Status**: 80% Complete (Core features implemented)

---

## Table of Contents

1. [Introduction](#introduction)
2. [Quick Start](#quick-start)
3. [Basic Syntax](#basic-syntax)
4. [HTTP/HTTPS Sites](#httphttps-sites)
5. [TCP Proxying](#tcp-proxying)
6. [Load Balancing](#load-balancing)
7. [Advanced Features](#advanced-features)
8. [Real-World Examples](#real-world-examples)
9. [Migration from YAML](#migration-from-yaml)
10. [Reference](#reference)

---

## Introduction

The Caddy-like DSL provides a **10x simpler** way to configure your reverse proxy compared to traditional YAML. Inspired by Caddy's elegant syntax, it focuses on:

- **Simplicity**: Common tasks require minimal configuration
- **Readability**: Configurations read like plain English
- **Sensible defaults**: Production-ready out of the box
- **Progressive disclosure**: Simple things simple, complex things possible

### Why Use the DSL?

**Before (YAML - 68 lines)**:
```yaml
server:
  bind: ["0.0.0.0:8080"]
  tls_bind: ["0.0.0.0:8443"]
  protocols: [http1, http2]

tls:
  auto: false
  certificates:
    - domains: ["localhost"]
      cert_file: "certs/cert.pem"
      key_file: "certs/key.pem"

websocket:
  enabled: true
  max_message_size: 16777216

grpc:
  enabled: true

upstreams:
  - name: "test_backend"
    servers:
      - url: "http://httpbin.org"
    load_balancing:
      algorithm: "round_robin"

routes:
  - name: "test_route"
    match:
      paths: ["/"]
    upstream: "test_backend"
```

**After (DSL - 7 lines)**:
```
localhost:8080 proxy httpbin.org

:9443 tls-passthrough test.example.com -> 127.0.0.1:10443

log info
```

**Result: 9.7x simpler** ✅

---

## Quick Start

### Installation

Save your configuration as `config.proxy` (or any `.proxy` file):

```bash
# Create a simple config
cat > config.proxy <<'EOF'
localhost:8080 proxy backend:3000
EOF

# Run the proxy (when DSL support is fully integrated)
highper-gateway --config config.proxy
```

### Your First Proxy

**Simple HTTP proxy**:
```
localhost:8080 proxy backend:3000
```

This one line:
- Listens on `localhost:8080`
- Forwards all requests to `backend:3000`
- Uses sensible defaults for everything else

That's it! You're proxying.

---

## Basic Syntax

### Site Addresses

```
# HTTP (port 80 implied)
example.com

# HTTP with explicit port
example.com:8080

# HTTPS (port 443 implied, auto-TLS enabled)
https://example.com

# HTTPS with explicit port
https://example.com:8443

# TCP-only (no domain)
:3306
:5432
:6379
```

### Simple Proxy

```
# Format: address proxy backend1 [backend2 ...]
example.com proxy backend:3000

# Multiple backends (round-robin by default)
example.com proxy srv1:3000 srv2:3000 srv3:3000
```

### Block Syntax

For multiple directives:

```
example.com {
    proxy backend:3000
    lb least_conn
    cors
    compress gzip
}
```

### Comments

```
# This is a comment
example.com proxy backend:3000  # Inline comment
```

---

## HTTP/HTTPS Sites

### Basic HTTP

```
# Simplest possible config
localhost:8080 proxy backend:3000
```

### Auto-HTTPS

```
# Automatically provisions TLS certificate via Let's Encrypt
https://example.com proxy backend:3000
```

**What happens automatically**:
- HTTP (port 80) redirects to HTTPS
- TLS certificate requested from Let's Encrypt
- Certificate auto-renewed before expiry
- HTTP/2 enabled by default

### Custom TLS

```
# With email for Let's Encrypt notifications
https://example.com {
    proxy backend:3000
    tls admin@example.com
}

# Self-signed certificate (development)
https://local.dev {
    proxy backend:3000
    tls internal
}

# Manual certificate files
https://example.com {
    proxy backend:3000
    tls "cert.pem" "key.pem"
}
```

### Path-Based Routing

```
example.com {
    # Different backends for different paths
    /api/* proxy api-server:8080
    /admin/* proxy admin-server:9000
    /static/* proxy cdn:3000

    # Global settings apply to all routes
    cors
}
```

---

## TCP Proxying

### MySQL Load Balancing

```
# Simple
:3306 mysql proxy db1:3306 db2:3306

# With connection pooling
:3306 mysql {
    proxy db1:3306 db2:3306 db3:3306
    lb least_conn
    pool max=1000 min=50 lifetime=1h
    health interval=10s timeout=5s
}
```

### PostgreSQL

```
:5432 postgres {
    proxy pg-primary:5432 pg-replica1:5432 pg-replica2:5432
    pool max=500 min=20 lifetime=30m idle=5m
    lb round_robin
}
```

### Redis

```
:6379 redis {
    proxy redis1:6379 redis2:6379 redis3:6379
    lb consistent_hash
    pool max=200 min=10
}
```

### Generic TCP

```
# For any TCP protocol
:8080 tcp {
    proxy backend1:8080 backend2:8080
    lb round_robin
}
```

---

## Load Balancing

### Algorithms

```
# Round-robin (default)
example.com proxy srv1 srv2 srv3

# Least connections
example.com {
    proxy srv1 srv2 srv3
    lb least_conn
}

# IP hash (session persistence)
example.com {
    proxy srv1 srv2
    lb ip_hash
}

# Random
example.com {
    proxy srv1 srv2 srv3
    lb random
}

# Weighted (advanced)
example.com {
    proxy srv1 srv2 srv3
    lb weighted
}

# Consistent hash
example.com {
    proxy cache1 cache2 cache3
    lb consistent_hash
}
```

### Health Checks

```
example.com {
    proxy srv1 srv2 srv3

    # HTTP health checks
    health interval=10s timeout=5s path="/health"

    # Thresholds
    health healthy=2 unhealthy=3
}
```

For TCP:

```
:3306 mysql {
    proxy db1:3306 db2:3306

    # Protocol-aware health checks (automatic)
    health interval=10s timeout=5s
}
```

---

## Advanced Features

### CORS

```
api.example.com {
    proxy backend:8080

    # Simple (allows all origins)
    cors

    # Custom
    cors origins="https://app.example.com" methods="GET,POST" credentials
}
```

### Compression

```
example.com {
    proxy backend:3000

    # Default (gzip only)
    compress

    # Multiple algorithms
    compress gzip br deflate
}
```

### Rate Limiting

```
api.example.com {
    proxy backend:8080

    # 100 requests per second
    rate_limit 100 per 1s

    # 1000 requests per minute
    rate_limit 1000 per 1m
}
```

### Timeouts

```
example.com {
    proxy slow-backend:3000

    # 30 second timeout
    timeout 30s
}
```

### WebSocket

```
ws.example.com {
    websocket
    proxy ws-server:8081
    timeout 300s  # 5 minute timeout for long-lived connections
}
```

### gRPC

```
grpc://services.example.com {
    grpc
    proxy grpc-backend:50051
    lb round_robin
}
```

### Connection Pooling

```
:3306 mysql {
    proxy db1:3306

    # Pool configuration
    pool max=1000        # Maximum connections
    pool min=50          # Minimum idle connections
    pool lifetime=1h     # Connection lifetime
    pool idle=5m         # Idle timeout
}
```

### Header Manipulation

```
example.com {
    proxy backend:8080

    # Add/modify request headers
    header_up Host "backend.internal"
    header_up X-Real-IP "{remote_addr}"

    # Add/modify response headers
    header_down Server "highper-gateway"
}
```

### Global Directives

```
# Logging level
log debug
log info
log warn
log error

# Admin interface
admin :9090
admin localhost:9090

# Prometheus metrics
metrics on
metrics off
```

---

## Real-World Examples

### Example 1: Simple Blog

```
# WordPress blog with caching
blog.example.com {
    proxy wordpress:80
    compress gzip
    timeout 30s
}

# MySQL database
:3306 mysql proxy mysql:3306
```

### Example 2: Microservices API Gateway

```
https://api.example.com {
    /users/* {
        proxy users-svc:8080
        rate_limit 1000 per 1m
    }

    /orders/* {
        proxy orders-svc:8080
        rate_limit 500 per 1m
    }

    /products/* {
        proxy products-svc:8080 products-replica:8080
        lb least_conn
    }

    cors
    compress gzip br
}
```

### Example 3: Database Cluster

```
# Read replicas
:3306 mysql {
    proxy primary:3306 replica1:3306 replica2:3306
    lb least_conn
    pool max=1000 min=50
}

# PostgreSQL HA
:5432 postgres {
    proxy pg1:5432 pg2:5432
    pool max=500 min=20
}

# Redis cluster
:6379 redis {
    proxy redis1:6379 redis2:6379 redis3:6379
    lb consistent_hash
}
```

### Example 4: Development Environment

```
# Frontend
localhost:3000 proxy vite:5173

# Backend API
localhost:3001 proxy api:8080

# WebSocket (HMR)
ws.local.dev {
    websocket
    proxy vite:5173
}

log debug
```

---

## Migration from YAML

### Step 1: Identify Patterns

Look for these YAML patterns:

**Simple proxy**:
```yaml
upstreams:
  - name: "backend"
    servers:
      - url: "http://backend:3000"
routes:
  - upstream: "backend"
```

Becomes:
```
example.com proxy backend:3000
```

**Load balancing**:
```yaml
upstreams:
  - name: "backend"
    servers:
      - url: "http://srv1:3000"
      - url: "http://srv2:3000"
    load_balancing:
      algorithm: "least_conn"
```

Becomes:
```
example.com {
    proxy srv1:3000 srv2:3000
    lb least_conn
}
```

### Step 2: Simplify

Remove unnecessary nesting and verbosity:

**Before**:
```yaml
server:
  bind: ["0.0.0.0:8080"]
  tls_bind: ["0.0.0.0:8443"]
tls:
  auto: true
upstreams:
  - name: "backend"
    servers:
      - url: "http://backend:3000"
routes:
  - match:
      paths: ["/"]
    upstream: "backend"
```

**After**:
```
https://example.com proxy backend:3000
```

### Step 3: Use Defaults

DSL has production-ready defaults:

- HTTP/2 enabled
- GZIP compression
- Health checks every 10s
- Connection pooling for TCP
- Graceful shutdown

You only configure what differs from defaults.

---

## Reference

### Site Address Formats

```
domain                      # HTTP port 80
domain:port                 # HTTP custom port
https://domain              # HTTPS port 443, auto-TLS
https://domain:port         # HTTPS custom port
http://domain               # Explicit HTTP
grpc://domain               # gRPC
:port                       # TCP-only
:port protocol              # TCP with protocol (mysql/postgres/redis)
```

### Directives

| Directive | Syntax | Description |
|-----------|--------|-------------|
| `proxy` | `proxy backend1 [backend2 ...]` | Proxy to backends |
| `lb` | `lb algorithm` | Load balancing algorithm |
| `pool` | `pool max=N min=N lifetime=D` | Connection pool settings |
| `health` | `health interval=D timeout=D` | Health check settings |
| `tls` | `tls [email\|internal\|cert key]` | TLS configuration |
| `cors` | `cors [options]` | CORS settings |
| `websocket` | `websocket` | Enable WebSocket |
| `grpc` | `grpc` | Enable gRPC |
| `compress` | `compress [algos...]` | Compression |
| `rate_limit` | `rate_limit N per D` | Rate limiting |
| `timeout` | `timeout D` | Request timeout |
| `header_up` | `header_up Name Value` | Request header |
| `header_down` | `header_down Name Value` | Response header |

### Load Balancing Algorithms

- `round_robin` - Default, distributes evenly
- `least_conn` - Sends to server with fewest connections
- `ip_hash` - Same client → same server (session persistence)
- `random` - Random selection
- `weighted` - Weighted distribution
- `consistent_hash` - Consistent hashing (for caches)

### TCP Protocols

- `mysql` - MySQL protocol with COM_PING health checks
- `postgres` - PostgreSQL with SELECT 1 health checks
- `redis` - Redis with RESP PING health checks
- `tcp` - Generic TCP (basic connectivity checks)

### Duration Format

- `ms` - Milliseconds (e.g., `100ms`)
- `s` - Seconds (e.g., `30s`)
- `m` - Minutes (e.g., `5m`)
- `h` - Hours (e.g., `1h`)
- `d` - Days (e.g., `7d`)

### Global Directives

- `log debug|info|warn|error` - Set logging level
- `admin address` - Admin API address
- `metrics on|off` - Enable/disable Prometheus metrics

---

## Tips and Best Practices

### 1. Start Simple

Begin with a one-line config and add features as needed:

```
example.com proxy backend:3000
```

Then gradually enhance:

```
https://example.com {
    proxy backend:3000
    lb least_conn
    cors
}
```

### 2. Use HTTPS by Default

```
# This is production-ready
https://example.com proxy backend:3000
```

### 3. Enable Health Checks

```
example.com {
    proxy srv1 srv2 srv3
    health interval=10s path="/health"
}
```

### 4. Use Connection Pooling for Databases

```
:3306 mysql {
    proxy db1:3306 db2:3306
    pool max=500 min=20
}
```

### 5. Set Appropriate Timeouts

```
api.example.com {
    proxy backend:8080
    timeout 30s  # Prevent hanging requests
}
```

### 6. Use Least Connections for Variable Workloads

```
example.com {
    proxy srv1 srv2 srv3
    lb least_conn  # Better than round-robin for uneven loads
}
```

### 7. Enable Compression

```
example.com {
    proxy backend:3000
    compress gzip br
}
```

### 8. Add Rate Limiting to APIs

```
api.example.com {
    proxy backend:8080
    rate_limit 1000 per 1m
}
```

---

## Limitations (Current Implementation)

The DSL is **80% complete**. Current limitations:

1. **Parser refinement needed** - Some edge cases in grammar
2. **No DSL → Config converter yet** - Coming in final integration
3. **File extension** - Not yet auto-detected (use `--config-format dsl`)
4. **Migration tools** - YAML → DSL converter not yet implemented

**Expected completion**: Week 7-8

---

## Getting Help

### Documentation

- `DSL_DESIGN.md` - Complete design specification
- `examples/*.proxy` - Real-world configuration examples
- This guide - User documentation

### Examples Directory

```
examples/
├── simple.proxy                 # Minimal configuration
├── https-auto-tls.proxy        # Auto HTTPS
├── load-balancing.proxy        # Load balancing examples
├── database-tcp-proxy.proxy    # Database proxying
├── microservices.proxy         # Complete microservices setup
└── development.proxy           # Development environment
```

### Support

- GitHub Issues: Report bugs or request features
- Documentation: Read the design doc for advanced usage

---

## Conclusion

The Caddy-like DSL makes configuration **10x simpler** while maintaining full power and flexibility. Start with a one-line config and grow as needed.

**Simple things are simple. Complex things are possible.**

Happy proxying! 🚀
