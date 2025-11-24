# Highper Gateway Configuration Guide

Complete guide for configuring the highper-gateway with all features: API Gateway, Static Files, PHP-FPM, WebSocket, gRPC, and more.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Basic Reverse Proxy](#basic-reverse-proxy)
3. [API Gateway with Hostname Routing](#api-gateway-with-hostname-routing)
4. [Static File Serving](#static-file-serving)
5. [PHP-FPM for WordPress/Laravel](#php-fpm-for-wordpresslaravel)
6. [WebSocket Proxying](#websocket-proxying)
7. [gRPC Proxying](#grpc-proxying)
8. [TLS/HTTPS Configuration](#tlshttps-configuration)
9. [Load Balancing](#load-balancing)
10. [Caching and Compression](#caching-and-compression)
11. [Complete Use Case Examples](#complete-use-case-examples)

---

## Quick Start

### Minimal Configuration (HTTP Proxy)

```yaml
# config.yaml
server:
  bind:
    - "0.0.0.0:8080"
  protocols:
    - Http1
    - Http2

routes:
  - name: "default"
    match_rules:
      paths: ["/*"]
    upstream: "backend"

upstreams:
  - name: "backend"
    servers:
      - "http://localhost:3000"
    load_balancing:
      algorithm: "round_robin"
```

**Run:**
```bash
cargo run -- --config config.yaml
```

---

## Basic Reverse Proxy

### Simple Load Balancing

```yaml
server:
  bind: ["0.0.0.0:8080"]
  protocols: [Http1, Http2]

routes:
  - name: "api"
    match_rules:
      paths: ["/api/*"]
    upstream: "api-servers"

  - name: "web"
    match_rules:
      paths: ["/*"]
    upstream: "web-servers"

upstreams:
  - name: "api-servers"
    servers:
      - "http://api1.internal:8080"
      - "http://api2.internal:8080"
      - "http://api3.internal:8080"
    load_balancing:
      algorithm: "round_robin"  # or: least_connections, ip_hash, weighted

  - name: "web-servers"
    servers:
      - "http://web1.internal:3000"
      - "http://web2.internal:3000"
    load_balancing:
      algorithm: "least_connections"
```

---

## API Gateway with Hostname Routing

### Per-Hostname Routes (JSON Configuration)

Create `hostname_routes.json`:

```json
{
  "version": "1.0",
  "hosts": [
    {
      "hostname": "api.example.com",
      "routes": [
        {
          "name": "users-api",
          "match_type": "prefix",
          "prefix": "/api/users/",
          "upstream": "users-service",
          "methods": ["GET", "POST", "PUT", "DELETE"],
          "timeout_ms": 5000,
          "middleware": ["auth", "rate-limit"],
          "metadata": {
            "team": "platform",
            "version": "v1"
          }
        },
        {
          "name": "orders-api",
          "match_type": "pattern",
          "pattern": "^/api/orders/\\d+$",
          "upstream": "orders-service",
          "methods": ["GET", "POST"],
          "timeout_ms": 3000,
          "middleware": []
        }
      ]
    },
    {
      "hostname": "admin.example.com",
      "routes": [
        {
          "name": "admin-panel",
          "match_type": "exact",
          "path": "/admin/dashboard",
          "upstream": "admin-service",
          "methods": ["GET"],
          "middleware": ["admin-auth"]
        }
      ]
    },
    {
      "hostname": "*.tenants.example.com",
      "routes": [
        {
          "name": "tenant-app",
          "match_type": "prefix",
          "prefix": "/",
          "upstream": "tenant-service",
          "methods": []
        }
      ]
    }
  ],
  "upstreams": {
    "users-service": {
      "servers": ["http://users-svc:8081", "http://users-svc:8082"],
      "algorithm": "round_robin",
      "health_check": {
        "interval_secs": 10,
        "timeout_secs": 5,
        "path": "/health",
        "expected_status": 200
      }
    },
    "orders-service": {
      "servers": ["http://orders-svc:8083"],
      "algorithm": "round_robin"
    },
    "admin-service": {
      "servers": ["http://admin-svc:8084"],
      "algorithm": "round_robin"
    },
    "tenant-service": {
      "servers": ["http://tenants-svc:8085"],
      "algorithm": "ip_hash"
    }
  }
}
```

**Load hostname routes in code:**

```rust
use highper_gateway::gateway::routing::HostnameRouter;
use std::sync::Arc;

// Create router
let router = Arc::new(HostnameRouter::new());

// Load from JSON file
let json = std::fs::read_to_string("hostname_routes.json")?;
router.load_from_json(&json).await?;

// Enable hot reload (watches file for changes)
router.enable_hot_reload("hostname_routes.json".to_string(), 5).await?;

// Integrate with handler
let handler = Handler::new(config)
    .with_hostname_router(router);
```

### Route Matching Priority

1. **Exact match** (fastest - O(1))
2. **Prefix match** (O(log n) - sorted by length)
3. **Pattern match** (regex - slowest but flexible)

---

## Static File Serving

### Basic Static Site

```rust
use highper_gateway::webserver::{StaticFileHandler, WebServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

let webserver_config = WebServerConfig {
    index_files: vec!["index.html".to_string(), "index.htm".to_string()],
    directory_listing: false,
    cache_control_max_age: 3600, // 1 hour
};

let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/html"),
    webserver_config
));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler);
```

### Static Site + API Proxy (Hybrid)

**Directory structure:**
```
/var/www/myapp/
├── index.html
├── css/
│   └── style.css
├── js/
│   └── app.js
└── images/
    └── logo.png
```

**Configuration:**
```yaml
server:
  bind: ["0.0.0.0:8080"]
  protocols: [Http1, Http2]

# Static files served first, then API proxy
routes:
  - name: "api"
    match_rules:
      paths: ["/api/*"]
    upstream: "api-backend"

upstreams:
  - name: "api-backend"
    servers:
      - "http://localhost:3000"
```

**Code:**
```rust
// Static handler for /var/www/myapp
let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/myapp"),
    WebServerConfig::default()
));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler);
```

**Request flow:**
- `GET /index.html` → Serves `/var/www/myapp/index.html`
- `GET /css/style.css` → Serves `/var/www/myapp/css/style.css`
- `GET /api/users` → Proxies to `http://localhost:3000/api/users`

---

## PHP-FPM for WordPress/Laravel

### WordPress Configuration

**PHP-FPM Configuration:**
```rust
use highper_gateway::webserver::{PhpFpmPool, PhpFpmConfig};
use std::sync::Arc;

let php_config = PhpFpmConfig {
    socket: "/run/php/php8.2-fpm.sock".to_string(), // or "127.0.0.1:9000"
    pool_size: 10,
    read_timeout: 30,
    write_timeout: 30,
};

let php_pool = Arc::new(PhpFpmPool::new(php_config));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler) // For CSS/JS/images
    .with_php_fpm_pool(php_pool);             // For .php files
```

**Directory:**
```
/var/www/wordpress/
├── index.php
├── wp-admin/
│   └── index.php
├── wp-content/
│   ├── themes/
│   └── plugins/
└── wp-includes/
```

**Request flow:**
- `GET /index.php` → PHP-FPM → WordPress
- `GET /wp-admin/index.php` → PHP-FPM → WordPress admin
- `GET /wp-content/themes/twentytwenty/style.css` → Static serve
- `GET /wp-content/uploads/image.jpg` → Static serve

### Laravel Configuration

```rust
let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/laravel/public"),
    WebServerConfig {
        index_files: vec!["index.php".to_string()],
        ..Default::default()
    }
));

let php_pool = Arc::new(PhpFpmPool::new(PhpFpmConfig {
    socket: "/run/php/php8.2-fpm.sock".to_string(),
    pool_size: 20,  // Higher for Laravel
    read_timeout: 60,
    write_timeout: 30,
}));
```

---

## WebSocket Proxying

### WebSocket Configuration

```yaml
websocket:
  enabled: true

routes:
  - name: "chat-websocket"
    match_rules:
      paths: ["/ws/chat"]
    upstream: "chat-server"

  - name: "realtime-api"
    match_rules:
      paths: ["/ws/realtime"]
    upstream: "realtime-server"

upstreams:
  - name: "chat-server"
    servers:
      - "http://localhost:8081"  # WebSocket backend
    load_balancing:
      algorithm: "ip_hash"  # Sticky sessions for WebSocket

  - name: "realtime-server"
    servers:
      - "http://localhost:8082"
```

**Features:**
- Automatic upgrade detection
- Bi-directional streaming
- Connection persistence
- Load balancing with sticky sessions

---

## gRPC Proxying

### gRPC Configuration

```yaml
grpc:
  enabled: true

routes:
  - name: "grpc-api"
    match_rules:
      paths: ["/grpc.service/*"]
      headers:
        content-type: ["application/grpc"]
    upstream: "grpc-backend"

upstreams:
  - name: "grpc-backend"
    servers:
      - "http://localhost:50051"
    load_balancing:
      algorithm: "round_robin"
```

---

## TLS/HTTPS Configuration

### Manual Certificates

```yaml
tls:
  cert_path: "/etc/ssl/certs/example.com.crt"
  key_path: "/etc/ssl/private/example.com.key"

server:
  bind: ["0.0.0.0:8080"]      # HTTP
  tls_bind: ["0.0.0.0:8443"]  # HTTPS
  protocols: [Http1, Http2]
```

### ACME/Let's Encrypt (Automatic)

```yaml
tls:
  acme:
    enabled: true
    email: "admin@example.com"
    domains:
      - "example.com"
      - "*.example.com"
    directory_url: "https://acme-v02.api.letsencrypt.org/directory"
    cert_cache_dir: "/etc/highper-gateway/certs"

server:
  bind: ["0.0.0.0:80"]        # For ACME HTTP-01 challenge
  tls_bind: ["0.0.0.0:443"]
```

### mTLS (Mutual TLS)

```yaml
tls:
  cert_path: "/etc/ssl/certs/server.crt"
  key_path: "/etc/ssl/private/server.key"
  client_ca_path: "/etc/ssl/certs/client-ca.crt"  # Client certificate CA
  verify_client: true
```

---

## Load Balancing

### Algorithms

#### 1. Round Robin (Default)
```yaml
upstreams:
  - name: "backend"
    servers:
      - "http://server1:8080"
      - "http://server2:8080"
      - "http://server3:8080"
    load_balancing:
      algorithm: "round_robin"
```

#### 2. Least Connections
```yaml
load_balancing:
  algorithm: "least_connections"
```

#### 3. IP Hash (Sticky Sessions)
```yaml
load_balancing:
  algorithm: "ip_hash"
```

#### 4. Weighted
```yaml
upstreams:
  - name: "backend"
    servers:
      - "http://server1:8080?weight=3"
      - "http://server2:8080?weight=1"
    load_balancing:
      algorithm: "weighted"
```

#### 5. Geographic (GeoIP-based)
```yaml
load_balancing:
  algorithm: "geographic"
  geoip_provider: "maxmind"
  geoip_db_path: "/usr/share/GeoIP/GeoLite2-City.mmdb"
```

---

## Caching and Compression

### Compression

```yaml
compression:
  enabled: true
  algorithms:
    - "gzip"
    - "brotli"
    - "zstd"
  min_size: 1024  # Only compress files > 1KB
  level: 6        # Compression level (1-9)
```

---

## Complete Use Case Examples

### Use Case 1: Multi-Tenant SaaS Platform

```json
{
  "version": "1.0",
  "hosts": [
    {
      "hostname": "*.app.example.com",
      "routes": [
        {
          "name": "tenant-app",
          "match_type": "prefix",
          "prefix": "/",
          "upstream": "tenant-backend",
          "methods": [],
          "middleware": ["tenant-auth", "rate-limit"]
        }
      ]
    },
    {
      "hostname": "api.example.com",
      "routes": [
        {
          "name": "public-api",
          "match_type": "prefix",
          "prefix": "/v1/",
          "upstream": "api-backend",
          "methods": ["GET", "POST"],
          "middleware": ["api-key-auth"]
        }
      ]
    }
  ],
  "upstreams": {
    "tenant-backend": {
      "servers": ["http://app-svc:8080"],
      "algorithm": "ip_hash"
    },
    "api-backend": {
      "servers": ["http://api-svc:8081", "http://api-svc:8082"],
      "algorithm": "least_connections"
    }
  }
}
```

### Use Case 2: WordPress + Static CDN

```rust
// WordPress on /var/www/wordpress
// Static assets on /var/www/cdn

let wordpress_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/wordpress"),
    WebServerConfig::default()
));

let php_pool = Arc::new(PhpFpmPool::new(PhpFpmConfig {
    socket: "/run/php/php8.2-fpm.sock".to_string(),
    pool_size: 50,
    read_timeout: 30,
    write_timeout: 30,
}));

let handler = Handler::new(config)
    .with_static_file_handler(wordpress_handler)
    .with_php_fpm_pool(php_pool);
```

### Use Case 3: Microservices API Gateway

```json
{
  "version": "1.0",
  "hosts": [
    {
      "hostname": "api.example.com",
      "routes": [
        {
          "name": "users",
          "match_type": "prefix",
          "prefix": "/users/",
          "upstream": "users-svc",
          "timeout_ms": 5000
        },
        {
          "name": "orders",
          "match_type": "prefix",
          "prefix": "/orders/",
          "upstream": "orders-svc",
          "timeout_ms": 10000
        },
        {
          "name": "payments",
          "match_type": "prefix",
          "prefix": "/payments/",
          "upstream": "payments-svc",
          "timeout_ms": 15000
        },
        {
          "name": "notifications",
          "match_type": "prefix",
          "prefix": "/notifications/",
          "upstream": "notifications-svc",
          "timeout_ms": 3000
        }
      ]
    }
  ],
  "upstreams": {
    "users-svc": {
      "servers": ["http://users:8080", "http://users:8081"],
      "algorithm": "round_robin",
      "health_check": {
        "interval_secs": 10,
        "path": "/health"
      }
    },
    "orders-svc": {
      "servers": ["http://orders:8080"],
      "algorithm": "round_robin"
    },
    "payments-svc": {
      "servers": ["http://payments:8080", "http://payments:8081", "http://payments:8082"],
      "algorithm": "least_connections"
    },
    "notifications-svc": {
      "servers": ["http://notifications:8080"],
      "algorithm": "round_robin"
    }
  }
}
```

### Use Case 4: Single Page Application (SPA)

```rust
// Serve React/Vue/Angular app from /var/www/spa

let spa_config = WebServerConfig {
    index_files: vec!["index.html".to_string()],
    directory_listing: false,
    cache_control_max_age: 31536000, // 1 year for static assets
};

let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/spa"),
    spa_config
));

// API routes proxy to backend
let handler = Handler::new(config)
    .with_static_file_handler(static_handler);
```

YAML:
```yaml
routes:
  - name: "api"
    match_rules:
      paths: ["/api/*"]
    upstream: "backend-api"

upstreams:
  - name: "backend-api"
    servers:
      - "http://localhost:3000"
```

---

## Performance Tips

1. **Use hostname router for multi-domain setups** (O(1) lookup vs O(n) route matching)
2. **Enable compression** for text-based responses
3. **Use IP hash** for WebSocket and stateful connections
4. **Configure appropriate pool sizes** for PHP-FPM based on traffic
5. **Enable caching headers** for static files
6. **Use health checks** to avoid routing to dead backends

## Monitoring

Check logs for:
- `Hostname router matched:` - API Gateway route hits
- `Serving static file:` - Static file serves
- `Processing PHP request:` - PHP-FPM requests
- `kTLS is available:` - Kernel TLS support detection

---

## Summary

| Feature | Configuration | Use Case |
|---------|--------------|----------|
| **API Gateway** | hostname_routes.json | Multi-domain routing |
| **Static Files** | StaticFileHandler | Static websites, SPAs |
| **PHP-FPM** | PhpFpmPool | WordPress, Laravel |
| **WebSocket** | websocket.enabled | Real-time apps |
| **gRPC** | grpc.enabled | Microservices |
| **TLS/ACME** | tls.acme | Automatic HTTPS |
| **Load Balancing** | algorithm | High availability |

For more examples, see the `examples/` directory.
