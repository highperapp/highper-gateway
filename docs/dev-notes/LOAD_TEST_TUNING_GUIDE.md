# Load Test Tuning Guide

## Overview

This document captures the OS/TCP tuning settings, architecture, and observations from load testing highper-gateway on DigitalOcean droplets.

## Architecture

### Components

| Component | Droplet Size | Private IP | Public IP | Count |
|-----------|-------------|------------|-----------|-------|
| highper-gateway | c-32 (32 vCPU, 64GB) | 10.20.0.2 | 143.110.184.230 | 1 |
| Backend | c-8 (8 vCPU, 16GB) | 10.20.0.6-8 | Various | 3 |
| Load Generator | c-32 (32 vCPU, 64GB) | 10.20.0.9-11 | Various | 3 |

### Network

- All components in same VPC: `highper-gateway-loadtest-vpc`
- VPC UUID: `7d4149fa-258a-4c0e-8279-7bc3e945a1ff`
- Region: `blr1` (Bangalore)
- Subnet: `10.20.0.0/16`

## OS/TCP Tuning Settings

### For Load Generators

```bash
# Connection handling
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.core.netdev_max_backlog=65535

# Connection reuse (critical for high RPS)
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=5
sysctl -w net.ipv4.tcp_max_tw_buckets=500000

# Port range
sysctl -w net.ipv4.ip_local_port_range="1024 65535"

# Buffer sizes
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216

# File descriptors
echo "* soft nofile 1000000" >> /etc/security/limits.conf
echo "* hard nofile 1000000" >> /etc/security/limits.conf
ulimit -n 1000000
```

### For highper-gateway

```bash
# Same as load generators
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=5
sysctl -w net.ipv4.tcp_max_tw_buckets=500000
sysctl -w net.core.netdev_max_backlog=65535
sysctl -w net.ipv4.ip_local_port_range="1024 65535"

# Systemd service should have LimitNOFILE=1048576
```

### For Backends

```bash
# Same basic tuning
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
```

## highper-gateway Configuration

```toml
[server]
bind = ["0.0.0.0:8080"]
workers = "auto"

[server.performance]
max_connections = 500000
backlog = 65535
tcp_nodelay = true
tcp_keepalive = true

[server.performance.connection_pool]
max_idle_per_host = 10000  # Critical: was 100 by default
idle_timeout = "120s"

[[upstreams]]
name = "backend"
servers = [
    { url = "http://10.20.0.6:80", weight = 1, max_conns = 100000 },
    { url = "http://10.20.0.7:80", weight = 1, max_conns = 100000 },
    { url = "http://10.20.0.8:80", weight = 1, max_conns = 100000 }
]

[upstreams.connection]
max_connections_per_upstream = 300000
min_idle_connections = 10000
connection_timeout = "30s"
tcp_nodelay = true
tcp_keepalive = true
circuit_breaker_enabled = false
retry_max_attempts = 0

[upstreams.health_check]
enabled = false

[[routes]]
name = "default"
upstream = "backend"

[routes.match]
paths = ["/"]

[observability]
log_level = "warn"
access_log = false
```

## Vegeta Load Generator Settings

```bash
# Optimal vegeta command
echo 'GET http://<TARGET>:8080/' | vegeta attack \
  -rate=<RPS> \
  -duration=<SECONDS>s \
  -workers=400 \
  -max-workers=800 \
  -keepalive \
  | vegeta report
```

Key flags:
- `-keepalive`: Reuse HTTP connections (critical)
- `-workers`: Concurrent workers (400-600)
- `-max-workers`: Max workers allowed (800-1200)

## Test Results Summary

### Public IP Testing (Cross-VPC)

Load generators in `default-blr1` VPC (10.122.0.x) connecting to proxy via public IP.

| Target RPS | Generators | Actual RPS | Success | P50 | Notes |
|------------|------------|------------|---------|-----|-------|
| 50k | 1 | 49.9k | 100% | 472µs | Single gen baseline |
| 100k | 1 | 69.6k | 100% | 1.5ms | Near gen limit |
| 200k | 3 x 67k | 201k | 100% | 7ms | Best result |
| 300k | 3 x 100k | 187k | 100% | 7ms | Hitting ceiling |

**Peak Performance (Public IP): ~200k RPS**

### Private IP Testing (Same VPC)

Load generators in `highper-gateway-loadtest-vpc` (10.20.0.x) connecting via private IP.

| Target RPS | Generators | Actual RPS | Success | P50 | Notes |
|------------|------------|------------|---------|-----|-------|
| 100k | 1 | 100k | 100% | 2.6ms | Excellent single-gen |
| 200k | 2 x 100k | 128k | 100% | 8ms | VPC bandwidth ceiling |
| 300k | 3 x 100k | 130k | 100% | 8ms | Same ceiling |

**Peak Performance (Private IP): ~130k RPS (VPC bandwidth limited)**

### Key Finding: VPC Bandwidth Limitation

When comparing public vs private IP with same-VPC generators:
- **Public IP (2 x 100k)**: ~192k RPS achieved
- **Private IP (2 x 100k)**: ~128k RPS achieved

The bottleneck is **DigitalOcean VPC internal bandwidth**, not highper-gateway. Use public IP for maximum throughput when generators are in same VPC.

## Observations and Issues

### 1. Connection Pool Fix (Successful)

**Problem**: Default `max_idle_per_host = 100` in Hyper client caused connection churn.

**Solution**: Modified `highper-gateway/src/proxy/handler.rs` to use config pool settings:
```rust
let pool_config = config.server.performance.connection_pool.clone();
let client = Client::with_config(None, Some(pool_config));
```

**Result**: Improved from ~110k to ~200k RPS (80% improvement)

### 2. VPC Cross-Subnet Issue

**Observation**: When generators are in different VPC (10.122.0.x) than proxy (10.20.0.x):
- Traffic goes through public IP
- Still achieved ~200k RPS
- Latency slightly higher but consistent

### 3. Ulimit Fix (Resolved)

**Problem**: File descriptor limits (ulimit -n) defaulted to 1024 instead of 1000000.

**Root Cause**: Settings in /etc/security/limits.conf don't apply without PAM configuration.

**Solution**:
1. Create `/etc/security/limits.d/99-loadtest.conf` with proper limits
2. Add `session required pam_limits.so` to `/etc/pam.d/sshd`
3. Restart SSH: `systemctl restart ssh`

**Result**: Eliminated "address already in use" errors, proper connection reuse working.

### 4. VPC Bandwidth Limitation (Identified)

**Issue**: Multi-generator tests hit ceiling at ~130k RPS with private IP.

**Root Cause**: DigitalOcean VPC internal bandwidth limitation.

**Evidence**:
- Private IP (2 x 100k): 128k RPS achieved
- Public IP (2 x 100k): 192k RPS achieved
- CPU usage low on all systems during test

**Workaround**: Use public IP for load testing when targeting >130k RPS.

### 5. Resource Utilization

During 200k RPS (public IP):
- **Proxy CPU**: ~57% (33% usr + 19% sys + 4% softirq)
- **Backend CPU**: <1% idle
- **Generator CPU**: ~70%

This suggests the bottleneck is NOT CPU but likely:
- Network bandwidth
- Connection pool management
- Some internal contention

## Profiling Results (2025-11-23)

### CPU Profile at 200k RPS

Using `perf record -F 999` during load:

**Top CPU Consumers:**
| % | Function | Component |
|---|----------|-----------|
| 1.18% | native_queued_spin_lock_slowpath | Kernel - Network TX queue |
| 0.75% | rep_movs_alternative | Kernel - TCP receive buffer copy |
| 0.74% | __fdget | Kernel - Socket FD lookup |
| 0.73% | aa_inet_msg_perm | Kernel - AppArmor security |

**Key Finding: highper-gateway is NOT the bottleneck!**
- All userspace functions < 1% CPU each
- No single hot spot in highper-gateway code
- Proxy uses only 7% total CPU at 200k RPS
- Code is highly optimized - evenly distributed workload

### Bottleneck Analysis

The 200k RPS ceiling is caused by:
1. **VPC Network Bandwidth**: Proxy-to-backend traffic on private IPs is limited
2. **Kernel Spinlock Contention**: Network TX queue has lock contention at high throughput

**Recommendation**: Scale backends horizontally (6-10 x c-8) to distribute network load.

### Load Balancer Testing (2025-11-23)

Tested DigitalOcean Load Balancers with single proxy backend:

| Configuration | Target RPS | Achieved RPS | P50 Latency | Notes |
|--------------|------------|--------------|-------------|-------|
| Direct to proxy | 300k | 207k | 7.4ms | No LB |
| lb-large (6 units) | 300k | 34k | 52ms | ~$72/month |
| lb-small (1 unit) | 300k | 7k | 240ms | ~$12/month |

**Key Finding**: DO Load Balancers are severely rate-limited compared to direct droplet access:
- lb-large: Only 17% of direct throughput
- lb-small: Only 3.5% of direct throughput

**Conclusion**: For high-throughput (>50k RPS), use DNS round-robin to multiple proxy instances instead of DO Load Balancer.

## Recommendations

### For Next Load Test Session

1. **Start Fresh**: Restart all components before testing
2. **Progressive Testing**: Start at 50k, increment by 50k
3. **Cool Down**: Wait 30s between tests
4. **Monitor Continuously**: Check CPU, connections during tests
5. **Single Generator First**: Establish baseline before multi-gen
6. **Verify ulimit**: Ensure `ulimit -n` shows 1000000 on all droplets
7. **Restart SSHD**: After running tune-droplet.sh, run `systemctl restart sshd`

### Cleanup Between Tests

If performance degrades, run this on each machine:

```bash
# Check TIME_WAIT connections
ss -s

# If many TIME_WAIT, wait 60s or restart services
# On proxy:
systemctl restart highper-gateway

# On load generators - clear TIME_WAIT by waiting or rebooting
# Verify: ss -s should show <100 total connections
```

### Verify Tuning

After applying tune-droplet.sh on each droplet:

```bash
# Verify sysctls
sysctl net.core.somaxconn        # Should be 65535
sysctl net.ipv4.tcp_tw_reuse     # Should be 1

# Verify ulimit (MUST restart sshd first)
ulimit -n                        # Should be 1000000

# Verify on proxy
systemctl show highper-gateway -p LimitNOFILE  # Should be 1048576
```

### Tuning Script

Create a setup script to apply all tuning at once:

```bash
#!/bin/bash
# tune-system.sh - Apply to all droplets

sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=5
sysctl -w net.ipv4.tcp_max_tw_buckets=500000
sysctl -w net.core.netdev_max_backlog=65535
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216

# For persistent changes, add to /etc/sysctl.conf
```

### Next Steps

1. **Investigate degradation**: Profile highper-gateway during multi-generator load
2. **Test backend scaling**: Try larger backend droplets or more instances
3. **Connection pool debugging**: Add logging to track pool state
4. **Consider SO_REUSEPORT**: May help with multi-generator contention

## Droplet IPs Reference

```bash
# Current Infrastructure (2025-11-23)
PROXY_PUBLIC=143.110.184.230
PROXY_PRIVATE=10.20.0.2

BACKEND_1_PUBLIC=64.227.131.145
BACKEND_1_PRIVATE=10.20.0.6
BACKEND_2_PUBLIC=64.227.165.18
BACKEND_2_PRIVATE=10.20.0.7
BACKEND_3_PUBLIC=139.59.91.86
BACKEND_3_PRIVATE=10.20.0.8

LOADGEN_1_PUBLIC=64.227.164.238
LOADGEN_1_PRIVATE=10.20.0.11
LOADGEN_2_PUBLIC=64.227.133.205
LOADGEN_2_PRIVATE=10.20.0.10
LOADGEN_3_PUBLIC=64.227.162.172
LOADGEN_3_PRIVATE=10.20.0.9
```
