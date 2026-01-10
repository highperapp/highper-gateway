# Comprehensive Stability Plan

**Date**: 2025-11-27
**Purpose**: Systematic approach to achieve production-ready stability
**Goal**: Build reliable foundations before any cloud deployment

---

## Lessons from Recent Failures

### What Went Wrong in Vultr Test

1. **Panic reactions**: Suggested nginx instead of understanding DSL syntax
2. **No local validation**: Deployed to cloud without testing locally first
3. **Missing examples**: No working configurations to reference
4. **Cost**: $3.91 wasted on debugging what should have been caught locally

### Root Causes Identified

- Insufficient local testing infrastructure
- Lack of validated configuration examples
- Missing observability/debugging tools
- No systematic validation workflow
- Rushed to cloud before ensuring basics work

---

## PHASE 1: Core Stability (Priority: CRITICAL)

### 1.1 Bug Fixes and Core Functionality ✅ (Partially Complete)

**Status**: DSL syntax issues understood, validation tools created

**Remaining Work**:
- [ ] Verify DSL parser handles all edge cases
- [ ] Test configuration loading from file
- [ ] Validate all load balancing algorithms work
- [ ] Test health checks actually mark backends as unhealthy
- [ ] Verify connection pooling doesn't leak connections
- [ ] Test graceful shutdown works correctly

**Validation Method**: Unit tests + integration tests

### 1.2 Documentation Examples ✅ (Complete)

**Status**: Created validated examples and comprehensive guide

**Files**:
- ✅ `examples/simple-load-balancer.proxy` - Local testing
- ✅ `examples/cloud-load-balancer.proxy` - Cloud deployment
- ✅ `examples/README.md` - Usage guide

**Additional Examples Needed**:
- [ ] TCP load balancing (MySQL, PostgreSQL, Redis)
- [ ] WebSocket proxy configuration
- [ ] gRPC load balancing
- [ ] Multi-domain routing
- [ ] TLS termination examples

### 1.3 Observability (CRITICAL - Missing)

**Current State**: Limited logging, no structured metrics

**Required Improvements**:

#### A. Logging Enhancements
```rust
// Need structured logging with context
- Request ID tracking
- Backend selection logging
- Health check state changes
- Connection pool statistics
- Error details with context
```

**Action Items**:
- [ ] Add request ID generation and propagation
- [ ] Log every backend selection decision
- [ ] Log health check results (pass/fail/timeout)
- [ ] Log connection pool stats every 30s
- [ ] Add error categorization (config, network, backend, timeout)

#### B. Metrics (Prometheus)
**Status**: Mentioned in config, implementation unknown

**Required Metrics**:
```
# Request metrics
http_requests_total{method, path, status}
http_request_duration_seconds{method, path}
http_requests_in_flight

# Backend metrics
backend_requests_total{backend, status}
backend_request_duration_seconds{backend}
backend_healthy{backend}  # 1 = healthy, 0 = unhealthy
backend_connections_active{backend}

# Load balancer metrics
lb_backend_selected_total{backend, algorithm}
lb_no_healthy_backend_total

# Connection pool metrics (for TCP)
pool_connections_active{upstream}
pool_connections_idle{upstream}
pool_connections_created_total{upstream}
pool_connections_closed_total{upstream}
```

**Action Items**:
- [ ] Verify Prometheus metrics are actually exposed
- [ ] Add all critical metrics listed above
- [ ] Create Grafana dashboard JSON
- [ ] Document metrics in `docs/OBSERVABILITY.md`

#### C. Health Check Visibility
**Current**: Unknown if health checks are working

**Required**:
- [ ] Admin API endpoint: `GET /admin/health` - Show all backends status
- [ ] Admin API endpoint: `GET /admin/backends` - Backend details + last check time
- [ ] Admin API endpoint: `GET /admin/stats` - Real-time statistics
- [ ] Logs showing health check transitions (healthy → unhealthy)

---

## PHASE 2: Local Testing Infrastructure (Priority: HIGH)

### 2.1 Docker Compose Setup ✅ (Validation scripts done, docker-compose needed)

**Goal**: Test full load balancing locally without any cloud resources

**Required Files**:

#### `docker-compose.loadtest.yml`
```yaml
version: '3.8'

services:
  # 3 backend services
  backend1:
    image: python:3.11-slim
    command: python -m http.server 8080
    networks:
      - loadtest
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/"]
      interval: 10s

  backend2:
    image: python:3.11-slim
    command: python -m http.server 8080
    networks:
      - loadtest

  backend3:
    image: python:3.11-slim
    command: python -m http.server 8080
    networks:
      - loadtest

  # Gateway (build from source)
  gateway:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
      - "9090:9090"  # Admin API
    volumes:
      - ./examples/docker-compose-lb.proxy:/config/gateway.proxy:ro
    command: start -c /config/gateway.proxy
    networks:
      - loadtest
    depends_on:
      - backend1
      - backend2
      - backend3

  # Load generator
  vegeta:
    image: peterevans/vegeta:latest
    networks:
      - loadtest
    depends_on:
      - gateway
    entrypoint: /bin/sh
    command: -c "sleep 10 && echo 'GET http://gateway:8080/' | vegeta attack -duration=30s -rate=100 | vegeta report"

networks:
  loadtest:
    driver: bridge
```

**Action Items**:
- [ ] Create `docker-compose.loadtest.yml`
- [ ] Create `examples/docker-compose-lb.proxy` config
- [ ] Create `Dockerfile` (multi-stage build)
- [ ] Create `scripts/test-docker.sh` - Run full docker-compose test
- [ ] Test and validate everything works locally

### 2.2 Rancher Desktop Fallback

**Note**: Only if docker-compose doesn't perform well

**Status**: Not recommended due to past performance issues

**Alternative**: Use plain Docker instead of Rancher Desktop if issues occur

### 2.3 Comprehensive Local Test Suite

**Goal**: Catch all issues before cloud deployment

**Test Scenarios**:

1. **Basic Load Balancing**
   - [ ] 3 backends, round-robin, verify even distribution
   - [ ] Verify all backends receive requests
   - [ ] Check request logs

2. **Health Check Handling**
   - [ ] Stop one backend, verify it's marked unhealthy
   - [ ] Verify requests only go to healthy backends
   - [ ] Restart backend, verify it becomes healthy again

3. **Load Balancing Algorithms**
   - [ ] Test round-robin distribution
   - [ ] Test least_conn (simulate uneven load)
   - [ ] Test ip_hash (same client → same backend)

4. **Connection Handling**
   - [ ] Test concurrent requests (100+ simultaneous)
   - [ ] Test keep-alive connections
   - [ ] Test connection timeouts

5. **Error Handling**
   - [ ] All backends down → proper error response
   - [ ] Backend timeout → failover to another backend
   - [ ] Backend returns 5xx → try another backend (if configured)

**Script**: `scripts/run-comprehensive-tests.sh`

---

## PHASE 3: Infrastructure API Management (Priority: MEDIUM)

### 3.1 API Key Management

**Goal**: Centralized, secure API key storage and validation

#### File: `.env.infrastructure` (gitignored)
```bash
# Vultr
VULTR_API_KEY="your-key-here"
VULTR_SSH_KEY_ID="your-ssh-key-id"

# Hetzner
HETZNER_API_TOKEN="your-token-here"

# PhoenixNAP
PNAP_CLIENT_ID="your-client-id"
PNAP_CLIENT_SECRET="your-secret"

# AWS (if using)
AWS_ACCESS_KEY_ID="your-key"
AWS_SECRET_ACCESS_KEY="your-secret"

# DigitalOcean (if using)
DO_TOKEN="your-token"
```

**Security**:
- [ ] Add `.env.infrastructure` to `.gitignore`
- [ ] Create `.env.infrastructure.example` template
- [ ] Document how to obtain each API key
- [ ] Add validation script to check keys work

### 3.2 Instance Availability Validation

**Goal**: Before any provisioning, validate available instances

#### Script: `scripts/infrastructure/check-availability.sh`
```bash
#!/bin/bash
# Check available instances across all providers

PROVIDER=${1:-all}  # vultr, hetzner, phoenixnap, or all

check_vultr() {
    echo "=== Vultr Availability ==="
    echo ""
    echo "Cloud Compute (vc2):"
    curl -s "https://api.vultr.com/v2/plans" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.plans[] | select(.type=="vc2") |
               "\(.id): \(.vcpu_count)vCPU, \(.ram)MB RAM, $\(.monthly_cost/100)/mo"'

    echo ""
    echo "High Frequency (vhf):"
    curl -s "https://api.vultr.com/v2/plans" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.plans[] | select(.type=="vhf") |
               "\(.id): \(.vcpu_count)vCPU, \(.ram)MB RAM, $\(.monthly_cost/100)/mo"'

    echo ""
    echo "Available Regions:"
    curl -s "https://api.vultr.com/v2/regions" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.regions[] | "\(.id): \(.city), \(.country)"'
}

check_hetzner() {
    echo "=== Hetzner Server Auction ==="
    echo ""
    echo "Visit: https://www.hetzner.com/sb"
    echo "Manual check required (no public API for auction)"
    echo ""
    echo "Typical availability:"
    echo "  - AX41 (Ryzen 5 3600, 64GB): €39/mo"
    echo "  - AX51 (Ryzen 7 3700X, 64GB): €54/mo"
    echo "  - AX101 (EPYC 7502P, 128GB): €189/mo"
}

check_phoenixnap() {
    echo "=== PhoenixNAP Bare Metal ==="
    echo ""
    echo "Checking available server types..."
    # Requires OAuth token
    echo "Note: Requires valid API credentials"
}

case $PROVIDER in
    vultr)
        check_vultr
        ;;
    hetzner)
        check_hetzner
        ;;
    phoenixnap)
        check_phoenixnap
        ;;
    all)
        check_vultr
        echo ""
        echo "=================================================="
        echo ""
        check_hetzner
        echo ""
        echo "=================================================="
        echo ""
        check_phoenixnap
        ;;
    *)
        echo "Unknown provider: $PROVIDER"
        echo "Usage: $0 [vultr|hetzner|phoenixnap|all]"
        exit 1
        ;;
esac
```

**Action Items**:
- [ ] Create availability check script for each provider
- [ ] Test API access for each provider
- [ ] Document current pricing and availability
- [ ] Create automated pricing update script

### 3.3 15 Use Case Scenarios

**Goal**: Pre-configured scripts for all common load test scenarios

#### Scenario Categories:

**HTTP/HTTPS Load Balancing (5 scenarios)**:
1. Basic HTTP load balancing (3 backends, round-robin)
2. HTTPS with TLS termination (3 backends, least_conn)
3. Multi-domain routing (3 domains, 9 backends total)
4. Path-based routing (/api, /static, /admin)
5. High-concurrency test (10K+ concurrent connections)

**TCP Proxying (3 scenarios)**:
6. MySQL load balancing (primary + 2 replicas)
7. PostgreSQL HA (2 servers, least_conn)
8. Redis cluster (3 nodes, consistent_hash)

**Advanced Features (4 scenarios)**:
9. WebSocket load balancing
10. gRPC load balancing
11. Health check failover (simulate backend failures)
12. Connection pooling stress test

**Performance Testing (3 scenarios)**:
13. Latency characterization (measure p50, p95, p99)
14. Throughput test (max RPS)
15. Extended stability test (7-day soak test)

#### Directory Structure:
```
scripts/loadtest/scenarios/
├── 01-basic-http-lb/
│   ├── config.proxy
│   ├── README.md
│   ├── provision-vultr.sh
│   ├── provision-hetzner.sh
│   ├── deploy.sh
│   ├── test.sh
│   └── cleanup.sh
├── 02-https-tls-termination/
│   ├── ...
├── 03-multi-domain/
│   ├── ...
...
├── 15-extended-stability/
│   ├── ...
└── README.md  # Overview of all scenarios
```

**Action Items**:
- [ ] Create scenario directory structure
- [ ] Write configuration for each scenario
- [ ] Create provider-specific provisioning scripts
- [ ] Add validation steps to each scenario
- [ ] Document expected results

---

## PHASE 4: Stability Validation (Priority: CRITICAL)

### 4.1 Pre-Cloud Deployment Checklist

**MUST PASS ALL before spending money on cloud**:

#### Local Validation (FREE)
- [ ] Configuration validates: `./scripts/validate-config.sh -c config.proxy`
- [ ] Gateway starts successfully
- [ ] All backends reachable from gateway
- [ ] Load balancing distributes evenly
- [ ] Health checks mark unhealthy backends correctly
- [ ] Metrics endpoint returns data: `curl http://localhost:9090/metrics`
- [ ] Admin API responds: `curl http://localhost:9090/admin/health`
- [ ] Logs show clear decision-making
- [ ] No errors in logs during normal operation
- [ ] Graceful shutdown works (no connection drops)

#### Docker Compose Validation (FREE)
- [ ] `docker-compose up` starts all services
- [ ] Gateway connects to all backends
- [ ] Load test shows even distribution
- [ ] Simulated failure handled correctly
- [ ] No container crashes
- [ ] No memory leaks over 1-hour test

#### Documentation Validation
- [ ] All examples have been tested locally
- [ ] Configuration guide is accurate
- [ ] Troubleshooting guide addresses common issues
- [ ] API keys documented with how to obtain them
- [ ] Each scenario has clear README

### 4.2 Cloud Validation Steps (Minimal Cost)

**Only proceed here after local validation passes**

#### Step 1: Smoke Test (Cost: $0.12 for 1 hour)
- [ ] Deploy minimal setup (1 proxy + 2 backends)
- [ ] Run basic connectivity test
- [ ] Verify load balancing works
- [ ] Check logs for errors
- [ ] Destroy immediately

#### Step 2: Full Scenario Test (Cost: $0.24 for 2 hours)
- [ ] Deploy complete scenario (1 proxy + 3 backends + 1 generator)
- [ ] Run comprehensive load test
- [ ] Verify all metrics being collected
- [ ] Check health check behavior
- [ ] Validate admin API accessibility
- [ ] Destroy immediately

#### Step 3: Extended Test (Cost: $2-3 for 8 hours)
- [ ] Only if smoke test passes
- [ ] Run performance characterization
- [ ] Collect latency percentiles
- [ ] Measure throughput limits
- [ ] Check for memory leaks
- [ ] Verify stability over time

### 4.3 Stability Metrics

**Define success criteria before testing**:

#### Functional Requirements
- ✅ Load balancing distribution: ±5% variance
- ✅ Health check response time: < 1s
- ✅ Failover time: < 5s (backend failure → routes removed)
- ✅ Request success rate: > 99.9% (assuming backends healthy)

#### Performance Requirements
- ✅ p50 latency: < 5ms added latency
- ✅ p99 latency: < 50ms added latency
- ✅ Throughput: > 10K RPS per vCPU
- ✅ Memory usage: < 100MB base + 10KB per connection

#### Reliability Requirements
- ✅ No crashes during 8-hour test
- ✅ No memory leaks (memory stable over time)
- ✅ No connection leaks (connections released properly)
- ✅ Graceful handling of all error conditions

---

## PHASE 5: Cost Optimization (Priority: MEDIUM)

### 5.1 Infrastructure Cost Tracking

**Goal**: Track every dollar spent on testing

#### File: `load-tests/COST_TRACKING.md`
```markdown
# Infrastructure Cost Tracking

| Date | Provider | Test Type | Duration | Instances | Cost | Success | Notes |
|------|----------|-----------|----------|-----------|------|---------|-------|
| 2025-11-27 | Vultr | Layer 7 LB | 45 min | 5x vc2-2c-4gb | $3.91 | ❌ | Config issues |
| ... | ... | ... | ... | ... | ... | ... | ... |

## Total Spent: $3.91
## Successful Tests: 0
## Failed Tests: 1
## Cost per Successful Test: N/A
```

**Action Items**:
- [ ] Create cost tracking spreadsheet
- [ ] Log every deployment with cost
- [ ] Calculate ROI for preparation work
- [ ] Set monthly testing budget

### 5.2 Budget Allocation

**Proposed Monthly Budget**: $50-100

**Allocation**:
- $10-20: Smoke tests and quick validation (10-20 tests × $1 each)
- $20-40: Performance characterization (5-10 tests × $3-5 each)
- $20-40: Extended stability tests (1-2 tests × $20-30 each)

**Break-even Analysis**:
- Current: $3.91 spent, 0 successful tests
- After improvements: Expect 90%+ success rate
- Value of preparation: Prevents $10-20 in failed tests

---

## PHASE 6: Remote Infrastructure vs Local Hardware

### 6.1 Cost Comparison

#### High-Power Laptop Option
**Specs**: Ryzen 9 / Core i9, 64GB RAM, 2TB NVMe
**Cost**: $2000-3000 one-time

**Pros**:
- No ongoing costs
- Always available
- Good for development and local testing
- Sufficient for most load testing scenarios

**Cons**:
- Limited scalability (max 16-32 cores)
- Can't simulate distributed systems well
- No geographic diversity for latency testing
- Hardware becomes outdated

#### Remote Infrastructure Option
**Monthly Cost**: $50-200 depending on usage

**Pros**:
- Scalable to 100s of vCPUs when needed
- Pay only for what you use (cloud instances)
- Geographic diversity for realistic testing
- Easy to simulate distributed deployments
- Latest hardware always available

**Cons**:
- Ongoing costs
- Requires good internet connection
- API management overhead

### 6.2 Recommendation

**Hybrid Approach**:

1. **Local Development** (Laptop or existing hardware)
   - All configuration development
   - Local validation
   - Docker-compose testing
   - Unit and integration tests
   - **Cost**: $0 (using existing hardware)

2. **Cloud for Load Testing** (When needed)
   - Smoke tests to validate configs
   - Performance benchmarking
   - Extended stability tests
   - Geographic latency testing
   - **Cost**: $50-100/month (only when actively testing)

**Bottom Line**: Don't buy expensive laptop yet. Use existing hardware for development + validation, only use cloud for actual load tests. This gives best of both worlds.

---

## PHASE 7: Upgrade Path Decision Criteria

### 7.1 Stability Gates

**Before considering Claude upgrade or expensive infrastructure**:

#### Gate 1: Local Stability ✅/❌
- [ ] All local tests pass consistently
- [ ] No panics or unexpected behavior
- [ ] Configuration examples all work
- [ ] Documentation accurate and complete
- [ ] Observability provides useful insights

#### Gate 2: Cloud Validation ✅/❌
- [ ] 5 successful smoke tests in a row
- [ ] 3 successful full scenario tests
- [ ] 1 successful 8-hour stability test
- [ ] Cost per test < $1 (smoke) or < $5 (full scenario)
- [ ] No configuration-related failures

#### Gate 3: Production Readiness ✅/❌
- [ ] All 15 scenarios documented and tested
- [ ] Monitoring and alerting configured
- [ ] Disaster recovery procedures documented
- [ ] Performance characteristics well-understood
- [ ] Capacity planning guidelines created

**Current Status**: Gate 1 in progress (60% complete)

### 7.2 When to Upgrade

**Claude Plan Upgrade**: Only after Gate 2 passes
- Rationale: Need stable foundation first
- Current plan sufficient for development and validation
- Upgrade only when doing extensive production deployment work

**Infrastructure Investment**: Only after Gate 3 passes
- Rationale: Need proven stability before large-scale testing
- Start with cloud instances for flexibility
- Consider dedicated servers only for extended (>1 month) testing

---

## Implementation Priority Order

### Week 1: Core Stability (In Progress)
- ✅ DSL syntax understanding and examples
- ✅ Validation scripts
- ✅ Infrastructure selection guide
- 🔄 Observability improvements (logging + metrics)
- 🔄 Bug fixes and edge case handling

### Week 2: Local Testing
- [ ] Docker-compose setup
- [ ] Comprehensive test suite
- [ ] All 15 scenarios defined
- [ ] Local validation of all scenarios

### Week 3: API Management & Automation
- [ ] API key management system
- [ ] Instance availability checking
- [ ] Provider-specific scripts for each scenario
- [ ] Cost tracking system

### Week 4: Validation & Stabilization
- [ ] Complete local validation (Gate 1)
- [ ] First successful cloud smoke test
- [ ] Performance characterization
- [ ] Documentation completion

### Week 5+: Production Readiness
- [ ] Extended stability testing
- [ ] All 15 scenarios tested on cloud
- [ ] Monitoring and alerting
- [ ] Capacity planning

---

## Success Metrics

### Short-term (2 weeks)
- [ ] 100% local test pass rate
- [ ] Docker-compose setup working reliably
- [ ] All documentation examples validated
- [ ] Zero configuration-related cloud failures

### Medium-term (1 month)
- [ ] 90%+ cloud test success rate
- [ ] < $50 total spent on testing
- [ ] 5+ scenarios validated on cloud
- [ ] Complete observability implemented

### Long-term (2 months)
- [ ] All 15 scenarios production-ready
- [ ] < $100 total spent on validation
- [ ] Performance characteristics documented
- [ ] Ready for actual production workload testing

---

## Risk Mitigation

### Technical Risks
1. **DSL bugs discovered**: Have YAML fallback ready, continue fixing DSL
2. **Performance issues**: Profile and optimize, document limitations
3. **Cloud provider issues**: Have scripts for 3+ providers ready

### Cost Risks
1. **Testing overruns budget**: Set hard limits, use auto-destroy scripts
2. **Forgotten resources**: Daily audit script, billing alerts
3. **Repeated failures**: Strict Gate 1 validation before any cloud spending

### Time Risks
1. **Taking too long**: Focus on core 5 scenarios first, expand later
2. **Context loss**: Comprehensive documentation at every step
3. **Scope creep**: Stick to plan, defer nice-to-haves

---

## Next Immediate Actions

1. **Observability** (Today)
   - Verify Prometheus metrics are exposed
   - Test admin API endpoints
   - Improve logging clarity

2. **Docker Compose** (Tomorrow)
   - Create docker-compose.loadtest.yml
   - Test full local load balancing
   - Validate health check behavior

3. **Bug Fixes** (This Week)
   - Test all load balancing algorithms
   - Verify health checks work correctly
   - Test connection pooling (if implemented)

4. **First Scenario** (Next Week)
   - Complete scenario 01 (basic HTTP LB)
   - Test locally until perfect
   - Document everything

---

## Conclusion

This is a **methodical, systematic approach** to stability:

1. ✅ No more panic reactions
2. ✅ No more cloud testing without local validation
3. ✅ No more guessing - everything tested and documented
4. ✅ No more wasted money - validation gates before spending
5. ✅ No more rushed work - stable foundations first

**Current Status**: Foundation laid, observability and local testing next

**Timeline**: 4-8 weeks to production-ready stability

**Cost**: < $100 total for complete validation

**ROI**: Every $1 spent on preparation saves $5-10 in failed tests

Let's build this right, then scale with confidence.
