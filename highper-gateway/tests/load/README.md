# Load Testing Framework - All 15 Use Case Scenarios

**Comprehensive load testing suite for Highper Gateway supporting local and cloud deployments.**

## Overview

This framework provides load testing scripts for all 15 use case scenarios with:
- ✅ **Local testing** via docker-compose (Windows/WSL compatible)
- ✅ **Cloud testing** via provider APIs (Vultr, PhoenixNap, Hetzner, DigitalOcean)
- ✅ **Cloud-agnostic** architecture with provider adapters
- ✅ **Baseline validation** against 207K+ req/s targets

---

## Historical Baselines

From previous testing:
- **DigitalOcean**: Achieved **207K req/s** (network-limited)
- **Current Target**: Validate improvements on local + cloud infrastructure

---

## Quick Start

### Local Testing (docker-compose)

```bash
# Test Scenario 02 (HTTP Load Balancing) locally
cd tests/load
./02-http-loadbalancer.sh --mode local

# All scenarios support local mode
./01-tcp-proxy.sh --mode local
./05-http3-quic.sh --mode local
```

### Cloud Testing (Vultr example)

```bash
# Provision infrastructure and run test on Vultr
export VULTR_API_KEY="your-api-key-here"
./02-http-loadbalancer.sh --mode cloud --provider vultr

# Supports multiple providers
./02-http-loadbalancer.sh --mode cloud --provider phoenixnap
./02-http-loadbalancer.sh --mode cloud --provider hetzner
```

---

## Architecture

### Directory Structure

```
tests/load/
├── README.md                           # This file
├── 01-tcp-proxy.sh                     # Layer 4 TCP proxying
├── 02-http-loadbalancer.sh             # HTTP/1.1 load balancing
├── 03-https-tls.sh                     # HTTPS/TLS termination
├── 04-api-gateway.sh                   # API Gateway (CORS, rate limiting)
├── 05-http3-quic.sh                    # HTTP/3 QUIC
├── 06-websocket.sh                     # WebSocket long-lived connections
├── 07-grpc-gateway.sh                  # gRPC gateway
├── 08-database-lb.sh                   # Database load balancing
├── 09-waf-mtls.sh                      # WAF + mTLS security
├── 10-multi-protocol.sh                # Hybrid multi-protocol
├── 11-cdn-caching.sh                   # CDN edge caching
├── 12-microservices.sh                 # Microservices discovery
├── 13-graphql-gateway.sh               # GraphQL gateway
├── 14-static-php.sh                    # Static + PHP-FPM
├── 15-geographic-lb.sh                 # Geographic load balancing
├── helpers/
│   ├── common.sh                       # Common functions
│   ├── cloud-provider.sh               # Cloud provider abstraction
│   ├── vegeta-helper.sh                # Vegeta wrapper
│   ├── k6-helper.sh                    # k6 wrapper
│   ├── grpcurl-helper.sh               # gRPC testing
│   ├── wscat-helper.sh                 # WebSocket testing
│   └── docker-helper.sh                # Docker/compose helpers
├── providers/
│   ├── local.sh                        # docker-compose (Windows/WSL)
│   ├── vultr.sh                        # Vultr API integration
│   ├── phoenixnap.sh                   # PhoenixNAP API integration
│   ├── hetzner.sh                      # Hetzner Cloud API
│   └── digitalocean.sh                 # DigitalOcean API (legacy)
├── docker/
│   ├── docker-compose.base.yml         # Base services
│   ├── docker-compose.scenario-01.yml  # TCP proxy
│   ├── docker-compose.scenario-02.yml  # HTTP LB
│   └── ... (one per scenario)
├── scenarios/
│   ├── vegeta/                         # Vegeta target files
│   │   ├── http-get.txt
│   │   ├── http-post.txt
│   │   ├── api-gateway.txt
│   │   └── cdn-caching.txt
│   └── k6/                             # k6 JavaScript scenarios
│       ├── websocket-load.js
│       ├── graphql-load.js
│       └── multi-protocol.js
├── configs/                            # Scenario-specific configs
│   ├── 01-tcp-proxy.yaml
│   ├── 02-http-lb.yaml
│   └── ... (one per scenario)
└── results/                            # Test results
    ├── local/                          # Local test results
    └── cloud/                          # Cloud test results
        ├── vultr/
        ├── phoenixnap/
        └── hetzner/
```

---

## Supported Tools

### Primary Tools (Installed via helpers)

| Tool | Purpose | Scenarios |
|------|---------|-----------|
| **vegeta** | HTTP/HTTPS load testing | 02, 03, 04, 09, 10, 11, 12, 14, 15 |
| **k6** | HTTP/WebSocket/GraphQL | 06, 10, 13 |
| **ghz** | gRPC benchmarking | 07, 10 |
| **iperf3** | TCP throughput | 01 |
| **sysbench** | MySQL/PostgreSQL | 08 |
| **redis-benchmark** | Redis testing | 08 |
| **h2load** | HTTP/2, HTTP/3 | 05, 07 |
| **wrk** | HTTP benchmarking | 02, 04 (optional) |

### Tool Installation

Tools are automatically installed by helper scripts when needed:

```bash
# Install all tools
./helpers/install-tools.sh

# Or install specific tool
./helpers/install-tools.sh vegeta k6
```

---

## Performance Baselines (from VALIDATION_REPORT.md)

| Scenario | Target Throughput | Target P99 Latency | Connections |
|----------|------------------|-------------------|-------------|
| 01. TCP Proxy | >1M conn/sec | <0.5ms | 100K |
| 02. HTTP LB | 600K-800K RPS | <2ms | 50K |
| 03. TLS | 500K-700K RPS | <3ms | 50K |
| 04. API Gateway | 600K-800K RPS | <2ms | 50K |
| 05. HTTP/3 | 500K-700K RPS | <1.5ms | 50K |
| 06. WebSocket | 300K msg/sec | <5ms | 1M |
| 07. gRPC | 400K-600K RPS | <1ms | 50K |
| 08. Database | 800K-1M qps | <0.8ms | 100K |
| 09. WAF+mTLS | 400K-600K RPS | <4ms | 50K |
| 10. Multi-Protocol | 600K-800K RPS | <2ms | 1M |
| 11. CDN (hit) | 2M+ RPS | <0.2ms | 50K |
| 11. CDN (miss) | 600K-800K RPS | <3ms | 50K |
| 12. Service Mesh | 500K-700K RPS | <2ms | 50K |
| 13. GraphQL | 300K-500K RPS | <5ms | 50K |
| 14. Static | 5M+ RPS | <0.5ms | 50K |
| 14. PHP | 20K-50K RPS | <50ms | 10K |
| 15. Geo LB | 600K-800K RPS | <2ms (+geo) | 50K |

---

## Cloud Provider Support

### Vultr

**API Authentication:**
```bash
export VULTR_API_KEY="your-api-key"
```

**Regions:**
- `ewr` (New Jersey, USA)
- `ord` (Chicago, USA)
- `dfw` (Dallas, USA)
- `sea` (Seattle, USA)
- `lax` (Los Angeles, USA)
- `atl` (Atlanta, USA)
- `ams` (Amsterdam, NL)
- `lhr` (London, UK)
- `fra` (Frankfurt, DE)
- `sjc` (Silicon Valley, USA)
- `syd` (Sydney, AU)
- `nrt` (Tokyo, JP)
- `sgp` (Singapore, SG)

**Instance Types:**
- `vc2-1c-1gb` - 1 vCPU, 1GB RAM ($5/mo)
- `vc2-2c-4gb` - 2 vCPU, 4GB RAM ($12/mo)
- `vc2-4c-8gb` - 4 vCPU, 8GB RAM ($24/mo)
- `vc2-8c-16gb` - 8 vCPU, 16GB RAM ($48/mo)
- `vhf-2c-4gb` - 2 vCPU, 4GB RAM, High Frequency ($18/mo)
- `vhf-4c-8gb` - 4 vCPU, 8GB RAM, High Frequency ($36/mo)

### PhoenixNAP

**API Authentication:**
```bash
export PHOENIXNAP_CLIENT_ID="your-client-id"
export PHOENIXNAP_CLIENT_SECRET="your-client-secret"
```

**Regions:**
- `PHX` (Phoenix, USA)
- `ASH` (Ashburn, USA)
- `SGP` (Singapore)
- `NLD` (Amsterdam, NL)
- `CHI` (Chicago, USA)
- `SEA` (Seattle, USA)

**Instance Types:**
- `s0.d1.small` - 1 vCPU, 1GB RAM
- `s1.c1.small` - 2 vCPU, 4GB RAM
- `s1.c1.medium` - 4 vCPU, 8GB RAM
- `s2.c1.medium` - 8 vCPU, 16GB RAM
- Bare Metal available for high-performance testing

### Hetzner Cloud

**API Authentication:**
```bash
export HCLOUD_TOKEN="your-token"
```

**Regions:**
- `nbg1` (Nuremberg, DE)
- `fsn1` (Falkenstein, DE)
- `hel1` (Helsinki, FI)
- `ash` (Ashburn, USA)
- `hil` (Hillsboro, USA)

**Instance Types:**
- `cx11` - 1 vCPU, 2GB RAM (€3.85/mo)
- `cx21` - 2 vCPU, 4GB RAM (€5.83/mo)
- `cx31` - 2 vCPU, 8GB RAM (€10.90/mo)
- `cx41` - 4 vCPU, 16GB RAM (€18.54/mo)
- `cx51` - 8 vCPU, 32GB RAM (€33.81/mo)
- `ccx13` - 2 vCPU, 8GB RAM (Dedicated, €15.30/mo)
- `ccx23` - 4 vCPU, 16GB RAM (Dedicated, €30.60/mo)

---

## Script Features

Each load testing script includes:

### 1. **Dual-Mode Operation**
- `--mode local`: Uses docker-compose (no cloud costs)
- `--mode cloud`: Provisions via provider API

### 2. **Progressive Load Testing**
- Warm-up: 10 seconds
- Load levels: 100, 1K, 10K, 50K, 100K, 200K+ RPS
- Duration: 60s per level
- Cooldown between levels

### 3. **Metrics Collection**
- Throughput (req/s, msg/s, conn/s)
- Latency (P50, P95, P99, P99.9, max)
- Error rate (timeouts, connection errors)
- Resource usage (CPU, memory, network)
- Protocol-specific metrics

### 4. **Baseline Validation**
- Compares against targets from VALIDATION_REPORT.md
- Pass/Fail criteria per scenario
- Regression detection
- Performance trend analysis

### 5. **Results Output**
- JSON for CI/CD integration
- Text summary for human review
- Grafana/Prometheus compatible
- CSV for spreadsheet analysis
- vegeta binary (.bin) for detailed analysis

### 6. **Cleanup & Recovery**
- Automatic cleanup on exit
- Graceful shutdown of services
- Resource deallocation (cloud instances)
- Result archival

---

## Usage Examples

### Example 1: Local HTTP Load Balancing Test

```bash
cd tests/load

# Run Scenario 02 locally with docker-compose
./02-http-loadbalancer.sh --mode local

# Output:
# [INFO] Starting Scenario 02: HTTP Load Balancing (Local)
# [INFO] Starting 3 backend servers via docker-compose
# [INFO] Starting Highper Gateway
# [INFO] Running progressive load test
# [INFO] Testing at 100 req/s... PASS (P99: 0.8ms)
# [INFO] Testing at 1,000 req/s... PASS (P99: 1.2ms)
# [INFO] Testing at 10,000 req/s... PASS (P99: 1.8ms)
# [INFO] Testing at 50,000 req/s... PASS (P99: 2.1ms)
# [SUCCESS] All load levels passed
# [INFO] Results saved to: results/local/02-http-loadbalancer/
```

### Example 2: Cloud HTTP/3 QUIC Test (Vultr)

```bash
export VULTR_API_KEY="your-api-key"

# Run Scenario 05 on Vultr with 4 instances
./05-http3-quic.sh \
    --mode cloud \
    --provider vultr \
    --region ewr \
    --instances 4 \
    --instance-type vhf-4c-8gb

# Output:
# [INFO] Provisioning 4 instances on Vultr (ewr, vhf-4c-8gb)
# [INFO] Instances created: 3x backend, 1x gateway
# [INFO] Deploying Highper Gateway binary
# [INFO] Configuring HTTP/3 QUIC
# [INFO] Running load test from client machines
# [INFO] Testing at 100,000 req/s... PASS (P99: 1.4ms)
# [INFO] Testing at 200,000 req/s... PASS (P99: 1.7ms)
# [SUCCESS] Target: 500K-700K RPS achieved: 542K RPS
# [INFO] Cleaning up instances
# [INFO] Total cost: $0.32 (48 minutes)
```

### Example 3: Geographic Load Balancing (Multi-Region)

```bash
export HCLOUD_TOKEN="your-token"

# Run Scenario 15 across 3 geographic regions
./15-geographic-lb.sh \
    --mode cloud \
    --provider hetzner \
    --regions "nbg1,ash,hel1" \
    --geoip-db GeoLite2-City.mmdb

# Output:
# [INFO] Deploying to 3 regions: Nuremberg, Ashburn, Helsinki
# [INFO] Testing geographic routing accuracy
# [INFO] US clients -> Ashburn backend (95% accuracy)
# [INFO] EU clients -> Nuremberg backend (97% accuracy)
# [INFO] Asia clients -> Nuremberg backend (fallback, 89% accuracy)
# [SUCCESS] Geographic routing working as expected
```

---

## Environment Variables

### Required for Cloud Testing

```bash
# Vultr
export VULTR_API_KEY="your-api-key"

# PhoenixNAP
export PHOENIXNAP_CLIENT_ID="your-client-id"
export PHOENIXNAP_CLIENT_SECRET="your-client-secret"

# Hetzner Cloud
export HCLOUD_TOKEN="your-token"

# DigitalOcean (legacy)
export DIGITALOCEAN_TOKEN="your-token"
```

### Optional Configuration

```bash
# Override default settings
export LOAD_TEST_DURATION=120          # Test duration per level (seconds)
export LOAD_TEST_WARMUP=30             # Warm-up duration (seconds)
export LOAD_TEST_MAX_RPS=500000        # Maximum RPS to test
export LOAD_TEST_RESULTS_DIR="./my-results"
export LOAD_TEST_KEEP_INSTANCES=true   # Don't cleanup cloud instances
```

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Load Testing

on:
  schedule:
    - cron: '0 2 * * 0'  # Weekly on Sunday 2 AM
  workflow_dispatch:

jobs:
  load-test-local:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        scenario: [
          '02-http-loadbalancer',
          '04-api-gateway',
          '11-cdn-caching'
        ]
    steps:
      - uses: actions/checkout@v3

      - name: Install Docker Compose
        run: |
          sudo apt-get update
          sudo apt-get install -y docker-compose

      - name: Run Load Test
        run: |
          cd highper-gateway/tests/load
          ./${{ matrix.scenario }}.sh --mode local --ci

      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: load-test-results-${{ matrix.scenario }}
          path: highper-gateway/tests/load/results/local/${{ matrix.scenario }}/

  load-test-cloud:
    runs-on: ubuntu-latest
    if: github.event_name == 'workflow_dispatch'
    steps:
      - uses: actions/checkout@v3

      - name: Run Cloud Load Test
        env:
          VULTR_API_KEY: ${{ secrets.VULTR_API_KEY }}
        run: |
          cd highper-gateway/tests/load
          ./02-http-loadbalancer.sh \
            --mode cloud \
            --provider vultr \
            --region ewr \
            --instances 4
```

---

## Troubleshooting

### WSL/Windows Issues

**Issue:** Docker networking not working in WSL2
```bash
# Solution: Restart Docker Desktop and WSL
wsl --shutdown
# Then restart Docker Desktop
```

**Issue:** Port conflicts on Windows
```bash
# Solution: Check which process is using the port
netstat -ano | findstr :8080
# Kill the process or change test port
export LOAD_TEST_PORT=8081
```

### Cloud Provider Issues

**Issue:** Vultr API rate limit exceeded
```bash
# Solution: Add delays between API calls
export VULTR_API_DELAY=2  # 2 seconds between calls
```

**Issue:** Hetzner Cloud quota exceeded
```bash
# Solution: Check quota and request increase
hcloud quota list
# Or test with fewer instances
./02-http-loadbalancer.sh --instances 2
```

---

## Performance Tips

### Local Testing (docker-compose)

1. **Increase Docker resources:**
   - Docker Desktop → Settings → Resources
   - CPUs: 6-8 (if available)
   - Memory: 8-12 GB
   - Swap: 2 GB

2. **Use host networking (Linux only):**
   ```bash
   export DOCKER_NETWORK_MODE=host
   ```

3. **Disable unnecessary logging:**
   ```bash
   export LOAD_TEST_VERBOSE=false
   ```

### Cloud Testing

1. **Use dedicated instances for high RPS:**
   - Vultr: High Frequency instances
   - Hetzner: Dedicated vCPU (ccx series)
   - PhoenixNAP: Bare Metal

2. **Select regions with low latency:**
   ```bash
   # Test latency to regions first
   ./helpers/test-region-latency.sh vultr
   ```

3. **Use multiple load generators:**
   ```bash
   ./02-http-loadbalancer.sh --load-generators 4
   ```

---

## Cost Estimation

### Local Testing
- **Cost:** $0 (uses local Docker)
- **Duration:** 10-20 minutes per scenario
- **Resource Usage:** ~4 CPU cores, ~8GB RAM

### Cloud Testing (Vultr example)

| Scenario | Instances | Duration | Est. Cost |
|----------|-----------|----------|-----------|
| Basic HTTP | 4x vc2-2c-4gb | 30 min | ~$0.25 |
| High RPS | 4x vhf-4c-8gb | 60 min | ~$0.75 |
| Multi-Region | 9x instances (3 regions) | 45 min | ~$1.10 |
| Full Suite (15 scenarios) | Varies | 8 hours | ~$8-12 |

**Note:** Costs are estimates based on hourly pricing. Actual costs may vary.

---

## Next Steps

1. **Start with local testing:**
   ```bash
   ./02-http-loadbalancer.sh --mode local
   ```

2. **Validate baselines locally**

3. **Run cloud tests for high RPS validation:**
   ```bash
   ./02-http-loadbalancer.sh --mode cloud --provider vultr
   ```

4. **Analyze results and iterate**

5. **Integrate into CI/CD**

---

## Support & Documentation

- **Full Documentation:** `../../docs/VALIDATION_REPORT.md`
- **Configuration Examples:** `../../examples/configs/yaml/`
- **Issues:** [GitHub Issues](https://github.com/highperapp/highper-gateway/issues)

---

**Framework Version:** 1.0.0
**Last Updated:** 2025-12-25
**Tested On:** Windows/WSL2, Ubuntu 22.04, Vultr, Hetzner Cloud
