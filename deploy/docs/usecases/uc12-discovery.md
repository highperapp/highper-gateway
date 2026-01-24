# Use Case 12: Service Discovery Integration

Dynamic service discovery with Consul, Kubernetes, or other providers.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP |
| Ports | 8080, 8500 |
| TLS Required | No |
| Privileged | No |
| Scaling | Sidecar |

## When to Use

- Dynamic microservices environments
- Kubernetes native deployments
- Consul-based service mesh
- Auto-scaling backends

## Architecture

```
                         ┌──────────────────────┐
    HTTP:8080  ────────▶ │   Highper Gateway    │
                         │                      │
                         │   Service Discovery  │
                         │   ┌────────────────┐ │
                         │   │ Consul / K8s   │ │
                         │   └───────┬────────┘ │
                         └───────────┼──────────┘
                                     │
                    ┌────────────────┼────────────────┐
                    ▼                ▼                ▼
              Service A        Service B        Service C
              (dynamic)        (dynamic)        (dynamic)
```

## Consul Integration

### Configuration

```yaml
discovery:
  provider: consul
  consul:
    address: "consul.service.local:8500"
    datacenter: dc1
    token_env: CONSUL_TOKEN
    refresh_interval: 10
    health_check_filter: passing

backends:
  - name: user-service
    discovery:
      service: user-service
      tags:
        - production
```

### Consul Service Registration

```json
{
  "service": {
    "name": "user-service",
    "tags": ["production"],
    "port": 8080,
    "check": {
      "http": "http://localhost:8080/health",
      "interval": "10s"
    }
  }
}
```

## Kubernetes Integration

### Configuration

```yaml
discovery:
  provider: kubernetes
  kubernetes:
    namespace: default
    label_selector: app=myapp
    port_name: http
```

### Kubernetes Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: user-service
  labels:
    app: myapp
spec:
  ports:
    - name: http
      port: 8080
  selector:
    app: user-service
```

## Dynamic Backend Updates

Backends automatically update when services change:

```yaml
backends:
  - name: api-service
    discovery:
      service: api-service
    strategy: round_robin
    health_check:
      enabled: true
      protocol: http
      path: /health
```

## Circuit Breaker

Protect against failing services:

```yaml
backends:
  - name: payment-service
    discovery:
      service: payment-service
    circuit_breaker:
      enabled: true
      threshold: 5
      timeout: 30000
```

## Retry Configuration

```yaml
backends:
  - name: order-service
    retry:
      attempts: 3
      per_try_timeout: 5000
      conditions:
        - connection_error
        - 502
        - 503
```

## Monitoring

- `highper_gateway_discovery_updates_total`
- `highper_gateway_backend_instances`
- `highper_gateway_circuit_breaker_state`

## Related Use Cases

- [UC04: API Gateway](./uc04-api-gateway.md) - Static backends
- [UC10: Hybrid](./uc10-hybrid.md) - Multi-protocol
