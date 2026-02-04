# Rust Proxy - Validated Use Cases

Quick-start configurations for common use cases. Each example is production-ready.

## 1. Simple Reverse Proxy

**Use Case:** Load balance traffic across multiple backend servers.

**config.yaml:**
```yaml
server:
  bind: ["0.0.0.0:80"]
  protocols: [Http1, Http2]

routes:
  - name: "default"
    match_rules:
      paths: ["/*"]
    upstream: "backend"

upstreams:
  - name: "backend"
    servers:
      - "http://192.168.1.10:8080"
      - "http://192.168.1.11:8080"
      - "http://192.168.1.12:8080"
    load_balancing:
      algorithm: "round_robin"
```

**Start:**
```bash
cargo run -- --config config.yaml
```

**Test:**
```bash
curl http://localhost/
```

---

## 2. Static File Server

**Use Case:** Serve a static website (HTML, CSS, JS, images).

**main.rs:**
```rust
use rust_proxy::webserver::{StaticFileHandler, WebServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

let config = Arc::new(Config::from_file("config.yaml")?);

let webserver_config = WebServerConfig {
    index_files: vec!["index.html".to_string()],
    directory_listing: false,
    cache_control_max_age: 3600,
};

let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/html"),
    webserver_config
));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler);

// Start server with handler
```

**Directory:**
```
/var/www/html/
├── index.html
├── style.css
├── app.js
└── logo.png
```

**Test:**
```bash
curl http://localhost/index.html
curl http://localhost/style.css
```

---

## 3. WordPress Hosting

**Use Case:** Host WordPress with PHP-FPM.

**main.rs:**
```rust
use rust_proxy::webserver::{StaticFileHandler, PhpFpmPool, PhpFpmConfig, WebServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

let config = Arc::new(Config::from_file("config.yaml")?);

// Static files (CSS, JS, images)
let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/wordpress"),
    WebServerConfig::default()
));

// PHP-FPM for .php files
let php_pool = Arc::new(PhpFpmPool::new(PhpFpmConfig {
    socket: "/run/php/php8.2-fpm.sock".to_string(),
    pool_size: 10,
    read_timeout: 30,
    write_timeout: 30,
}));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler)
    .with_php_fpm_pool(php_pool);
```

**Prerequisites:**
```bash
# Install PHP-FPM
sudo apt install php8.2-fpm

# Download WordPress
wget https://wordpress.org/latest.tar.gz
tar xzf latest.tar.gz
sudo mv wordpress /var/www/
```

**Test:**
```bash
curl http://localhost/          # WordPress homepage
curl http://localhost/wp-admin/ # WordPress admin
```

---

## 4. Multi-Domain API Gateway

**Use Case:** Route different domains to different backends.

**hostname_routes.json:**
```json
{
  "version": "1.0",
  "hosts": [
    {
      "hostname": "api.example.com",
      "routes": [
        {
          "name": "api-v1",
          "match_type": "prefix",
          "prefix": "/v1/",
          "upstream": "api-backend-v1",
          "methods": ["GET", "POST"],
          "timeout_ms": 5000,
          "middleware": []
        }
      ]
    },
    {
      "hostname": "admin.example.com",
      "routes": [
        {
          "name": "admin-panel",
          "match_type": "prefix",
          "prefix": "/",
          "upstream": "admin-backend",
          "methods": [],
          "timeout_ms": 10000,
          "middleware": []
        }
      ]
    }
  ],
  "upstreams": {
    "api-backend-v1": {
      "servers": ["http://api-v1:8080", "http://api-v1:8081"],
      "algorithm": "round_robin"
    },
    "admin-backend": {
      "servers": ["http://admin:8082"],
      "algorithm": "round_robin"
    }
  }
}
```

**main.rs:**
```rust
use rust_proxy::gateway::routing::HostnameRouter;
use std::sync::Arc;

let router = Arc::new(HostnameRouter::new());

// Load routes
let json = std::fs::read_to_string("hostname_routes.json")?;
router.load_from_json(&json).await?;

let handler = Handler::new(config)
    .with_hostname_router(router);
```

**Test:**
```bash
curl -H "Host: api.example.com" http://localhost/v1/users
curl -H "Host: admin.example.com" http://localhost/dashboard
```

---

## 5. Microservices Gateway

**Use Case:** Route /users, /orders, /payments to different microservices.

**hostname_routes.json:**
```json
{
  "version": "1.0",
  "hosts": [
    {
      "hostname": "api.company.com",
      "routes": [
        {
          "name": "users-service",
          "match_type": "prefix",
          "prefix": "/users/",
          "upstream": "users-svc",
          "methods": [],
          "timeout_ms": 5000,
          "middleware": []
        },
        {
          "name": "orders-service",
          "match_type": "prefix",
          "prefix": "/orders/",
          "upstream": "orders-svc",
          "methods": [],
          "timeout_ms": 10000,
          "middleware": []
        },
        {
          "name": "payments-service",
          "match_type": "prefix",
          "prefix": "/payments/",
          "upstream": "payments-svc",
          "methods": [],
          "timeout_ms": 15000,
          "middleware": []
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
        "timeout_secs": 5,
        "path": "/health",
        "expected_status": 200
      }
    },
    "orders-svc": {
      "servers": ["http://orders:8080"],
      "algorithm": "round_robin"
    },
    "payments-svc": {
      "servers": ["http://payments:8080", "http://payments:8081"],
      "algorithm": "least_connections"
    }
  }
}
```

**Test:**
```bash
curl http://localhost/users/123
curl http://localhost/orders/456
curl http://localhost/payments/789
```

---

## 6. WebSocket Proxy

**Use Case:** Proxy WebSocket connections for real-time apps.

**config.yaml:**
```yaml
server:
  bind: ["0.0.0.0:8080"]
  protocols: [Http1, Http2]

websocket:
  enabled: true

routes:
  - name: "chat-websocket"
    match_rules:
      paths: ["/ws/chat"]
    upstream: "chat-backend"

  - name: "http-api"
    match_rules:
      paths: ["/api/*"]
    upstream: "api-backend"

upstreams:
  - name: "chat-backend"
    servers:
      - "http://localhost:9001"
    load_balancing:
      algorithm: "ip_hash"  # Sticky sessions

  - name: "api-backend"
    servers:
      - "http://localhost:9002"
```

**Test:**
```bash
# WebSocket connection
websocat ws://localhost:8080/ws/chat

# Regular HTTP
curl http://localhost:8080/api/status
```

---

## 7. SPA + API (React/Vue/Angular)

**Use Case:** Serve React/Vue/Angular frontend + proxy API to backend.

**Directory:**
```
/var/www/spa/
├── index.html
├── static/
│   ├── css/
│   ├── js/
│   └── media/
└── favicon.ico
```

**main.rs:**
```rust
use rust_proxy::webserver::{StaticFileHandler, WebServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/spa"),
    WebServerConfig {
        index_files: vec!["index.html".to_string()],
        directory_listing: false,
        cache_control_max_age: 31536000, // 1 year for static assets
    }
));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler);
```

**config.yaml:**
```yaml
routes:
  - name: "api"
    match_rules:
      paths: ["/api/*"]
    upstream: "backend"

upstreams:
  - name: "backend"
    servers:
      - "http://localhost:3000"
```

**Request Flow:**
- `GET /` → `/var/www/spa/index.html`
- `GET /static/js/app.js` → `/var/www/spa/static/js/app.js`
- `GET /api/users` → Proxies to `http://localhost:3000/api/users`

---

## 8. HTTPS with Let's Encrypt

**Use Case:** Automatic HTTPS with Let's Encrypt.

**config.yaml:**
```yaml
tls:
  acme:
    enabled: true
    email: "admin@example.com"
    domains:
      - "example.com"
      - "www.example.com"
    directory_url: "https://acme-v02.api.letsencrypt.org/directory"
    cert_cache_dir: "/etc/rust-proxy/certs"

server:
  bind: ["0.0.0.0:80"]       # For ACME challenge
  tls_bind: ["0.0.0.0:443"]  # HTTPS
  protocols: [Http1, Http2]

routes:
  - name: "default"
    match_rules:
      paths: ["/*"]
    upstream: "backend"

upstreams:
  - name: "backend"
    servers:
      - "http://localhost:3000"
```

**Test:**
```bash
curl https://example.com/
```

---

## 9. Multi-Tenant SaaS

**Use Case:** Wildcard domain routing (*.app.example.com).

**hostname_routes.json:**
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
          "upstream": "app-backend",
          "methods": [],
          "middleware": [],
          "timeout_ms": 5000
        }
      ]
    },
    {
      "hostname": "api.example.com",
      "routes": [
        {
          "name": "public-api",
          "match_type": "prefix",
          "prefix": "/",
          "upstream": "api-backend",
          "methods": [],
          "middleware": [],
          "timeout_ms": 3000
        }
      ]
    }
  ],
  "upstreams": {
    "app-backend": {
      "servers": ["http://app:8080"],
      "algorithm": "ip_hash"
    },
    "api-backend": {
      "servers": ["http://api:8081"],
      "algorithm": "round_robin"
    }
  }
}
```

**Test:**
```bash
curl -H "Host: tenant1.app.example.com" http://localhost/
curl -H "Host: tenant2.app.example.com" http://localhost/
curl -H "Host: api.example.com" http://localhost/v1/status
```

---

## 10. Laravel Application

**Use Case:** Host Laravel PHP framework.

**main.rs:**
```rust
use rust_proxy::webserver::{StaticFileHandler, PhpFpmPool, PhpFpmConfig, WebServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

let static_handler = Arc::new(StaticFileHandler::new(
    PathBuf::from("/var/www/laravel/public"),
    WebServerConfig {
        index_files: vec!["index.php".to_string()],
        directory_listing: false,
        cache_control_max_age: 3600,
    }
));

let php_pool = Arc::new(PhpFpmPool::new(PhpFpmConfig {
    socket: "/run/php/php8.2-fpm.sock".to_string(),
    pool_size: 20,
    read_timeout: 60,
    write_timeout: 30,
}));

let handler = Handler::new(config)
    .with_static_file_handler(static_handler)
    .with_php_fpm_pool(php_pool);
```

**Test:**
```bash
curl http://localhost/          # Laravel homepage
curl http://localhost/api/user  # Laravel API
```

---

## Configuration Decision Tree

```
Need multi-domain routing?
├─ Yes → Use API Gateway (hostname_routes.json)
└─ No  → Use basic routes (config.yaml)

Need to serve static files?
├─ Yes → Add StaticFileHandler
└─ No  → Skip

Need PHP support?
├─ Yes → Add PhpFpmPool
└─ No  → Skip

Need WebSocket?
├─ Yes → Enable websocket.enabled
└─ No  → Skip

Need HTTPS?
├─ Manual certs → Set tls.cert_path
└─ Automatic → Set tls.acme.enabled
```

---

## Performance Checklist

- [ ] Use **hostname router** for multi-domain (O(1) vs O(n))
- [ ] Enable **compression** for text responses
- [ ] Use **IP hash** for WebSocket/sticky sessions
- [ ] Configure **PHP-FPM pool size** based on traffic
- [ ] Enable **caching headers** for static files
- [ ] Add **health checks** for upstreams
- [ ] Use **HTTP/2** for multiplexing
- [ ] Enable **kTLS** for Linux servers (auto-detected)

---

## Quick Reference

| Feature | Enable How | Config |
|---------|-----------|--------|
| API Gateway | `with_hostname_router()` | hostname_routes.json |
| Static Files | `with_static_file_handler()` | Document root path |
| PHP-FPM | `with_php_fpm_pool()` | Socket path + pool size |
| WebSocket | `websocket.enabled` | config.yaml |
| HTTPS | `tls.cert_path` or `tls.acme` | config.yaml |
| Load Balancing | `algorithm` | config.yaml upstreams |

---

## Support

For more details, see:
- [CONFIGURATION_GUIDE.md](CONFIGURATION_GUIDE.md) - Full configuration reference
- [examples/](examples/) - Code examples
- [tests/](tests/) - Integration tests as examples

Report issues: https://github.com/yourusername/rust-proxy/issues
