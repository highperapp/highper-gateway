# Use Case 02: HTTP Load Balancer

Layer 7 HTTP load balancing with path-based routing, health checks, and header manipulation.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP (Layer 7) |
| Ports | 80, 8080 |
| TLS Required | No |
| Privileged | Optional |
| Scaling | Horizontal |

## When to Use

- Distributing HTTP traffic across multiple backend servers
- Path-based routing (e.g., `/api` to one backend, `/static` to another)
- Adding/removing HTTP headers
- Basic load balancing without TLS

## Architecture

```
                    ┌─────────────────────┐
    HTTP:80   ────▶ │                     │ ────▶ Backend Pool A
                    │   Highper Gateway   │
    HTTP:8080 ────▶ │    (HTTP L7 LB)     │ ────▶ Backend Pool B
                    │                     │
                    └─────────────────────┘
```

## Quick Start

### 1. Deploy Configuration

```bash
# Copy template
cp configs/yaml/uc02-http-lb.yaml /etc/highper-gateway/config.yaml

# Edit backend servers
vi /etc/highper-gateway/config.yaml
```

### 2. Configure Backends

```yaml
backends:
  - name: web-servers
    strategy: round_robin
    servers:
      - address: "10.0.1.10:8080"
      - address: "10.0.1.11:8080"
      - address: "10.0.1.12:8080"
    health_check:
      enabled: true
      protocol: http
      path: /health
      interval: 10
```

### 3. Configure Routes

```yaml
listeners:
  - name: http-main
    protocol: http
    bind: "0.0.0.0:80"
    routes:
      - match:
          path_prefix: /api
        backend: api-servers
      - match:
          path_prefix: /
        backend: web-servers
```

### 4. Start Gateway

```bash
systemctl start highper-gateway
systemctl status highper-gateway
```

## Load Balancing Strategies

| Strategy | Description | Use When |
|----------|-------------|----------|
| `round_robin` | Distribute evenly | Default, equal servers |
| `least_conn` | Fewest active connections | Variable request times |
| `ip_hash` | Sticky by client IP | Session affinity needed |
| `random` | Random selection | Testing |

## Health Checks

```yaml
health_check:
  enabled: true
  protocol: http
  path: /health
  interval: 10      # Check every 10 seconds
  timeout: 5        # 5 second timeout
  threshold: 3      # 3 failures to mark unhealthy
```

## Header Manipulation

```yaml
headers:
  request:
    add:
      - name: X-Forwarded-Proto
        value: http
      - name: X-Request-ID
        value: "$request_id"
    remove:
      - X-Internal-Header
  response:
    add:
      - name: X-Served-By
        value: highper-gateway
```

## Monitoring

### Key Metrics

- `highper_gateway_requests_total` - Total requests
- `highper_gateway_request_duration_seconds` - Latency histogram
- `highper_gateway_backend_up` - Backend health status
- `highper_gateway_active_connections` - Current connections

### Health Check

```bash
curl http://localhost:8081/health
curl http://localhost:9090/metrics
```

## Scaling

### Horizontal Scaling

1. Deploy multiple gateway instances
2. Place behind L4 load balancer (AWS NLB, HAProxy, etc.)
3. Use shared configuration

### Terraform (AWS)

```bash
cd deploy/terraform/aws
terraform apply -var="use_case=02" -var="instance_count=3"
```

### Kubernetes (Helm)

```bash
helm install gateway ./helm/highper-gateway \
  --set useCase="02" \
  --set replicaCount=3
```

## Troubleshooting

### Backend Not Receiving Traffic

1. Check health status: `curl http://localhost:8081/health`
2. Verify backend is reachable from gateway
3. Check health check path exists on backend

### High Latency

1. Check `least_conn` strategy for variable workloads
2. Verify backend capacity
3. Review connection pool settings

### 502 Bad Gateway

1. Backend server is down
2. Health check failing
3. Connection timeout too short

## Related Use Cases

- [UC03: HTTPS/TLS](./uc03-https-tls.md) - Add TLS termination
- [UC04: API Gateway](./uc04-api-gateway.md) - Add rate limiting, auth
- [UC06: WebSocket](./uc06-websocket.md) - Add WebSocket support
