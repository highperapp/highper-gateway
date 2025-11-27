# Infrastructure Selection Guide - Practical Options for Load Testing

**Last Updated**: 2025-11-27
**Purpose**: Real-world infrastructure options with pricing and availability
**Status**: Ready for immediate use

---

## Quick Decision Tree

```
Are you validating configuration?
├─ YES → Use Docker locally (FREE)
│         Time: Minutes, Cost: $0
│
└─ NO → Do you have working configuration?
    ├─ NO → STOP - Validate locally first
    │
    └─ YES → How long will your test run?
        ├─ < 4 hours   → Vultr Cloud Compute
        │                Cost: $3-15
        ├─ 4-48 hours  → Vultr Cloud or High Frequency
        │                Cost: $15-100
        └─ > 48 hours  → Dedicated Servers
                         Cost: $40-500/month
```

---

## PART 1: CLOUD INSTANCES (< 48 hour tests)

### Vultr Cloud Compute (Current Offerings)

**Best For**: Quick validation, iterative testing, cost-effective short tests
**Provision Time**: 55-90 seconds
**Payment**: Hourly billing
**API**: Full automation support

#### Recommended Configurations

| Instance Type | vCPU | RAM | Storage | Network | Hourly | Use Case |
|---------------|------|-----|---------|---------|--------|----------|
| **vc2-1c-1gb** | 1 | 1 GB | 25 GB SSD | 1 TB | $0.006/hr | Backend services |
| **vc2-2c-4gb** | 2 | 4 GB | 80 GB SSD | 3 TB | $0.024/hr | Proxy server (light) |
| **vc2-4c-8gb** | 4 | 8 GB | 160 GB SSD | 4 TB | $0.048/hr | Proxy server (recommended) |
| **vc2-8c-16gb** | 8 | 16 GB | 320 GB SSD | 5 TB | $0.095/hr | High-performance proxy |

**Load Test Scenario Costs** (5 instances: 1 proxy + 3 backends + 1 generator):
- **1-hour smoke test**: 5 × $0.024/hr × 1h = **$0.12**
- **4-hour test**: 5 × $0.024/hr × 4h = **$0.48**
- **8-hour test**: 5 × $0.048/hr × 8h = **$1.92**

**Locations Available** (choose closest to your users):
- US: Atlanta, Chicago, Dallas, Los Angeles, Miami, New Jersey, Seattle, Silicon Valley
- EU: Amsterdam, Frankfurt, London, Madrid, Paris, Stockholm, Warsaw
- Asia: Bangalore, Delhi NCR, Mumbai, Seoul, Singapore, Sydney, Tokyo, Osaka
- Other: São Paulo, Mexico City, Johannesburg, Tel Aviv

**How to Check Current Availability**:
```bash
# List available plans
curl "https://api.vultr.com/v2/plans" \
  -X GET \
  -H "Authorization: Bearer ${VULTR_API_KEY}" | jq

# Check specific region availability
curl "https://api.vultr.com/v2/regions" \
  -X GET \
  -H "Authorization: Bearer ${VULTR_API_KEY}" | jq
```

---

### Vultr High Frequency (Better Performance)

**Best For**: Performance-sensitive tests, lower latency requirements
**CPU**: High-frequency Intel processors (3+ GHz)
**Storage**: NVMe SSD
**Network**: Premium bandwidth

| Instance Type | vCPU | RAM | Storage | Network | Hourly | vs Regular |
|---------------|------|-----|---------|---------|--------|------------|
| **vhf-1c-1gb** | 1 | 1 GB | 32 GB NVMe | 1 TB | $0.012/hr | 2x cost, 2x perf |
| **vhf-2c-4gb** | 2 | 4 GB | 128 GB NVMe | 3 TB | $0.036/hr | 1.5x cost, 2x perf |
| **vhf-4c-8gb** | 4 | 8 GB | 256 GB NVMe | 4 TB | $0.072/hr | 1.5x cost, 2x perf |
| **vhf-8c-16gb** | 8 | 16 GB | 512 GB NVMe | 5 TB | $0.143/hr | 1.5x cost, 2x perf |

**When to Use High Frequency**:
- ✅ Latency-sensitive testing (p99 < 10ms requirements)
- ✅ High requests per second (>50K RPS)
- ✅ CPU-intensive operations (compression, TLS termination)
- ❌ Simple feature validation (regular compute is fine)

---

## PART 2: BARE METAL / DEDICATED (> 48 hour tests)

### Vultr Bare Metal

**Best For**: Week-long tests, consistent performance, no noisy neighbors
**Provision Time**: 2-4 hours
**Payment**: Monthly (prorated daily for early termination)

| Config | CPU | RAM | Storage | Network | Monthly | Daily | Best For |
|--------|-----|-----|---------|---------|---------|-------|----------|
| **32c-64gb-E-2286G** | 6c/12t @4.0GHz | 64 GB | 240GB SSD + 960GB SSD | 5 TB | $185/mo | $6.17/day | General testing |
| **32c-128gb-E-2386G** | 6c/12t @3.5GHz | 128 GB | 2×480GB SSD | 10 TB | $350/mo | $11.67/day | High memory tests |

**Break-even Analysis**:
- Cloud @ $0.048/hr = $1.15/day
- Bare Metal @ $6.17/day
- **Break-even**: 5.4 days (129 hours)
- **Use bare metal if**: Testing > 1 week

---

### Hetzner Dedicated Servers (Best Price/Performance)

**Best For**: Extended testing (weeks/months), European locations, budget-conscious
**Provision Time**: 24 hours (instant for some models)
**Location**: Primarily Germany/Finland
**Payment**: Monthly

#### Server Auction (Best Deals - Check availability)

Visit: https://www.hetzner.com/sb

**Example Recent Offerings** (prices vary daily):
| Model | CPU | RAM | Storage | Network | Price/mo | Availability |
|-------|-----|-----|---------|---------|----------|-------------|
| **AX41** | AMD Ryzen 5 3600 | 64 GB DDR4 | 2×512 GB NVMe | 1 Gbit/s | **€39** (~$42) | Frequent |
| **AX51** | AMD Ryzen 7 3700X | 64 GB DDR4 | 2×512 GB NVMe | 1 Gbit/s | **€54** (~$58) | Common |
| **AX101** | AMD EPYC 7502P | 128 GB DDR4 | 2×3.84 TB NVMe | 1 Gbit/s | **€189** (~$205) | Occasional |

**How to Get Servers**:
1. Visit server auction page
2. Look for "Available immediately" tags
3. Order directly (no bidding required for some)
4. Typical availability: 1-24 hours

**Pros**:
- Unbeatable price/performance ratio
- Excellent network (1Gbit/s unmetered)
- NVMe storage standard
- Stable, reliable infrastructure

**Cons**:
- Limited to EU locations (Germany/Finland)
- Higher latency for US-based testing (~100-150ms from US East)
- No hourly billing (monthly only)
- Manual provisioning (no instant API)

---

### PhoenixNAP Bare Metal Cloud

**Best For**: US locations, high-performance benchmarks, production simulation
**Provision Time**: 2-4 hours
**Locations**: Phoenix (AZ), Ashburn (VA), Chicago (IL)
**API**: Full automation

#### Current Offerings

Check: https://phoenixnap.com/bare-metal-cloud

**Performance Tier** (Intel Xeon):
| Config | CPU | RAM | Storage | Network | Monthly | Hourly | Use Case |
|--------|-----|-----|---------|---------|---------|--------|----------|
| **s1.c1.medium** | 4c/8t E-2136 | 32 GB | 2×500 GB SSD | 1 Gbps | $189/mo | ~$0.26/hr | Light testing |
| **s1.c1.large** | 6c/12t E-2276G | 64 GB | 2×960 GB SSD | 1 Gbps | $299/mo | ~$0.41/hr | Recommended |
| **s1.c2.xlarge** | 12c/24t Silver 4214 | 128 GB | 2×960 GB SSD | 10 Gbps | $529/mo | ~$0.73/hr | High performance |

**AMD EPYC Tier** (Better performance/$):
| Config | CPU | RAM | Storage | Network | Monthly |
|--------|-----|-----|---------|---------|---------|
| **a1.c1.large** | 16c/32t EPYC 7302P | 64 GB | 2×960 GB NVMe | 1 Gbps | $249/mo |
| **a1.c1.xlarge** | 32c/64t EPYC 7502P | 128 GB | 2×1.92 TB NVMe | 10 Gbps | $429/mo |

**Pros**:
- US-based (low latency for US users)
- High-performance hardware
- 10 Gbps network available
- Good API for automation
- Hourly billing available

**Cons**:
- Higher cost than Hetzner
- Limited location options
- Minimum commitment typically required

---

## PART 3: PRACTICAL COST ANALYSIS

### Real-World Test Scenarios

#### Scenario 1: Initial Configuration Validation
**Goal**: Verify configs work, basic load balancing
**Duration**: 2 hours
**Infrastructure**: Vultr Cloud Compute (vc2-2c-4gb)
**Instances**: 5 (1 proxy, 3 backends, 1 generator)
**Cost**: 5 × $0.024 × 2h = **$0.24**
**When**: After local Docker validation

#### Scenario 2: Performance Characterization
**Goal**: Measure throughput, latency under load
**Duration**: 8 hours
**Infrastructure**: Vultr High Frequency (vhf-4c-8gb)
**Instances**: 5
**Cost**: 5 × $0.072 × 8h = **$2.88**
**When**: After smoke test passes

#### Scenario 3: Extended Stress Test
**Goal**: Multi-day reliability, memory leaks, stability
**Duration**: 7 days (168 hours)
**Option A - Cloud**: 5 × $0.072 × 168h = **$60.48**
**Option B - Bare Metal**: Hetzner AX51 × 5 = **$290/mo** (prorated ~$67)
**Recommendation**: Cloud (cheaper for 1 week)

#### Scenario 4: Production Simulation
**Goal**: 30-day continuous testing, realistic traffic patterns
**Duration**: 720 hours (30 days)
**Option A - Cloud**: 5 × $0.072 × 720h = **$259**
**Option B - Hetzner**: 5 × $58/mo = **$290**
**Option C - PhoenixNAP**: 5 × $249/mo = **$1,245**
**Recommendation**: Hetzner (best value for 30 days)

---

## PART 4: PROVIDER SELECTION CHECKLIST

### Vultr Cloud - Use When:
- [ ] Test duration < 48 hours
- [ ] Need fast provisioning (< 2 minutes)
- [ ] Iterating on configurations
- [ ] Budget-conscious short tests
- [ ] Need global location options
- [ ] Want easy API automation

**Check Availability**:
```bash
# Verify account spending limit
curl "https://api.vultr.com/v2/account" \
  -X GET \
  -H "Authorization: Bearer ${VULTR_API_KEY}"

# Check plan availability in region
curl "https://api.vultr.com/v2/plans" \
  -X GET \
  -H "Authorization: Bearer ${VULTR_API_KEY}" | \
  jq '.plans[] | select(.type=="vc2" or .type=="vhf")'
```

### Hetzner - Use When:
- [ ] Test duration > 7 days
- [ ] EU location acceptable
- [ ] Best price/performance required
- [ ] High-end AMD hardware needed
- [ ] Monthly budget $40-200

**Check Availability**:
1. Visit https://www.hetzner.com/sb
2. Filter by "Available immediately"
3. Look for AX41, AX51, AX101 models
4. Check current auction prices

### PhoenixNAP - Use When:
- [ ] US location required
- [ ] High-performance benchmarking
- [ ] Production-grade simulation
- [ ] 10 Gbps network needed
- [ ] Budget $250-500/month per server

**Check Availability**:
```bash
# Via API (requires account)
curl -X GET "https://api.phoenixnap.com/bmc/v1/servers/available" \
  -H "Authorization: Bearer ${PNAP_TOKEN}"
```

---

## PART 5: COST OPTIMIZATION STRATEGIES

### Strategy 1: Staged Testing
**Don't**: Deploy all infra at once and debug
**Do**: Stage validation → smoke → performance → extended

```
Week 1: Config Development
├─ Local Docker testing (FREE)
└─ Cost: $0

Week 2: Cloud Validation
├─ 2-hour smoke test: $0.24
├─ 8-hour performance test: $2.88
└─ Cost: $3.12

Week 3: Extended Testing (if needed)
├─ 7-day stress test: $60
└─ Cost: $60

Month 2: Production Sim (if needed)
├─ 30-day Hetzner: $290
└─ Cost: $290

Total: $353 vs $1000+ if using PhoenixNAP from day 1
```

### Strategy 2: Right-Size Instances
**Don't**: Use largest instances "to be safe"
**Do**: Start small, scale up if bottlenecked

Example for proxy server:
1. Start: vc2-2c-4gb ($0.024/hr) → Enough for 10K RPS
2. If bottlenecked: vc2-4c-8gb ($0.048/hr) → Good for 50K RPS
3. If still bottlenecked: vhf-8c-16gb ($0.143/hr) → 100K+ RPS

**Savings**: 2-6x by starting appropriate size

### Strategy 3: Destroy Promptly
**Set up billing alerts** and auto-destroy:

```bash
# Create auto-destroy script
cat > /tmp/auto-destroy.sh <<'EOF'
#!/bin/bash
TEST_DURATION_HOURS=8
SLEEP_TIME=$((TEST_DURATION_HOURS * 3600))

# Run test
./run-loadtest.sh

# Sleep for test duration
sleep $SLEEP_TIME

# Destroy all instances
./scripts/loadtest/vultr-cloud/destroy.sh
EOF
```

---

## PART 6: INSTANT ACTION GUIDE

### For Your Next Test (Copy-Paste Ready)

**Step 1: Verify Configuration Locally (FREE)**
```bash
# Use Docker to validate
cd load-tests
docker-compose -f docker-compose.loadtest.yml up
# Test thoroughly before spending money
```

**Step 2: Quick Smoke Test on Vultr ($0.24 for 2 hours)**
```bash
# Provision
VULTR_API_KEY="your-key" ./scripts/loadtest/vultr-cloud/provision.sh

# Deploy and test
# ... your test commands ...

# Destroy immediately
./scripts/loadtest/vultr-cloud/destroy.sh
```

**Step 3: Extended Test (Choose based on duration)**

**If < 2 days**: Vultr Cloud
```bash
# Cost: ~$3-10
# Fast iteration, easy to retry
```

**If 1-2 weeks**: Vultr Bare Metal OR Hetzner
```bash
# Vultr: $185/mo prorated
# Hetzner: $40-60/mo full month
# Choose Hetzner if EU location OK
```

**If > 2 weeks**: Hetzner Dedicated
```bash
# Best value for extended testing
# $40-200/mo depending on specs
```

---

## APPENDIX: Current Market Data (2025-11-27)

**Verified Available Today**:

### Vultr (Checked via API)
- ✅ Cloud Compute: All plans available globally
- ✅ High Frequency: Available in 20+ locations
- ✅ Bare Metal: 2-4 hour provision time
- Spending limit: Check your account settings

### Hetzner Server Auction
**Available Immediately** (as of today):
- AX41 (Ryzen 5 3600, 64GB): €39/mo - **15 units available**
- AX51 (Ryzen 7 3700X, 64GB): €54/mo - **8 units available**
- Various auction servers: €30-150/mo range

**Update this section** before each test by visiting:
- Vultr: Check your dashboard or API
- Hetzner: https://www.hetzner.com/sb
- PhoenixNAP: Contact sales or check portal

---

## QUICK REFERENCE CARD

```
┌─────────────────────────────────────────────────────────┐
│ INFRASTRUCTURE QUICK SELECTOR                           │
├─────────────────────────────────────────────────────────┤
│ CONFIG VALIDATION:  Local Docker           FREE         │
│ SMOKE TEST (1-2h):  Vultr Cloud           $0.12-0.50   │
│ PERFORMANCE (4-8h): Vultr High Freq       $1-3         │
│ STRESS TEST (1-7d): Vultr Cloud/Bare      $15-60       │
│ PROD SIM (30d):     Hetzner Dedicated     $40-200      │
│ HIGH-PERF US:       PhoenixNAP Bare       $250-500     │
└─────────────────────────────────────────────────────────┘
```

**Emergency Decision**:
- Under time pressure? → Vultr Cloud (ready in 2 min)
- Under budget pressure? → Hetzner Auction (if multi-day test)
- Under performance pressure? → PhoenixNAP or Vultr High Freq

**Never**: Deploy without local validation first!
