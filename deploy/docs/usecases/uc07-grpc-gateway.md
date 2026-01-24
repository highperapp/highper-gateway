# Use Case 07: gRPC Gateway

High-performance gRPC load balancer with service-based routing and gRPC health checking.

## Overview

| Property | Value |
|----------|-------|
| Protocol | gRPC (HTTP/2) |
| Ports | 9090, 443 |
| TLS Required | Optional |
| Privileged | Optional |
| Scaling | Horizontal |

## When to Use

- Load balancing gRPC microservices
- Service-based routing (route by gRPC service name)
- gRPC health checking
- Protocol translation (gRPC-Web)

## Architecture

```
                         ┌──────────────────────┐
    gRPC:9090  ────────▶ │                      │ ────▶ user.UserService
                         │   Highper Gateway    │
    gRPCs:443  ────────▶ │   (gRPC Gateway)     │ ────▶ product.ProductService
                         │                      │
                         │  • Service Routing   │ ────▶ order.OrderService
                         │  • Health Checks     │
                         │  • Load Balancing    │
                         └──────────────────────┘
```

## Quick Start

### 1. Deploy Configuration

```bash
cp configs/yaml/uc07-grpc-gateway.yaml /etc/highper-gateway/config.yaml
```

### 2. Configure Service Routing

```yaml
listeners:
  - name: grpc-main
    protocol: grpc
    bind: "0.0.0.0:9090"
    routes:
      - match:
          service: "user.UserService"
        backend: user-grpc
      - match:
          service: "product.ProductService"
        backend: product-grpc
      - match:
          service: "*"
        backend: default-grpc
```

### 3. Configure Backends

```yaml
backends:
  - name: user-grpc
    strategy: round_robin
    protocol: grpc
    servers:
      - address: "user-service:9090"
      - address: "user-service-2:9090"
    health_check:
      enabled: true
      protocol: grpc
      service: "grpc.health.v1.Health"
      interval: 10
```

### 4. Start Gateway

```bash
systemctl start highper-gateway
```

## gRPC Health Checking

Highper Gateway uses the standard gRPC health checking protocol.

### Backend Requirements

Your gRPC services must implement `grpc.health.v1.Health`:

```protobuf
service Health {
  rpc Check(HealthCheckRequest) returns (HealthCheckResponse);
  rpc Watch(HealthCheckRequest) returns (stream HealthCheckResponse);
}
```

### Configuration

```yaml
health_check:
  enabled: true
  protocol: grpc
  service: "grpc.health.v1.Health"
  interval: 10
  timeout: 5
```

## Load Balancing Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| `round_robin` | Even distribution | Stateless services |
| `least_conn` | Fewest active streams | Long-lived streams |
| `random` | Random selection | Testing |

## gRPC Settings

### Message Size

```yaml
grpc:
  max_message_size: 16777216  # 16MB
```

### Keepalive

```yaml
grpc:
  keepalive:
    time: 30000             # Send ping after 30s idle
    timeout: 5000           # Wait 5s for ping response
    permit_without_stream: true
```

### Compression

```yaml
grpc:
  compression:
    enabled: true
    algorithms:
      - gzip
      - snappy
```

## TLS Configuration

### Server TLS (gRPCs)

```yaml
tls:
  default:
    cert: /etc/highper-gateway/certs/server.crt
    key: /etc/highper-gateway/certs/server.key

listeners:
  - name: grpc-tls
    protocol: grpc
    bind: "0.0.0.0:443"
    tls: default
```

### Backend TLS

```yaml
backends:
  - name: secure-grpc
    protocol: grpc
    tls:
      enabled: true
      ca: /etc/highper-gateway/certs/ca.crt
    servers:
      - address: "secure-service:9090"
```

## Testing

### Using grpcurl

```bash
# List services
grpcurl -plaintext localhost:9090 list

# Call method
grpcurl -plaintext -d '{"id": "123"}' \
  localhost:9090 user.UserService/GetUser

# Health check
grpcurl -plaintext localhost:9090 grpc.health.v1.Health/Check
```

### Using grpc-health-probe

```bash
grpc-health-probe -addr=localhost:9090
```

## Kubernetes Deployment

```bash
helm install grpc-gateway ./helm/highper-gateway \
  --set useCase="07" \
  --set replicaCount=3 \
  --set service.type=LoadBalancer
```

### Service Configuration

```yaml
# values.yaml
useCase: "07"
service:
  type: LoadBalancer
  annotations:
    # For AWS NLB with HTTP/2
    service.beta.kubernetes.io/aws-load-balancer-type: "nlb"
```

## Monitoring

### Key Metrics

- `highper_gateway_grpc_requests_total` - Total gRPC calls
- `highper_gateway_grpc_request_duration_seconds` - Call latency
- `highper_gateway_grpc_stream_messages_total` - Stream messages
- `highper_gateway_backend_up{protocol="grpc"}` - Backend health

### Example Queries

```promql
# gRPC error rate
sum(rate(highper_gateway_grpc_requests_total{status!="OK"}[5m])) /
sum(rate(highper_gateway_grpc_requests_total[5m]))

# P99 latency by service
histogram_quantile(0.99,
  sum(rate(highper_gateway_grpc_request_duration_seconds_bucket[5m])) by (service, le)
)
```

## Troubleshooting

### Connection Refused

- Verify backend is running: `grpcurl -plaintext backend:9090 list`
- Check firewall rules for port 9090
- Verify DNS resolution

### Unavailable (Code 14)

- Backend health check failing
- All backends unhealthy
- Check: `curl http://localhost:8081/health`

### Deadline Exceeded (Code 4)

- Request timeout too short
- Backend overloaded
- Increase timeout or add more backends

### Unimplemented (Code 12)

- Service/method not found
- Check service routing configuration
- Verify service name matches exactly

## Related Use Cases

- [UC04: API Gateway](./uc04-api-gateway.md) - Add REST API support
- [UC10: Hybrid](./uc10-hybrid.md) - Mix gRPC with HTTP
- [UC12: Discovery](./uc12-discovery.md) - Dynamic service discovery
