# Use Case 08: Database Load Balancer

Specialized database load balancing for MySQL, PostgreSQL, and Redis with connection pooling.

## Overview

| Property | Value |
|----------|-------|
| Protocol | TCP (Database-specific) |
| Ports | 3306, 5432, 6379 |
| TLS Required | Optional |
| Privileged | Yes |
| Scaling | HA Pair |

## When to Use

- Database connection pooling
- Read replica load balancing
- Database failover
- Connection limiting

## Architecture

```
                         ┌──────────────────────┐
    MySQL:3306   ──────▶ │                      │ ────▶ MySQL Primary
                         │   Highper Gateway    │ ────▶ MySQL Replica
    Postgres:5432 ─────▶ │  (Database LB)       │ ────▶ PostgreSQL
                         │                      │
    Redis:6379   ──────▶ │  • Connection Pool   │ ────▶ Redis Cluster
                         │  • Health Checks     │
                         └──────────────────────┘
```

## Quick Start

```bash
cp configs/yaml/uc08-database-lb.yaml /etc/highper-gateway/config.yaml
systemctl start highper-gateway
```

## Configuration

### MySQL Load Balancer

```yaml
listeners:
  - name: mysql
    protocol: tcp
    bind: "0.0.0.0:3306"
    backend: mysql-pool

backends:
  - name: mysql-pool
    strategy: least_conn
    servers:
      - address: "mysql-primary:3306"
        weight: 10
        role: primary
      - address: "mysql-replica-1:3306"
        weight: 5
        role: replica
    health_check:
      enabled: true
      protocol: tcp
      interval: 5
```

### PostgreSQL

```yaml
backends:
  - name: postgres-pool
    strategy: least_conn
    servers:
      - address: "postgres-primary:5432"
      - address: "postgres-replica:5432"
    connection:
      max_connections: 500
      idle_timeout: 300000
```

### Redis

```yaml
backends:
  - name: redis-pool
    strategy: round_robin
    servers:
      - address: "redis-1:6379"
      - address: "redis-2:6379"
      - address: "redis-3:6379"
    connection:
      max_connections: 5000
      idle_timeout: 60000
```

## Connection Pooling

```yaml
connection_pool:
  enabled: true
  min_idle: 10
  max_idle: 100
  max_lifetime: 3600000
  validation_interval: 30000
```

## Health Checks

TCP health checks for database ports:

```yaml
health_check:
  enabled: true
  protocol: tcp
  interval: 5
  timeout: 2
  threshold: 3
```

## Read/Write Splitting

Route writes to primary, reads to replicas:

```yaml
# Application connects to different ports
listeners:
  - name: mysql-write
    bind: "0.0.0.0:3306"
    backend: mysql-primary
  - name: mysql-read
    bind: "0.0.0.0:3307"
    backend: mysql-replicas
```

## Monitoring

Key metrics:
- `highper_gateway_db_connections_active`
- `highper_gateway_db_connections_idle`
- `highper_gateway_backend_up{type="database"}`

## Related Use Cases

- [UC01: TCP Proxy](./uc01-tcp-proxy.md) - Generic TCP
- [UC12: Discovery](./uc12-discovery.md) - Dynamic backends
