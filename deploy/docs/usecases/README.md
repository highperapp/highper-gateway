# Highper Gateway Use Cases

This directory contains documentation for all 15 supported deployment use cases.

## Use Case Overview

| ID | Use Case | Ports | Config | Complexity |
|----|----------|-------|--------|------------|
| [01](./uc01-tcp-proxy.md) | Layer 4 TCP Proxy | 80, 443, 3306, 5432 | `uc01-tcp-proxy.yaml` | Low |
| [02](./uc02-http-lb.md) | HTTP Load Balancer | 80, 8080 | `uc02-http-lb.yaml` | Low |
| [03](./uc03-https-tls.md) | HTTPS/TLS Termination | 443, 8443 | `uc03-https-tls.yaml` | Medium |
| [04](./uc04-api-gateway.md) | API Gateway | 8080, 443 | `uc04-api-gateway.yaml` | Medium |
| [05](./uc05-http3-quic.md) | HTTP/3 QUIC | 443/UDP | `uc05-http3-quic.yaml` | High |
| [06](./uc06-websocket.md) | WebSocket LB | 80, 443, 8080 | `uc06-websocket.yaml` | Medium |
| [07](./uc07-grpc-gateway.md) | gRPC Gateway | 9090, 443 | `uc07-grpc-gateway.yaml` | Medium |
| [08](./uc08-database-lb.md) | Database LB | 3306, 5432, 6379 | `uc08-database-lb.yaml` | Medium |
| [09](./uc09-waf-mtls.md) | WAF + mTLS | 443 | `uc09-waf-mtls.yaml` | High |
| [10](./uc10-hybrid.md) | Hybrid Multi-Protocol | Multiple | `uc10-hybrid.yaml` | High |
| [11](./uc11-cdn-edge.md) | CDN Edge Caching | 80, 443 | `uc11-cdn-edge.yaml` | Medium |
| [12](./uc12-discovery.md) | Service Discovery | 8080, 8500 | `uc12-discovery.yaml` | Medium |
| [13](./uc13-graphql.md) | GraphQL Gateway | 4000, 443 | `uc13-graphql.yaml` | Medium |
| [14](./uc14-static-fcgi.md) | Static + FastCGI | 80, 443 | `uc14-static-fcgi.yaml` | Low |
| [15](./uc15-geo-lb.md) | Geo Load Balancing | 80, 443 | `uc15-geo-lb.yaml` | High |

## Choosing a Use Case

### By Protocol

| Protocol | Use Cases |
|----------|-----------|
| TCP (L4) | 01, 08 |
| HTTP (L7) | 02, 11, 14 |
| HTTPS | 03, 04, 09 |
| HTTP/2 | 04, 10 |
| HTTP/3 | 05 |
| WebSocket | 06 |
| gRPC | 07 |
| GraphQL | 13 |

### By Feature

| Feature | Use Cases |
|---------|-----------|
| TLS Termination | 03, 04, 05, 06, 07, 09, 10, 11, 13, 14, 15 |
| Rate Limiting | 04, 13 |
| Caching | 11 |
| WAF | 09 |
| mTLS | 09 |
| Service Discovery | 12 |
| Geographic Routing | 15 |

### By Deployment Scale

| Scale | Recommended Use Cases |
|-------|----------------------|
| Single Server | 01, 02, 14 |
| Horizontal Cluster | 02, 03, 04, 06, 07, 10, 13 |
| HA Pair | 08 |
| Edge/PoP | 09, 11, 15 |
| Sidecar | 12 |

## Quick Start

1. Choose your use case from the table above
2. Copy the configuration template:
   ```bash
   cp configs/yaml/uc02-http-lb.yaml /etc/highper-gateway/config.yaml
   ```
3. Customize backend servers and settings
4. Start the gateway:
   ```bash
   systemctl start highper-gateway
   ```

## Configuration Formats

Each use case has configurations in two formats:

- **YAML** (`configs/yaml/`) - Human-readable, recommended for most users
- **HCL/DSL** (`configs/dsl/`) - HashiCorp-style syntax, for Terraform users

## Common Patterns

### Adding Backends

```yaml
backends:
  - name: my-backend
    strategy: round_robin  # or: least_conn, ip_hash, random
    servers:
      - address: "10.0.1.10:8080"
        weight: 1
      - address: "10.0.1.11:8080"
        weight: 1
    health_check:
      enabled: true
      protocol: http
      path: /health
      interval: 10
```

### Enabling TLS

```yaml
tls:
  default:
    cert: /etc/highper-gateway/certs/server.crt
    key: /etc/highper-gateway/certs/server.key
    protocols:
      - TLSv1.2
      - TLSv1.3
```

### Route Matching

```yaml
routes:
  - match:
      path_prefix: /api
    backend: api-servers
  - match:
      path_regex: "\\.(css|js|png)$"
    backend: static-servers
  - match:
      headers:
        - name: X-Custom-Header
          value: special
    backend: special-servers
```

## Support

For detailed configuration options, see the main [Deployment Strategy](../../DEPLOYMENT_STRATEGY.md) document.
