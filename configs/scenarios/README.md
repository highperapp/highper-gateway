# Load Testing Scenarios for Highper Gateway

This directory contains 15 comprehensive load testing scenarios covering all major use cases and protocols supported by Highper Gateway.

## Scenario Overview

| # | Scenario | Protocol | Target RPS | Target Connections | Focus |
|---|----------|----------|------------|--------------------|-------|
| 01 | Layer 4 TCP Load Balancer | TCP | 600K+ | 3M+ | Pure TCP proxying |
| 02 | Layer 7 HTTP | HTTP/1.1 | 600K+ | 3M+ | HTTP load balancing |
| 03 | Layer 7 HTTPS/TLS Termination | HTTPS | 500K+ | 2M+ | TLS termination overhead |
| 04 | API Gateway | HTTPS | 500K+ | 1.5M+ | RESTful APIs with CORS, rate limiting |
| 05 | HTTP/3 (QUIC) Multi-Protocol | HTTP/3, HTTP/2, HTTP/1.1 | 400K+ | 1M+ | QUIC performance |
| 06 | WebSocket Load Balancer | WebSocket | 100K msg/sec | 2M+ | Long-lived connections |
| 07 | gRPC Gateway | gRPC/HTTP/2 | 300K+ RPC/sec | 1M+ | Bidirectional streaming |
| 08 | Database Load Balancer | TCP (MySQL/PostgreSQL) | 500K+ queries/sec | 1M+ | Connection pooling |
| 09 | Secure API with WAF + mTLS | HTTPS + mTLS | 400K+ | 1M+ | Security overhead |
| 10 | Hybrid Multi-Protocol | All protocols | 500K+ | 2M+ | Protocol diversity |
| 11 | CDN Edge with Caching | HTTPS, HTTP/3 | 800K+ | 2M+ | Cache hit ratio |
| 12 | Microservices with Discovery | HTTPS, HTTP/2 | 450K+ | 1.5M+ | Circuit breaker, retry |
| 13 | GraphQL Gateway | GraphQL/HTTPS | 200K+ queries/sec | 1M+ | Query complexity |
| 14 | Static + PHP-FPM | HTTPS, FastCGI | 600K+ (static), 150K+ (PHP) | 1.5M+ | Hybrid serving |
| 15 | Geographic Load Balancer | HTTPS, HTTP/3 | 550K+ | 2M+ | Geo-routing, latency |

## Configuration Format

All scenarios use Highper Gateway's DSL configuration syntax, which is more concise than YAML while remaining readable:

```
https://example.com {
    proxy backend1:8080 backend2:8080
    lb least_conn
    health interval=10s path="/health"
    compress gzip
}
```

## Backend Placeholders

All scenario files use placeholder backend addresses:
- `BACKEND_1:8080`
- `BACKEND_2:8080`
- `BACKEND_3:8080`

The deployment script (`scripts/loadtest/vultr/deploy.sh`) will automatically replace these placeholders with actual backend server IPs during deployment.

## Usage

### Manual Deployment

```bash
# Copy a scenario to the proxy server
scp configs/scenarios/scenario-02-layer7-http.yaml proxy:/etc/highper/config.yaml

# Restart the gateway
ssh proxy "systemctl restart highper-gateway"
```

### Automated Load Testing

```bash
# Run a specific scenario (automated deployment)
./scripts/loadtest/run.sh vultr scenario-02

# This will:
# 1. Provision infrastructure on Vultr
# 2. Deploy Rust fast-backend to backend servers
# 3. Replace BACKEND_* placeholders with actual IPs
# 4. Deploy Highper Gateway with the scenario config
# 5. Run load tests with vegeta/wrk2
# 6. Collect results and metrics
# 7. Cleanup infrastructure
```

## Configuration Features

### Common Features Across All Scenarios

- **Load Balancing Algorithms**: `round_robin`, `least_conn`, `ip_hash`
- **Health Checks**: Configurable intervals, paths, and protocols
- **Connection Pooling**: Min/max idle connections, idle timeouts
- **Rate Limiting**: Global, per-IP, per-cert, per-region
- **Compression**: gzip, brotli, qpack (HTTP/3)
- **Metrics**: Prometheus endpoint on port 9090
- **Buffer Pooling**: Pre-allocated buffers for zero-copy operations
- **Backpressure**: Memory and connection limits

### Scenario-Specific Features

#### Scenario 01-03: Core Load Balancing
- Pure TCP proxying (Scenario 01)
- HTTP/1.1 with keepalive (Scenario 02)
- TLS termination and HTTP/2 (Scenario 03)

#### Scenario 04-07: API and Streaming
- CORS and API authentication (Scenario 04)
- QUIC/HTTP/3 support (Scenario 05)
- WebSocket sticky sessions (Scenario 06)
- gRPC bidirectional streaming (Scenario 07)

#### Scenario 08-10: Specialized Proxying
- Database connection pooling (Scenario 08)
- WAF rules and mTLS (Scenario 09)
- Multi-protocol hybrid (Scenario 10)

#### Scenario 11-15: Advanced Features
- Cache with 32GB capacity (Scenario 11)
- Circuit breaker and retry (Scenario 12)
- GraphQL query batching (Scenario 13)
- FastCGI to PHP-FPM (Scenario 14)
- GeoIP routing (Scenario 15)

## System Optimizations

All scenarios assume the following system optimizations are applied (via `scripts/loadtest/common/system-tuning.sh`):

- `net.ipv4.tcp_tw_reuse = 1` - TIME_WAIT socket reuse
- `net.ipv4.tcp_fin_timeout = 15` - Reduced TIME_WAIT duration
- `net.core.somaxconn = 65535` - Connection backlog
- `fs.file-max = 10000000` - File descriptor limit
- `net.ipv4.tcp_congestion_control = bbr` - BBR congestion control
- `net.ipv4.ip_local_port_range = 1024 65535` - Port range

See `docs/SYSTEM_OPTIMIZATIONS.md` for complete details.

## Expected Results

### Performance Targets

| Metric | Conservative | Target | Stretch |
|--------|--------------|--------|---------|
| **RPS (HTTP)** | 400K | 600K | 800K+ |
| **Connections** | 1M | 2M | 3M+ |
| **P50 Latency** | < 2ms | < 1ms | < 0.5ms |
| **P99 Latency** | < 15ms | < 10ms | < 5ms |
| **P99.9 Latency** | < 50ms | < 25ms | < 15ms |
| **Error Rate** | < 0.1% | < 0.01% | < 0.001% |
| **CPU (Proxy)** | 70% | 60% | 50% |
| **Memory (Proxy)** | 60% | 50% | 40% |

### Infrastructure Sizing

See `docs/RESOURCE_SIZING_GUIDE.md` for:
- Baseline high-end configuration
- Right-sizing based on utilization
- Downsizing paths for cost optimization
- Scenario-specific recommendations

## Results Documentation

After each test, results will be saved to `results/load-test-<timestamp>/`:

```
results/load-test-20250127-120000/
├── metadata.json                 # Infrastructure specs, versions
├── summary.txt                   # Human-readable summary
├── metrics/
│   ├── prometheus/              # Prometheus snapshots
│   ├── grafana/                 # Dashboard exports (PNG)
│   └── system-stats.json        # CPU, memory, network
├── vegeta/
│   ├── generator-1-results.bin  # Raw vegeta results
│   ├── generator-1-report.txt   # Vegeta text report
│   └── generator-1-metrics.json # Vegeta JSON metrics
└── logs/
    ├── gateway.log              # Gateway logs
    ├── backend-1.log            # Backend logs
    └── deployment.log           # Deployment logs
```

## Validation

To validate that a scenario configuration is syntactically correct:

```bash
# Parse DSL (when DSL parser is ready)
highper-gateway --validate-config configs/scenarios/scenario-02-layer7-http.yaml

# Convert to internal format
highper-gateway --convert-dsl configs/scenarios/scenario-02-layer7-http.yaml
```

## Contributing

When adding new scenarios:

1. Follow the naming convention: `scenario-NN-description.yaml`
2. Include comprehensive comments explaining the scenario purpose
3. Set realistic performance targets based on protocol overhead
4. Document any special requirements (certificates, databases, etc.)
5. Update this README with the new scenario details

## References

- [Highper Gateway Documentation](../../README.md)
- [DSL Syntax Guide](../../highper-gateway/src/config/dsl_parser.rs)
- [Load Testing Guide](../../scripts/loadtest/README.md)
- [System Optimizations](../../docs/SYSTEM_OPTIMIZATIONS.md)
- [Resource Sizing Guide](../../docs/RESOURCE_SIZING_GUIDE.md)
