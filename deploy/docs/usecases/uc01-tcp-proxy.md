# Use Case 01: Layer 4 TCP Proxy

High-performance Layer 4 TCP proxy for transparent traffic forwarding without protocol inspection.

## Overview

| Property | Value |
|----------|-------|
| Protocol | TCP (Layer 4) |
| Ports | 80, 443, 3306, 5432 |
| TLS Required | No (passthrough) |
| Privileged | Yes |
| Scaling | Horizontal |

## When to Use

- Database connection pooling (MySQL, PostgreSQL)
- TLS passthrough (terminate at backend)
- Non-HTTP protocols
- Maximum performance with minimal overhead
- Protocol-agnostic load balancing

## Architecture

```
                    ┌─────────────────────┐
    TCP:80    ────▶ │                     │ ────▶ Web Servers
    TCP:443   ────▶ │   Highper Gateway   │ ────▶ TLS Backends
    TCP:3306  ────▶ │    (L4 TCP Proxy)   │ ────▶ MySQL Cluster
    TCP:5432  ────▶ │                     │ ────▶ PostgreSQL
                    └─────────────────────┘
```

## Quick Start

```bash
# Deploy configuration
cp configs/yaml/uc01-tcp-proxy.yaml /etc/highper-gateway/config.yaml

# Start gateway
systemctl start highper-gateway
```

## Configuration

### TCP Listeners

```yaml
listeners:
  - name: mysql-proxy
    protocol: tcp
    bind: "0.0.0.0:3306"
    backend: mysql-servers
    connection:
      timeout: 30000
      keepalive: true
```

### Backend Pools

```yaml
backends:
  - name: mysql-servers
    strategy: least_conn
    servers:
      - address: "mysql-primary:3306"
        weight: 10
      - address: "mysql-replica:3306"
        weight: 5
    health_check:
      enabled: true
      protocol: tcp
      interval: 5
```

## Load Balancing Strategies

| Strategy | Best For |
|----------|----------|
| `round_robin` | Equal capacity servers |
| `least_conn` | Database connections |
| `ip_hash` | Session persistence |

## Health Checks

TCP health checks verify port connectivity:

```yaml
health_check:
  enabled: true
  protocol: tcp
  interval: 5
  timeout: 2
  threshold: 3
```

## Performance Tuning

```yaml
io_uring:
  entries: 8192      # Larger ring for high throughput
  sq_poll: true      # Kernel-side polling

connection:
  max_connections: 65535
  timeout: 30000
```

## Monitoring

```bash
# Check active connections
ss -tnp | grep highper-gateway

# Health endpoint
curl http://localhost:8081/health
```

## Related Use Cases

- [UC08: Database LB](./uc08-database-lb.md) - Database-specific features
- [UC03: HTTPS/TLS](./uc03-https-tls.md) - TLS termination instead of passthrough
