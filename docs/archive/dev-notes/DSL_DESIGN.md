# Caddy-like Configuration DSL Design

**Goal**: Reduce configuration complexity by 10x (from ~70 lines YAML to ~7-10 lines DSL)

**Inspiration**: Caddy's elegant, intuitive configuration syntax

---

## Design Principles

1. **Sensible Defaults**: Everything optional, with production-ready defaults
2. **Natural Language**: Read like English sentences
3. **Minimal Syntax**: No unnecessary punctuation or nesting
4. **Convention over Configuration**: Common patterns should be trivial
5. **Progressive Disclosure**: Simple things simple, complex things possible

---

## Core Syntax

### Basic HTTP Reverse Proxy

**YAML** (13 lines):
```yaml
server:
  bind: ["0.0.0.0:80"]

upstreams:
  - name: "backend"
    servers:
      - url: "http://localhost:3000"

routes:
  - name: "main"
    match:
      paths: ["/"]
    upstream: "backend"
```

**DSL** (1 line):
```
http://example.com proxy localhost:3000
```

### HTTP with TLS

**YAML** (20+ lines):
```yaml
server:
  bind: ["0.0.0.0:80"]
  tls_bind: ["0.0.0.0:443"]

tls:
  auto: true
  acme:
    email: "admin@example.com"
    directory: "https://acme-v02.api.letsencrypt.org/directory"

upstreams:
  - name: "backend"
    servers:
      - url: "http://localhost:3000"

routes:
  - name: "main"
    match:
      paths: ["/"]
    upstream: "backend"
```

**DSL** (1 line):
```
https://example.com proxy localhost:3000
```

**Auto-magic**:
- HTTP automatically redirects to HTTPS
- TLS certificate auto-provisioned via ACME/Let's Encrypt
- Production-ready defaults

### Load Balancing

**YAML** (15 lines):
```yaml
upstreams:
  - name: "backend"
    servers:
      - url: "http://server1:3000"
      - url: "http://server2:3000"
      - url: "http://server3:3000"
    load_balancing:
      algorithm: "round_robin"
    health_check:
      enabled: true
      interval: 10s

routes:
  - name: "main"
    upstream: "backend"
```

**DSL** (1 line):
```
example.com proxy server1:3000 server2:3000 server3:3000
```

**Defaults**: round-robin, health checks enabled

### Custom Load Balancing

```
example.com {
    proxy server1:3000 server2:3000 server3:3000
    lb least_conn
}
```

### Path-Based Routing

**YAML** (25+ lines):
```yaml
routes:
  - name: "api"
    match:
      paths: ["/api/*"]
    upstream: "api_backend"
  - name: "static"
    match:
      paths: ["/static/*"]
    upstream: "static_backend"
```

**DSL** (2 lines):
```
example.com /api/* proxy api.local:8080
example.com /static/* proxy cdn.local:9000
```

### WebSocket Support

**DSL**:
```
example.com /ws/* {
    websocket
    proxy ws.local:8080
}
```

### gRPC Support

**DSL**:
```
grpc.example.com {
    grpc
    proxy grpc-backend:50051
}
```

### TCP Proxy (MySQL)

**YAML** (20+ lines):
```yaml
tcp:
  bind: "0.0.0.0:3306"
  protocol: mysql
  load_balancing: round_robin
  enable_pooling: true
  upstreams:
    - name: "mysql"
      backends:
        - addr: "10.0.1.10:3306"
        - addr: "10.0.1.11:3306"
```

**DSL** (1 line):
```
:3306 mysql proxy 10.0.1.10:3306 10.0.1.11:3306
```

### TCP Proxy (PostgreSQL) with Advanced Features

**DSL** (4 lines):
```
:5432 postgres {
    proxy 10.0.2.10:5432 10.0.2.11:5432
    pool max=500 min=20
}
```

### Multiple Sites

**DSL** (5 lines):
```
api.example.com proxy localhost:8080
web.example.com proxy localhost:3000
admin.example.com proxy localhost:9000
ws.example.com /ws/* websocket proxy localhost:8081
metrics.example.com /metrics prometheus
```

---

## Grammar Specification (pest)

### Top-Level Structure

```pest
config = { SOI ~ (site | import | global_directive)* ~ EOI }

site = {
    address ~ block?
  | address ~ simple_proxy
}

address = {
    scheme? ~ domain ~ port? ~ path?
  | ":" ~ port  // TCP-only (no domain)
}

scheme = { "http://" | "https://" | "grpc://" }
domain = @{ (ASCII_ALPHANUMERIC | "." | "-")+ }
port = { ":" ~ ASCII_DIGIT+ }
path = { "/" ~ path_segment* }

block = { "{" ~ directive* ~ "}" }
simple_proxy = { "proxy" ~ backend+ }
```

### Directives

```pest
directive = {
    proxy_directive
  | tls_directive
  | websocket_directive
  | grpc_directive
  | tcp_directive
  | lb_directive
  | pool_directive
  | health_directive
  | timeout_directive
  | cors_directive
  | compress_directive
  | rate_limit_directive
}

proxy_directive = { "proxy" ~ backend+ }
backend = { address | upstream_name }

tcp_directive = {
    ("mysql" | "postgres" | "redis" | "tcp") ~ proxy_directive
}

lb_directive = {
    "lb" ~ ("round_robin" | "least_conn" | "ip_hash" | "random" | "weighted" | "consistent_hash")
}

pool_directive = {
    "pool" ~ pool_option*
}
pool_option = {
    "max=" ~ number
  | "min=" ~ number
  | "lifetime=" ~ duration
}

health_directive = {
    "health" ~ health_option*
}
health_option = {
    "interval=" ~ duration
  | "timeout=" ~ duration
  | "path=" ~ string
}

tls_directive = {
    "tls" ~ email?
  | "tls" ~ "internal"  // Self-signed for dev
  | "tls" ~ cert_file ~ key_file
}

cors_directive = {
    "cors" ~ cors_option*
}

rate_limit_directive = {
    "rate_limit" ~ rate ~ ("per" ~ duration)?
}
```

### Values

```pest
number = @{ ASCII_DIGIT+ }
duration = @{ number ~ ("ms" | "s" | "m" | "h") }
string = @{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
```

---

## Complete Examples

### 1. Simple HTTP Proxy

**DSL**:
```
localhost:8080 proxy backend:3000
```

**Equivalent YAML**: 13 lines → **1 line** (13x reduction)

### 2. Production HTTPS Site with Load Balancing

**DSL**:
```
https://api.example.com {
    proxy server1:8080 server2:8080 server3:8080
    lb least_conn
    health interval=10s
    cors
    rate_limit 100 per 1s
    compress gzip br
}
```

**Equivalent YAML**: ~50 lines → **8 lines** (6.25x reduction)

### 3. Microservices Architecture

**DSL**:
```
# API Gateway
api.example.com {
    /users/*    proxy users-svc:8080
    /orders/*   proxy orders-svc:8080
    /products/* proxy products-svc:8080
    /auth/*     proxy auth-svc:8080

    cors
    rate_limit 1000 per 1m
}

# Admin Interface
admin.example.com {
    proxy admin-ui:3000
    tls internal
}

# WebSocket Server
ws.example.com {
    websocket
    proxy ws-server:8081
}

# gRPC Services
grpc://services.example.com {
    grpc
    proxy grpc-backend:50051
}

# Database Load Balancer
:3306 mysql {
    proxy db1:3306 db2:3306 db3:3306
    pool max=1000 min=50
    lb least_conn
}

:5432 postgres {
    proxy pg1:5432 pg2:5432
    pool max=500 min=20
}

:6379 redis {
    proxy redis1:6379 redis2:6379 redis3:6379
    lb consistent_hash
}
```

**Equivalent YAML**: ~200 lines → **35 lines** (5.7x reduction)

### 4. Development Environment

**DSL**:
```
localhost:3000 proxy backend:8080
localhost:3001 proxy frontend:5173
localhost:3002 proxy docs:4000

# Auto-TLS for local dev
local.dev {
    tls internal
    proxy localhost:3000
}
```

---

## Implementation Plan

### Phase 1: Parser (Week 7)

1. **Define pest grammar** (`src/config/dsl.pest`)
2. **Implement parser** (`src/config/dsl_parser.rs`)
3. **AST structures** (`src/config/dsl_ast.rs`)
4. **DSL → YAML converter** (`src/config/dsl_to_yaml.rs`)
5. **Tests** (20+ test cases)

### Phase 2: Integration (Week 8)

1. **CLI support** (`--config-format dsl|yaml`)
2. **Auto-detection** (file extension: `.proxy` or `.caddy`)
3. **Migration tool** (`yaml-to-dsl` converter)
4. **Validation** (comprehensive error messages)
5. **Documentation** (examples, migration guide)

---

## DSL Features Summary

### Core Features
- [x] Simple proxy (`example.com proxy backend:3000`)
- [x] TLS auto-provisioning (`https://...`)
- [x] Load balancing (`lb least_conn`)
- [x] Path-based routing (`/api/* proxy ...`)
- [x] Multiple backends (`proxy srv1 srv2 srv3`)

### Protocol Support
- [x] HTTP/1.1 and HTTP/2 (default)
- [x] HTTPS with auto-TLS
- [x] WebSocket (`websocket` directive)
- [x] gRPC (`grpc://...` or `grpc` directive)
- [x] TCP protocols: MySQL, PostgreSQL, Redis

### Advanced Features
- [x] Connection pooling (`pool max=500`)
- [x] Health checks (`health interval=10s`)
- [x] CORS (`cors`)
- [x] Rate limiting (`rate_limit 100 per 1s`)
- [x] Compression (`compress gzip br`)
- [x] Timeouts (`timeout 30s`)

### Developer Experience
- [x] Sensible defaults (minimal config)
- [x] Natural syntax (reads like English)
- [x] Progressive disclosure (simple → complex)
- [x] Excellent error messages
- [x] Migration tools (YAML ↔ DSL)

---

## File Extension

Suggested: `.proxy` or `.caddy`

Example: `config.proxy`

---

## Comparison: YAML vs DSL

### YAML Config (68 lines)
```yaml
server:
  bind:
    - "0.0.0.0:8080"
  tls_bind:
    - "0.0.0.0:8443"
  workers: "2"
  protocols:
    - http1
    - http2

tls:
  auto: false
  certificates:
    - domains: ["localhost"]
      cert_file: "certs/cert.pem"
      key_file: "certs/key.pem"
  passthrough:
    enabled: true
    bind: ["0.0.0.0:9443"]
    routes:
      - server_name: "test.example.com"
        upstream: "127.0.0.1:10443"
    timeout: 60s

websocket:
  enabled: true
  max_message_size: 16777216
  ping_interval: 30
  timeout: 300

grpc:
  enabled: true
  max_message_size: 4194304
  timeout_seconds: 30

upstreams:
  - name: "test_backend"
    servers:
      - url: "http://httpbin.org"
    load_balancing:
      algorithm: "round_robin"

routes:
  - name: "test_route"
    match:
      paths: ["/"]
    upstream: "test_backend"

observability:
  logging:
    level: "info"
    format: "pretty"
  metrics:
    enabled: false
```

### DSL Config (7 lines)

```
# Main site
localhost:8080 proxy httpbin.org

# TLS passthrough
:9443 tls-passthrough test.example.com -> 127.0.0.1:10443

# Logging
log info
```

**Reduction**: 68 lines → **7 lines** = **9.7x simpler** ✅

---

## Benefits

### 1. **Developer Productivity**
- 10x less config to write and maintain
- Faster onboarding (easier to learn)
- Less context switching (fewer lines to scan)

### 2. **Reduced Errors**
- Fewer lines = fewer typos
- Sensible defaults = fewer misconfigurations
- Better validation = clearer error messages

### 3. **Better Readability**
- Natural language syntax
- Self-documenting
- Easier code reviews

### 4. **Backward Compatible**
- YAML still supported
- Migration tools provided
- Can mix formats (import)

---

## Next Steps

1. ✅ Design complete (this document)
2. ⏭ Implement pest grammar
3. ⏭ Build parser and AST
4. ⏭ Create DSL → Config converter
5. ⏭ Add tests and validation
6. ⏭ Write documentation and examples
