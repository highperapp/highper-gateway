# Performance Tuning Guide

This guide covers performance optimization strategies for the Rust reverse proxy, from OS-level tuning to application configuration and benchmarking.

## Table of Contents

1. [Performance Tiers](#performance-tiers)
2. [OS-Level Tuning](#os-level-tuning)
3. [Application Configuration](#application-configuration)
4. [Network Optimization](#network-optimization)
5. [Hardware Recommendations](#hardware-recommendations)
6. [Benchmarking](#benchmarking)
7. [Profiling](#profiling)
8. [Common Performance Patterns](#common-performance-patterns)
9. [Troubleshooting](#troubleshooting)

---

## Performance Tiers

Based on load testing results, the proxy achieves different performance tiers:

| Tier | Throughput | P99 Latency | Configuration |
|------|-----------|-------------|---------------|
| **Tier 1** | 1k req/s | < 100ms | Default settings |
| **Tier 2** | 10k req/s | < 50ms | Basic tuning |
| **Tier 3** | 50k req/s | < 25ms | Advanced tuning |
| **Tier 4** | 100k+ req/s | < 10ms | Expert tuning + hardware |

**Current Achievement:** Tier 2+ (10k req/s with p99 = 12.4ms)

---

## OS-Level Tuning

### Linux Kernel Parameters

Add these to `/etc/sysctl.conf` or `/etc/sysctl.d/99-rust-proxy.conf`:

```bash
# File descriptor limits
fs.file-max = 2097152
fs.nr_open = 2097152

# Network tuning
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65536
net.ipv4.tcp_max_syn_backlog = 65536

# TCP connection handling
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 300
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.tcp_keepalive_intvl = 15

# TCP buffer sizes (bytes)
net.core.rmem_default = 262144
net.core.rmem_max = 16777216
net.core.wmem_default = 262144
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216

# Increase number of incoming connections backlog
net.ipv4.tcp_max_tw_buckets = 1440000

# Disable SYN cookies (if not under attack)
# net.ipv4.tcp_syncookies = 0

# Enable TCP Fast Open
net.ipv4.tcp_fastopen = 3

# BBR congestion control (Linux 4.9+)
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# Increase local port range
net.ipv4.ip_local_port_range = 10000 65535
```

Apply changes:
```bash
sudo sysctl -p
```

### User Limits

Edit `/etc/security/limits.conf`:

```
rust-proxy soft nofile 65536
rust-proxy hard nofile 65536
rust-proxy soft nproc 4096
rust-proxy hard nproc 4096
```

Verify:
```bash
ulimit -n  # File descriptors
ulimit -u  # Processes
```

### Transparent Huge Pages

For better memory performance:

```bash
# Check current setting
cat /sys/kernel/mm/transparent_hugepage/enabled

# Set to 'madvise' (recommended)
echo madvise | sudo tee /sys/kernel/mm/transparent_hugepage/enabled

# Make persistent (add to /etc/rc.local or systemd service)
```

### CPU Governor

Set CPU governor to 'performance' for consistent latency:

```bash
# Check current governor
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Set to performance
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Make persistent
sudo apt-get install cpufrequtils
sudo systemctl disable ondemand
echo 'GOVERNOR="performance"' | sudo tee /etc/default/cpufrequtils
sudo systemctl restart cpufrequtils
```

---

## Application Configuration

### Connection Pool Tuning

**Default Configuration** (Tier 1):
```toml
[upstreams.connection]
max_connections_per_upstream = 100
min_idle_connections = 0
connection_timeout = "5s"
idle_timeout = "60s"

[upstreams.connection.connection_pool]
max_idle_per_host = 100
min_idle_per_host = 0
prewarm = false
```

**Optimized Configuration** (Tier 2+):
```toml
[upstreams.connection]
max_connections_per_upstream = 500
min_idle_connections = 50
connection_timeout = "5s"
idle_timeout = "90s"

[upstreams.connection.connection_pool]
max_idle_per_host = 200
min_idle_per_host = 50
prewarm = true
metrics_enabled = true
```

**High-Performance Configuration** (Tier 3+):
```toml
[upstreams.connection]
max_connections_per_upstream = 2000
min_idle_connections = 100
connection_timeout = "3s"
idle_timeout = "120s"

[upstreams.connection.connection_pool]
max_idle_per_host = 500
min_idle_per_host = 100
prewarm = true
metrics_enabled = true
```

**Tuning Guidelines:**
- `max_connections_per_upstream`: Set to expected peak concurrent requests × 1.2
- `min_idle_connections`: Set to average concurrent requests
- `prewarm`: Always enable for production
- For 10k req/s sustained: 500 connections
- For 50k req/s sustained: 2000 connections

### Worker Threads

```toml
[server]
workers = 0  # Auto-detect (recommended)
# workers = 8  # Or set explicitly
```

**Guidelines:**
- `workers = 0`: Auto-detect (CPU cores)
- For CPU-bound workloads: CPU cores
- For I/O-bound workloads: CPU cores × 2
- For mixed workloads: CPU cores × 1.5

### Buffer Sizes

```toml
[server.buffers]
read_buffer_size = 16384   # 16 KB (up from 8 KB default)
write_buffer_size = 16384  # 16 KB
```

**Guidelines:**
- Small payloads (< 1 KB): 8 KB
- Medium payloads (1-10 KB): 16 KB
- Large payloads (> 10 KB): 32 KB
- Very large payloads: 64 KB

### Timeout Configuration

**Balanced (recommended):**
```toml
[upstreams.timeouts]
connect = "5s"
request = "30s"
idle = "90s"
```

**Low-latency (aggressive):**
```toml
[upstreams.timeouts]
connect = "2s"
request = "10s"
idle = "30s"
```

**Long-running requests:**
```toml
[upstreams.timeouts]
connect = "10s"
request = "300s"  # 5 minutes
idle = "120s"
```

### Retry Configuration

**Conservative (default):**
```toml
[upstreams.retry]
max_attempts = 3
backoff_base = "100ms"
backoff_max = "10s"
```

**Aggressive (low-latency):**
```toml
[upstreams.retry]
max_attempts = 2
backoff_base = "50ms"
backoff_max = "5s"
```

**Disabled (when idempotency not guaranteed):**
```toml
[upstreams.retry]
max_attempts = 1  # Effectively disabled
```

### Circuit Breaker Tuning

**Default:**
```toml
[upstreams.circuit_breaker]
enabled = true
failure_threshold = 5
success_threshold = 2
timeout = "60s"
half_open_max_requests = 3
```

**Sensitive (fail fast):**
```toml
[upstreams.circuit_breaker]
enabled = true
failure_threshold = 3
success_threshold = 2
timeout = "30s"
half_open_max_requests = 1
```

**Tolerant (allow more failures):**
```toml
[upstreams.circuit_breaker]
enabled = true
failure_threshold = 10
success_threshold = 5
timeout = "120s"
half_open_max_requests = 5
```

---

## Network Optimization

### TCP Tuning

**Enable TCP Fast Open** (reduces connection latency):
```bash
# On proxy server
net.ipv4.tcp_fastopen = 3
```

**BBR Congestion Control** (better throughput):
```bash
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
```

Verify:
```bash
sysctl net.ipv4.tcp_congestion_control
```

### Network Interface Tuning

**Check current settings:**
```bash
ethtool -g eth0
ethtool -k eth0
```

**Increase ring buffer sizes:**
```bash
sudo ethtool -G eth0 rx 4096 tx 4096
```

**Enable offloading features:**
```bash
sudo ethtool -K eth0 tso on gso on gro on
```

**Disable features that may increase latency:**
```bash
# GRO can increase latency - disable for ultra-low latency
sudo ethtool -K eth0 gro off
```

### DNS Optimization

**Use local DNS cache:**
```bash
sudo apt-get install systemd-resolved
sudo systemctl enable systemd-resolved
```

**Or use dnsmasq:**
```bash
sudo apt-get install dnsmasq
echo "cache-size=10000" | sudo tee -a /etc/dnsmasq.conf
sudo systemctl restart dnsmasq
```

---

## Hardware Recommendations

### CPU

| Load Level | Minimum | Recommended | High-Performance |
|------------|---------|-------------|------------------|
| < 1k req/s | 2 cores | 4 cores | 8 cores |
| 1-10k req/s | 4 cores | 8 cores | 16 cores |
| 10-50k req/s | 8 cores | 16 cores | 32 cores |
| > 50k req/s | 16 cores | 32 cores | 64 cores |

**CPU Features:**
- Modern x86_64 architecture (Intel Xeon or AMD EPYC)
- High clock speed (3.0+ GHz boost)
- Large L3 cache (16+ MB)
- AVX2 or AVX-512 support (for SIMD optimizations)

### Memory

| Load Level | Minimum | Recommended | High-Performance |
|------------|---------|-------------|------------------|
| < 1k req/s | 2 GB | 4 GB | 8 GB |
| 1-10k req/s | 4 GB | 8 GB | 16 GB |
| 10-50k req/s | 8 GB | 16 GB | 32 GB |
| > 50k req/s | 16 GB | 32 GB | 64 GB |

**Memory Considerations:**
- DDR4-3200 or faster
- ECC memory for production
- Account for connection pool: ~1 MB per 1000 connections

### Network

**Bandwidth:**
- 1 Gbps: Up to 10k req/s
- 10 Gbps: Up to 100k req/s
- 25+ Gbps: > 100k req/s

**Network Card Features:**
- SR-IOV support
- RSS (Receive Side Scaling)
- Multiple TX/RX queues
- Hardware offloading (TSO, GSO, GRO)

### Storage

**Not critical** for the proxy itself, but important for:
- Logs: SSD with 100+ IOPS
- Metrics/data: NVMe SSD for high-volume metrics

---

## Benchmarking

### Load Testing Tools

**k6 (Scenario-based testing):**
```bash
k6 run --vus 100 --duration 30s load-tests/k6-load-test.js
```

**vegeta (Constant-rate testing):**
```bash
echo "GET http://localhost:8080/" | vegeta attack -rate=10000 -duration=30s | vegeta report
```

**wrk (High-performance benchmarking):**
```bash
wrk -t12 -c400 -d30s http://localhost:8080/
```

**Apache Bench (Quick tests):**
```bash
ab -n 10000 -c 100 http://localhost:8080/
```

### Benchmark Scenarios

**1. Baseline Test (Tier 1)**
```bash
# Target: 1k req/s for 60s
echo "GET http://localhost:8080/" | vegeta attack -rate=1000 -duration=60s | tee results.bin | vegeta report

# Expected:
# p50 < 10ms
# p99 < 100ms
# Success rate > 99.9%
```

**2. Performance Test (Tier 2)**
```bash
# Target: 10k req/s for 60s
echo "GET http://localhost:8080/" | vegeta attack -rate=10000 -duration=60s | tee results.bin | vegeta report

# Expected (with optimization):
# p50 < 5ms
# p99 < 50ms
# Success rate > 99.9%
```

**3. Stress Test (Tier 3)**
```bash
# Target: 50k req/s for 60s
echo "GET http://localhost:8080/" | vegeta attack -rate=50000 -duration=60s | tee results.bin | vegeta report

# Expected (with advanced tuning):
# p50 < 3ms
# p99 < 25ms
# Success rate > 99.5%
```

**4. Spike Test**
```bash
# Ramp up from 100 to 10k req/s
k6 run --vus 1 --stage 10s:100,20s:1000,30s:5000,40s:10000 load-tests/k6-load-test.js
```

**5. Soak Test (Endurance)**
```bash
# Sustained load for 1 hour
echo "GET http://localhost:8080/" | vegeta attack -rate=5000 -duration=3600s | tee results.bin | vegeta report

# Monitor for:
# - Memory leaks
# - Connection pool exhaustion
# - File descriptor leaks
```

### Interpreting Results

**Key Metrics:**
- **Throughput**: Actual req/s achieved
- **Latency**: p50, p95, p99, max
- **Error Rate**: Should be < 0.1%
- **Resource Usage**: CPU, memory, network

**Performance Targets:**

| Metric | Good | Acceptable | Poor |
|--------|------|------------|------|
| P50 latency | < 5ms | 5-20ms | > 20ms |
| P99 latency | < 50ms | 50-200ms | > 200ms |
| Error rate | < 0.01% | 0.01-0.1% | > 0.1% |
| CPU usage | < 70% | 70-85% | > 85% |

---

## Profiling

### CPU Profiling

**Using perf:**
```bash
# Record for 30 seconds
sudo perf record -F 99 -p $(pgrep rust-proxy) -g -- sleep 30

# Generate report
sudo perf report

# Generate flame graph
sudo perf script | ./flamegraph.pl > flame.svg
```

**Using criterion (for benchmarks):**
```bash
cd rust-proxy
cargo bench --bench optimization_bench
```

### Memory Profiling

**Using valgrind (massif):**
```bash
valgrind --tool=massif --massif-out-file=massif.out ./target/release/rust-proxy

# Visualize
ms_print massif.out
```

**Using heaptrack:**
```bash
heaptrack ./target/release/rust-proxy
heaptrack_gui heaptrack.rust-proxy.*.gz
```

### Network Profiling

**Using tcpdump:**
```bash
sudo tcpdump -i eth0 -w capture.pcap 'port 8080'
```

**Using ss (socket statistics):**
```bash
# Connection states
ss -tan | awk '{print $1}' | sort | uniq -c

# Connection queue depths
ss -tan state established '( dport = :8080 or sport = :8080 )'
```

### Application Metrics

Monitor via Prometheus:
```bash
# Check connection pool usage
http_connection_pool_active

# Check request duration
histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))

# Check error rate
rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m])
```

---

## Common Performance Patterns

### Pattern 1: Connection Pool Exhaustion

**Symptoms:**
- High p99 latency (> 100ms)
- Errors: "Connection pool exhausted"
- Flat throughput despite increasing load

**Solution:**
```toml
[upstreams.connection]
max_connections_per_upstream = 1000  # Increase from 100
min_idle_connections = 100           # Pre-warm
```

**Verification:**
```bash
# Check connection pool metrics
curl http://localhost:9090/metrics | grep connection_pool
```

### Pattern 2: File Descriptor Exhaustion

**Symptoms:**
- Errors: "Too many open files"
- Connection refused errors
- Degraded performance

**Solution:**
```bash
# Increase ulimits
ulimit -n 65536

# Verify
lsof -p $(pgrep rust-proxy) | wc -l
```

### Pattern 3: CPU Saturation

**Symptoms:**
- CPU usage > 90%
- Increasing latency
- Decreased throughput

**Solutions:**
1. Scale horizontally (add more instances)
2. Increase worker threads
3. Optimize application code (check with profiler)

### Pattern 4: Memory Pressure

**Symptoms:**
- High memory usage
- Swap usage
- OOM killer events

**Solutions:**
```toml
# Reduce buffer sizes
[server.buffers]
read_buffer_size = 8192  # Down from 16384

# Reduce connection pool
[upstreams.connection]
max_connections_per_upstream = 300  # Down from 500
```

### Pattern 5: Network Saturation

**Symptoms:**
- Network usage at line rate
- Packet drops
- Retransmissions

**Solutions:**
1. Upgrade network interface
2. Enable compression (if applicable)
3. Scale horizontally

---

## Troubleshooting

### High Latency

**Check 1: Connection Pool**
```bash
curl http://localhost:9090/metrics | grep -E "connection_pool_(active|idle)"
```

If `active ≈ max`, increase `max_connections_per_upstream`.

**Check 2: Backend Latency**
```bash
curl http://localhost:9090/metrics | grep upstream_request_duration
```

If high, the issue is backend, not proxy.

**Check 3: CPU Usage**
```bash
top -p $(pgrep rust-proxy)
```

If > 80%, add more workers or scale horizontally.

**Check 4: Network Latency**
```bash
ping backend-server
traceroute backend-server
```

### Low Throughput

**Check 1: Worker Threads**
```bash
# Check current workers
ps -eLf | grep rust-proxy | wc -l
```

**Check 2: System Limits**
```bash
# File descriptors
lsof -p $(pgrep rust-proxy) | wc -l
ulimit -n

# Check for errors
dmesg | grep -i error
```

**Check 3: Network Bandwidth**
```bash
iftop -i eth0
# or
nload eth0
```

### Memory Leaks

**Monitor over time:**
```bash
# Watch memory usage
watch -n 1 'ps aux | grep rust-proxy'

# Check with Prometheus
# memory_usage_bytes (if exposed)
```

**Profile with valgrind:**
```bash
valgrind --leak-check=full ./target/release/rust-proxy
```

### Connection Issues

**Check connection states:**
```bash
ss -tan | awk '{print $1}' | sort | uniq -c
```

Look for:
- Too many `TIME_WAIT`: Increase `net.ipv4.tcp_tw_reuse`
- Too many `CLOSE_WAIT`: Application not closing connections
- Too many `SYN_SENT`: Connection timeout issues

**Check listen queue:**
```bash
ss -lnt
```

If `Send-Q` or `Recv-Q` is high, increase `net.core.somaxconn`.

---

## Performance Tuning Checklist

### Tier 1 → Tier 2 (1k → 10k req/s)

- [ ] Apply OS-level tuning (sysctl)
- [ ] Increase file descriptor limits
- [ ] Optimize connection pool (500 connections)
- [ ] Enable connection pre-warming
- [ ] Increase buffer sizes to 16 KB
- [ ] Enable BBR congestion control
- [ ] Run load tests and verify p99 < 50ms

### Tier 2 → Tier 3 (10k → 50k req/s)

- [ ] All Tier 2 optimizations
- [ ] Increase connection pool to 2000
- [ ] Tune worker threads (CPU cores × 2)
- [ ] Optimize network interface (ring buffers, offloading)
- [ ] Enable CPU performance governor
- [ ] Disable unnecessary services
- [ ] Consider NUMA tuning
- [ ] Run stress tests and verify p99 < 25ms

### Tier 3 → Tier 4 (50k → 100k+ req/s)

- [ ] All Tier 3 optimizations
- [ ] Hardware: 16+ cores, 32+ GB RAM, 10+ Gbps network
- [ ] Kernel bypass (io_uring, DPDK consideration)
- [ ] CPU pinning and NUMA optimization
- [ ] Custom kernel compilation with optimizations
- [ ] Consider multi-instance deployment
- [ ] Advanced profiling and optimization
- [ ] Run endurance tests

---

## Additional Resources

- [Linux Performance](http://www.brendangregg.com/linuxperf.html) by Brendan Gregg
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Load Testing Guide](../load-tests/README.md)
- [Benchmarking Results](../WEEK10_BENCHMARK_RESULTS.md)
- [Connection Pool Optimization](../BUFFER_POOL_IMPROVEMENTS.md)
