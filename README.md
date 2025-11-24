# Highper Gateway

A high-performance reverse proxy and API gateway written in Rust, designed for extreme throughput and low latency.

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
- Rate limiting (token bucket, sliding window)
- JWT authentication and validation
- mTLS (mutual TLS) support
- Web Application Firewall (WAF) integration
- Request body validation and size limits
- Account lockout protection

### Observability
- Prometheus metrics endpoint
- OpenTelemetry tracing
- Structured JSON logging
- Request correlation IDs
- Real-time dashboard

### Advanced Features
- Hot configuration reload (zero downtime)
- WASM plugin system
- GraphQL gateway with schema stitching
- API aggregation
- Compression (gzip, brotli, zstd)
- Response caching

## Quick Start

### Build from Source

```bash
git clone https://github.com/yourusername/highper-gateway.git
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

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
