# Highper Gateway

A high-performance, production-ready API gateway and load balancer written in Rust.

## Overview

Highper Gateway is a modern, cloud-native gateway solution designed for extreme performance, reliability, and flexibility. Built with Rust, it combines the safety and speed of systems programming with advanced features typically found only in enterprise-grade proxies.

## Key Features

- **High Performance**: Built with Rust and Tokio async runtime for maximum throughput
- **Multi-Protocol Support**: HTTP/1.1, HTTP/2, HTTP/3 (QUIC), gRPC, WebSocket, TCP
- **Advanced Load Balancing**: Round-robin, least-conn, IP hash, weighted, geographic routing
- **Security**: TLS termination, mTLS, ACME/Let's Encrypt, WAF (4 engines), rate limiting
- **Modern Protocols**: HTTP/3 QUIC using Cloudflare's quiche for superior performance
- **Service Discovery**: Consul, etcd integration with circuit breaker pattern
- **CDN Features**: Multi-tier caching (in-memory, Redis), edge caching
- **GraphQL Gateway**: Schema stitching and federation support
- **Database Proxying**: MySQL, PostgreSQL, Redis load balancing
- **PHP Support**: FastCGI/PHP-FPM integration with static file serving
- **Observability**: Prometheus metrics, distributed tracing, structured logging
- **12-Factor Methodology**: 100% compliance with environment-based configuration

## 15 Production Scenarios

Highper Gateway supports 15 comprehensive production scenarios, all **Production Ready**:

| # | Scenario | Status | Key Features |
|---|----------|--------|--------------|
| 01 | Layer 4 TCP | ✅ | Pure TCP proxying, connection pooling |
| 02 | Layer 7 HTTP | ✅ | HTTP/1.1 load balancing, advanced routing |
| 03 | HTTPS/TLS | ✅ | TLS termination, ACME, mTLS, OCSP |
| 04 | API Gateway | ✅ | Rate limiting (Token Bucket, Sliding Window) |
| 05 | HTTP/3 QUIC | ✅ | Cloudflare quiche, 25% faster throughput |
| 06 | WebSocket | ✅ | WebSocket load balancing, sticky sessions |
| 07 | gRPC Gateway | ✅ | HTTP/2 gRPC, streaming support |
| 08 | Database LB | ✅ | MySQL, PostgreSQL, Redis proxying |
| 09 | WAF + mTLS | ✅ | 4 WAF engines, client cert validation |
| 10 | Multi-Protocol | ✅ | Hybrid TCP/HTTP/WebSocket routing |
| 11 | CDN Caching | ✅ | InMemory, Redis, multi-tier caching |
| 12 | Service Discovery | ✅ | Consul, etcd, circuit breaker |
| 13 | GraphQL | ✅ | Schema stitching, federation |
| 14 | PHP-FPM | ✅ | FastCGI protocol, static files |
| 15 | Geographic LB | ✅ | MaxMind, IP2Location geo-routing |

See [Comprehensive Validation](docs/validation/COMPREHENSIVE_VALIDATION.md) for detailed status.

## Quick Start

### Prerequisites

- Rust 1.75+ (with cargo)
- Docker and Docker Compose (for testing)
- Linux or WSL2 (for optimal performance)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/YOUR_ORG/highper-gateway.git
cd highper-gateway/highper-gateway

# Build release binary
cargo build --release

# Binary will be at: target/release/highper-gateway
```

### Run a Simple Example

```bash
# Start a simple HTTP load balancer
./target/release/highper-gateway --config examples/configs/http-basic.toml

# Or use the DSL configuration
./target/release/highper-gateway --config examples/configs/http-basic-dsl.hcl
```

### Run Tests

```bash
# Run all 15 scenario tests
cd tests/load
bash test-scenario-01-tcp-native.sh
bash test-scenario-02-native.sh
# ... (see tests/load/ for all scenarios)

# Or use the automated test runner
for i in {01..15}; do
    timeout 120 bash test-scenario-${i}*.sh
    sleep 5
done
```

## Performance

### Benchmarks

Expected performance on modern hardware (8 vCPU, 16 GB RAM):

| Metric | Local Development | Cloud (Production) |
|--------|-------------------|-------------------|
| Max RPS | 5,000 - 10,000 | 50,000 - 100,000 |
| Concurrent Connections | 10,000 | 100,000 - 250,000 |
| P50 Latency | <2ms | <2ms |
| P95 Latency | <5ms | <5ms |
| P99 Latency | <10ms | <10ms |
| CPU Usage @ 50k RPS | N/A | <70% |
| Memory Usage | <2 GB | <8 GB |

See [Load Testing Strategy](docs/testing/LOAD_TESTING_STRATEGY.md) for detailed benchmarks and methodology.

### Comparison with Competitors

| Feature | Highper Gateway | Nginx | HAProxy | Envoy |
|---------|----------------|-------|---------|-------|
| HTTP/3 QUIC | ✅ (quiche) | ✅ | ❌ | ✅ |
| gRPC | ✅ Native | ✅ | ❌ | ✅ Native |
| GraphQL | ✅ Native | ❌ | ❌ | ❌ |
| PHP-FPM | ✅ Native | ✅ | ❌ | ❌ |
| Service Discovery | ✅ Native | ❌ | ❌ | ✅ Native |
| WAF | ✅ (4 engines) | ✅ | ❌ | ✅ |
| Memory Safety | ✅ (Rust) | ❌ (C) | ❌ (C) | ❌ (C++) |
| Configuration | TOML/HCL | Nginx conf | HAProxy conf | YAML |
| Plugin System | ✅ WASM | ✅ Native | ❌ | ✅ WASM |

## Configuration

### TOML Configuration

```toml
[server]
workers = 8
max_connections = 100000

[[listeners]]
bind = "0.0.0.0:8080"
protocol = "http"

[[upstreams]]
name = "backend"
addresses = ["127.0.0.1:8001", "127.0.0.1:8002"]
load_balancing.algorithm = "round_robin"

[[routes]]
path = "/api/*"
upstream = "backend"
```

### HCL/DSL Configuration

```hcl
server {
  workers = 8
  max_connections = 100000
}

listener "http" {
  bind = "0.0.0.0:8080"

  route "/api/*" {
    upstream = ["127.0.0.1:8001", "127.0.0.1:8002"]
    load_balancing = "round_robin"
  }
}
```

See [examples/configs/](examples/configs/) for more configuration examples.

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────┐
│                       Highper Gateway                        │
├─────────────────────────────────────────────────────────────┤
│  Listeners (HTTP/1.1, HTTP/2, HTTP/3, TCP, WebSocket)       │
├─────────────────────────────────────────────────────────────┤
│  Middleware (WAF, Rate Limit, Auth, Compression, Logging)   │
├─────────────────────────────────────────────────────────────┤
│  Routing (Path, Host, Header, Method, Geographic)           │
├─────────────────────────────────────────────────────────────┤
│  Load Balancing (Round-robin, Least-conn, IP hash, Geo)     │
├─────────────────────────────────────────────────────────────┤
│  Service Discovery (Consul, etcd, DNS, Static)              │
├─────────────────────────────────────────────────────────────┤
│  Connection Pool (HTTP, TCP, Database)                      │
├─────────────────────────────────────────────────────────────┤
│  Backends (HTTP servers, Databases, PHP-FPM, gRPC)          │
└─────────────────────────────────────────────────────────────┘
```

See [Architecture Documentation](docs/architecture/) for detailed design.

## Security

- **TLS/mTLS**: Full TLS 1.2/1.3 support with client certificate validation
- **ACME/Let's Encrypt**: Automatic certificate management
- **WAF**: Four WAF engines (ModSecurity, Coraza, AWS WAF, Custom)
- **Rate Limiting**: Token bucket, sliding window, distributed rate limiting
- **OCSP Stapling**: Automatic OCSP response caching
- **Security Headers**: Automatic security header injection
- **Input Validation**: Comprehensive request validation

See [Security Documentation](docs/architecture/SECURITY.md) for details.

## Documentation

### Core Documentation
- [Architecture](docs/architecture/) - System design and deployment patterns
- [Development](docs/development/) - Plugin development and contribution guide
- [Operations](docs/operations/) - Optimization and scaling guides
- [Validation](docs/validation/) - Implementation status and validation reports
- [Testing](docs/testing/) - Load testing strategies and results

### Quick Links
- **[Comprehensive Validation Report](docs/validation/COMPREHENSIVE_VALIDATION.md)** - Complete validation of all 15 scenarios
- **[Load Testing Strategy](docs/testing/LOAD_TESTING_STRATEGY.md)** - Local and cloud testing plans
- **[Deployment Scenarios](docs/architecture/DEPLOYMENT_SCENARIOS.md)** - Production deployment patterns
- **[Extreme Scale Optimization](docs/operations/EXTREME_SCALE_OPTIMIZATION.md)** - Million RPS optimization guide
- **[Plugin Integration Guide](docs/development/PLUGIN_INTEGRATION_GUIDE.md)** - WASM plugin development

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/YOUR_ORG/highper-gateway.git
cd highper-gateway/highper-gateway
cargo build

# Run tests
cargo test

# Run specific scenario test
cd tests/load
bash test-scenario-02-native.sh
```

## License

[Add your license here]

## Roadmap

- [ ] HTTP/4 support when specification is finalized
- [ ] Kubernetes operator for automatic deployment
- [ ] Web UI for configuration and monitoring
- [ ] AI-powered traffic analysis and DDoS protection
- [ ] Advanced circuit breaker patterns
- [ ] Multi-datacenter global load balancing
- [ ] Serverless function integration

## Support

- **Documentation**: [docs/](docs/)
- **Issues**: [GitHub Issues](https://github.com/YOUR_ORG/highper-gateway/issues)
- **Discussions**: [GitHub Discussions](https://github.com/YOUR_ORG/highper-gateway/discussions)

## Acknowledgments

- Built with [Tokio](https://tokio.rs/) async runtime
- HTTP/3 powered by [Cloudflare quiche](https://github.com/cloudflare/quiche)
- GraphQL support via [async-graphql](https://github.com/async-graphql/async-graphql)
- WAF engines: [ModSecurity](https://github.com/SpiderLabs/ModSecurity), [Coraza](https://github.com/corazawaf/coraza)

---

**Status**: Production Ready ✅ | **Version**: 1.0 | **Last Updated**: January 10, 2026
