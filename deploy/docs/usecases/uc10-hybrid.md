# Use Case 10: Hybrid Multi-Protocol Gateway

Multi-protocol gateway supporting HTTP, HTTPS, WebSocket, and gRPC on a single deployment.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP, HTTPS, WS, gRPC |
| Ports | 80, 443, 8080, 9090 |
| TLS Required | Partial |
| Privileged | Yes |
| Scaling | Horizontal |

## When to Use

- Unified gateway for multiple protocols
- Microservices with mixed protocols
- Gradual protocol migration
- Simplified infrastructure

## Architecture

```
    HTTP:80     ─────▶ ┌──────────────────────┐ ────▶ Web Backends
    HTTPS:443   ─────▶ │                      │ ────▶ API Backends
    HTTP:8080   ─────▶ │   Highper Gateway    │ ────▶ Internal APIs
    gRPC:9090   ─────▶ │   (Multi-Protocol)   │ ────▶ gRPC Services
    WebSocket   ─────▶ │                      │ ────▶ WS Backends
                       └──────────────────────┘
```

## Quick Start

```bash
cp configs/yaml/uc10-hybrid.yaml /etc/highper-gateway/config.yaml
systemctl start highper-gateway
```

## Configuration

### HTTP to HTTPS Redirect

```yaml
listeners:
  - name: http-main
    protocol: http
    bind: "0.0.0.0:80"
    routes:
      - match:
          path_prefix: /
        redirect:
          to: https
          code: 301
```

### HTTPS with Multiple Routes

```yaml
listeners:
  - name: https-main
    protocol: https
    bind: "0.0.0.0:443"
    tls: default
    websocket:
      enabled: true
    routes:
      - match:
          path_prefix: /api/v1
        backend: api-v1
      - match:
          path_prefix: /api/v2
        backend: api-v2
      - match:
          path_prefix: /ws
        backend: ws-servers
      - match:
          path_prefix: /graphql
        backend: graphql-servers
```

### gRPC Listener

```yaml
listeners:
  - name: grpc-main
    protocol: grpc
    bind: "0.0.0.0:9090"
    routes:
      - match:
          service: "*"
        backend: grpc-servers
```

## Backend Configuration

```yaml
backends:
  - name: api-v1
    strategy: least_conn
    servers:
      - address: "api-v1:8080"

  - name: api-v2
    strategy: round_robin
    servers:
      - address: "api-v2:8080"

  - name: ws-servers
    strategy: ip_hash
    servers:
      - address: "ws:8080"

  - name: grpc-servers
    protocol: grpc
    servers:
      - address: "grpc:9090"
```

## Routing Priority

Routes are evaluated in order. Place specific routes before catch-all:

```yaml
routes:
  - match: { path: /api/v2/special }   # Most specific
  - match: { path_prefix: /api/v2 }    # More specific
  - match: { path_prefix: /api }       # General
  - match: { path_prefix: / }          # Catch-all (last)
```

## Monitoring

Separate metrics per protocol:
- `highper_gateway_http_requests_total`
- `highper_gateway_grpc_requests_total`
- `highper_gateway_websocket_connections_total`

## Related Use Cases

- [UC04: API Gateway](./uc04-api-gateway.md) - HTTP-only with features
- [UC07: gRPC Gateway](./uc07-grpc-gateway.md) - gRPC-only
