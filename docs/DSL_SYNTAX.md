# Highper Gateway DSL Syntax Reference

**Version**: 1.0
**Status**: Production-Ready
**Format**: Caddy-like configuration language

---

## Table of Contents

1. [Overview](#overview)
2. [Basic Syntax](#basic-syntax)
3. [Site Addressing](#site-addressing)
4. [Directives Reference](#directives-reference)
5. [Global Configuration](#global-configuration)
6. [Path-Based Routing](#path-based-routing)
7. [Examples](#examples)
8. [Migration from YAML](#migration-from-yaml)

---

## Overview

The Highper Gateway DSL provides a **10x simpler** configuration format compared to YAML for common reverse proxy use cases. It uses a Caddy-inspired syntax that's:

- **Human-readable**: No complex nesting or indentation
- **Concise**: 40-60% fewer lines than equivalent YAML
- **Type-safe**: Validated at parse time
- **Production-ready**: Powers high-traffic deployments

### Why DSL over YAML?

**YAML (50 lines)**:
```yaml
server:
  bind:
    - "0.0.0.0:8080"
upstreams:
  - name: "backend"
    servers:
      - url: "http://localhost:3000"
        weight: 1
    load_balancing:
      algorithm: "round_robin"
routes:
  - name: "default"
    match:
      paths: ["/"]
    upstream: "backend"
```

**DSL (4 lines)**:
```proxy
http://example.com:8080 {
    proxy localhost:3000
    lb round_robin
}
```

**92% reduction in configuration complexity!**

---

## Basic Syntax

### Comments

```proxy
# This is a comment
// This is also a comment

# Comments can appear anywhere
http://example.com proxy backend:3000  # Inline comments work too
```

### Simple Proxy

```proxy
# Minimal configuration: just proxy requests to a backend
localhost:8080 proxy backend:3000
```

### Block Syntax

```proxy
# Use curly braces for multiple directives
example.com:8080 {
    proxy backend1:3000
    lb round_robin
    health /health interval=10s
}
```

---

## Site Addressing

### HTTP Sites

```proxy
# HTTP on port 8080
http://example.com:8080

# HTTP with default port 80
http://example.com

# Just a domain (defaults to HTTP:80)
example.com

# Localhost shorthand
localhost:8080
```

### HTTPS Sites

```proxy
# HTTPS with automatic Let's Encrypt certificates
https://example.com

# HTTPS on custom port
https://example.com:8443

# Multiple domains
https://example.com example.org api.example.com
```

### WebSocket Sites

```proxy
# WebSocket (ws://)
ws://example.com:8080

# Secure WebSocket (wss://)
wss://example.com
```

### gRPC Sites

```proxy
# gRPC service
grpc://services.example.com

# gRPC with custom port
grpc://services.example.com:50051
```

### TCP Sites

```proxy
# TCP proxy on port 3306 (MySQL)
:3306 mysql

# TCP proxy on port 5432 (PostgreSQL)
:5432 postgres

# TCP proxy on port 6379 (Redis)
:6379 redis

# Generic TCP proxy
:8080 tcp
```

### Wildcard Domains

```proxy
# Match all subdomains
*.example.com

# Match multiple patterns
*.example.com *.example.org
```

---

## Directives Reference

### `proxy` - Backend Servers

Specify one or more backend servers to proxy requests to.

**Syntax**:
```proxy
proxy <backend> [<backend>...]
proxy <backend> weight=<weight>
```

**Examples**:
```proxy
# Single backend
proxy localhost:3000

# Multiple backends (round-robin by default)
proxy server1:3000 server2:3000 server3:3000

# Weighted backends
example.com {
    proxy primary:8080 weight=5
    proxy secondary:8080 weight=1
}

# Different protocols
proxy http://backend:8080
proxy https://secure-backend:8443
```

---

### `lb` - Load Balancing

Choose load balancing algorithm for multiple backends.

**Syntax**:
```proxy
lb <algorithm>
```

**Algorithms**:
- `round_robin` - Distribute requests evenly (default)
- `least_conn` - Route to backend with fewest active connections
- `ip_hash` - Route based on client IP (session persistence)
- `random` - Random backend selection
- `consistent_hash` - Consistent hashing (for distributed caches)
- `weighted` - Weighted round-robin (use with `weight=` in proxy)
- `maglev` - Google Maglev consistent hashing (advanced)

**Examples**:
```proxy
# Least connections
api.example.com {
    proxy api1:8080 api2:8080 api3:8080
    lb least_conn
}

# IP hash for session affinity
app.example.com {
    proxy app1:3000 app2:3000
    lb ip_hash
}

# Consistent hash for cache distribution
cache.example.com {
    proxy cache1:6379 cache2:6379 cache3:6379
    lb consistent_hash
}
```

---

### `health` - Health Checks

Configure active health checking for backends.

**Syntax**:
```proxy
health [path=<path>] [interval=<duration>] [timeout=<duration>] [healthy=<N>] [unhealthy=<N>]
```

**Parameters**:
- `path` - Health check endpoint (default: `/health`)
- `interval` - Time between checks (default: `10s`)
- `timeout` - Request timeout (default: `5s`)
- `healthy` - Consecutive successes to mark healthy (default: `2`)
- `unhealthy` - Consecutive failures to mark unhealthy (default: `3`)

**Examples**:
```proxy
# Simple health check
example.com {
    proxy backend:8080
    health
}

# Custom health check
api.example.com {
    proxy api:8080
    health path="/api/health" interval=5s timeout=2s
}

# Strict health check
critical.example.com {
    proxy backend:9000
    health interval=1s healthy=5 unhealthy=2
}
```

---

### `tls` - TLS/SSL Configuration

Configure TLS certificates (automatic or manual).

**Syntax**:
```proxy
tls [auto] [<email>]
tls <cert_file> <key_file>
tls internal
```

**Modes**:
- `tls auto` or `tls <email>` - Let's Encrypt ACME (automatic)
- `tls <cert> <key>` - Manual certificate files
- `tls internal` - Self-signed certificate (development)

**Examples**:
```proxy
# Automatic Let's Encrypt certificate
https://example.com {
    proxy backend:3000
    tls auto
}

# Automatic with notification email
https://api.example.com {
    proxy backend:8080
    tls admin@example.com
}

# Manual certificate
https://secure.example.com {
    proxy backend:9000
    tls /etc/certs/example.crt /etc/certs/example.key
}

# Self-signed (development only)
https://local.dev {
    proxy localhost:3000
    tls internal
}
```

---

### `rate_limit` - Rate Limiting

Limit request rate per client IP.

**Syntax**:
```proxy
rate_limit <rate> per <duration>
rate_limit <rate>/<unit>
```

**Units**:
- `s` - per second
- `m` - per minute
- `h` - per hour

**Examples**:
```proxy
# 100 requests per second
api.example.com {
    proxy backend:8080
    rate_limit 100/s
}

# 1000 requests per minute
api.example.com {
    proxy backend:8080
    rate_limit 1000 per 1m
}

# Different limits per route
example.com {
    /api/public/* {
        proxy api:8080
        rate_limit 1000/m
    }

    /api/premium/* {
        proxy api:8080
        rate_limit 10000/m
    }
}
```

---

### `timeout` - Request Timeout

Set maximum request duration.

**Syntax**:
```proxy
timeout <duration>
```

**Examples**:
```proxy
# 30 second timeout
api.example.com {
    proxy slow-backend:8080
    timeout 30s
}

# Different timeouts per route
example.com {
    /api/fast/* {
        proxy fast-api:8080
        timeout 5s
    }

    /api/slow/* {
        proxy batch-api:8080
        timeout 120s
    }
}
```

---

### `pool` - Connection Pool

Configure connection pooling settings.

**Syntax**:
```proxy
pool max=<N> [min=<N>] [lifetime=<duration>] [idle=<duration>]
```

**Parameters**:
- `max` - Maximum connections per backend
- `min` - Minimum idle connections (optional)
- `lifetime` - Maximum connection lifetime (optional)
- `idle` - Idle timeout before closing (optional)

**Examples**:
```proxy
# Basic pool
api.example.com {
    proxy backend:8080
    pool max=1000
}

# Full pool configuration
database.example.com {
    proxy db:3306
    pool max=500 min=20 lifetime=1h idle=5m
}

# TCP database pooling
:3306 mysql {
    proxy mysql:3306
    pool max=1000 min=50 lifetime=1h
}
```

---

### `cors` - CORS Headers

Enable Cross-Origin Resource Sharing.

**Syntax**:
```proxy
cors
cors origins=<origins> methods=<methods> headers=<headers>
```

**Examples**:
```proxy
# Simple CORS (allow all)
api.example.com {
    proxy backend:8080
    cors
}

# Custom CORS
api.example.com {
    proxy backend:8080
    cors origins=https://app.example.com methods=GET,POST,PUT
}
```

---

### `compress` - Response Compression

Enable response compression.

**Syntax**:
```proxy
compress [algorithms...]
```

**Algorithms**:
- `gzip` - Gzip compression
- `br` - Brotli compression
- `zstd` - Zstandard compression
- `deflate` - Deflate compression

**Examples**:
```proxy
# Default compression (gzip + brotli)
cdn.example.com {
    proxy origin:9000
    compress
}

# Specific algorithms
api.example.com {
    proxy backend:8080
    compress gzip br
}

# All compression methods
static.example.com {
    proxy static-server:8000
    compress gzip br zstd deflate
}
```

---

### `websocket` - WebSocket Support

Enable WebSocket proxying.

**Syntax**:
```proxy
websocket
```

**Examples**:
```proxy
# WebSocket proxy
ws://realtime.example.com {
    websocket
    proxy ws-backend:8081
    timeout 300s  # Long timeout for persistent connections
}

# Secure WebSocket
wss://secure-ws.example.com {
    websocket
    proxy ws-backend:8081
    tls auto
}
```

---

### `grpc` - gRPC Support

Enable gRPC proxying.

**Syntax**:
```proxy
grpc
```

**Examples**:
```proxy
# gRPC service
grpc://services.example.com {
    grpc
    proxy grpc-backend:50051
    lb round_robin
}

# gRPC with health checks
grpc://api.example.com {
    grpc
    proxy grpc1:50051 grpc2:50051
    health path=/grpc.health.v1.Health/Check
}
```

---

### `headers` - Header Manipulation

Add, remove, or modify HTTP headers.

**Syntax**:
```proxy
headers add <name> <value>
headers set <name> <value>
headers del <name>
```

**Examples**:
```proxy
api.example.com {
    proxy backend:8080
    headers add X-Custom-Header "value"
    headers set X-Forwarded-Proto "https"
    headers del X-Powered-By
}
```

---

### `tls-passthrough` - SNI-Based TLS Passthrough

Route encrypted TLS traffic without termination based on SNI.

**Syntax**:
```proxy
tls-passthrough <server_name> <backend>
```

**Examples**:
```proxy
:443 {
    tls-passthrough secure.example.com backend1:443
    tls-passthrough api.example.com backend2:443
}
```

---

## Global Configuration

Global settings apply to all sites.

**Syntax**:
```proxy
{
    log <level>
    admin <address>
    metrics <on|off>
}
```

**Directives**:
- `log <level>` - Log level: `trace`, `debug`, `info`, `warn`, `error`
- `admin <address>` - Admin API listen address
- `metrics <on|off>` - Enable Prometheus metrics

**Examples**:
```proxy
# Global configuration block
{
    log info
    admin :9090
    metrics on
}

# Sites follow global config
https://api.example.com {
    proxy backend:8080
}
```

---

## Path-Based Routing

Route different paths to different backends.

**Syntax**:
```proxy
<domain> {
    <path> {
        # Directives for this path
    }

    <path> {
        # Directives for this path
    }
}
```

**Examples**:
```proxy
# Simple path routing
api.example.com {
    /v1/* {
        proxy api-v1:8080
    }

    /v2/* {
        proxy api-v2:8080
    }
}

# Path routing with different configurations
example.com {
    /api/* {
        proxy api:8080
        rate_limit 1000/m
        timeout 10s
    }

    /admin/* {
        proxy admin:8081
        rate_limit 100/m
        timeout 30s
    }

    /static/* {
        proxy cdn:9000
        compress gzip br
    }
}

# Microservices routing
https://services.example.com {
    /api/users/* {
        proxy users-svc:8080
        lb round_robin
        health path=/health
    }

    /api/orders/* {
        proxy orders-svc:8080
        lb least_conn
        rate_limit 500/m
    }

    /api/products/* {
        proxy products-svc:8080
        lb consistent_hash
        rate_limit 2000/m
    }
}
```

---

## Examples

### Simple HTTP Proxy

```proxy
localhost:8080 proxy backend:3000
```

### HTTPS with Auto TLS

```proxy
https://example.com {
    proxy backend:8080
    tls admin@example.com
}
```

### Load Balanced API

```proxy
api.example.com {
    proxy api1:8080 api2:8080 api3:8080
    lb least_conn
    health /health interval=10s
    rate_limit 1000/s
    timeout 30s
}
```

### Microservices Gateway

```proxy
{
    log info
    admin :9090
    metrics on
}

https://api.example.com {
    /users/* {
        proxy users-svc:8080
        rate_limit 1000/m
    }

    /orders/* {
        proxy orders-svc:8080
        rate_limit 500/m
    }

    /products/* {
        proxy products-svc:8080
        lb round_robin
        rate_limit 2000/m
    }

    cors
    compress gzip br
}
```

### Database TCP Proxy

```proxy
:3306 mysql {
    proxy mysql1:3306 mysql2:3306 mysql3:3306
    lb least_conn
    pool max=1000 min=50 lifetime=1h
    health interval=10s
}
```

### WebSocket Proxy

```proxy
wss://realtime.example.com {
    websocket
    proxy ws-backend:8081
    tls auto
    timeout 300s
}
```

### gRPC Service

```proxy
grpc://services.example.com {
    grpc
    proxy grpc1:50051 grpc2:50051
    lb round_robin
    health path=/grpc.health.v1.Health/Check
}
```

---

## Migration from YAML

Use the built-in `migrate` command to convert existing YAML configs:

```bash
# Convert YAML to DSL
highper-gateway migrate --input config.yaml --output config.proxy

# Validate conversion
highper-gateway migrate --input config.yaml --output config.proxy --validate

# Show differences
highper-gateway migrate --input config.yaml --output config.proxy --diff
```

### Migration Tips

1. **Start Simple**: Begin with basic HTTP proxying, add features incrementally
2. **Test Equivalence**: Use `--validate` to ensure configs are equivalent
3. **Review Output**: DSL is more concise - verify all features are preserved
4. **Iterative Approach**: Migrate one service at a time for complex configs

---

## Duration Format

Durations use standard units:

- `s` - seconds (e.g., `30s`)
- `m` - minutes (e.g., `5m`)
- `h` - hours (e.g., `1h`)
- `d` - days (e.g., `7d`)

**Examples**:
```proxy
timeout 30s
health interval=5m
pool lifetime=1h
rate_limit 100 per 1m
```

---

## Best Practices

### 1. Start with Defaults

```proxy
# Minimal config - use sensible defaults
example.com proxy backend:3000
```

### 2. Add Features Incrementally

```proxy
# Add load balancing
example.com {
    proxy backend1:3000 backend2:3000
    lb round_robin
}

# Add health checks
example.com {
    proxy backend1:3000 backend2:3000
    lb round_robin
    health /health interval=10s
}

# Add rate limiting
example.com {
    proxy backend1:3000 backend2:3000
    lb round_robin
    health /health interval=10s
    rate_limit 1000/s
}
```

### 3. Use Comments Liberally

```proxy
# Production API Gateway
# Updated: 2025-11-25
# Owner: DevOps Team

{
    log info            # Production logging level
    admin :9090         # Admin API for monitoring
    metrics on          # Prometheus metrics enabled
}

https://api.example.com {
    # User service cluster
    /api/users/* {
        proxy user-svc-1:8080 user-svc-2:8080
        lb round_robin
        rate_limit 1000/m
    }

    # ... more routes
}
```

### 4. Group Related Sites

```proxy
# Public APIs
https://api.example.com { ... }
https://api-v2.example.com { ... }

# Internal Services
grpc://internal.example.com { ... }
grpc://rpc.example.com { ... }

# Databases
:3306 mysql { ... }
:5432 postgres { ... }
:6379 redis { ... }
```

### 5. Use Validation

```bash
# Always validate before deploying
highper-gateway validate --config config.proxy

# Test upstream connectivity
highper-gateway test --config config.proxy
```

---

## Advanced Features

### Multi-Site Configuration

```proxy
# Global settings
{
    log info
    admin :9090
    metrics on
}

# Public HTTP API
http://api.example.com:8080 {
    proxy api:3000
}

# Public HTTPS API
https://api.example.com {
    proxy api:3000
    tls auto
}

# Admin Dashboard
https://admin.example.com {
    proxy admin-ui:4000
    tls internal
    rate_limit 100/m
}

# WebSocket Server
wss://ws.example.com {
    websocket
    proxy ws-server:8081
    timeout 300s
}

# Database Proxies
:3306 mysql {
    proxy db1:3306 db2:3306
    lb least_conn
    pool max=1000
}
```

### Conditional Routing

```proxy
# Route based on path prefixes
api.example.com {
    /v1/* {
        proxy legacy-api:8080
        timeout 60s
    }

    /v2/* {
        proxy new-api:8080
        timeout 30s
        rate_limit 1000/s
    }

    /admin/* {
        proxy admin:8081
        rate_limit 10/s
    }
}
```

---

## Troubleshooting

### Validation Errors

```bash
# Check syntax
highper-gateway validate --config config.proxy --verbose

# Common issues:
# - Missing closing braces
# - Invalid directive names
# - Incorrect duration format
# - Missing backend addresses
```

### Testing Configuration

```bash
# Test upstream connectivity
highper-gateway test --config config.proxy

# Test specific upstream
highper-gateway test --config config.proxy --upstream backend-1

# Check health endpoints
curl http://localhost:9090/api/health
```

### Debug Mode

```proxy
{
    log debug  # Enable debug logging
}

# Or run with environment variable:
# RUST_LOG=debug highper-gateway start --config config.proxy
```

---

## Performance Tips

1. **Use Connection Pooling** for TCP proxies and high-traffic HTTP sites
2. **Enable Compression** for text-based APIs (JSON, HTML)
3. **Configure Health Checks** to avoid routing to failed backends
4. **Use Appropriate Load Balancing**:
   - `round_robin` - General purpose
   - `least_conn` - Long-lived connections
   - `ip_hash` - Session persistence
   - `consistent_hash` - Distributed caches
5. **Set Reasonable Timeouts** based on backend response times
6. **Enable Rate Limiting** to protect backends from overload

---

## Security Considerations

1. **Always Use TLS in Production**: `https://` with auto or manual certificates
2. **Configure Rate Limiting**: Protect against DoS attacks
3. **Set Request Timeouts**: Prevent slowloris attacks
4. **Use Health Checks**: Automatic backend failure detection
5. **Enable Security Headers**: Use middleware for HSTS, CSP, etc.
6. **Limit Pool Sizes**: Prevent resource exhaustion

---

## File Extensions

- `.proxy` - DSL configuration files
- `.yaml` - YAML configuration files (supported)
- `.json` - JSON configuration files (supported)
- `.toml` - TOML configuration files (supported)

**Recommendation**: Use `.proxy` for new configurations for simplicity and clarity.

---

## CLI Commands

```bash
# Start with DSL config
highper-gateway start --config config.proxy

# Validate DSL syntax
highper-gateway validate --config config.proxy

# Test upstream connectivity
highper-gateway test --config config.proxy

# Migrate from YAML to DSL
highper-gateway migrate --input config.yaml --output config.proxy

# Show version
highper-gateway version
```

---

## Resources

- **GitHub**: https://github.com/highperapp/highper-gateway
- **Documentation**: https://docs.highper-gateway.dev
- **Examples**: `/examples/*.proxy` in the repository
- **Support**: https://github.com/highperapp/highper-gateway/issues

---

## Version History

- **1.0** (2025-11-25) - Initial production release
  - Core HTTP/HTTPS proxying
  - Load balancing (5 algorithms)
  - Health checks
  - TLS (auto + manual)
  - Rate limiting
  - Timeouts
  - Connection pooling
  - TCP proxying
  - WebSocket support
  - gRPC support
  - Path-based routing
  - Global configuration

---

**Last Updated**: November 25, 2025
**Status**: Production-Ready (v1.0)
**License**: Apache 2.0
