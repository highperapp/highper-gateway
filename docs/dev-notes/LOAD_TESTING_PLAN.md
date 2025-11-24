# Load Testing Plan - Rust Reverse Proxy

**Document Version:** 1.0
**Date:** November 17, 2025
**Status:** Active

## Table of Contents

1. [Overview](#overview)
2. [Testing Tools](#testing-tools)
3. [Expected Performance Targets](#expected-performance-targets)
4. [Test Scenarios](#test-scenarios)
5. [Test Execution Plan](#test-execution-plan)
6. [Success Criteria](#success-criteria)
7. [Infrastructure Requirements](#infrastructure-requirements)
8. [Results Documentation](#results-documentation)

---

## Overview

This document outlines the comprehensive load testing strategy for the Rust Reverse Proxy. The goal is to validate performance characteristics, identify bottlenecks, and ensure the proxy meets production-ready performance targets.

### Objectives

1. **Baseline Performance**: Establish baseline metrics for throughput and latency
2. **Scalability**: Validate performance under increasing load (100 → 10,000+ concurrent connections)
3. **Stability**: Ensure stable operation under sustained high load
4. **Resource Efficiency**: Measure CPU and memory usage under various load patterns
5. **Competitive Validation**: Compare against nginx, Caddy, HAProxy benchmarks

---

## Testing Tools

### Primary Tools (Installed)

| Tool | Version | Purpose | Strengths |
|------|---------|---------|-----------|
| **k6** | v0.48.0 | Complex scenario testing | JavaScript scripting, great reporting, cloud integration |
| **vegeta** | v12.11.1 | Constant-rate load testing | Precise rate control, simple, excellent for finding max throughput |

### Tool Selection by Use Case

- **k6**: Complex workflows, multi-stage tests, HTTP/2, WebSocket, detailed metrics
- **vegeta**: Constant throughput tests, simple HTTP benchmarks, quick validation

---

## Expected Performance Targets

### Reference Benchmarks (Competitor Data)

Based on public benchmarks and architectural capabilities:

| Proxy | Throughput (req/s) | Latency p50 | Latency p99 | Memory (1k conn) | CPU (1 core) |
|-------|-------------------|-------------|-------------|------------------|--------------|
| **nginx** | 50,000-80,000 | 1-2ms | 5-10ms | ~10 MB | ~60% @ 50k req/s |
| **Caddy** | 30,000-50,000 | 2-5ms | 10-20ms | ~20 MB | ~70% @ 30k req/s |
| **HAProxy** | 60,000-100,000 | 1-3ms | 8-15ms | ~15 MB | ~50% @ 60k req/s |
| **Envoy** | 40,000-70,000 | 2-4ms | 10-25ms | ~30 MB | ~80% @ 40k req/s |

### Highper Gateway Performance Targets

Based on Rust's zero-cost abstractions, async runtime efficiency, and current architecture:

#### Target Tier 1: Baseline (MUST ACHIEVE)
**Goal**: Match nginx baseline performance

| Metric | Target | Rationale |
|--------|--------|-----------|
| **Max Throughput** | ≥ 50,000 req/s | Competitive with nginx baseline |
| **Latency p50** | ≤ 5ms | Acceptable for most use cases |
| **Latency p95** | ≤ 15ms | Good tail latency |
| **Latency p99** | ≤ 50ms | Acceptable worst-case |
| **Memory** | ≤ 50 MB @ 1k connections | Efficient memory usage |
| **CPU** | ≤ 70% @ 50k req/s (single core) | Efficient CPU utilization |
| **Error Rate** | < 0.1% | High reliability |
| **Connection Pool Hit Rate** | ≥ 80% | Effective connection reuse |

#### Target Tier 2: Competitive (SHOULD ACHIEVE)
**Goal**: Exceed nginx, approach HAProxy

| Metric | Target | Rationale |
|--------|--------|-----------|
| **Max Throughput** | ≥ 75,000 req/s | Better than nginx, approaching HAProxy |
| **Latency p50** | ≤ 3ms | Excellent response time |
| **Latency p95** | ≤ 10ms | Great tail latency |
| **Latency p99** | ≤ 25ms | Low worst-case latency |
| **Memory** | ≤ 30 MB @ 1k connections | Very efficient |
| **CPU** | ≤ 60% @ 75k req/s | Excellent efficiency |
| **WebSocket Throughput** | ≥ 100k messages/s | High-performance WebSocket |
| **TLS Handshakes** | ≥ 10,000/s | Fast TLS termination |

#### Target Tier 3: Exceptional (STRETCH GOAL)
**Goal**: Compete with HAProxy and specialized proxies

| Metric | Target | Rationale |
|--------|--------|-----------|
| **Max Throughput** | ≥ 100,000 req/s | HAProxy-level performance |
| **Latency p50** | ≤ 2ms | Near-optimal |
| **Latency p95** | ≤ 5ms | Excellent tail latency |
| **Latency p99** | ≤ 15ms | Minimal worst-case |
| **Memory** | ≤ 20 MB @ 1k connections | Extremely efficient |
| **HTTP/3 Throughput** | ≥ 40,000 req/s | Leading-edge protocol support |

### Performance by Protocol

| Protocol | Expected Throughput | Expected Latency (p95) | Priority |
|----------|---------------------|------------------------|----------|
| **HTTP/1.1** | 50,000-100,000 req/s | ≤ 10ms | Critical |
| **HTTP/2** | 40,000-80,000 req/s | ≤ 15ms | High |
| **HTTP/3 (QUIC)** | 30,000-50,000 req/s | ≤ 20ms | Medium |
| **WebSocket** | 100,000+ msg/s | ≤ 5ms | High |
| **TLS Passthrough** | 80,000-120,000 req/s | ≤ 8ms | High |
| **gRPC** | 30,000-60,000 req/s | ≤ 15ms | Medium |

---

## Test Scenarios

### Scenario 1: Simple HTTP GET (Baseline)
**File**: `scripts/k6-http-simple.js`
**Purpose**: Establish baseline throughput and latency

**Configuration**:
- Virtual Users: 100 → 500
- Duration: 3 minutes
- Request: `GET /`
- Expected Throughput: 50,000+ req/s
- Expected p95 Latency: < 10ms

**Success Criteria**:
- ✅ Throughput ≥ 50,000 req/s
- ✅ p95 latency ≤ 15ms
- ✅ p99 latency ≤ 50ms
- ✅ Error rate < 0.1%

### Scenario 2: HTTP POST with JSON Payload
**File**: `scripts/k6-http-post.js`
**Purpose**: Test request body processing and parsing

**Configuration**:
- Virtual Users: 50 → 200
- Duration: 3 minutes
- Request: `POST /api/data` with 1KB JSON payload
- Expected Throughput: 30,000+ req/s
- Expected p95 Latency: < 20ms

**Success Criteria**:
- ✅ Throughput ≥ 30,000 req/s
- ✅ p95 latency ≤ 20ms
- ✅ p99 latency ≤ 75ms
- ✅ Error rate < 0.1%

### Scenario 3: HTTPS/TLS Termination
**File**: `scripts/k6-https-tls.js`
**Purpose**: Test TLS handshake and encrypted traffic performance

**Configuration**:
- Virtual Users: 100 → 400
- Duration: 3 minutes
- Request: `GET https://localhost:8443/`
- Expected Throughput: 40,000+ req/s
- Expected TLS Handshake: < 50ms (p95)
- Expected p95 Latency: < 25ms

**Success Criteria**:
- ✅ Throughput ≥ 40,000 req/s
- ✅ TLS handshake p95 ≤ 100ms
- ✅ p95 latency ≤ 25ms
- ✅ p99 latency ≤ 100ms
- ✅ Error rate < 0.1%

### Scenario 4: WebSocket Connections
**File**: `scripts/k6-websocket.js`
**Purpose**: Test WebSocket upgrade and bidirectional messaging

**Configuration**:
- Virtual Users: 50 → 200
- Duration: 3 minutes
- Connection lifetime: 5 seconds
- Messages per connection: 10
- Expected Message Throughput: 100,000+ msg/s
- Expected Message Latency: < 10ms (p95)

**Success Criteria**:
- ✅ Connection success rate ≥ 99%
- ✅ Message throughput ≥ 100,000 msg/s
- ✅ Message p95 latency ≤ 10ms
- ✅ Connection p95 latency ≤ 200ms

### Scenario 5: Constant Rate Throughput (vegeta)
**File**: `scenarios/vegeta-http-simple.txt`
**Purpose**: Find maximum sustainable throughput

**Configuration**:
```bash
# Ramp test to find max throughput
for rate in 1000 5000 10000 25000 50000 75000 100000; do
  ./scripts/run-vegeta-test.sh vegeta-http-simple.txt $rate 30s
done
```

**Success Criteria**:
- ✅ Identify max sustainable rate where p99 < 100ms
- ✅ Document degradation curve
- ✅ Error rate < 0.1% at max rate

### Scenario 6: Connection Pool Efficiency
**Purpose**: Validate connection pooling effectiveness

**Configuration**:
- Backend: Single upstream server
- Load pattern: Sustained 10,000 req/s for 5 minutes
- Connection pool: max 100 connections

**Metrics to Collect**:
- Connection pool hit rate
- Idle connection count over time
- New connection rate
- Connection reuse count

**Success Criteria**:
- ✅ Connection pool hit rate ≥ 80%
- ✅ Idle connections stabilize within 2 minutes
- ✅ New connection rate < 10/s after warmup

### Scenario 7: Load Balancing Distribution
**Purpose**: Validate load balancer fairness and efficiency

**Configuration**:
- Backends: 3 upstream servers (equal capacity)
- Load balancing: Round-robin
- Duration: 5 minutes
- Load: 30,000 req/s

**Success Criteria**:
- ✅ Request distribution: 33% ± 2% per backend
- ✅ No backend gets > 40% of traffic
- ✅ Latency variance < 20% across backends

### Scenario 8: Spike Test
**Purpose**: Test behavior under sudden traffic spikes

**Configuration**:
```javascript
stages: [
  { duration: '1m', target: 100 },   // Normal load
  { duration: '10s', target: 2000 }, // Sudden spike
  { duration: '1m', target: 2000 },  // Sustained high load
  { duration: '10s', target: 100 },  // Spike down
  { duration: '1m', target: 100 },   // Return to normal
]
```

**Success Criteria**:
- ✅ No errors during spike
- ✅ Recovery within 10 seconds
- ✅ Latency returns to baseline within 30 seconds

### Scenario 9: Soak/Endurance Test
**Purpose**: Validate stability under sustained load

**Configuration**:
- Virtual Users: 500 (constant)
- Duration: 2 hours
- Request Rate: ~25,000 req/s
- Monitor: Memory leaks, CPU drift, connection leaks

**Success Criteria**:
- ✅ No memory growth > 5% over 2 hours
- ✅ CPU usage stable (variance < 10%)
- ✅ No connection leaks
- ✅ Latency stable (p95 variance < 20%)
- ✅ Error rate < 0.01%

### Scenario 10: Breakpoint Test
**Purpose**: Find the breaking point

**Configuration**:
```javascript
stages: [
  { duration: '2m', target: 100 },
  { duration: '2m', target: 500 },
  { duration: '2m', target: 1000 },
  { duration: '2m', target: 2000 },
  { duration: '2m', target: 5000 },
  { duration: '2m', target: 10000 },
  // Continue until failure
]
```

**Success Criteria**:
- ✅ Document max VU before errors
- ✅ Document max throughput before degradation
- ✅ Document failure mode (graceful vs cascade)

---

## Test Execution Plan

### Phase 1: Baseline Testing (Days 1-2)
**Goal**: Establish baseline performance metrics

**Tests**:
1. Run Scenario 1 (Simple HTTP GET)
2. Run Scenario 2 (HTTP POST)
3. Run Scenario 5 (vegeta constant rate)

**Deliverables**:
- Baseline throughput metrics
- Baseline latency distribution
- Initial bottleneck identification

### Phase 2: Protocol Testing (Days 3-4)
**Goal**: Validate performance across all supported protocols

**Tests**:
1. Run Scenario 3 (HTTPS/TLS)
2. Run Scenario 4 (WebSocket)
3. Add HTTP/2 scenario
4. Add gRPC scenario (if applicable)

**Deliverables**:
- Per-protocol performance profiles
- TLS overhead measurement
- WebSocket performance characteristics

### Phase 3: Feature Testing (Days 5-6)
**Goal**: Validate feature-specific performance

**Tests**:
1. Run Scenario 6 (Connection pooling)
2. Run Scenario 7 (Load balancing)
3. Test rate limiting overhead
4. Test circuit breaker overhead

**Deliverables**:
- Connection pool efficiency report
- Load balancer distribution analysis
- Feature overhead measurements

### Phase 4: Stress Testing (Days 7-8)
**Goal**: Test stability and limits

**Tests**:
1. Run Scenario 8 (Spike test)
2. Run Scenario 9 (Soak test - 2 hours)
3. Run Scenario 10 (Breakpoint test)

**Deliverables**:
- Stability report
- Maximum capacity documentation
- Failure mode analysis

### Phase 5: Comparative Testing (Days 9-10)
**Goal**: Compare against nginx/Caddy/HAProxy

**Tests**:
1. Set up identical nginx configuration
2. Run identical load tests
3. Document performance delta
4. Identify areas for optimization

**Deliverables**:
- Comparative performance report
- Competitive positioning validation
- Optimization recommendations

---

## Success Criteria

### Overall Success Metrics

The load testing phase is considered **successful** if:

#### Tier 1 (Baseline - MUST PASS)
- ✅ **Throughput**: ≥ 50,000 req/s (HTTP/1.1)
- ✅ **Latency p95**: ≤ 15ms
- ✅ **Latency p99**: ≤ 50ms
- ✅ **Error Rate**: < 0.1% under normal load
- ✅ **Memory**: ≤ 50 MB for 1,000 connections
- ✅ **Stability**: 2-hour soak test with no leaks

#### Tier 2 (Competitive - SHOULD PASS)
- ✅ **Throughput**: ≥ 75,000 req/s
- ✅ **Latency p95**: ≤ 10ms
- ✅ **Latency p99**: ≤ 25ms
- ✅ **TLS Throughput**: ≥ 40,000 req/s
- ✅ **WebSocket**: ≥ 100,000 msg/s

#### Tier 3 (Exceptional - NICE TO HAVE)
- ✅ **Throughput**: ≥ 100,000 req/s
- ✅ **Latency p50**: ≤ 2ms
- ✅ **Competitive**: Within 10% of HAProxy performance

### Failure Conditions

The following conditions indicate **critical issues** requiring immediate attention:

- ❌ Throughput < 30,000 req/s
- ❌ Error rate > 1% under normal load
- ❌ p99 latency > 500ms
- ❌ Memory leaks detected in soak test
- ❌ Connection leaks detected
- ❌ Crashes under sustained load
- ❌ Performance degrades > 50% vs nginx

---

## Infrastructure Requirements

### Test Machine Specifications

**Recommended**:
- CPU: 8+ cores (AMD Ryzen 9 / Intel i7-12700K or better)
- RAM: 16 GB minimum, 32 GB recommended
- Network: 10 Gbps NIC (or loopback testing)
- OS: Linux (Ubuntu 22.04 LTS or newer)
- Kernel: 5.15+ (for io_uring support)

### Backend Server Setup

For realistic testing, use a simple backend server:

```bash
# Simple HTTP backend (Python)
python3 -m http.server 9000

# Or nginx backend
nginx -c nginx-backend.conf
```

### Monitoring Setup

Monitor these metrics during tests:
- CPU usage (per-core and overall): `htop`, `mpstat`
- Memory usage: `free -m`, `smem`
- Network throughput: `iftop`, `nethogs`
- File descriptors: `lsof | wc -l`
- Connection states: `ss -s`
- Disk I/O: `iostat`

**Recommended Tools**:
```bash
# Install monitoring tools
sudo apt install -y htop iftop nethogs sysstat dstat

# During test, run:
dstat -tcmndl 1
```

### Load Generator Placement

**Best Practice**:
- Run load generator on **separate machine** from proxy
- Use direct network connection (avoid WiFi)
- Use multiple load generators for > 100k req/s

**Alternative (Single Machine)**:
- For < 50k req/s, same machine is acceptable
- Use loopback interface (127.0.0.1)
- Monitor CPU contention

---

## Results Documentation

### Required Artifacts

For each test scenario, document:

1. **Test Configuration**
   - Script/scenario file
   - Command line used
   - Duration and load pattern
   - Target URL and backend setup

2. **Raw Results**
   - k6 JSON output
   - vegeta binary results
   - System metrics (CPU, memory, network)

3. **Summary Metrics**
   - Throughput (req/s)
   - Latency distribution (p50, p95, p99, max)
   - Error rate
   - Resource usage

4. **Analysis**
   - Bottlenecks identified
   - Unexpected behavior
   - Optimization opportunities

### Results Directory Structure

```
load-tests/results/
├── baseline/
│   ├── http-simple_20251117_143022.json
│   ├── http-post_20251117_143534.json
│   └── vegeta_1000rps_20251117_144012.bin
├── protocols/
│   ├── https-tls_20251118_091234.json
│   ├── websocket_20251118_102345.json
│   └── http2_20251118_113456.json
├── stress/
│   ├── spike_20251119_081234.json
│   ├── soak_20251119_140000.json
│   └── breakpoint_20251119_183045.json
└── comparative/
    ├── highper-gateway_20251120_100000.json
    ├── nginx_20251120_110000.json
    └── comparison_report.md
```

### Final Report Template

Create `LOAD_TEST_RESULTS.md` with:

```markdown
# Load Test Results - Rust Reverse Proxy

## Executive Summary
- Max Throughput Achieved: XXX req/s
- Latency p95/p99: XXms / XXms
- Tier Achieved: [Tier 1 / Tier 2 / Tier 3]
- Overall Status: [PASS / FAIL]

## Detailed Results
[Tables and graphs]

## Bottlenecks Identified
[List of bottlenecks]

## Optimization Recommendations
[Actionable improvements]

## Competitive Comparison
[vs nginx, Caddy, HAProxy]
```

---

## Appendix: Quick Start

### Running First Load Test

```bash
# 1. Start the proxy
cd highper-gateway
cargo build --release
./target/release/highper-gateway --config config.toml

# 2. Start a backend server (separate terminal)
python3 -m http.server 9000

# 3. Run a simple load test (separate terminal)
cd load-tests/scripts
./run-k6-test.sh k6-http-simple.js

# 4. Or run vegeta test
./run-vegeta-test.sh vegeta-http-simple.txt 1000 30s
```

### Interpreting Results

**k6 Output**:
```
http_req_duration..............: avg=5.23ms  p(95)=12.45ms p(99)=25.67ms
http_reqs......................: 150000  5000/s
http_req_failed................: 0.03%
```
- ✅ GOOD: p95 < 15ms, throughput 5000/s, error rate < 0.1%

**vegeta Output**:
```
Requests      [total, rate, throughput]  30000, 1000.00, 998.23
Duration      [total, attack, wait]      30.02s, 30s, 15.234ms
Latencies     [min, mean, 50, 90, 95, 99, max]
              823µs, 5.234ms, 4.123ms, 9.876ms, 12.345ms, 25.678ms, 125.432ms
Success       [ratio]                    99.97%
```
- ✅ GOOD: Throughput ~1000 req/s, p95 12.3ms, success 99.97%

---

## Next Steps

After load testing completion:
1. Create `LOAD_TEST_RESULTS.md` with findings
2. File GitHub issues for identified bottlenecks
3. Update `COMPREHENSIVE_EVALUATION_2025.md` with results
4. Proceed to E2E testing (Days 3-4)

---

**Document Status**: Ready for use
**Estimated Testing Duration**: 10 days (2 hours/day)
**Expected Completion**: November 27, 2025
