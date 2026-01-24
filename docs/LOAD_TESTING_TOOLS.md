# Load Testing Tools & Infrastructure Guide

## Comprehensive Guide to Load Testing Tools for Highper Gateway

**Last Updated:** January 11, 2026
**Purpose:** Document all tools used and aspired for load testing various protocols and use cases
**Audience:** DevOps, QA Engineers, Performance Testers

---

## Table of Contents

1. [Currently Used Tools](#currently-used-tools)
2. [Aspired/Future Tools](#aspiredfuture-tools)
3. [Protocol-Specific Tools](#protocol-specific-tools)
4. [Infrastructure Setup](#infrastructure-setup)
5. [Standalone Testbed Project Proposal](#standalone-testbed-project-proposal)
6. [Tool Comparison Matrix](#tool-comparison-matrix)
7. [Installation & Setup](#installation--setup)
8. [Best Practices](#best-practices)

---

## Currently Used Tools

### 1. **Vegeta** (Primary HTTP Load Generator)

**Purpose:** HTTP/1.1 and HTTP/2 load testing
**License:** MIT
**Language:** Go
**Version Used:** v12.11.1

**Capabilities:**
- ✅ HTTP/1.1 and HTTP/2 support
- ✅ Constant rate testing (1K-10K RPS)
- ✅ Custom headers and payloads
- ✅ Detailed metrics (P50, P95, P99 latency)
- ✅ Binary and text output formats
- ✅ JSON report generation
- ❌ No HTTP/3 support
- ❌ No WebSocket support

**Usage in Our Tests:**
```bash
# Attack at 5000 req/s for 10 seconds
echo "GET http://localhost:9000/api/ping" | vegeta attack \
    -rate=5000 \
    -duration=10s \
    -timeout=5s \
    -workers=8 \
    -keepalive=true \
    > results.bin

# Generate reports
vegeta report -type=json < results.bin > results.json
vegeta report -type=text < results.bin > results.txt
```

**Scenarios Using Vegeta:**
- Scenario 1: TCP Proxying (via HTTP)
- Scenario 2: HTTP/1.1 Load Balancing
- Scenario 3: HTTPS/TLS Termination
- Scenario 4: API Gateway with Rate Limiting
- Scenario 11: CDN Edge Caching

**Pros:**
- ✅ Simple, single binary
- ✅ Very fast (Go concurrency)
- ✅ Accurate constant-rate testing
- ✅ Great reporting

**Cons:**
- ❌ HTTP only (no gRPC, WebSocket, etc.)
- ❌ Limited scripting (no JS/Lua)
- ❌ No HTTP/3 support

---

### 2. **cURL** (Protocol Testing)

**Purpose:** Manual testing, protocol validation
**License:** MIT-style
**Language:** C
**Version:** 7.x-8.x

**Capabilities:**
- ✅ HTTP/1.1, HTTP/2, HTTP/3 support
- ✅ TLS/mTLS testing
- ✅ Custom headers
- ✅ WebSocket upgrade (basic)
- ✅ Certificate validation
- ✅ Verbose debugging

**Usage in Our Tests:**
```bash
# HTTP/3 testing
curl --http3 -v https://localhost:8443/api/ping

# mTLS testing
curl --cert client.crt --key client.key \
     --cacert ca.crt \
     https://localhost:8443/secure

# gRPC-Web testing
curl -X POST https://localhost:9000/grpc.Service/Method \
     -H "Content-Type: application/grpc-web"
```

**Scenarios Using cURL:**
- Scenario 3: TLS validation
- Scenario 5: HTTP/3 QUIC validation
- Scenario 9: WAF + mTLS testing

**Pros:**
- ✅ Universal availability
- ✅ Excellent protocol support
- ✅ Great for debugging

**Cons:**
- ❌ Not for high-load testing
- ❌ Manual/scripted only

---

### 3. **Docker & Docker Compose** (Backend Infrastructure)

**Purpose:** Test backend services
**License:** Apache 2.0
**Version:** 24.x

**Capabilities:**
- ✅ Isolated test environments
- ✅ Multi-container orchestration
- ✅ Network isolation
- ✅ Resource limits
- ✅ Health checks

**Usage in Our Tests:**
```yaml
# docker-compose-prebuilt.yml
version: '3.8'
services:
  backend-1:
    image: docker-backend-http-1
    ports:
      - "8001:8000"
  backend-2:
    image: docker-backend-http-2
    ports:
      - "8002:8000"
  backend-3:
    image: docker-backend-http-3
    ports:
      - "8003:8000"
```

**Scenarios Using Docker:**
- All scenarios (backend services)
- Database backends (MySQL, PostgreSQL, Redis)
- PHP-FPM containers
- WebSocket servers
- gRPC servers

---

### 4. **jq** (JSON Processing)

**Purpose:** Parse and analyze test results
**License:** MIT
**Language:** C
**Version:** 1.7.1

**Usage:**
```bash
# Extract P99 latency from vegeta results
cat results.json | jq -r '.latencies.p99'

# Calculate success rate
cat results.json | jq -r '(.success_ratio * 100)'

# Pretty print results
cat results.json | jq '.'
```

---

### 5. **Bash Scripts** (Test Orchestration)

**Purpose:** Test scenario automation
**License:** GPL

**Current Architecture:**
```
tests/load/
├── run-all-scenarios.sh       # Master test runner
├── test-scenario-01-tcp.sh    # Individual scenario
├── test-scenario-02-http.sh
├── ...
├── helpers/
│   └── common.sh              # Shared utilities
└── docker/
    └── docker-compose-*.yml
```

**Capabilities:**
- Sequential test execution
- Cleanup and teardown
- Result aggregation
- Timeout protection

---

## Aspired/Future Tools

### 1. **k6** (Modern Load Testing)

**Why We Need It:**
- ✅ JavaScript-based scripting (flexible scenarios)
- ✅ HTTP/1.1, HTTP/2, WebSocket, gRPC support
- ✅ Built-in metrics and thresholds
- ✅ Cloud integration (k6 Cloud)
- ✅ Better than vegeta for complex scenarios

**License:** AGPL v3 (core), Commercial (cloud)
**Language:** Go
**Website:** https://k6.io

**Proposed Usage:**
```javascript
// k6-http-scenario.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
  stages: [
    { duration: '30s', target: 1000 },  // Ramp up
    { duration: '1m', target: 5000 },   // Stay at 5K
    { duration: '30s', target: 0 },     // Ramp down
  ],
};

export default function () {
  let res = http.get('http://localhost:9000/api/ping');
  check(res, {
    'status is 200': (r) => r.status === 200,
    'response time < 5ms': (r) => r.timings.duration < 5,
  });
}
```

**Priority:** P1 (High)
**Estimated Implementation:** 1-2 weeks

---

### 2. **ghz** (gRPC Load Testing)

**Why We Need It:**
- ✅ Native gRPC load testing
- ✅ Supports streaming (unary, client, server, bidirectional)
- ✅ Similar to vegeta but for gRPC
- ✅ Detailed metrics

**License:** Apache 2.0
**Language:** Go
**Website:** https://ghz.sh

**Proposed Usage:**
```bash
# gRPC load test
ghz --insecure \
    --proto ./api.proto \
    --call grpc.Service.Method \
    -d '{"name":"test"}' \
    -c 100 \
    -n 10000 \
    localhost:9000
```

**Priority:** P2 (Medium-High)
**Current Status:** Missing from environment (exit code 125 in tests)

---

### 3. **h2load** (HTTP/2 Benchmarking)

**Why We Need It:**
- ✅ Part of nghttp2 project
- ✅ Excellent HTTP/2 load testing
- ✅ Supports HTTP/3 (with QUIC)
- ✅ Multiplexing testing

**License:** MIT
**Language:** C++
**Website:** https://nghttp2.org/documentation/h2load.1.html

**Proposed Usage:**
```bash
# HTTP/2 load test
h2load -n 100000 -c 100 -t 4 https://localhost:9000/

# HTTP/3 load test
h2load --h3 -n 100000 -c 100 https://localhost:8443/
```

**Priority:** P2 (Medium-High)
**Use Cases:** HTTP/2 and HTTP/3 specific testing

---

### 4. **wrk** (HTTP Benchmarking)

**Why We Need It:**
- ✅ Lua scripting for complex scenarios
- ✅ Very high performance (C + LuaJIT)
- ✅ Custom request generation
- ✅ Better than vegeta for scripted tests

**License:** Apache 2.0
**Language:** C + Lua
**Website:** https://github.com/wg/wrk

**Proposed Usage:**
```bash
# Simple benchmark
wrk -t12 -c400 -d30s http://localhost:9000/api/ping

# With Lua script
wrk -t12 -c400 -d30s -s script.lua http://localhost:9000/
```

**Priority:** P2 (Medium)

---

### 5. **Gatling** (Complex Scenario Testing)

**Why We Need It:**
- ✅ Complex user journey testing
- ✅ Scala/Java DSL for scenarios
- ✅ Excellent reporting (HTML)
- ✅ Distributed load testing
- ✅ CI/CD integration

**License:** Apache 2.0
**Language:** Scala/Java
**Website:** https://gatling.io

**Proposed Usage:**
```scala
// User journey simulation
class APIGatewaySimulation extends Simulation {
  val httpProtocol = http
    .baseUrl("http://localhost:9000")

  val scn = scenario("API Gateway Test")
    .exec(http("List Products")
      .get("/api/products"))
    .pause(1)
    .exec(http("Get Product")
      .get("/api/products/1"))

  setUp(
    scn.inject(rampUsers(5000) during (60 seconds))
  ).protocols(httpProtocol)
}
```

**Priority:** P3 (Low-Medium)
**Use Case:** Complex multi-step scenarios

---

### 6. **Locust** (Python-Based Load Testing)

**Why We Need It:**
- ✅ Python scripting (accessible)
- ✅ Web UI for monitoring
- ✅ Distributed load testing
- ✅ WebSocket and gRPC support

**License:** MIT
**Language:** Python
**Website:** https://locust.io

**Proposed Usage:**
```python
# locustfile.py
from locust import HttpUser, task, between

class APIGatewayUser(HttpUser):
    wait_time = between(0.1, 0.5)

    @task
    def ping(self):
        self.client.get("/api/ping")

    @task(3)
    def get_data(self):
        self.client.get("/api/data")
```

**Priority:** P3 (Low-Medium)

---

### 7. **Apache JMeter** (Enterprise Load Testing)

**Why We Consider It:**
- ✅ GUI-based test creation
- ✅ Mature and stable
- ✅ Extensive plugin ecosystem
- ✅ Multi-protocol support

**License:** Apache 2.0
**Language:** Java

**Priority:** P4 (Low)
**Reason for Low Priority:** Heavy, complex, better alternatives exist

---

## Protocol-Specific Tools

### HTTP/3 QUIC Testing

**Tools Needed:**
1. **h2load** (with HTTP/3 support)
2. **quiche-client** (Cloudflare's QUIC client)
3. **curl** (with HTTP/3 enabled)

**Installation:**
```bash
# Build curl with HTTP/3
git clone --recursive https://github.com/curl/curl.git
cd curl
./buildconf
./configure --with-openssl --with-quiche=/path/to/quiche
make && make install
```

**Current Status:** ❌ Not available (exit code 2 in tests)
**Priority:** P2

---

### WebSocket Testing

**Tools Needed:**
1. **wscat** (WebSocket CLI tool)
2. **k6** (with WebSocket extension)
3. **Artillery** (WebSocket scenarios)

**Installation:**
```bash
# wscat
npm install -g wscat

# Test WebSocket connection
wscat -c ws://localhost:9000/ws
```

**Current Status:** ⚠️ Backend missing (exit code 125)
**Priority:** P2

---

### gRPC Testing

**Tools Needed:**
1. **ghz** (Primary gRPC load tester)
2. **grpcurl** (gRPC curl equivalent)
3. **BloomRPC** (GUI client)

**Installation:**
```bash
# ghz
go install github.com/bojand/ghz/cmd/ghz@latest

# grpcurl
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

**Current Status:** ❌ ghz not installed (exit code 125)
**Priority:** P1

---

### GraphQL Testing

**Tools Needed:**
1. **k6** (with GraphQL support)
2. **Artillery** (GraphQL scenarios)
3. **GraphQL Bench**

**Installation:**
```bash
# Artillery with GraphQL plugin
npm install -g artillery artillery-plugin-graphql

# GraphQL Bench
cargo install graphql-bench
```

**Current Status:** ⚠️ Backend missing
**Priority:** P2

---

### Database Load Testing

**Tools Needed:**
1. **sysbench** (MySQL/PostgreSQL)
2. **redis-benchmark** (Redis)
3. **pgbench** (PostgreSQL built-in)

**Installation:**
```bash
# sysbench
apt-get install sysbench

# Redis benchmark (included with Redis)
redis-benchmark -h localhost -p 6379 -c 100 -n 100000

# pgbench (included with PostgreSQL)
pgbench -h localhost -p 5432 -U postgres -c 100 -t 1000 testdb
```

**Current Status:** ⚠️ Containers missing
**Priority:** P2

---

## Infrastructure Setup

### Required Infrastructure Components

#### 1. **Backend Services**

**HTTP Backends:**
```dockerfile
# Simple HTTP backend (Node.js example)
FROM node:20-alpine
COPY server.js /app/
CMD ["node", "/app/server.js"]
```

**Current Status:** ✅ 3 HTTP backends working
**Needed:** gRPC, GraphQL, WebSocket, Database backends

---

#### 2. **Docker Images Needed**

**Priority P1 (High):**
- [ ] gRPC backend server
- [ ] WebSocket server
- [ ] MySQL/PostgreSQL/Redis backends

**Priority P2 (Medium):**
- [ ] GraphQL backend
- [ ] PHP-FPM with app
- [ ] Geo-location backend

**Priority P3 (Low):**
- [ ] Microservices (for discovery testing)
- [ ] Multi-protocol hybrid backend

---

#### 3. **Network Configuration**

**Current Limitation:** Sequential testing on same ports

**Proposed Solution:** Port-based isolation
```bash
# Scenario 1: Gateway 9001, Backends 8101-8103
# Scenario 2: Gateway 9002, Backends 8201-8203
# Scenario 3: Gateway 9003, Backends 8301-8303
# ...
# Scenario 15: Gateway 9015, Backends 8501-8503
```

**Benefits:**
- ✅ Parallel test execution
- ✅ No cleanup conflicts
- ✅ Faster test runs (30-45 min → 5-10 min)

---

## Standalone Testbed Project Proposal

### Project Name: **"Universal Load Testing Testbed"**

### Vision
A configurable, protocol-agnostic load testing framework that can test any reverse proxy, API gateway, or load balancer with various protocols and scenarios.

### Architecture

```
universal-testbed/
├── config/
│   ├── testbed.yaml              # Main configuration
│   ├── scenarios/
│   │   ├── 01-tcp.yaml
│   │   ├── 02-http.yaml
│   │   ├── 03-https.yaml
│   │   ├── ...
│   │   └── 15-geo.yaml
├── backends/
│   ├── http/
│   │   ├── Dockerfile
│   │   └── server.js
│   ├── grpc/
│   │   ├── Dockerfile
│   │   └── server.go
│   ├── websocket/
│   ├── graphql/
│   ├── database/
│   └── php-fpm/
├── tools/
│   ├── install.sh                # Install all load testing tools
│   ├── setup.sh                  # Setup test environment
│   └── cleanup.sh                # Cleanup after tests
├── runners/
│   ├── sequential.sh             # Run tests one by one
│   ├── parallel.sh               # Run tests in parallel
│   └── selective.sh              # Run specific scenarios
├── results/
│   └── [timestamp]/
│       ├── scenario-01/
│       ├── scenario-02/
│       └── summary.html
├── docs/
│   ├── ARCHITECTURE.md
│   ├── SCENARIO_GUIDE.md
│   └── TOOL_GUIDE.md
└── README.md
```

---

### Configuration Format

**testbed.yaml:**
```yaml
testbed:
  name: "Highper Gateway Test Suite"
  version: "1.0.0"
  parallel: true
  timeout: 300

proxy:
  binary: "./target/release/highper-gateway"
  config_format: "toml"

scenarios:
  - id: 01
    name: "Layer 4 TCP Proxying"
    protocol: "tcp"
    duration: "120s"
    rates: [1000, 2000, 3000, 4000, 5000]
    backends:
      count: 3
      type: "http"
      ports: [8101, 8102, 8103]
    gateway:
      port: 9001
      config: "configs/scenario-01-tcp.toml"
    tools:
      - vegeta

  - id: 02
    name: "HTTP/1.1 Load Balancing"
    protocol: "http"
    duration: "120s"
    rates: [500, 1000, 2000, 3000, 5000]
    backends:
      count: 3
      type: "http"
      ports: [8201, 8202, 8203]
    gateway:
      port: 9002
      config: "configs/scenario-02-http.toml"
    tools:
      - vegeta
      - wrk

  - id: 07
    name: "gRPC Gateway"
    protocol: "grpc"
    duration: "180s"
    rps: [1000, 5000, 10000]
    backends:
      count: 2
      type: "grpc"
      ports: [8701, 8702]
    gateway:
      port: 9007
      config: "configs/scenario-07-grpc.toml"
    tools:
      - ghz
```

---

### Features

**1. Multi-Protocol Support:**
- HTTP/1.1, HTTP/2, HTTP/3
- WebSocket
- gRPC
- GraphQL
- TCP
- Database protocols (MySQL, PostgreSQL, Redis)
- FastCGI (PHP-FPM)

**2. Configurable Scenarios:**
- YAML-based configuration
- Easy to add new scenarios
- Parameterized tests

**3. Multiple Load Testing Tools:**
- Vegeta (HTTP)
- k6 (multi-protocol)
- ghz (gRPC)
- wrk (HTTP with Lua)
- h2load (HTTP/2, HTTP/3)
- Custom tools

**4. Parallel Execution:**
- Port-based isolation
- Concurrent scenario execution
- Resource management

**5. Result Aggregation:**
- JSON, HTML, CSV output
- Comparative analysis
- Trend analysis over time

**6. Docker Integration:**
- Pre-built backend images
- One-command setup
- Isolated environments

**7. CI/CD Ready:**
- GitHub Actions integration
- GitLab CI support
- Jenkins integration

---

### Benefits

**For Highper Gateway:**
- ✅ Faster testing (5-10 min vs 30-45 min)
- ✅ Reproducible results
- ✅ Easy to add new scenarios
- ✅ Better debugging

**For Other Projects:**
- ✅ Reusable testbed for any proxy
- ✅ Open source contribution opportunity
- ✅ Community-driven improvements
- ✅ Standardized benchmarking

**For DevOps:**
- ✅ Simple setup (one command)
- ✅ Clear documentation
- ✅ Automated cleanup
- ✅ Cloud-ready

---

### Implementation Phases

**Phase 1: Foundation (2-3 weeks)**
- [ ] Project structure
- [ ] Configuration parser
- [ ] Backend Docker images (HTTP, gRPC, WebSocket)
- [ ] Basic test runner (sequential)

**Phase 2: Core Features (3-4 weeks)**
- [ ] Parallel test execution
- [ ] Tool integration (vegeta, k6, ghz)
- [ ] Result aggregation
- [ ] HTML reporting

**Phase 3: Advanced Features (2-3 weeks)**
- [ ] All 15 scenarios implemented
- [ ] Database backends
- [ ] GraphQL backend
- [ ] PHP-FPM backend

**Phase 4: Polish (1-2 weeks)**
- [ ] Documentation
- [ ] CI/CD integration
- [ ] Example configurations
- [ ] Video tutorials

**Total Estimated Time: 8-12 weeks**

---

## Tool Comparison Matrix

| Tool | Protocol | Scripting | Complexity | Performance | Cloud | License |
|------|----------|-----------|------------|-------------|-------|---------|
| **Vegeta** | HTTP | ❌ | Low | ⭐⭐⭐⭐⭐ | ❌ | MIT |
| **k6** | HTTP/WS/gRPC | ✅ JS | Medium | ⭐⭐⭐⭐ | ✅ | AGPL/Commercial |
| **wrk** | HTTP | ✅ Lua | Medium | ⭐⭐⭐⭐⭐ | ❌ | Apache 2.0 |
| **ghz** | gRPC | ❌ | Low | ⭐⭐⭐⭐ | ❌ | Apache 2.0 |
| **h2load** | HTTP/2/3 | ❌ | Low | ⭐⭐⭐⭐⭐ | ❌ | MIT |
| **Locust** | Multi | ✅ Python | Medium | ⭐⭐⭐ | ✅ | MIT |
| **Gatling** | Multi | ✅ Scala | High | ⭐⭐⭐⭐ | ✅ | Apache 2.0 |
| **JMeter** | Multi | ✅ GUI | High | ⭐⭐ | ❌ | Apache 2.0 |

---

## Installation & Setup

### Quick Start (Install All Tools)

```bash
#!/bin/bash
# install-load-testing-tools.sh

# Vegeta
wget https://github.com/tsenart/vegeta/releases/download/v12.11.1/vegeta_12.11.1_linux_amd64.tar.gz
tar xzf vegeta_12.11.1_linux_amd64.tar.gz
mv vegeta ~/bin/

# k6
wget https://github.com/grafana/k6/releases/download/v0.48.0/k6-v0.48.0-linux-amd64.tar.gz
tar xzf k6-v0.48.0-linux-amd64.tar.gz
mv k6-v0.48.0-linux-amd64/k6 ~/bin/

# ghz
go install github.com/bojand/ghz/cmd/ghz@latest

# wrk
git clone https://github.com/wg/wrk.git
cd wrk && make && mv wrk ~/bin/

# h2load (from nghttp2)
sudo apt-get install nghttp2-client

# jq
wget https://github.com/jqlang/jq/releases/download/jq-1.7.1/jq-linux-amd64 -O ~/bin/jq
chmod +x ~/bin/jq

echo "✅ All load testing tools installed!"
```

---

## Best Practices

### 1. **Test Environment Isolation**
- Use Docker networks
- Unique ports per scenario
- Clean up after each test

### 2. **Realistic Load Patterns**
- Ramp up gradually
- Sustained load period
- Ramp down gracefully

### 3. **Metrics to Collect**
- **Throughput:** Requests per second
- **Latency:** P50, P95, P99, P99.9
- **Success Rate:** % of 2xx responses
- **Error Rate:** % of 4xx/5xx responses
- **Resource Usage:** CPU, memory, network

### 4. **Test Duration**
- Warm-up: 30 seconds
- Sustained: 1-5 minutes
- Cool-down: 30 seconds

### 5. **Result Storage**
- Timestamp-based directories
- JSON + human-readable formats
- Keep historical results for comparison

---

## Conclusion

This document provides a comprehensive overview of load testing tools for Highper Gateway. The proposed **Standalone Testbed Project** would significantly improve testing efficiency and create a valuable open-source contribution.

**Next Steps:**
1. ✅ Install missing tools (ghz, k6, h2load)
2. ✅ Implement parallel testing with port isolation
3. ✅ Create standalone testbed project (8-12 weeks)
4. ✅ Document all scenarios and configurations

---

**Maintained By:** Highper Gateway Team
**Last Updated:** January 11, 2026
**License:** Apache 2.0
