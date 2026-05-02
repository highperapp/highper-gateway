# Highper Gateway

A high-performance reverse proxy and API gateway written in Rust, designed for extreme throughput and low latency.

> **Release status (2026-05-02):** v1.0 has not been GA-tagged. The codebase is **v1.0-rc**;
> 14 release blockers (B1–B14) are tracked in [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md)
> §4.1 and must close before any v1.0 announcement. Earlier statements implying
> production-ready / v1.0 status in this README and `CHANGELOG.md` predate that audit
> and are being reconciled.

## Performance

- **200,000+ RPS** on a single node (DigitalOcean Premium AMD, 4 vCPU)
- **Sub-millisecond P99 latency** under load
- Minimal memory footprint with zero-copy operations

## Features

### Core Proxy
- HTTP/1.1, HTTP/2, and HTTP/3 (QUIC) support
- WebSocket proxying with connection multiplexing
- gRPC proxying and load balancing
- TCP/L4 proxying for databases (MySQL, PostgreSQL, Redis)
- TLS termination with SNI routing
- Automatic certificate management (ACME/Let's Encrypt)

### Load Balancing
- Round-robin, weighted, least-connections, IP-hash
- Consistent hashing for cache-friendly routing
- Active and passive health checks
- Circuit breaker pattern

### Security

**OWASP Top 10 2021 Compliance**: Grade A (96/100)

#### Security Headers Middleware
- X-Content-Type-Options (MIME-sniffing protection)
- X-Frame-Options (clickjacking protection)
- X-XSS-Protection (XSS defense for legacy browsers)
- Strict-Transport-Security/HSTS (SSL stripping prevention)
- Content-Security-Policy/CSP (XSS and injection defense)
- Referrer-Policy (privacy protection)
- Permissions-Policy (browser feature control)
- Three presets: strict, default (balanced), relaxed

#### Request Size Limits & DoS Protection
- Configurable request body size limits (default 10MB)
- Pre-buffering Content-Length validation
- 413 Payload Too Large responses
- Per-route size configuration
- Custom error messages
- Zero memory overhead

#### Additional Security Features
- Rate limiting (token bucket, sliding window)
- JWT authentication and validation
- mTLS (mutual TLS) support
- Web Application Firewall (WAF) integration
- Request body validation
- Account lockout protection
- TLS 1.2+ enforcement with strong cipher suites
- OCSP stapling

See [Security Features Guide](docs/SECURITY_FEATURES.md) for complete documentation.

### Observability
- Prometheus metrics endpoint
- OpenTelemetry tracing
- Structured JSON logging
- Request correlation IDs
- Real-time dashboard

### Advanced Features
- **PHP-FPM & Static Files** - Full web server capabilities (NEW!)
  - FastCGI/PHP-FPM integration with connection pooling
  - High-performance static file serving (zero-copy sendfile, kTLS)
  - Nginx-style try_files patterns
  - WordPress, Laravel, and custom PHP applications
  - See [PHP-FPM Quick Start](PHP_FPM_QUICK_START.md)
- Hot configuration reload (zero downtime)
- WASM plugin system
- GraphQL gateway with schema stitching
- API aggregation
- Compression (gzip, brotli, zstd)
- Response caching

## Quick Start

### Build from Source

```bash
git clone https://github.com/highperapp/highper-gateway.git
cd highper-gateway
cargo build --release
```

### Run

```bash
# Test configuration
./target/release/highper-gateway test --config config.yaml

# Start proxy
./target/release/highper-gateway start --config config.yaml
```

### Docker

```bash
# Build image
docker build -t highper-gateway:latest -f deployment/docker/Dockerfile .

# Run
docker run -p 8080:8080 -v $(pwd)/config.yaml:/etc/highper-gateway/config.yaml highper-gateway:latest
```

## Configuration

Highper Gateway supports YAML and TOML configuration formats, plus a Caddy-like DSL.

### Basic Example (YAML)

```yaml
listeners:
  - address: "0.0.0.0:8080"
    protocol: http

routes:
  - path: /api
    upstream:
      servers:
        - address: "127.0.0.1:3000"
          weight: 1
      load_balancing: round_robin
      health_check:
        interval: 10s
        timeout: 5s
```

### DSL Example

```
http://0.0.0.0:8080 {
    route /api/* {
        upstream http://127.0.0.1:3000
        lb_policy round_robin
    }

    rate_limit {
        requests 1000
        window 1m
    }
}
```

### PHP-FPM Example (DSL)

Serve WordPress, Laravel, or any PHP application:

```
https://blog.example.com {
    root "/var/www/wordpress"
    index index.php index.html

    # Serve static assets directly
    /wp-content/* {
        static_files
        try_files $uri =404
    }

    # PHP files via FastCGI
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=100
        proxy localhost:9000
    }

    # Pretty permalinks (WordPress/Laravel style)
    /* {
        try_files $uri $uri/ /index.php
    }

    tls admin@example.com
}
```

**Features**:
- Zero-copy static file serving with kTLS
- Connection pooling to PHP-FPM
- Nginx-compatible try_files patterns
- Automatic TLS with Let's Encrypt

Try it now: `cd demo/php-fpm && cargo run -- --config demo.dsl`

See [PHP-FPM Quick Start Guide](PHP_FPM_QUICK_START.md) for complete documentation.

## Deployment

### Systemd

```bash
sudo cp deployment/systemd/highper-gateway.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable highper-gateway
sudo systemctl start highper-gateway
```

### Kubernetes

```bash
kubectl apply -f deployment/kubernetes/
```

See [deployment/](deployment/) for detailed deployment guides.

## Performance Tuning

For optimal performance:

1. **OS Tuning**: Run `scripts/performance-tune.sh` to configure kernel parameters
2. **CPU Affinity**: Pin workers to specific cores
3. **Connection Pooling**: Configure upstream connection pools
4. **Buffer Sizes**: Tune based on your payload sizes

See [docs/dev-notes/PERFORMANCE_TUNING_GUIDE.md](docs/dev-notes/PERFORMANCE_TUNING_GUIDE.md) for details.

## Monitoring

### Prometheus

Metrics are exposed at `/metrics` on the admin port (default: 9090).

```yaml
admin:
  address: "127.0.0.1:9090"
  metrics: true
```

### Grafana Dashboard

Import the dashboard from `monitoring/grafana-dashboard.json`.

## Documentation

- [Security Features Guide](docs/SECURITY_FEATURES.md)
- [Configuration Guide](docs/dev-notes/CONFIGURATION_GUIDE.md)
- [Docker Deployment](deployment/docker/README.md)
- [Kubernetes Deployment](deployment/kubernetes/README.md)
- [Performance Tuning](docs/dev-notes/PERFORMANCE_TUNING_GUIDE.md)
- [Security Hardening](docs/dev-notes/SECURITY_HARDENING_GUIDE.md)

## Architecture

```
                    +-------------+
                    |   Client    |
                    +------+------+
                           |
                    +------v------+
                    |  Listener   |
                    | (HTTP/H2/H3)|
                    +------+------+
                           |
              +------------+------------+
              |            |            |
        +-----v-----+ +----v----+ +-----v-----+
        |Middleware | | Router  | |  Cache    |
        |  Chain    | |         | |           |
        +-----+-----+ +----+----+ +-----+-----+
              |            |            |
              +------------+------------+
                           |
                    +------v------+
                    |Load Balancer|
                    +------+------+
                           |
              +------------+------------+
              |            |            |
        +-----v-----+ +----v----+ +-----v-----+
        | Upstream1 | |Upstream2| | Upstream3 |
        +-----------+ +---------+ +-----------+
```

## License

Apache 2.0 License - see [LICENSE](LICENSE) for details.

This allows you to use Highper Gateway in commercial products and create paid plugins with different licenses.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
