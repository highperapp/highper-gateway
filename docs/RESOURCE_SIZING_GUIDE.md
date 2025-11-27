# Resource Sizing Guide for Load Testing

Guide for right-sizing infrastructure based on test results and target metrics.

## Current Baseline (High-End)

### Initial Test Configuration

| Component | Spec | Cost/Hour | Purpose |
|-----------|------|-----------|---------|
| **Proxy** | 32-core, 256GB RAM | ~$1.20 | Highper Gateway |
| **Backend** (×3) | 8-core, 64GB RAM each | ~$0.40 × 3 | Rust fast-backend |
| **Generator** (×3) | 16-core, 128GB RAM each | ~$0.60 × 3 | vegeta/wrk2 |
| **Total** | 112 cores, 640GB RAM | ~$4.20/hour | Full stack |

**Test Duration**: 3 minutes
**Estimated Cost**: ~$18/test (including provisioning time)

## Resource Sizing Strategy

### Phase 1: Establish Baseline (Current)
**Goal**: Prove we can hit 600K+ RPS with high-end hardware

```bash
# Use current configuration
VULTR_PROXY_PLAN="vbm-32c-256gb"
VULTR_BACKEND_PLAN="vbm-8c-64gb"
VULTR_GENERATOR_PLAN="vbm-16c-128gb"
LOAD_TEST_BACKEND_COUNT=3
LOAD_TEST_GENERATOR_COUNT=3
```

**Expected Results**:
- 600K+ RPS sustained
- P99 latency < 10ms
- CPU usage: 40-60% (indicates headroom)
- Memory usage: 30-50% (indicates over-provisioned)

### Phase 2: Right-Size Based on Utilization

After baseline tests, analyze resource usage:

```bash
# Collect during test
ssh proxy "mpstat 1 180 > cpu_usage.txt &"
ssh proxy "free -s 1 -c 180 > mem_usage.txt &"
ssh proxy "sar -n DEV 1 180 > net_usage.txt &"
```

**Decision Matrix**:

| CPU Usage | Memory Usage | Action |
|-----------|--------------|--------|
| <40% | <40% | Downsize 1 tier |
| 40-70% | 40-70% | Optimal - keep size |
| >70% | >70% | Keep or upsize |

### Phase 3: Incremental Downsizing

**Proxy Server Downsize Path**:
```
32c/256GB → 24c/192GB → 16c/128GB → 12c/96GB
  (100%)      (75%)        (50%)       (37%)
```

**Backend Downsize Path**:
```
8c/64GB → 6c/48GB → 4c/32GB
 (100%)    (75%)     (50%)
```

**Generator Downsize Path** (or scale horizontally):
```
16c/128GB → 12c/96GB → 8c/64GB
  (100%)      (75%)      (50%)

OR increase count: 3 generators → 5 generators (distribute load)
```

## Recommended Sizing for Each Scenario

### Scenario 1-3: L4/L7 Basic HTTP
**Expected Load**: 600K RPS

Recommended:
```bash
Proxy: 16c/128GB (50% of baseline)
Backend (×3): 6c/48GB (75% of baseline)
Generator (×3): 12c/96GB (75% of baseline)
Estimated cost: ~$10/test (45% savings)
```

### Scenario 4-7: API Gateway, WebSocket, gRPC
**Expected Load**: 400-600K RPS

Recommended:
```bash
Proxy: 24c/192GB (75% of baseline)
Backend (×3): 8c/64GB (100% of baseline - protocol overhead)
Generator (×4): 12c/96GB (more generators for connection diversity)
Estimated cost: ~$15/test (17% savings)
```

### Scenario 8: Database Load Balancer
**Expected Load**: 500K queries/sec

Recommended:
```bash
Proxy: 16c/128GB
Backend (×5): 8c/64GB (more backends for DB connection pooling)
Generator (×3): 12c/96GB
Estimated cost: ~$16/test
```

### Scenario 9: WAF + mTLS
**Expected Load**: 400K RPS (higher CPU for WAF)

Recommended:
```bash
Proxy: 32c/256GB (WAF processing intensive - keep 100%)
Backend (×3): 6c/48GB (WAF is at proxy, not backend)
Generator (×3): 12c/96GB
Estimated cost: ~$13/test (28% savings)
```

### Scenario 13: High Concurrency Stress Test
**Expected Load**: 3M concurrent connections

Recommended:
```bash
Proxy: 32c/256GB (connection state intensive - keep 100%)
Backend (×3): 8c/64GB (keep for stability)
Generator (×5): 16c/128GB (need more for 3M connections)
Estimated cost: ~$25/test (increase for stress test)
```

## Cost Optimization Strategies

### 1. Horizontal Scaling vs Vertical Scaling

**Vertical** (Bigger servers):
- ✅ Simpler configuration
- ✅ Lower network overhead
- ❌ More expensive
- ❌ Single point of bottleneck

**Horizontal** (More servers):
- ✅ Better load distribution
- ✅ Can use cheaper instances
- ✅ Validates real-world HA setup
- ❌ More complex coordination

**Example**:
```bash
# Option A: 3× 8c/64GB backends = $1.20/hr
BACKEND_PLAN="vbm-8c-64gb"
BACKEND_COUNT=3

# Option B: 6× 4c/32GB backends = $1.20/hr (same cost, better distribution)
BACKEND_PLAN="vbm-4c-32GB"
BACKEND_COUNT=6
```

### 2. Instance Count Flexibility

Make counts configurable:

```bash
# In .env
LOAD_TEST_BACKEND_COUNT=3      # Adjust: 3, 5, 10
LOAD_TEST_GENERATOR_COUNT=3    # Adjust: 3, 5, 7
```

Scripts automatically:
- Provision N instances
- Configure load balancing
- Distribute test load
- Aggregate results

### 3. Provider Cost Comparison

After testing on Vultr, compare:

| Provider | 32c/256GB | 8c/64GB | 3hr Test Cost |
|----------|-----------|---------|---------------|
| Vultr | $1.20/hr | $0.40/hr | ~$18 |
| Hetzner | $0.15/hr | $0.05/hr | ~$3.50 (80% cheaper!) |
| PhoenixNAP | $1.50/hr | $0.45/hr | ~$20 |

**Recommendation**: Validate on Vultr, then switch to Hetzner for cost-effective testing.

## Right-Sizing Workflow

### Step 1: Run Baseline Test
```bash
# Use high-end configuration
./scripts/loadtest/run.sh vultr scenario-02

# Monitor resource usage
# - Check results/*/metrics/system-stats.json
# - Analyze CPU/memory/network utilization
```

### Step 2: Analyze Results
```bash
# Check proxy resource usage
cat results/load-test-*/metadata.json | jq '.proxy_resources'

# Key metrics:
- CPU usage average
- Memory usage peak
- Network throughput
- Connection count peak
```

### Step 3: Calculate Right-Size
```bash
# Formula:
new_cores = baseline_cores × (target_usage / current_usage)
new_memory = baseline_memory × (target_usage / current_usage)

# Target 60-70% utilization for optimal cost/performance
```

### Step 4: Test with Smaller Instance
```bash
# Update .env
VULTR_PROXY_PLAN="vbm-24c-192gb"  # Downsized

# Re-run test
./scripts/loadtest/run.sh vultr scenario-02

# Compare results
diff results/test-1/summary.txt results/test-2/summary.txt
```

### Step 5: Document Final Sizing

Create sizing recommendation file:
```bash
# configs/scenarios/scenario-02-sizing.txt
Validated Configuration for Scenario 02:
Proxy: 16c/128GB (60% CPU, 45% memory)
Backend: 6c/48GB × 3
Generator: 12c/96GB × 3
Target RPS: 650K (achieved: 652K)
P99 Latency: <8ms
Cost per test: $10.50
Savings: 42% vs baseline
```

## Lessons Learned Template

After each sizing iteration, document:

```markdown
## Scenario XX - Resource Sizing Results

### Test 1: Baseline (High-End)
- Config: 32c/256GB proxy, 8c/64GB × 3 backends
- Results: 680K RPS, P99: 4.2ms
- Utilization: CPU 42%, Memory 38%
- Cost: $18/test
- **Conclusion**: Over-provisioned

### Test 2: Downsized
- Config: 16c/128GB proxy, 6c/48GB × 3 backends
- Results: 665K RPS, P99: 5.1ms
- Utilization: CPU 68%, Memory 62%
- Cost: $10/test
- **Conclusion**: Optimal - 97% performance at 45% cost

### Final Recommendation
- Proxy: 16c/128GB
- Backend: 6c/48GB × 3
- Savings: $8/test (44% reduction)
```

## Scaling Rules of Thumb

Based on expected results:

### CPU Scaling
- **L4 Proxy**: 1 core per 50K connections
- **L7 Proxy**: 1 core per 25K RPS
- **WAF**: 1 core per 15K RPS
- **Backend**: 1 core per 100K RPS (simple responses)

### Memory Scaling
- **Proxy**: 4GB + (connections × 64KB)
- **Backend**: 2GB + (connections × 32KB)
- **Generator**: 2GB per 100K concurrent connections

### Network
- **1Gbps**: ~125K RPS (1KB responses)
- **10Gbps**: ~1.25M RPS (1KB responses)
- **40Gbps**: ~5M RPS (1KB responses)

## Future Optimization Opportunities

1. **Kernel Bypass** (DPDK/io_uring): 2-3x RPS improvement
2. **CPU Pinning**: 10-15% latency reduction
3. **NUMA Optimization**: 20-30% better memory bandwidth
4. **Custom Kernel**: Remove unnecessary features
5. **Dedicated NICs**: Bypass kernel network stack

## Monitoring Sizing Metrics

During tests, automatically capture:

```bash
# In results/*/metadata.json
{
  "infrastructure": {
    "proxy": {
      "cpu_cores": 32,
      "memory_gb": 256,
      "utilization": {
        "cpu_avg": 42,
        "cpu_peak": 68,
        "memory_avg": 38,
        "memory_peak": 52
      }
    }
  },
  "sizing_recommendation": {
    "optimal_proxy": "16c/128GB",
    "estimated_savings": "45%"
  }
}
```

## Summary

Start high-end to prove capabilities, then incrementally downsize while monitoring:
- ✅ Performance metrics (RPS, latency)
- ✅ Resource utilization (CPU, memory, network)
- ✅ Cost per test
- ✅ Stability (error rates, timeouts)

**Goal**: Find the minimum viable configuration that meets performance targets with 60-70% resource utilization.
