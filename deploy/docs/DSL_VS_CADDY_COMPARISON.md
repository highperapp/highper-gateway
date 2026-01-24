# Configuration Format Comparison: Highper Gateway vs Caddy

This document compares Highper Gateway's configuration formats (YAML and HCL/DSL) with Caddy's Caddyfile format.

## Overview

| Feature | Highper Gateway YAML | Highper Gateway HCL | Caddy (Caddyfile) |
|---------|---------------------|---------------------|-------------------|
| Format Type | Data serialization | Declarative config | Custom DSL |
| Learning Curve | Low | Medium | Low-Medium |
| Validation | Schema-based | Schema-based | Built-in |
| IDE Support | Excellent | Good | Limited |
| Terraform Familiarity | No | Yes | No |
| Comments | # only | # and // | # |
| Multi-line Strings | Yes | Yes (heredoc) | Yes |

---

## Side-by-Side Examples

### Example 1: Basic HTTP Reverse Proxy

#### Caddy (Caddyfile)
```caddyfile
example.com {
    reverse_proxy localhost:8080
}
```

#### Highper Gateway (YAML)
```yaml
listeners:
  - name: https-main
    protocol: https
    bind: "0.0.0.0:443"
    routes:
      - match:
          host: example.com
          path_prefix: /
        backend: web-servers

backends:
  - name: web-servers
    servers:
      - address: "localhost:8080"
```

#### Highper Gateway (HCL/DSL)
```hcl
listener "https-main" {
  protocol = "https"
  bind     = "0.0.0.0:443"

  route {
    match {
      host        = "example.com"
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

backend "web-servers" {
  server {
    address = "localhost:8080"
  }
}
```

---

### Example 2: Load Balancing with Health Checks

#### Caddy (Caddyfile)
```caddyfile
example.com {
    reverse_proxy backend-1:8080 backend-2:8080 {
        lb_policy round_robin
        health_uri /health
        health_interval 10s
    }
}
```

#### Highper Gateway (YAML)
```yaml
listeners:
  - name: https
    protocol: https
    bind: "0.0.0.0:443"
    routes:
      - match:
          path_prefix: /
        backend: web-servers

backends:
  - name: web-servers
    strategy: round_robin
    servers:
      - address: "backend-1:8080"
      - address: "backend-2:8080"
    health_check:
      enabled: true
      protocol: http
      path: /health
      interval: 10
```

#### Highper Gateway (HCL/DSL)
```hcl
listener "https" {
  protocol = "https"
  bind     = "0.0.0.0:443"

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

backend "web-servers" {
  strategy = "round_robin"

  server {
    address = "backend-1:8080"
  }

  server {
    address = "backend-2:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}
```

---

### Example 3: API Gateway with Rate Limiting

#### Caddy (Caddyfile)
```caddyfile
api.example.com {
    route /v1/* {
        rate_limit {
            zone api_zone {
                key    {remote_host}
                events 100
                window 1s
            }
        }
        reverse_proxy api-server:8080
    }
}
```

#### Highper Gateway (YAML)
```yaml
listeners:
  - name: api-gateway
    protocol: https
    bind: "0.0.0.0:443"
    routes:
      - match:
          host: api.example.com
          path_prefix: /v1
        backend: api-servers
        rate_limit:
          requests_per_second: 100
          burst: 20
          by_client_ip: true

backends:
  - name: api-servers
    servers:
      - address: "api-server:8080"
```

#### Highper Gateway (HCL/DSL)
```hcl
listener "api-gateway" {
  protocol = "https"
  bind     = "0.0.0.0:443"

  route {
    match {
      host        = "api.example.com"
      path_prefix = "/v1"
    }
    backend = "api-servers"

    rate_limit {
      requests_per_second = 100
      burst               = 20
      by_client_ip        = true
    }
  }
}

backend "api-servers" {
  server {
    address = "api-server:8080"
  }
}
```

---

### Example 4: TLS Configuration

#### Caddy (Caddyfile)
```caddyfile
example.com {
    tls /path/to/cert.pem /path/to/key.pem {
        protocols tls1.2 tls1.3
        ciphers TLS_AES_256_GCM_SHA384 TLS_CHACHA20_POLY1305_SHA256
    }
    reverse_proxy localhost:8080
}
```

#### Highper Gateway (YAML)
```yaml
tls:
  default:
    cert: /path/to/cert.pem
    key: /path/to/key.pem
    protocols:
      - TLSv1.2
      - TLSv1.3
    ciphers:
      - TLS_AES_256_GCM_SHA384
      - TLS_CHACHA20_POLY1305_SHA256

listeners:
  - name: https
    protocol: https
    bind: "0.0.0.0:443"
    tls: default
    routes:
      - match:
          path_prefix: /
        backend: web-servers
```

#### Highper Gateway (HCL/DSL)
```hcl
tls "default" {
  cert = "/path/to/cert.pem"
  key  = "/path/to/key.pem"

  protocols = ["TLSv1.2", "TLSv1.3"]
  ciphers   = [
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256"
  ]
}

listener "https" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}
```

---

### Example 5: WebSocket Support

#### Caddy (Caddyfile)
```caddyfile
example.com {
    @websocket {
        header Upgrade websocket
    }
    reverse_proxy @websocket ws-server:8080
    reverse_proxy web-server:8080
}
```

#### Highper Gateway (YAML)
```yaml
listeners:
  - name: https
    protocol: https
    bind: "0.0.0.0:443"
    websocket:
      enabled: true
    routes:
      - match:
          path_prefix: /ws
          headers:
            - name: Upgrade
              value: websocket
        backend: ws-servers
      - match:
          path_prefix: /
        backend: web-servers
```

#### Highper Gateway (HCL/DSL)
```hcl
listener "https" {
  protocol = "https"
  bind     = "0.0.0.0:443"

  websocket {
    enabled = true
  }

  route {
    match {
      path_prefix = "/ws"
      header {
        name  = "Upgrade"
        value = "websocket"
      }
    }
    backend = "ws-servers"
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}
```

---

## Feature Comparison Table

| Feature | Highper YAML | Highper HCL | Caddy |
|---------|-------------|-------------|-------|
| **Simplicity** | | | |
| Minimal config for basic proxy | 10 lines | 12 lines | 3 lines |
| Automatic HTTPS | Manual | Manual | Automatic |
| Default values | Extensive | Extensive | Automatic |
| **Advanced Features** | | | |
| Load balancing strategies | 5+ | 5+ | 4 |
| Health checks | Full control | Full control | Basic |
| Circuit breaker | Yes | Yes | Via plugin |
| Rate limiting | Native | Native | Via plugin |
| WebSocket | Native | Native | Native |
| gRPC | Native | Native | Native |
| HTTP/3 QUIC | Native | Native | Native |
| **Configuration** | | | |
| Reusable TLS profiles | Yes | Yes | Limited |
| Named backends | Yes | Yes | No |
| Weighted backends | Yes | Yes | Yes |
| Conditional routing | Full | Full | Matchers |
| **Operations** | | | |
| Hot reload | Yes | Yes | Yes |
| Validation | Schema | Schema | Built-in |
| Metrics exposure | Prometheus | Prometheus | Prometheus |

---

## When to Use Each Format

### Use Highper Gateway YAML when:
- Team is familiar with Kubernetes/Docker configs
- Want maximum IDE support and validation
- Prefer data-centric configuration
- Using CI/CD pipelines that process YAML

### Use Highper Gateway HCL/DSL when:
- Team uses Terraform for infrastructure
- Want consistent syntax across IaC tools
- Prefer more expressive configuration
- Building complex routing logic

### Use Caddy when:
- Need simplest possible configuration
- Automatic HTTPS is critical
- Single-server deployment
- Minimal learning curve required

---

## Migration Guide: Caddy to Highper Gateway

### Step 1: Convert Site Blocks to Listeners

**Caddy:**
```caddyfile
example.com {
    ...
}
```

**Highper (HCL):**
```hcl
listener "example-com" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  ...
}
```

### Step 2: Convert reverse_proxy to Backend + Route

**Caddy:**
```caddyfile
reverse_proxy backend:8080
```

**Highper (HCL):**
```hcl
route {
  match {
    path_prefix = "/"
  }
  backend = "my-backend"
}

backend "my-backend" {
  server {
    address = "backend:8080"
  }
}
```

### Step 3: Convert Matchers to Match Blocks

**Caddy:**
```caddyfile
@api path /api/*
reverse_proxy @api api:8080
```

**Highper (HCL):**
```hcl
route {
  match {
    path_prefix = "/api"
  }
  backend = "api-backend"
}
```

---

## Conclusion

| Aspect | Winner |
|--------|--------|
| Simplicity | Caddy |
| Enterprise Features | Highper Gateway |
| Performance Tuning | Highper Gateway |
| Team Familiarity (K8s) | Highper YAML |
| Team Familiarity (Terraform) | Highper HCL |
| Automatic HTTPS | Caddy |
| Advanced Load Balancing | Highper Gateway |
| io_uring Performance | Highper Gateway |

Both tools have their strengths. Caddy excels at simplicity and automatic HTTPS, while Highper Gateway provides more control, better performance with io_uring, and richer enterprise features like circuit breakers, advanced health checks, and geographic load balancing.
