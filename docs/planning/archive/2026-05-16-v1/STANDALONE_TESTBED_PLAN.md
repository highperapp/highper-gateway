# Standalone Universal Testbed - Development Plan

**Project:** Universal Load Testing Testbed for Highper Gateway
**Version:** 1.0
**Date:** January 11, 2026
**Status:** Planning Phase
**License:** Apache 2.0

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Project Goals](#project-goals)
3. [Architecture Overview](#architecture-overview)
4. [Protocol-Specific Tool Integration](#protocol-specific-tool-integration)
5. [15 Use Case Scenario Mapping](#15-use-case-scenario-mapping)
6. [Technical Specifications](#technical-specifications)
7. [Implementation Phases](#implementation-phases)
8. [Development Timeline](#development-timeline)
9. [Resource Requirements](#resource-requirements)
10. [Deliverables](#deliverables)
11. [Success Criteria](#success-criteria)
12. [Risk Assessment](#risk-assessment)

---

## Executive Summary

### Project Overview

Create a **universal, standalone, configurable load testing testbed** that:
- Wraps protocol-specific testing tools (vegeta, k6, ghz, ws-benchmark, etc.)
- Supports all 15 Highper Gateway use case scenarios
- Provides unified YAML-based configuration
- Enables parallel test execution
- Generates comprehensive reports
- Can be reused for any reverse proxy/gateway testing

### Key Benefits

1. **Unified Interface**: Single YAML config for all protocols
2. **Parallel Execution**: Run multiple scenarios concurrently (30-45 min → 5-10 min)
3. **Reusability**: Test any reverse proxy, not just Highper Gateway
4. **Extensibility**: Easy to add new protocols and scenarios
5. **Comprehensive**: Pre-built backends for all protocols
6. **CI/CD Ready**: Integrate with GitHub Actions, GitLab CI, Jenkins

### Expected Timeline

- **Phase 1-2 (MVP)**: 2-3 weeks (basic HTTP/TCP scenarios)
- **Phase 3-4 (Advanced)**: 3-4 weeks (all protocols, parallel execution)
- **Phase 5 (Polish)**: 1-2 weeks (reporting, documentation)
- **Total**: 6-9 weeks for complete implementation

---

## Project Goals

### Primary Goals

1. **Universal Testing Framework**
   - Support all 15 Highper Gateway scenarios
   - Support testing ANY reverse proxy (nginx, HAProxy, Envoy, etc.)
   - YAML-based configuration for easy customization

2. **Protocol Coverage**
   - ✅ Layer 4: TCP, UDP
   - ✅ Layer 7 HTTP: HTTP/1.1, HTTP/2, HTTP/3/QUIC
   - ✅ WebSocket: WS, WSS
   - ✅ gRPC: Unary, streaming
   - ✅ Database: MySQL, PostgreSQL, Redis, MongoDB
   - ✅ GraphQL: Queries, mutations, subscriptions
   - ✅ FastCGI: PHP-FPM

3. **Performance Testing Types**
   - Load testing (constant rate)
   - Stress testing (spike, breakpoint)
   - Soak testing (endurance)
   - Benchmark testing (max throughput)

4. **Parallel Execution**
   - Port-based isolation (9000-9099 range)
   - Container-based isolation
   - Resource management
   - Concurrent scenario execution

### Secondary Goals

1. **Developer Experience**
   - Simple CLI: `testbed run scenario-01`
   - Interactive mode: `testbed interactive`
   - Watch mode: `testbed watch scenario-01`

2. **Reporting & Analytics**
   - Real-time dashboard (web UI)
   - JSON/YAML output
   - HTML reports with graphs
   - Prometheus metrics export
   - Historical comparison

3. **CI/CD Integration**
   - GitHub Actions workflow
   - Docker Compose for portability
   - Exit codes for pass/fail
   - Artifact generation

---

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Universal Testbed CLI                     │
│                  (Rust/Python/Go binary)                     │
└──────────────────────────┬──────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │   Config    │ │   Backend   │ │   Load      │
    │   Parser    │ │   Manager   │ │   Generator │
    │  (YAML)     │ │  (Docker)   │ │   Wrapper   │
    └─────────────┘ └─────────────┘ └─────────────┘
           │               │               │
           │               │               │
           ▼               ▼               ▼
    ┌─────────────────────────────────────────────┐
    │         Scenario Execution Engine           │
    │  - Sequential runner                        │
    │  - Parallel runner (port isolation)         │
    │  - Resource manager                         │
    └─────────────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │   vegeta    │ │     k6      │ │     ghz     │
    │   (HTTP)    │ │ (HTTP/WS)   │ │   (gRPC)    │
    └─────────────┘ └─────────────┘ └─────────────┘
           │               │               │
           ▼               ▼               ▼
    ┌─────────────────────────────────────────────┐
    │          Results Aggregator                 │
    │  - Metrics collection                       │
    │  - Report generation                        │
    │  - Comparison engine                        │
    └─────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. **Config Parser**
```yaml
# testbed.yaml
scenarios:
  - id: scenario-01
    name: "Layer 4 TCP Proxying"
    protocol: tcp
    backend:
      type: docker
      image: "testbed/tcp-echo-server:latest"
      ports: [8001, 8002, 8003]
    gateway:
      config: "configs/scenario-01.toml"
      port: 9001
    load:
      tool: "tcp-bench"
      duration: "60s"
      connections: 1000
      rate: "5000/s"
    success_criteria:
      throughput_min: 5000
      latency_p99_max: 10ms
      error_rate_max: 0.1%
```

#### 2. **Backend Manager**
- Docker Compose orchestration
- Health check verification
- Port allocation
- Container lifecycle management

#### 3. **Load Generator Wrapper**
- Protocol-specific tool selection
- Unified metric collection
- Result normalization
- Error handling

#### 4. **Scenario Execution Engine**
- Sequential execution (default)
- Parallel execution (with isolation)
- Resource limits enforcement
- Timeout management

#### 5. **Results Aggregator**
- Real-time metrics streaming
- Historical data storage
- Comparison against baselines
- Report generation (HTML/JSON/YAML)

---

## Protocol-Specific Tool Integration

### Tool Selection Matrix

| Protocol | Primary Tool | Backup Tool | Scenario Coverage |
|----------|--------------|-------------|-------------------|
| **TCP** | `tcp-benchmark` | Custom Rust tool | S01 |
| **HTTP/1.1** | `vegeta` | `k6`, `wrk` | S02, S04, S11 |
| **HTTP/2** | `h2load` | `k6` | S02, S07 |
| **HTTP/3/QUIC** | `quic-bench` | Custom | S05 |
| **HTTPS/TLS** | `vegeta` + TLS | `k6` | S03, S09 |
| **WebSocket** | `ws-benchmark` | `k6` | S06 |
| **gRPC** | `ghz` | `grpcurl` + script | S07 |
| **MySQL** | `sysbench` | `mysqlslap` | S08 |
| **PostgreSQL** | `pgbench` | Custom | S08 |
| **Redis** | `redis-benchmark` | `memtier` | S08, S11 |
| **MongoDB** | `mongo-perf` | Custom | S08 |
| **GraphQL** | `k6` + extension | Custom | S13 |
| **PHP-FPM** | `vegeta` + FastCGI | Custom | S14 |

### Tool Wrapper Interface

All tool wrappers implement a common interface:

```rust
// Rust interface (or equivalent in Python/Go)
trait LoadGenerator {
    fn name(&self) -> &str;
    fn protocols(&self) -> Vec<Protocol>;
    fn prepare(&mut self, config: &ScenarioConfig) -> Result<()>;
    fn execute(&mut self) -> Result<LoadTestResults>;
    fn cleanup(&mut self) -> Result<()>;
}

struct LoadTestResults {
    throughput: f64,          // requests/second
    latency_p50: Duration,
    latency_p95: Duration,
    latency_p99: Duration,
    error_rate: f64,          // percentage
    total_requests: u64,
    failed_requests: u64,
    duration: Duration,
    custom_metrics: HashMap<String, f64>,
}
```

### Tool Integration Details

#### 1. **vegeta** (HTTP/1.1, HTTPS)
```bash
# Wrapper command generation
echo "GET http://localhost:9002/" | \
  vegeta attack \
    -rate=5000/s \
    -duration=60s \
    -timeout=30s \
    -keepalive=true \
    -workers=8 \
  | vegeta report -type=json > results.json

# Parse results.json and normalize to LoadTestResults
```

#### 2. **k6** (HTTP/2, WebSocket, GraphQL)
```javascript
// Generated k6 script
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '30s', target: 100 },
    { duration: '60s', target: 500 },
    { duration: '30s', target: 0 },
  ],
};

export default function() {
  let res = http.get('http://localhost:9002/');
  check(res, { 'status is 200': (r) => r.status === 200 });
}
```

#### 3. **ghz** (gRPC)
```bash
# Wrapper command generation
ghz \
  --insecure \
  --proto ./protos/service.proto \
  --call helloworld.Greeter.SayHello \
  -d '{"name":"testbed"}' \
  -c 100 \
  -n 10000 \
  --rps 1000 \
  localhost:9007 \
  --format=json > results.json
```

#### 4. **sysbench** (MySQL)
```bash
# Wrapper command generation
sysbench \
  --db-driver=mysql \
  --mysql-host=localhost \
  --mysql-port=9008 \
  --mysql-user=test \
  --mysql-password=test \
  --threads=16 \
  --time=60 \
  --report-interval=10 \
  oltp_read_write \
  run \
  --table-size=100000 \
  | tee results.txt
```

#### 5. **redis-benchmark** (Redis)
```bash
# Wrapper command generation
redis-benchmark \
  -h localhost \
  -p 9008 \
  -t set,get \
  -n 1000000 \
  -c 50 \
  -d 1024 \
  --csv > results.csv
```

#### 6. **ws-benchmark** (WebSocket)
```bash
# Custom Node.js script or use ws-benchmark
ws-benchmark \
  ws://localhost:9006/ws \
  --connections 500 \
  --duration 60 \
  --rate 1000 \
  --message '{"type":"ping"}' \
  --json > results.json
```

---

## 15 Use Case Scenario Mapping

### Scenario Configuration Files

Each scenario has a dedicated YAML config:

```
testbed/
├── scenarios/
│   ├── scenario-01-tcp.yaml
│   ├── scenario-02-http.yaml
│   ├── scenario-03-https.yaml
│   ├── scenario-04-api-gateway.yaml
│   ├── scenario-05-http3.yaml
│   ├── scenario-06-websocket.yaml
│   ├── scenario-07-grpc.yaml
│   ├── scenario-08-database.yaml
│   ├── scenario-09-waf-mtls.yaml
│   ├── scenario-10-hybrid.yaml
│   ├── scenario-11-cdn-cache.yaml
│   ├── scenario-12-discovery.yaml
│   ├── scenario-13-graphql.yaml
│   ├── scenario-14-php-fpm.yaml
│   └── scenario-15-geo-lb.yaml
```

### Scenario Details

#### **Scenario 01: Layer 4 TCP Proxying**

```yaml
# scenarios/scenario-01-tcp.yaml
id: scenario-01
name: "Layer 4 TCP - Pure TCP Proxying"
protocol: tcp
description: "Pure Layer 4 TCP proxying with connection pooling"

backend:
  type: docker
  image: "testbed/tcp-echo-server:latest"
  replicas: 3
  ports: [8001, 8002, 8003]
  health_check:
    type: tcp_connect
    interval: 5s
    timeout: 2s

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-01-tcp.toml"
  port: 9001
  wait_ready: 5s

load:
  tool: "tcp-benchmark"
  phases:
    - name: "warmup"
      duration: 30s
      connections: 100
      rate: 1000
    - name: "ramp-up"
      duration: 60s
      connections: 500
      rate: 5000
    - name: "sustained"
      duration: 60s
      connections: 1000
      rate: 5000
    - name: "cooldown"
      duration: 30s
      connections: 100
      rate: 1000

success_criteria:
  throughput_min: 5000        # req/s
  latency_p99_max: 10ms
  error_rate_max: 0.1%
  memory_max: 100MB

reporting:
  formats: [json, html, prometheus]
  output_dir: "results/scenario-01"
  baseline_comparison: true
```

#### **Scenario 02: HTTP/1.1 Load Balancing**

```yaml
# scenarios/scenario-02-http.yaml
id: scenario-02
name: "Layer 7 HTTP - HTTP/1.1 Load Balancing"
protocol: http1
description: "Round-robin load balancing with health checks"

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 3
  ports: [8101, 8102, 8103]
  health_check:
    type: http_get
    path: "/health"
    interval: 5s
    timeout: 2s
    expected_status: 200

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-02-http.toml"
  port: 9002
  wait_ready: 5s

load:
  tool: "vegeta"
  phases:
    - name: "baseline"
      duration: 60s
      rate: 500
      target: "http://localhost:9002/"
    - name: "increase"
      duration: 60s
      rate: 2000
      target: "http://localhost:9002/"
    - name: "peak"
      duration: 60s
      rate: 5000
      target: "http://localhost:9002/"

validation:
  # Validate load balancer distribution
  check_distribution:
    enabled: true
    tolerance: 5%  # Each backend should get 33% ± 5%

success_criteria:
  throughput_min: 5000
  latency_p95_max: 10ms
  latency_p99_max: 50ms
  error_rate_max: 0.1%
  distribution_variance_max: 5%

reporting:
  formats: [json, html]
  output_dir: "results/scenario-02"
  charts:
    - latency_histogram
    - throughput_over_time
    - backend_distribution
```

#### **Scenario 03: HTTPS/TLS Termination**

```yaml
# scenarios/scenario-03-https.yaml
id: scenario-03
name: "HTTPS/TLS Termination"
protocol: https
description: "TLS 1.3 termination with mTLS and OCSP stapling"

setup:
  certificates:
    - type: self-signed
      common_name: "localhost"
      output: "certs/scenario-03"
    - type: client-cert
      common_name: "test-client"
      output: "certs/scenario-03-client"

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 3
  ports: [8201, 8202, 8203]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-03-https.toml"
  port: 9003
  tls:
    enabled: true
    cert: "certs/scenario-03/cert.pem"
    key: "certs/scenario-03/key.pem"
    mtls: true
    ocsp_stapling: true

load:
  tool: "vegeta"
  tls_config:
    insecure_skip_verify: true
    client_cert: "certs/scenario-03-client/cert.pem"
    client_key: "certs/scenario-03-client/key.pem"
  phases:
    - name: "tls-handshake-test"
      duration: 30s
      rate: 100
      target: "https://localhost:9003/"
      keepalive: false  # Force TLS handshakes
    - name: "sustained-load"
      duration: 90s
      rate: 2000
      target: "https://localhost:9003/"
      keepalive: true

metrics:
  tls_handshake_duration: true
  tls_version: true
  cipher_suite: true

success_criteria:
  throughput_min: 2000
  latency_p99_max: 100ms
  tls_handshake_p95_max: 100ms
  error_rate_max: 0.1%
```

#### **Scenario 04: API Gateway with Rate Limiting**

```yaml
# scenarios/scenario-04-api-gateway.yaml
id: scenario-04
name: "API Gateway with Rate Limiting"
protocol: http1
description: "Token bucket and sliding window rate limiting"

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 1
  ports: [8301]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-04-rate-limit.toml"
  port: 9004

load:
  tool: "vegeta"
  phases:
    # Test rate limit: 100 req/s per client
    - name: "within-limit"
      duration: 30s
      rate: 50
      target: "http://localhost:9004/api/data"
      headers:
        Authorization: "Bearer token-client-1"

    - name: "at-limit"
      duration: 30s
      rate: 100
      target: "http://localhost:9004/api/data"
      headers:
        Authorization: "Bearer token-client-1"

    - name: "exceed-limit"
      duration: 30s
      rate: 150
      target: "http://localhost:9004/api/data"
      headers:
        Authorization: "Bearer token-client-1"

validation:
  check_rate_limit:
    enabled: true
    expected_limit: 100
    tolerance: 5
    status_code_on_limit: 429

success_criteria:
  # Within limit: should succeed
  phase_1_error_rate_max: 0.1%
  # At limit: should succeed
  phase_2_error_rate_max: 5%
  # Exceed limit: should get 429 errors
  phase_3_error_429_min: 30%
```

#### **Scenario 05: HTTP/3 QUIC**

```yaml
# scenarios/scenario-05-http3.yaml
id: scenario-05
name: "HTTP/3 QUIC"
protocol: http3
description: "HTTP/3 over QUIC using Cloudflare quiche"

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 3
  ports: [8401, 8402, 8403]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-05-http3.toml"
  port: 9005
  protocol: http3

setup:
  certificates:
    - type: self-signed
      common_name: "localhost"
      output: "certs/scenario-05"

load:
  tool: "quic-bench"  # or custom HTTP/3 client
  tls_config:
    insecure_skip_verify: true
  phases:
    - name: "http3-load"
      duration: 60s
      rate: 1000
      target: "https://localhost:9005/"
      protocol: "h3"

success_criteria:
  throughput_min: 1000
  latency_p99_max: 100ms
  error_rate_max: 1%
  protocol_version: "h3"
```

#### **Scenario 06: WebSocket Load Balancer**

```yaml
# scenarios/scenario-06-websocket.yaml
id: scenario-06
name: "WebSocket Load Balancer"
protocol: websocket
description: "WebSocket connection pooling and message routing"

backend:
  type: docker
  image: "testbed/websocket-server:latest"
  replicas: 3
  ports: [8501, 8502, 8503]
  health_check:
    type: websocket
    path: "/ws"
    interval: 10s

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-06-websocket.toml"
  port: 9006

load:
  tool: "ws-benchmark"
  phases:
    - name: "connection-test"
      duration: 60s
      connections: 500
      messages_per_connection: 100
      message_rate: 1000
      target: "ws://localhost:9006/ws"
      payload: '{"type":"ping","data":"test"}'

validation:
  check_distribution:
    enabled: true
    tolerance: 10%

success_criteria:
  connection_success_rate_min: 99%
  message_throughput_min: 50000  # messages/s
  message_latency_p99_max: 50ms
  error_rate_max: 0.5%
```

#### **Scenario 07: gRPC Gateway**

```yaml
# scenarios/scenario-07-grpc.yaml
id: scenario-07
name: "gRPC Gateway"
protocol: grpc
description: "gRPC load balancing with streaming support"

setup:
  proto_files:
    - "protos/helloworld.proto"
    - "protos/echo.proto"

backend:
  type: docker
  image: "testbed/grpc-server:latest"
  replicas: 3
  ports: [8601, 8602, 8603]
  health_check:
    type: grpc
    service: "grpc.health.v1.Health"
    interval: 10s

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-07-grpc.toml"
  port: 9007

load:
  tool: "ghz"
  phases:
    - name: "unary-rpc"
      duration: 60s
      connections: 100
      rps: 5000
      proto: "protos/helloworld.proto"
      call: "helloworld.Greeter.SayHello"
      data: '{"name":"testbed"}'

    - name: "streaming-rpc"
      duration: 60s
      connections: 50
      rps: 1000
      proto: "protos/echo.proto"
      call: "echo.Echo.ServerStreamingEcho"
      data: '{"message":"test"}'

success_criteria:
  unary_throughput_min: 5000
  unary_latency_p99_max: 50ms
  streaming_throughput_min: 1000
  streaming_latency_p99_max: 100ms
  error_rate_max: 0.1%
```

#### **Scenario 08: Database Load Balancer**

```yaml
# scenarios/scenario-08-database.yaml
id: scenario-08
name: "Database Load Balancer"
protocol: database
description: "MySQL, PostgreSQL, Redis load balancing"

tests:
  - name: "mysql-test"
    backend:
      type: docker
      image: "mysql:8.0"
      replicas: 3
      ports: [8701, 8702, 8703]
      env:
        MYSQL_ROOT_PASSWORD: "test"
        MYSQL_DATABASE: "testdb"

    gateway:
      config: "examples/scenario-08-mysql.toml"
      port: 9008

    load:
      tool: "sysbench"
      test: "oltp_read_write"
      threads: 16
      duration: 60s
      table_size: 10000

  - name: "postgresql-test"
    backend:
      type: docker
      image: "postgres:16"
      replicas: 2
      ports: [8711, 8712]
      env:
        POSTGRES_PASSWORD: "test"
        POSTGRES_DB: "testdb"

    gateway:
      config: "examples/scenario-08-postgres.toml"
      port: 9009

    load:
      tool: "pgbench"
      clients: 10
      duration: 60s
      scale: 10

  - name: "redis-test"
    backend:
      type: docker
      image: "redis:7"
      replicas: 3
      ports: [8721, 8722, 8723]

    gateway:
      config: "examples/scenario-08-redis.toml"
      port: 9010

    load:
      tool: "redis-benchmark"
      clients: 50
      requests: 100000
      tests: ["set", "get", "incr", "lpush", "lpop"]

success_criteria:
  mysql_throughput_min: 1000  # transactions/s
  postgresql_throughput_min: 500
  redis_throughput_min: 50000
  error_rate_max: 0.5%
```

#### **Scenario 09: WAF + mTLS**

```yaml
# scenarios/scenario-09-waf-mtls.yaml
id: scenario-09
name: "WAF + mTLS"
protocol: https
description: "Web Application Firewall with mutual TLS"

setup:
  certificates:
    - type: self-signed
      common_name: "localhost"
      output: "certs/scenario-09"
    - type: client-cert
      common_name: "valid-client"
      output: "certs/scenario-09-client-valid"
    - type: client-cert
      common_name: "malicious-client"
      output: "certs/scenario-09-client-bad"

  waf_rules:
    - "rules/owasp-crs.conf"
    - "rules/custom-rules.conf"

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 2
  ports: [8801, 8802]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-09-waf-mtls.toml"
  port: 9011
  features:
    waf: "modsecurity"  # or coraza, aws-waf, custom
    mtls: true

load:
  tool: "vegeta"
  phases:
    # Legitimate traffic
    - name: "legitimate-traffic"
      duration: 30s
      rate: 100
      target: "https://localhost:9011/api/user"
      tls_config:
        client_cert: "certs/scenario-09-client-valid/cert.pem"
        client_key: "certs/scenario-09-client-valid/key.pem"
      body: '{"username":"john","email":"john@example.com"}'

    # Attack patterns (should be blocked)
    - name: "sql-injection"
      duration: 30s
      rate: 50
      target: "https://localhost:9011/api/user?id=1' OR '1'='1"
      tls_config:
        client_cert: "certs/scenario-09-client-valid/cert.pem"
        client_key: "certs/scenario-09-client-valid/key.pem"

    - name: "xss-attack"
      duration: 30s
      rate: 50
      target: "https://localhost:9011/api/comment"
      tls_config:
        client_cert: "certs/scenario-09-client-valid/cert.pem"
        client_key: "certs/scenario-09-client-valid/key.pem"
      body: '{"text":"<script>alert(1)</script>"}'

validation:
  check_waf_blocks:
    enabled: true
    expected_block_rate_phase_2: 95%  # SQL injection
    expected_block_rate_phase_3: 95%  # XSS
    block_status_code: 403

success_criteria:
  legitimate_traffic_error_rate_max: 0.1%
  attack_block_rate_min: 95%
  false_positive_rate_max: 1%
  latency_overhead_max: 5ms  # WAF overhead
```

#### **Scenario 10: Hybrid Multi-Protocol**

```yaml
# scenarios/scenario-10-hybrid.yaml
id: scenario-10
name: "Hybrid Multi-Protocol"
protocol: hybrid
description: "HTTP + WebSocket + gRPC on same gateway"

backend:
  services:
    - name: "http-api"
      type: docker
      image: "testbed/http-backend:latest"
      ports: [8901]

    - name: "websocket"
      type: docker
      image: "testbed/websocket-server:latest"
      ports: [8902]

    - name: "grpc-service"
      type: docker
      image: "testbed/grpc-server:latest"
      ports: [8903]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-10-hybrid.toml"
  routes:
    - path: "/api/*"
      backend: "http-api:8901"
      protocol: http
    - path: "/ws"
      backend: "websocket:8902"
      protocol: websocket
    - path: "/grpc/*"
      backend: "grpc-service:8903"
      protocol: grpc
  port: 9012

load:
  parallel: true
  phases:
    - name: "http-load"
      tool: "vegeta"
      duration: 60s
      rate: 2000
      target: "http://localhost:9012/api/data"

    - name: "websocket-load"
      tool: "ws-benchmark"
      duration: 60s
      connections: 200
      messages_per_connection: 500
      target: "ws://localhost:9012/ws"

    - name: "grpc-load"
      tool: "ghz"
      duration: 60s
      connections: 100
      rps: 1000
      proto: "protos/helloworld.proto"
      call: "helloworld.Greeter.SayHello"
      target: "localhost:9012"

success_criteria:
  http_throughput_min: 2000
  websocket_throughput_min: 50000
  grpc_throughput_min: 1000
  overall_error_rate_max: 0.5%
```

#### **Scenario 11: CDN Edge Caching**

```yaml
# scenarios/scenario-11-cdn-cache.yaml
id: scenario-11
name: "CDN Edge Caching"
protocol: http1
description: "Multi-tier in-memory and Redis caching"

setup:
  redis:
    type: docker
    image: "redis:7"
    port: 6379

backend:
  type: docker
  image: "testbed/http-backend:latest"
  replicas: 1
  ports: [9001]
  # Slow backend to demonstrate cache benefits
  response_delay: 100ms

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-11-cache.toml"
  port: 9013
  cache:
    l1_memory: "256MB"
    l2_redis: "redis://localhost:6379"
    ttl: 60s

load:
  tool: "vegeta"
  phases:
    # First run: cold cache
    - name: "cold-cache"
      duration: 30s
      rate: 100
      target: "http://localhost:9013/static/image.jpg"

    # Second run: warm cache (should be much faster)
    - name: "warm-cache"
      duration: 30s
      rate: 1000
      target: "http://localhost:9013/static/image.jpg"

    # Mixed content
    - name: "mixed-content"
      duration: 60s
      rate: 500
      targets:
        - "http://localhost:9013/static/image1.jpg"
        - "http://localhost:9013/static/image2.jpg"
        - "http://localhost:9013/static/style.css"
        - "http://localhost:9013/api/dynamic"  # no-cache

metrics:
  cache_hit_rate: true
  cache_l1_hits: true
  cache_l2_hits: true
  cache_misses: true
  backend_requests: true

validation:
  check_cache_effectiveness:
    warm_cache_hit_rate_min: 95%
    warm_cache_latency_max: 5ms  # vs 100ms backend
    backend_request_reduction_min: 90%

success_criteria:
  cold_cache_latency_p99_max: 150ms
  warm_cache_latency_p99_max: 10ms
  cache_hit_rate_min: 80%
  error_rate_max: 0.1%
```

#### **Scenario 12: Microservices Discovery**

```yaml
# scenarios/scenario-12-discovery.yaml
id: scenario-12
name: "Microservices Discovery"
protocol: http1
description: "Consul/etcd service discovery with circuit breaker"

setup:
  consul:
    type: docker
    image: "consul:latest"
    port: 8500

  etcd:
    type: docker
    image: "bitnami/etcd:latest"
    port: 2379

backend:
  services:
    - name: "user-service"
      type: docker
      image: "testbed/http-backend:latest"
      replicas: 2
      ports: [9101, 9102]
      register_consul: true
      tags: ["user", "v1"]

    - name: "order-service"
      type: docker
      image: "testbed/http-backend:latest"
      replicas: 2
      ports: [9111, 9112]
      register_consul: true
      tags: ["order", "v1"]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-12-discovery.toml"
  port: 9014
  service_discovery:
    backend: "consul"
    address: "localhost:8500"
  circuit_breaker:
    enabled: true
    failure_threshold: 5
    timeout: 30s

load:
  tool: "vegeta"
  phases:
    # Normal operation
    - name: "normal-load"
      duration: 30s
      rate: 100
      target: "http://localhost:9014/user-service/api/users"

    # Simulate service failure
    - name: "service-failure"
      pre_hook: "docker stop user-service-1"
      duration: 30s
      rate: 100
      target: "http://localhost:9014/user-service/api/users"

    # Recovery
    - name: "recovery"
      pre_hook: "docker start user-service-1"
      duration: 30s
      rate: 100
      target: "http://localhost:9014/user-service/api/users"

validation:
  check_circuit_breaker:
    enabled: true
    expected_open_during_phase_2: true
    expected_half_open_during_phase_3: true
    recovery_time_max: 30s

success_criteria:
  normal_error_rate_max: 0.1%
  failure_error_rate_max: 100%  # Expected with circuit breaker
  recovery_success_rate_min: 95%
  service_discovery_latency_max: 10ms
```

#### **Scenario 13: GraphQL Gateway**

```yaml
# scenarios/scenario-13-graphql.yaml
id: scenario-13
name: "GraphQL Gateway"
protocol: graphql
description: "GraphQL schema stitching and federation"

setup:
  schemas:
    - service: "users"
      schema: "schemas/users.graphql"
      port: 9201
    - service: "posts"
      schema: "schemas/posts.graphql"
      port: 9202
    - service: "comments"
      schema: "schemas/comments.graphql"
      port: 9203

backend:
  services:
    - name: "users-graphql"
      type: docker
      image: "testbed/graphql-server:latest"
      ports: [9201]
      schema: "schemas/users.graphql"

    - name: "posts-graphql"
      type: docker
      image: "testbed/graphql-server:latest"
      ports: [9202]
      schema: "schemas/posts.graphql"

    - name: "comments-graphql"
      type: docker
      image: "testbed/graphql-server:latest"
      ports: [9203]
      schema: "schemas/comments.graphql"

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-13-graphql.toml"
  port: 9015
  graphql:
    federation: true
    schema_stitching: true

load:
  tool: "k6"
  script: "scripts/graphql-load.js"
  phases:
    - name: "simple-queries"
      duration: 30s
      vus: 50
      queries:
        - "{ users { id name email } }"
        - "{ posts { id title content } }"

    - name: "complex-queries"
      duration: 30s
      vus: 50
      queries:
        - "{ users { id name posts { title comments { text } } } }"

    - name: "mutations"
      duration: 30s
      vus: 20
      mutations:
        - "mutation { createUser(name: 'John', email: 'john@example.com') { id } }"

success_criteria:
  simple_query_throughput_min: 500
  complex_query_throughput_min: 100
  mutation_throughput_min: 50
  query_latency_p99_max: 200ms
  error_rate_max: 0.5%
```

#### **Scenario 14: Static + PHP-FPM**

```yaml
# scenarios/scenario-14-php-fpm.yaml
id: scenario-14
name: "Static + PHP-FPM"
protocol: http1
description: "FastCGI protocol with PHP-FPM backend"

setup:
  php_fpm:
    type: docker
    image: "php:8.2-fpm"
    port: 9000
    config: "configs/php-fpm.conf"
    volumes:
      - "./test-files/php:/var/www/html"

backend:
  services:
    - name: "php-fpm"
      type: docker
      image: "php:8.2-fpm"
      ports: [9000]
      protocol: fastcgi

    - name: "static-files"
      type: docker
      image: "testbed/http-backend:latest"
      ports: [9301]

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-14-php.toml"
  port: 9016
  routes:
    - path: "*.php"
      backend: "php-fpm:9000"
      protocol: fastcgi
    - path: "/static/*"
      backend: "static-files:9301"
      protocol: http

load:
  tool: "vegeta"
  phases:
    - name: "static-files"
      duration: 30s
      rate: 1000
      target: "http://localhost:9016/static/image.jpg"

    - name: "php-scripts"
      duration: 30s
      rate: 500
      target: "http://localhost:9016/index.php"

    - name: "mixed-load"
      duration: 60s
      rate: 750
      targets:
        - "http://localhost:9016/static/style.css"
        - "http://localhost:9016/info.php"
        - "http://localhost:9016/api.php?action=get"

success_criteria:
  static_throughput_min: 1000
  php_throughput_min: 500
  static_latency_p99_max: 10ms
  php_latency_p99_max: 50ms
  error_rate_max: 0.1%
```

#### **Scenario 15: Geographic Load Balancing**

```yaml
# scenarios/scenario-15-geo-lb.yaml
id: scenario-15
name: "Geographic Load Balancing"
protocol: http1
description: "MaxMind GeoIP-based load balancing"

setup:
  geoip_database:
    type: "maxmind"
    database: "GeoLite2-City.mmdb"
    # or ip2location: "IP2LOCATION-LITE-DB11.BIN"

backend:
  regions:
    - name: "us-east"
      type: docker
      image: "testbed/http-backend:latest"
      ports: [9401, 9402]
      region: "us-east-1"

    - name: "eu-west"
      type: docker
      image: "testbed/http-backend:latest"
      ports: [9411, 9412]
      region: "eu-west-1"

    - name: "ap-south"
      type: docker
      image: "testbed/http-backend:latest"
      ports: [9421, 9422]
      region: "ap-south-1"

gateway:
  binary: "./target/release/highper-gateway"
  config: "examples/scenario-15-geo-lb.toml"
  port: 9017
  geo_routing:
    enabled: true
    database: "GeoLite2-City.mmdb"
    fallback_region: "us-east"

load:
  tool: "vegeta"
  phases:
    # Simulate US traffic
    - name: "us-traffic"
      duration: 30s
      rate: 100
      target: "http://localhost:9017/"
      headers:
        X-Forwarded-For: "8.8.8.8"  # US IP

    # Simulate EU traffic
    - name: "eu-traffic"
      duration: 30s
      rate: 100
      target: "http://localhost:9017/"
      headers:
        X-Forwarded-For: "185.60.216.35"  # EU IP

    # Simulate APAC traffic
    - name: "apac-traffic"
      duration: 30s
      rate: 100
      target: "http://localhost:9017/"
      headers:
        X-Forwarded-For: "103.77.196.1"  # APAC IP

validation:
  check_geo_routing:
    enabled: true
    expected_routing:
      - source_ip: "8.8.8.8"
        expected_backend: "us-east"
      - source_ip: "185.60.216.35"
        expected_backend: "eu-west"
      - source_ip: "103.77.196.1"
        expected_backend: "ap-south"

success_criteria:
  routing_accuracy_min: 95%
  geo_lookup_latency_max: 1ms
  overall_throughput_min: 300
  error_rate_max: 0.5%
```

---

## Technical Specifications

### Implementation Language Options

#### Option A: Rust (Recommended)
**Pros:**
- Same language as Highper Gateway
- Excellent performance
- Strong typing and safety
- Great CLI libraries (clap, tokio)

**Cons:**
- Longer development time
- Smaller pool of contributors

**Estimated Dev Time:** 8-10 weeks

#### Option B: Python
**Pros:**
- Rapid development
- Excellent libraries (Click, PyYAML, Docker SDK)
- Easy scripting and integration
- Large contributor pool

**Cons:**
- Slower performance
- Runtime dependencies

**Estimated Dev Time:** 6-8 weeks

#### Option C: Go
**Pros:**
- Good performance
- Strong concurrency
- Simple deployment (single binary)
- Good Docker/Kubernetes libraries

**Cons:**
- Less familiar for team
- Moderate development speed

**Estimated Dev Time:** 7-9 weeks

**Recommendation:** **Python for MVP (Phases 1-2)**, **Rust for production (Phases 3-5)**

---

### Project Structure

```
universal-testbed/
├── README.md
├── LICENSE (Apache 2.0)
├── requirements.txt / Cargo.toml
│
├── src/
│   ├── main.py / main.rs
│   ├── cli.py / cli.rs
│   ├── config/
│   │   ├── parser.py
│   │   └── validator.py
│   ├── backends/
│   │   ├── manager.py
│   │   ├── docker_backend.py
│   │   ├── kubernetes_backend.py
│   │   └── health_checker.py
│   ├── load_generators/
│   │   ├── base.py
│   │   ├── vegeta_wrapper.py
│   │   ├── k6_wrapper.py
│   │   ├── ghz_wrapper.py
│   │   ├── sysbench_wrapper.py
│   │   ├── redis_bench_wrapper.py
│   │   └── custom_tcp_bench.py
│   ├── runners/
│   │   ├── sequential.py
│   │   ├── parallel.py
│   │   └── resource_manager.py
│   ├── results/
│   │   ├── aggregator.py
│   │   ├── reporter.py
│   │   ├── comparator.py
│   │   └── exporter.py
│   └── utils/
│       ├── port_allocator.py
│       ├── cert_generator.py
│       └── logger.py
│
├── scenarios/
│   ├── scenario-01-tcp.yaml
│   ├── scenario-02-http.yaml
│   ├── ... (13 more scenarios)
│   └── templates/
│       └── scenario-template.yaml
│
├── backends/
│   ├── http-backend/
│   │   ├── Dockerfile
│   │   └── server.py
│   ├── tcp-echo/
│   │   ├── Dockerfile
│   │   └── echo.rs
│   ├── grpc-server/
│   │   ├── Dockerfile
│   │   └── server.go
│   ├── websocket-server/
│   │   ├── Dockerfile
│   │   └── server.js
│   ├── graphql-server/
│   │   ├── Dockerfile
│   │   └── server.ts
│   └── README.md
│
├── scripts/
│   ├── install-tools.sh
│   ├── setup-environment.sh
│   ├── generate-certs.sh
│   ├── pre-build-backends.sh
│   └── ci-runner.sh
│
├── configs/
│   ├── gateway/
│   │   ├── scenario-01.toml
│   │   ├── scenario-02.toml
│   │   └── ... (13 more configs)
│   └── testbed-defaults.yaml
│
├── results/
│   ├── .gitkeep
│   └── baselines/
│       ├── scenario-01-baseline.json
│       └── ... (14 more baselines)
│
├── docs/
│   ├── GETTING_STARTED.md
│   ├── CONFIGURATION.md
│   ├── ADDING_SCENARIOS.md
│   ├── TOOL_INTEGRATION.md
│   └── API_REFERENCE.md
│
├── tests/
│   ├── test_config_parser.py
│   ├── test_backend_manager.py
│   ├── test_load_generators.py
│   └── test_runners.py
│
├── docker-compose.yml
├── Makefile
└── .github/
    └── workflows/
        ├── ci.yml
        └── release.yml
```

---

### CLI Design

```bash
# Main commands
testbed --help
testbed version
testbed list                           # List all scenarios
testbed validate <scenario-file>       # Validate YAML config
testbed run <scenario-id>              # Run single scenario
testbed run --all                      # Run all scenarios
testbed run --parallel scenario-01,02,03  # Run specific scenarios in parallel

# Advanced options
testbed run scenario-01 \
  --gateway-binary ./target/release/highper-gateway \
  --output-dir ./my-results \
  --format json,html \
  --compare-baseline \
  --verbose

# Interactive mode
testbed interactive                    # TUI for scenario selection

# Utilities
testbed setup                          # Install tools, build backends
testbed backends list                  # List available backends
testbed backends build                 # Build all backend Docker images
testbed backends start scenario-01     # Start backends for scenario
testbed backends stop                  # Stop all backends

# Reporting
testbed report generate results/scenario-01/*.json
testbed report compare scenario-01-v1.json scenario-01-v2.json
testbed report baseline set scenario-01 results/baseline.json
testbed report dashboard               # Launch web dashboard

# CI/CD integration
testbed ci-run \
  --scenarios scenario-01,scenario-02 \
  --threshold-file thresholds.yaml \
  --exit-on-failure
```

### Example Usage

```bash
# 1. Setup environment (one-time)
$ testbed setup
✓ Checking tool dependencies...
  - vegeta: installed (v12.11.1)
  - k6: installed (v0.48.0)
  - ghz: not found, installing...
✓ Building backend Docker images...
  - testbed/http-backend: built
  - testbed/tcp-echo: built
  - testbed/grpc-server: built
✓ Setup complete!

# 2. List scenarios
$ testbed list
Available scenarios:
  01. Layer 4 TCP Proxying
  02. HTTP/1.1 Load Balancing
  03. HTTPS/TLS Termination
  ... (12 more)

# 3. Run a single scenario
$ testbed run scenario-01
[14:23:45] Starting scenario: Layer 4 TCP Proxying
[14:23:46] ✓ Starting backends (3 replicas)
[14:23:48] ✓ Starting gateway on port 9001
[14:23:50] ✓ Running health checks
[14:23:52] ✓ Starting load test (tcp-benchmark)
[14:23:52]   Phase 1/4: warmup (30s)
[14:24:22]   Phase 2/4: ramp-up (60s)
[14:25:22]   Phase 3/4: sustained (60s)
[14:26:22]   Phase 4/4: cooldown (30s)
[14:26:52] ✓ Load test complete
[14:26:52] ✓ Collecting results
[14:26:53] ✓ Generating report

Results Summary:
  Status: PASS ✓
  Throughput: 5,234 req/s (target: 5,000)
  Latency p99: 8.7ms (target: <10ms)
  Error rate: 0.02% (target: <0.1%)
  Duration: 3m 8s

Report saved to: results/scenario-01/report-20260111-142653.html

# 4. Run multiple scenarios in parallel
$ testbed run --parallel scenario-01,scenario-02,scenario-03
[14:30:00] Starting 3 scenarios in parallel...
[14:30:00] ✓ scenario-01: Starting backends
[14:30:00] ✓ scenario-02: Starting backends
[14:30:00] ✓ scenario-03: Starting backends
...
[14:35:23] All scenarios complete!

Summary:
  scenario-01: PASS ✓ (3m 8s)
  scenario-02: PASS ✓ (3m 15s)
  scenario-03: PASS ✓ (3m 42s)

Total time: 5m 23s (vs 10m 5s sequential)
Time saved: 4m 42s (47%)

# 5. Compare against baseline
$ testbed run scenario-01 --compare-baseline
...
Results vs Baseline:
  Throughput: 5,234 req/s (+4.2% vs baseline 5,021 req/s) ✓
  Latency p99: 8.7ms (-12% vs baseline 9.9ms) ✓
  Error rate: 0.02% (same as baseline) ✓

# 6. CI/CD integration
$ testbed ci-run --scenarios all --exit-on-failure
...
Final Status: 14/15 PASS, 1 FAIL
  scenario-05 (HTTP/3): FAIL - QUIC protocol not available

Exit code: 1  # Non-zero for CI/CD failure
```

---

## Implementation Phases

### Phase 1: Foundation & Basic HTTP (2-3 weeks)

**Goal:** MVP with basic HTTP/TCP scenarios

**Deliverables:**
- [ ] CLI skeleton (Python/Rust)
- [ ] YAML config parser
- [ ] Docker backend manager
- [ ] vegeta wrapper (HTTP/1.1)
- [ ] TCP benchmark tool (custom or wrapper)
- [ ] Sequential runner
- [ ] JSON result exporter
- [ ] Scenarios: 01 (TCP), 02 (HTTP)

**Acceptance Criteria:**
- Can run scenario-01 and scenario-02
- Results in JSON format
- Success/failure determination
- Basic error handling

---

### Phase 2: Advanced Protocols (2-3 weeks)

**Goal:** Add HTTPS, WebSocket, gRPC support

**Deliverables:**
- [ ] k6 wrapper (HTTP/2, WebSocket)
- [ ] ghz wrapper (gRPC)
- [ ] Certificate generator utility
- [ ] TLS configuration
- [ ] Scenarios: 03 (HTTPS), 06 (WebSocket), 07 (gRPC)
- [ ] HTML report generation

**Acceptance Criteria:**
- HTTPS with self-signed certs working
- WebSocket load testing functional
- gRPC load testing functional
- HTML reports with graphs

---

### Phase 3: Database & Specialized Protocols (2-3 weeks)

**Goal:** Add database, GraphQL, PHP-FPM support

**Deliverables:**
- [ ] sysbench wrapper (MySQL)
- [ ] pgbench wrapper (PostgreSQL)
- [ ] redis-benchmark wrapper (Redis)
- [ ] GraphQL k6 script generation
- [ ] PHP-FPM backend setup
- [ ] Scenarios: 08 (Database), 13 (GraphQL), 14 (PHP-FPM)

**Acceptance Criteria:**
- Database load testing working
- GraphQL queries/mutations tested
- PHP-FPM FastCGI protocol working

---

### Phase 4: Parallel Execution & Advanced Features (2-3 weeks)

**Goal:** Parallel testing, advanced scenarios

**Deliverables:**
- [ ] Port allocation system
- [ ] Parallel runner with isolation
- [ ] Resource manager (CPU, memory limits)
- [ ] Rate limiting validation (scenario-04)
- [ ] WAF testing (scenario-09)
- [ ] Caching validation (scenario-11)
- [ ] Service discovery (scenario-12)
- [ ] Scenarios: 04, 09, 10, 11, 12

**Acceptance Criteria:**
- Can run all 15 scenarios in parallel
- Time reduction: 30-45 min → 5-10 min
- Resource limits enforced
- No port/container conflicts

---

### Phase 5: Polish & Production Ready (1-2 weeks)

**Goal:** Production-ready release

**Deliverables:**
- [ ] Web dashboard (real-time monitoring)
- [ ] Prometheus metrics export
- [ ] Baseline comparison engine
- [ ] CI/CD integration (GitHub Actions)
- [ ] Comprehensive documentation
- [ ] Installation scripts
- [ ] Pre-built backend images (Docker Hub)
- [ ] Release v1.0.0

**Acceptance Criteria:**
- All 15 scenarios passing
- Complete documentation
- CI/CD workflows working
- Docker images published
- Ready for community use

---

## Development Timeline

### Gantt Chart (9 weeks)

```
Week 1-2:   Phase 1 - Foundation & Basic HTTP
Week 3-4:   Phase 2 - Advanced Protocols
Week 5-6:   Phase 3 - Database & Specialized
Week 7-8:   Phase 4 - Parallel Execution
Week 9:     Phase 5 - Polish & Release
```

### Milestones

- **Week 2:** MVP - Can test TCP and HTTP scenarios
- **Week 4:** Protocol coverage - HTTP/2, HTTPS, WebSocket, gRPC
- **Week 6:** Database testing - MySQL, PostgreSQL, Redis
- **Week 8:** Parallel execution - All 15 scenarios in 5-10 minutes
- **Week 9:** v1.0.0 release - Production ready

---

## Resource Requirements

### Development Team

**Minimum:**
- 1 full-time developer (Python/Rust)
- 20-40 hours/week
- Duration: 9 weeks

**Recommended:**
- 1 lead developer (backend/infrastructure)
- 1 DevOps engineer (Docker, CI/CD)
- Part-time: 10-20 hours/week each
- Duration: 6-8 weeks

### Infrastructure

**Development:**
- Linux machine (16 cores, 32GB RAM)
- Docker/Podman
- 50GB disk space

**CI/CD:**
- GitHub Actions (free tier sufficient)
- Docker Hub account (free tier)
- Optional: Self-hosted runner for faster builds

### Tools & Dependencies

**Required:**
- Docker 24+
- Python 3.11+ or Rust 1.75+
- Load testing tools (vegeta, k6, ghz, etc.)

**Optional:**
- Redis (for caching tests)
- Consul/etcd (for service discovery)
- MaxMind GeoIP database (for geo-LB tests)

---

## Deliverables

### Code Deliverables

1. **Source code** (Apache 2.0 license)
   - GitHub repository: `highperapp/universal-testbed`
   - Well-documented, tested code
   - CI/CD pipelines

2. **Docker images** (published to Docker Hub)
   - `testbed/http-backend:latest`
   - `testbed/tcp-echo:latest`
   - `testbed/grpc-server:latest`
   - `testbed/websocket-server:latest`
   - `testbed/graphql-server:latest`

3. **CLI binary releases**
   - Linux (x86_64, arm64)
   - macOS (x86_64, arm64)
   - Windows (x86_64)

### Documentation Deliverables

1. **User Documentation**
   - Getting Started Guide
   - Configuration Reference
   - Scenario Writing Guide
   - Troubleshooting Guide

2. **Developer Documentation**
   - Architecture Overview
   - Adding New Tools
   - Contributing Guide
   - API Reference

3. **Example Scenarios**
   - All 15 Highper Gateway scenarios
   - 5 generic proxy scenarios
   - Custom scenario templates

---

## Success Criteria

### Technical Success Criteria

- ✅ All 15 Highper Gateway scenarios implemented
- ✅ Parallel execution reduces time by 70%+ (30-45 min → 5-10 min)
- ✅ Support for 7+ protocols (TCP, HTTP/1-3, WS, gRPC, DB, GraphQL)
- ✅ 90%+ test coverage for core modules
- ✅ CI/CD integration working

### User Experience Criteria

- ✅ Single-command scenario execution
- ✅ Clear, actionable error messages
- ✅ Progress indicators during tests
- ✅ Comprehensive HTML reports
- ✅ Easy YAML configuration

### Community Adoption Criteria

- ✅ 100+ GitHub stars in first month
- ✅ 5+ external contributors
- ✅ Used by 3+ projects beyond Highper Gateway
- ✅ Comprehensive documentation
- ✅ Active issue resolution (< 7 day response time)

---

## Risk Assessment

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Tool compatibility issues | Medium | High | Extensive testing, fallback tools |
| Docker performance bottleneck | Low | Medium | Optimize images, use host networking |
| Port allocation conflicts | Medium | Medium | Dynamic port allocation, cleanup |
| Parallel execution race conditions | Medium | High | Proper isolation, resource locking |
| Protocol-specific tool bugs | Low | Low | Use mature, stable tools |

### Project Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Scope creep | High | High | Strict phase gates, MVP focus |
| Timeline delays | Medium | Medium | 20% buffer time, phased delivery |
| Resource constraints | Low | High | Start with Python MVP, iterate |
| Dependency updates breaking changes | Low | Low | Pin versions, regular updates |

### Mitigation Strategies

1. **Scope Management**
   - Define clear phase deliverables
   - Defer nice-to-have features to post-v1.0
   - Regular progress reviews

2. **Technical Debt**
   - Code reviews for all changes
   - Automated testing (unit, integration)
   - Documentation as code

3. **Community Building**
   - Early release (alpha/beta)
   - Solicit feedback
   - Clear contribution guidelines

---

## Future Enhancements (Post-v1.0)

### Short-term (v1.1-v1.3)

- [ ] Kubernetes backend support
- [ ] Cloud provider integration (AWS, GCP, Azure)
- [ ] Distributed load generation (multi-machine)
- [ ] Real-time web dashboard
- [ ] Custom metric plugins

### Medium-term (v1.4-v2.0)

- [ ] Machine learning-based performance analysis
- [ ] Automated bottleneck detection
- [ ] Cost optimization recommendations
- [ ] Multi-cluster testing
- [ ] Performance regression detection

### Long-term (v2.0+)

- [ ] SaaS offering (hosted testbed)
- [ ] Visual scenario builder (GUI)
- [ ] Integration with APM tools (Datadog, New Relic)
- [ ] Cloud-native performance testing
- [ ] AI-powered test generation

---

## Conclusion

This standalone universal testbed will:

1. **Accelerate testing** - 70% time reduction through parallel execution
2. **Improve coverage** - All 15 Highper Gateway scenarios validated
3. **Enable reusability** - Test any reverse proxy, not just Highper Gateway
4. **Foster community** - Open-source, extensible framework
5. **Ensure quality** - Automated, repeatable, consistent testing

**Next Steps:**
1. Review and approve this plan
2. Allocate resources (1-2 developers, 6-9 weeks)
3. Set up GitHub repository
4. Begin Phase 1 implementation
5. Deliver MVP in 2-3 weeks

**Estimated ROI:**
- Development cost: 6-9 weeks (1-2 developers)
- Time saved per test run: 25-40 minutes
- Annual test runs: ~500 (daily tests + CI/CD)
- Annual time saved: 200-330 hours (5-8 weeks)
- **Break-even: 2 months of use**

---

**Document Status:** Ready for Implementation
**Approval Required:** Yes
**Next Review Date:** After Phase 1 completion

**Questions or Feedback?**
Open an issue at: https://github.com/highperapp/universal-testbed/issues
