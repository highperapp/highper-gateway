# Load Testing Framework - Quick Start Guide

## Overview

This load testing framework supports all 15 Highper Gateway scenarios with dual-mode operation:

- **Local Mode**: Docker Compose on Windows/WSL (development, quick iteration)
- **Cloud Mode**: Vultr, PhoenixNAP, Hetzner via APIs (production-scale testing)

**Historical Baseline**: 207K req/s achieved on DigitalOcean

## Prerequisites

### Local Testing
- Docker & Docker Compose
- Bash shell (WSL on Windows)
- Tools: `vegeta`, `jq`, `iperf3`

### Cloud Testing
- API keys for your chosen provider:
  - `export VULTR_API_KEY=your_key`
  - `export PHOENIXNAP_API_KEY=your_key`
  - `export HETZNER_API_KEY=your_key`

## Quick Start

### Local Testing (Docker Compose)

```bash
# Scenario 01: TCP Proxy
cd tests/load
./scenarios/01-tcp-proxy.sh

# Scenario 02: HTTP Load Balancer (targets 200K+ req/s)
./scenarios/02-http-loadbalancer.sh

# View results
ls -lh results/local/02-http-loadbalancer/
cat results/local/02-http-loadbalancer/*/summary.txt
```

### Cloud Testing (Vultr)

```bash
# Set API key
export VULTR_API_KEY=your_vultr_api_key

# Run on Vultr cloud infrastructure
export LOAD_TEST_MODE=vultr
./scenarios/02-http-loadbalancer.sh

# Results saved to results/cloud/vultr/02-http-loadbalancer/
```

## Configuration

### Environment Variables

```bash
# Mode selection
export LOAD_TEST_MODE=local              # local, vultr, phoenixnap, hetzner

# Test parameters
export LOAD_TEST_DURATION=60             # Test duration (seconds)
export LOAD_TEST_WARMUP=10               # Warmup duration (seconds)
export LOAD_TEST_RATE_MAX=500000         # Maximum rate (req/s)

# Infrastructure
export LOAD_TEST_KEEP_INSTANCES=false    # Keep cloud instances after test
export LOAD_TEST_VERBOSE=true            # Verbose logging

# Provider-specific
export VULTR_REGION=ewr                  # Vultr region (optional, auto-selects)
export VULTR_PLAN=vc2-8c-16gb           # Vultr plan (optional, auto-selects)
```

### Scenario-Specific Options

#### Scenario 01: TCP Proxy
```bash
export LOAD_TEST_TCP_CONNECTIONS=10000   # Parallel connections
```

#### Scenario 02: HTTP Load Balancer
```bash
export LOAD_TEST_RATE_START=10000        # Starting rate (req/s)
export LOAD_TEST_RATE_STEP=20000         # Rate increment
export LOAD_TEST_ENDPOINT=/api/ping      # Test endpoint
```

## Framework Structure

```
tests/load/
├── README.md                    # Comprehensive documentation
├── QUICKSTART.md               # This file
│
├── scenarios/                   # Test scenarios (15 total)
│   ├── 01-tcp-proxy.sh         # Layer 4 TCP proxying
│   ├── 02-http-loadbalancer.sh # HTTP/1.1 load balancing
│   └── ...                     # (03-15 to be created)
│
├── docker/                      # Docker infrastructure
│   ├── docker-compose.base.yml # Base services (gateway, backends, redis, postgres)
│   ├── docker-compose.scenario-*.yml  # Scenario-specific overlays
│   │
│   ├── backends/
│   │   ├── rust-http/          # Ultra-fast Rust HTTP backend
│   │   └── node-api/           # Node.js API backend
│   │
│   ├── configs/
│   │   └── gateway.toml        # Gateway configuration
│   │
│   └── static/                 # Static test files
│
├── providers/                   # Cloud provider adapters
│   ├── provider-interface.sh   # Abstract provider API
│   ├── vultr.sh                # Vultr implementation
│   ├── phoenixnap.sh           # PhoenixNAP (to be created)
│   └── hetzner.sh              # Hetzner (to be created)
│
├── helpers/
│   └── common.sh               # Shared helper functions
│
└── results/                    # Test results (gitignored)
    ├── local/
    └── cloud/
        ├── vultr/
        ├── phoenixnap/
        └── hetzner/
```

## Backend Architecture

### Why Rust Backends?

Based on your feedback, we use **ultra-fast Rust HTTP backends** instead of nginx:

- **Minimal overhead**: Bare-metal performance for pure gateway testing
- **Stack alignment**: Rust gateway + Rust backends = consistent stack
- **Past experience**: You mentioned "rust backend for hello world" worked well
- **Control**: Full control over response behavior and headers

### Backend Features

The Rust backends (`tests/load/docker/backends/rust-http`) provide:

- **Health checks**: `/health` endpoint
- **Variable response sizes**: `/small` (100B), `/medium` (10KB), `/large` (100KB)
- **All HTTP methods**: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS
- **Backend identification**: `X-Backend-Host`, `X-Request-ID` headers
- **CORS support**: Full preflight and headers

### Gateway Configuration Highlights

The gateway config (`tests/load/docker/configs/gateway.toml`) includes:

1. **Dynamic IPs/Ports**: Environment variable support for cloud deployments
   ```toml
   host = "${BACKEND_HTTP_1_HOST:-172.20.0.20}"
   port = "${BACKEND_HTTP_1_PORT:-8000}"
   ```

2. **All HTTP Methods**: Scenarios 02 and 04 support full REST
   ```toml
   methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
   ```

3. **Compression**: Includes msgpack, excludes gRPC (protocol-level compression)
   ```toml
   types = ["application/json", "application/msgpack", ...]
   ```

4. **Multi-Tier Headers**: X-Forwarded-* for TCP LB → HTTP LB → Server chains
   ```toml
   allow_headers = ["X-Forwarded-For", "X-Forwarded-Proto", "X-Forwarded-Host", "X-Real-IP"]
   ```

## Performance Targets

Based on `docs/VALIDATION_REPORT.md` and historical data:

| Scenario | Target | Historical |
|----------|--------|------------|
| 01 - TCP Proxy | 500K+ conn/s | - |
| 02 - HTTP LB | 200K+ req/s | **207K req/s (DigitalOcean)** |
| 03 - HTTPS/TLS | 150K+ req/s | - |
| 04 - API Gateway | 100K+ req/s | - |
| 05 - HTTP/3 | 100K+ req/s | - |

## Cloud Provider Comparison

### Vultr
- **API**: Mature, well-documented
- **Regions**: Global coverage
- **Plans**: Flexible, high-performance options
- **Status**: ✅ Implemented

### PhoenixNAP
- **API**: Enterprise-grade
- **Servers**: Dedicated, bare-metal
- **Performance**: Excellent for high-throughput testing
- **Status**: 🔨 To be implemented

### Hetzner
- **API**: Simple, effective
- **Cost**: Competitive pricing
- **Performance**: Good CPU performance
- **Status**: 🔨 To be implemented

## Common Tasks

### Running All Scenarios Locally

```bash
# Run all implemented scenarios
for scenario in scenarios/*.sh; do
    echo "Running $scenario..."
    bash "$scenario"
done
```

### Comparing Cloud Providers

```bash
# Test on multiple providers
for provider in vultr phoenixnap hetzner; do
    export LOAD_TEST_MODE=$provider
    ./scenarios/02-http-loadbalancer.sh
done

# Compare results
diff results/cloud/vultr/*/summary.txt results/cloud/phoenixnap/*/summary.txt
```

### Debugging

```bash
# Verbose mode
export LOAD_TEST_VERBOSE=true

# Check Docker logs
docker-compose -f docker/docker-compose.base.yml logs gateway
docker-compose -f docker/docker-compose.base.yml logs backend-http-1

# Keep cloud instances for investigation
export LOAD_TEST_KEEP_INSTANCES=true
```

## Next Steps

1. **Review results**: Check `results/` directory
2. **Compare with baseline**: Did you beat 207K req/s?
3. **Tune parameters**: Adjust rates, durations for your use case
4. **Scale up**: Try cloud providers for production-scale testing
5. **Create scenarios 03-15**: Extend framework for remaining scenarios

## Troubleshooting

### Docker Compose Fails

```bash
# Check Docker is running
docker ps

# Rebuild containers
docker-compose -f docker/docker-compose.base.yml build --no-cache
```

### Vegeta Not Found

```bash
# Install vegeta
go install github.com/tsenart/vegeta@latest
# Or
./helpers/install-tools.sh vegeta
```

### Cloud API Errors

```bash
# Verify API key
echo $VULTR_API_KEY

# Test API manually
curl -H "Authorization: Bearer $VULTR_API_KEY" https://api.vultr.com/v2/account
```

## Support

- **Documentation**: See `tests/load/README.md` for comprehensive details
- **Validation Report**: See `docs/VALIDATION_REPORT.md` for implementation status
- **Security**: See `docs/SECURITY.md` for security features tested

---

**Ready to beat 207K req/s?** 🚀

Start with local testing, tune your configuration, then scale to cloud providers!
