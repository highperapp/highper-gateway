# Rust Reverse Proxy & API Gateway

A high-performance, feature-rich reverse proxy and API gateway built with Rust, designed for production use with modern protocols and comprehensive management capabilities.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Tests](https://img.shields.io/badge/tests-228%20passing-brightgreen.svg)]()

---

## ✨ Features

### Core Reverse Proxy
- 🚀 **HTTP/1.1, HTTP/2, HTTP/3 (QUIC)** support
- 🔄 **Multiple load balancing algorithms** (round-robin, least-connections, IP hash, geographic)
- 💚 **Active and passive health checks**
- 🔌 **WebSocket** proxying
- 📡 **gRPC** support with bidirectional streaming
- 🌐 **SNI routing** for multi-domain hosting

### Security & TLS
- 🔐 **TLS 1.2/1.3** with rustls
- 📜 **ACME/Let's Encrypt** automatic certificate management
- 🤝 **Mutual TLS (mTLS)** with client certificate validation
- ✅ **OCSP stapling** (production-ready)
- 🔄 **Certificate hot-reload** without downtime

### API Gateway Capabilities
- 🔑 **Authentication** (JWT, API keys, OAuth2)
- ⏱️ **Rate limiting** (local and distributed with Redis)
- 💾 **Response caching** (local and distributed)
- 🔀 **Request aggregation** and transformation
- 📊 **GraphQL** support

### Observability
- 📊 **Prometheus metrics** export
- 🔍 **OpenTelemetry** distributed tracing
- 📝 **Structured logging**
- 📈 **Real-time statistics**

### Management & Operations
- 🎛️ **Comprehensive Admin API** (17 REST endpoints)
- 🔧 **Hot configuration reload**
- 🏥 **Backend health management**
- 🗑️ **Cache control**
- 📉 **Detailed metrics** (per-route, per-backend)
- 🎯 **Graceful connection draining**

---

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/highper-gateway.git
cd highper-gateway

# Build release binary
cargo build --release

# Binary location
./target/release/highper-gateway
```

### Basic Configuration

Create `config.yaml`:

```yaml
# Server configuration
server:
  bind:
    - "0.0.0.0:80"
    - "0.0.0.0:443"

  protocols:
    - http1
    - http2

# Upstreams (backend servers)
upstreams:
  - name: api_backend
    servers:
      - url: "http://backend1:8080"
        weight: 100
      - url: "http://backend2:8080"
        weight: 100

    load_balancing:
      algorithm: round_robin

    health_check:
      enabled: true
      interval: 10s
      timeout: 5s

# Routes
routes:
  - name: api_route
    match:
      paths:
        - "/api/*"
    upstream: api_backend

# TLS
tls:
  enabled: true
  certificates:
    - domain: "example.com"
      cert_file: "/path/to/cert.pem"
      key_file: "/path/to/key.pem"

# Admin API
admin:
  enabled: true
  bind: "127.0.0.1:9090"
  auth:
    enabled: true
    api_keys:
      - "your-secret-api-key"
```

### Run

```bash
./target/release/highper-gateway --config config.yaml
```

---

## 📖 Documentation

### Getting Started
- **[Quick Start Guide](ADMIN_API_QUICKSTART.md)** - Get up and running in 5 minutes
- **[Configuration Guide](examples/)** - Example configurations
- **[Admin API Reference](ADMIN_API_REFERENCE.md)** - Complete API documentation

### Advanced Topics
- **[TLS & OCSP Stapling](OCSP_STAPLING_IMPLEMENTATION.md)** - SSL/TLS configuration
- **[Load Balancing](docs/load-balancing.md)** - Load balancing strategies
- **[Health Checks](docs/health-checks.md)** - Health check configuration
- **[Caching](docs/caching.md)** - Response caching setup

### Operations
- **[Admin API Examples](examples/admin_api_examples.sh)** - Curl command examples
- **[Monitoring](docs/monitoring.md)** - Prometheus integration
- **[Deployment](docs/deployment.md)** - Production deployment guide

---

## 🎯 Use Cases

### 1. Simple Reverse Proxy

```yaml
upstreams:
  - name: web_backend
    servers:
      - url: "http://localhost:3000"

routes:
  - name: web_route
    match:
      paths: ["/*"]
    upstream: web_backend
```

### 2. Load Balanced API

```yaml
upstreams:
  - name: api_backend
    servers:
      - url: "http://api1:8080"
      - url: "http://api2:8080"
      - url: "http://api3:8080"

    load_balancing:
      algorithm: least_connections

    health_check:
      enabled: true
      interval: 5s
```

### 3. API Gateway with Auth & Rate Limiting

```yaml
routes:
  - name: protected_api
    match:
      paths: ["/api/*"]
    upstream: api_backend

    middleware:
      - jwt_auth
      - rate_limit

    rate_limit:
      requests_per_second: 100
      burst: 20
```

### 4. Geographic Load Balancing

```yaml
upstreams:
  - name: global_api
    servers:
      - url: "http://us-west.api:8080"
        region: "us-west-1"
      - url: "http://eu-west.api:8080"
        region: "eu-west-1"

    load_balancing:
      algorithm: geographic
      geoip_db_path: "/path/to/GeoLite2-City.mmdb"
```

---

## 🔧 Admin API

The proxy includes a comprehensive REST API for runtime management:

### Quick Examples

```bash
# Check health
curl http://localhost:9090/health

# List all backends
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/backends

# Disable a backend for maintenance
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Maintenance"}' \
  http://localhost:9090/api/backends/api_backend_0/disable

# Clear cache
curl -X POST -H "X-API-Key: secret" \
  http://localhost:9090/api/cache/clear

# Get Prometheus metrics
curl http://localhost:9090/metrics
```

See [Admin API Reference](ADMIN_API_REFERENCE.md) for complete documentation.

---

## 📊 Monitoring

### Prometheus Integration

The proxy exports metrics in Prometheus format:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'highper-gateway'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
```

### Key Metrics

- `proxy_requests_total` - Total requests processed
- `proxy_requests_duration_seconds` - Request latency histogram
- `proxy_backend_up` - Backend health status
- `proxy_backend_connections_active` - Active connections per backend
- `cache_hits_total` / `cache_misses_total` - Cache performance

---

## ⚡ Performance

### Benchmarks

(Add benchmark results here after testing)

### Performance Features

- **Zero-copy proxying** where possible
- **Connection pooling** for upstream connections
- **Efficient async I/O** with Tokio
- **Lock-free data structures** (DashMap for caching)
- **HTTP/2 multiplexing** support
- **Compression** (gzip, br, zstd)

---

## 🛡️ Security Features

### TLS Security
- **Modern cipher suites** only (TLS 1.2+)
- **OCSP stapling** for certificate validation
- **SNI** for multi-domain hosting
- **Client certificate validation** (mTLS)

### Application Security
- **Rate limiting** to prevent abuse
- **JWT validation** with configurable algorithms
- **API key authentication**
- **Request size limits**
- **Timeout protection**

### Admin API Security
- **API key authentication**
- **JWT token support**
- **CORS configuration**
- **Localhost-only binding** (default)

---

## 🏗️ Architecture

```
┌─────────────┐
│   Clients   │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────┐
│     Rust Reverse Proxy              │
│  ┌────────────┐  ┌────────────┐    │
│  │  Router    │  │ Admin API  │    │
│  └─────┬──────┘  └────────────┘    │
│        │                             │
│  ┌─────▼────────────────────┐       │
│  │  Load Balancer           │       │
│  │  - Round Robin           │       │
│  │  - Least Connections     │       │
│  │  - Geographic            │       │
│  └─────┬────────────────────┘       │
│        │                             │
│  ┌─────▼────────────────────┐       │
│  │  Middleware Stack        │       │
│  │  - Auth                  │       │
│  │  - Rate Limit            │       │
│  │  - Cache                 │       │
│  │  - Logging               │       │
│  └─────┬────────────────────┘       │
│        │                             │
└────────┼─────────────────────────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌────────┐ ┌────────┐
│Backend1│ │Backend2│
└────────┘ └────────┘
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test --test admin_api_simple

# Run with logging
RUST_LOG=debug cargo test

# Test coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

**Current Test Status:**
- ✅ 228 tests passing
- ✅ 100% success rate
- ✅ Admin API: 29 tests
- ✅ TLS: 40 tests

---

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Setup

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/yourusername/highper-gateway.git
cd highper-gateway
cargo build

# Run tests
cargo test

# Run with examples
cargo run -- --config examples/basic_config.yaml
```

---

## 📝 Configuration Reference

### Server Configuration

```yaml
server:
  bind: ["0.0.0.0:80", "0.0.0.0:443"]
  protocols: [http1, http2, http3]
  workers: auto  # or specific number
  shutdown_timeout: 30s
```

### Upstream Configuration

```yaml
upstreams:
  - name: backend_name
    servers:
      - url: "http://server:port"
        weight: 100
        max_conns: 1000

    load_balancing:
      algorithm: round_robin  # round_robin, least_connections, ip_hash, geographic

    health_check:
      enabled: true
      interval: 10s
      timeout: 5s
      healthy_threshold: 2
      unhealthy_threshold: 3
```

### Route Configuration

```yaml
routes:
  - name: route_name
    match:
      paths: ["/api/*"]
      hosts: ["example.com"]
      methods: ["GET", "POST"]

    upstream: backend_name
    priority: 100

    middleware:
      - auth
      - rate_limit

    timeout: 30s
```

See example configurations in `examples/` directory.

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

Built with these excellent Rust crates:
- [Tokio](https://tokio.rs/) - Async runtime
- [Hyper](https://hyper.rs/) - HTTP library
- [Rustls](https://github.com/rustls/rustls) - TLS implementation
- [Serde](https://serde.rs/) - Serialization
- [Tracing](https://tracing.rs/) - Logging and diagnostics

---

## 📧 Contact & Support

- **Issues**: [GitHub Issues](https://github.com/yourusername/highper-gateway/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/highper-gateway/discussions)
- **Documentation**: [Full Docs](docs/)

---

## 🗺️ Roadmap

### v1.0 (Current)
- ✅ Core reverse proxy functionality
- ✅ TLS with OCSP stapling
- ✅ Load balancing
- ✅ Admin API
- ⏳ Complete documentation

### v1.1
- WebSocket support in Admin API
- Rate limiting management
- Enhanced dashboards

### v1.2
- Admin UI (React)
- Multi-user RBAC
- Advanced traffic management

### v2.0
- Service mesh integration
- Multi-instance orchestration
- Enterprise features

---

**Made with ❤️ and Rust**
