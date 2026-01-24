# Use Case 04: API Gateway

Full-featured API gateway with rate limiting, JWT authentication, CORS, and request transformation.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTPS (Layer 7) |
| Ports | 443, 8080 |
| TLS Required | Yes |
| Privileged | Optional |
| Scaling | Horizontal |

## When to Use

- Exposing microservices as a unified API
- Implementing rate limiting per client/endpoint
- JWT-based authentication
- Request/response transformation
- CORS handling

## Architecture

```
                         ┌──────────────────────┐
    HTTPS:443  ────────▶ │                      │ ────▶ User Service
                         │   Highper Gateway    │
    (Rate Limited)       │    (API Gateway)     │ ────▶ Product Service
                         │                      │
    - JWT Auth           │  • Rate Limiting     │ ────▶ Order Service
    - CORS               │  • Authentication    │
    - Transform          │  • Transformation    │
                         └──────────────────────┘
```

## Quick Start

### 1. Generate TLS Certificate

```bash
# Self-signed for testing
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/highper-gateway/certs/server.key \
  -out /etc/highper-gateway/certs/server.crt
```

### 2. Deploy Configuration

```bash
cp configs/yaml/uc04-api-gateway.yaml /etc/highper-gateway/config.yaml
```

### 3. Configure JWT Secret

```bash
# Set environment variable
echo 'JWT_SECRET=your-secret-key-here' >> /etc/default/highper-gateway
```

### 4. Start Gateway

```bash
systemctl start highper-gateway
```

## Rate Limiting

### Global Rate Limit

```yaml
rate_limiting:
  enabled: true
  default:
    requests_per_second: 1000
    burst: 200
  by_client_ip: true
```

### Per-Route Rate Limit

```yaml
routes:
  - match:
      path_prefix: /v1/users
    backend: user-service
    rate_limit:
      requests_per_second: 100
      burst: 20
```

### Rate Limit Response

When rate limited, clients receive:
- HTTP 429 Too Many Requests
- `Retry-After` header with seconds to wait
- `X-RateLimit-Remaining` header

## Authentication

### JWT Configuration

```yaml
auth:
  jwt:
    enabled: true
    secret_env: JWT_SECRET
    algorithms:
      - HS256
      - RS256
    header: Authorization
    prefix: Bearer
```

### Protected Routes

```yaml
routes:
  - match:
      path_prefix: /v1/orders
    backend: order-service
    auth:
      required: true
```

### Extracting Claims

JWT claims are available as variables:
- `$jwt_sub` - Subject
- `$jwt_exp` - Expiration
- `$jwt_aud` - Audience

## CORS Configuration

```yaml
cors:
  enabled: true
  allow_origins:
    - "https://example.com"
    - "https://*.example.com"
  allow_methods:
    - GET
    - POST
    - PUT
    - DELETE
    - OPTIONS
  allow_headers:
    - Authorization
    - Content-Type
  expose_headers:
    - X-Request-ID
  max_age: 3600
  allow_credentials: true
```

## Request Transformation

### Add Headers

```yaml
transform:
  request:
    headers:
      add:
        - name: X-Gateway-Version
          value: "1.0"
        - name: X-Request-ID
          value: "$request_id"
        - name: X-User-ID
          value: "$jwt_sub"
```

### Remove Headers

```yaml
transform:
  request:
    headers:
      remove:
        - X-Internal-Header
  response:
    headers:
      remove:
        - Server
        - X-Powered-By
```

## Retry Configuration

```yaml
backends:
  - name: user-service
    retry:
      attempts: 3
      per_try_timeout: 5000
      conditions:
        - connection_error
        - 502
        - 503
        - 504
```

## Monitoring

### Key Metrics

- `highper_gateway_rate_limit_exceeded_total` - Rate limit hits
- `highper_gateway_auth_failures_total` - Auth failures
- `highper_gateway_request_duration_seconds` - Latency by route

### Example Prometheus Queries

```promql
# Rate limit hit rate
rate(highper_gateway_rate_limit_exceeded_total[5m])

# Auth failure rate
rate(highper_gateway_auth_failures_total[5m])

# P99 latency by route
histogram_quantile(0.99, rate(highper_gateway_request_duration_seconds_bucket[5m]))
```

## Kubernetes Deployment

```bash
helm install api-gateway ./helm/highper-gateway \
  --set useCase="04" \
  --set replicaCount=3 \
  --set tls.enabled=true \
  --set-file tls.cert=./certs/server.crt \
  --set-file tls.key=./certs/server.key
```

## Security Best Practices

1. **Always use HTTPS** in production
2. **Rotate JWT secrets** regularly
3. **Set appropriate rate limits** per endpoint
4. **Use short JWT expiration** times
5. **Enable access logging** for audit

## Troubleshooting

### 401 Unauthorized

- Check JWT secret matches between issuer and gateway
- Verify token not expired
- Check `Authorization` header format: `Bearer <token>`

### 429 Too Many Requests

- Client exceeded rate limit
- Check `Retry-After` header
- Consider increasing limits or burst

### CORS Errors

- Verify origin is in allow list
- Check preflight (OPTIONS) requests work
- Ensure credentials mode matches configuration

## Related Use Cases

- [UC03: HTTPS/TLS](./uc03-https-tls.md) - TLS-only gateway
- [UC09: WAF + mTLS](./uc09-waf-mtls.md) - Add WAF protection
- [UC13: GraphQL](./uc13-graphql.md) - GraphQL-specific gateway
