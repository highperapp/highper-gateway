# Performance Tuning Quick Reference

A quick reference guide for performance tuning the Rust reverse proxy.

## Quick Start

```bash
# 1. Apply OS-level tuning (requires root)
sudo ./scripts/performance-tune.sh apply

# 2. Use optimized configuration
cp config-production-secure.toml config.toml

# 3. Edit connection pool settings
nano config.toml  # Adjust max_connections_per_upstream

# 4. Run load tests
cd load-tests
./run-all-tests.sh

# 5. Monitor performance
./scripts/performance-monitor.sh
```

## Performance Tiers

| Target | Config | Command |
|--------|--------|---------|
| 1k req/s | Default | `max_connections_per_upstream = 100` |
| 10k req/s | Optimized | `max_connections_per_upstream = 500` |
| 50k req/s | Advanced | `max_connections_per_upstream = 2000` |

## Critical Settings

### Connection Pool (config.toml)

```toml
[upstreams.connection]
max_connections_per_upstream = 500  # ← Increase for higher load
min_idle_connections = 50           # ← Pre-warm connections
connection_timeout = "5s"
idle_timeout = "90s"

[upstreams.connection.connection_pool]
max_idle_per_host = 200
min_idle_per_host = 50
prewarm = true                       # ← Always enable
metrics_enabled = true               # ← Monitor pool usage
```

### OS Tuning (requires root)

```bash
# Quick one-liner tuning
sudo sysctl -w net.core.somaxconn=65535
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=65536
sudo sysctl -w fs.file-max=2097152

# Or use script
sudo ./scripts/performance-tune.sh apply
```

## Common Issues and Fixes

### Issue: High P99 Latency (> 50ms)

**Cause:** Connection pool exhaustion

**Fix:**
```toml
[upstreams.connection]
max_connections_per_upstream = 1000  # Double it
min_idle_connections = 100           # Increase pre-warming
```

**Verify:**
```bash
curl http://localhost:9090/metrics | grep connection_pool_active
```

### Issue: "Too many open files"

**Cause:** File descriptor limit

**Fix:**
```bash
# Check current limit
ulimit -n

# Increase limit
sudo sh -c 'echo "rust-proxy soft nofile 65536" >> /etc/security/limits.conf'
sudo sh -c 'echo "rust-proxy hard nofile 65536" >> /etc/security/limits.conf'

# Or use script
sudo ./scripts/performance-tune.sh apply
```

### Issue: Low Throughput (< expected)

**Cause:** CPU saturation or network limits

**Fix:**
```bash
# Check CPU usage
top -p $(pgrep rust-proxy)

# If > 80%, scale horizontally or increase workers
# config.toml:
[server]
workers = 16  # Increase from auto-detect

# Check network
iftop -i eth0
```

### Issue: Memory Growing Continuously

**Cause:** Potential memory leak or connection pool too large

**Fix:**
```bash
# Monitor memory
watch -n 1 'ps aux | grep rust-proxy'

# Reduce connection pool
[upstreams.connection]
max_connections_per_upstream = 300  # Reduce
```

## Load Testing Commands

```bash
# Baseline (1k req/s)
echo "GET http://localhost:8080/" | vegeta attack -rate=1000 -duration=30s | vegeta report

# Performance (10k req/s)
echo "GET http://localhost:8080/" | vegeta attack -rate=10000 -duration=30s | vegeta report

# Stress (50k req/s)
echo "GET http://localhost:8080/" | vegeta attack -rate=50000 -duration=30s | vegeta report

# Monitor during test
./scripts/performance-monitor.sh
```

## Metrics to Monitor

### Prometheus Queries

```promql
# Request rate
rate(http_requests_total[5m])

# P99 latency (in ms)
histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) * 1000

# Error rate (%)
rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m]) * 100

# Connection pool usage (%)
connection_pool_active / connection_pool_max * 100
```

### Command Line

```bash
# Real-time monitoring
./scripts/performance-monitor.sh

# Check metrics endpoint
curl http://localhost:9090/metrics

# Connection states
ss -tan | awk '{print $1}' | sort | uniq -c

# File descriptors
lsof -p $(pgrep rust-proxy) | wc -l
```

## Performance Targets

| Metric | Good | Acceptable | Poor |
|--------|------|------------|------|
| P50 latency | < 5ms | 5-20ms | > 20ms |
| P99 latency | < 50ms | 50-200ms | > 200ms |
| Error rate | < 0.01% | 0.01-0.1% | > 0.1% |
| CPU usage | < 70% | 70-85% | > 85% |
| Memory | < 3 GB | 3-4 GB | > 4 GB |

## Tuning Checklist

### Level 1: Basic (1k → 10k req/s)

- [ ] Apply OS tuning: `sudo ./scripts/performance-tune.sh apply`
- [ ] Set connection pool to 500
- [ ] Enable connection pre-warming
- [ ] Increase buffer sizes to 16 KB
- [ ] Run load test: `vegeta attack -rate=10000`
- [ ] Verify P99 < 50ms

### Level 2: Advanced (10k → 50k req/s)

- [ ] All Level 1 items
- [ ] Set connection pool to 2000
- [ ] Tune worker threads (CPU cores × 2)
- [ ] Enable BBR: `sudo sysctl -w net.ipv4.tcp_congestion_control=bbr`
- [ ] Optimize NIC: `sudo ethtool -G eth0 rx 4096 tx 4096`
- [ ] Run stress test: `vegeta attack -rate=50000`
- [ ] Verify P99 < 25ms

### Level 3: Expert (50k+ req/s)

- [ ] All Level 2 items
- [ ] Hardware: 16+ cores, 32+ GB RAM, 10+ Gbps NIC
- [ ] CPU pinning and NUMA optimization
- [ ] Set CPU governor to performance
- [ ] Profile with perf and optimize hotspots
- [ ] Consider multi-instance deployment

## Configuration Templates

### Default (Tier 1: 1k req/s)

```toml
[upstreams.connection]
max_connections_per_upstream = 100
min_idle_connections = 0
prewarm = false
```

### Optimized (Tier 2: 10k req/s)

```toml
[upstreams.connection]
max_connections_per_upstream = 500
min_idle_connections = 50
prewarm = true

[server.buffers]
read_buffer_size = 16384
write_buffer_size = 16384
```

### High-Performance (Tier 3: 50k req/s)

```toml
[upstreams.connection]
max_connections_per_upstream = 2000
min_idle_connections = 100
prewarm = true

[server]
workers = 16

[server.buffers]
read_buffer_size = 32768
write_buffer_size = 32768

[upstreams.timeouts]
connect = "3s"
request = "20s"
```

## Troubleshooting Commands

```bash
# Check configuration
./rust-proxy validate --config config.toml

# View logs
journalctl -u rust-proxy -f

# Check health
curl http://localhost:9090/health

# Check metrics
curl http://localhost:9090/metrics

# Monitor system
htop
iotop
iftop

# Profile CPU
sudo perf record -F 99 -p $(pgrep rust-proxy) -g -- sleep 30
sudo perf report

# Check connections
ss -tan | grep :8080

# Check file descriptors
ls /proc/$(pgrep rust-proxy)/fd | wc -l
```

## Quick Decision Tree

```
High latency?
├─ YES: Connection pool full?
│   ├─ YES → Increase max_connections_per_upstream
│   └─ NO: Backend slow?
│       ├─ YES → Optimize backend
│       └─ NO → Check CPU/network
└─ NO: Low throughput?
    ├─ YES: CPU saturated?
    │   ├─ YES → Scale horizontally or increase workers
    │   └─ NO → Check network bandwidth
    └─ NO: High error rate?
        ├─ YES: Check circuit breaker settings
        └─ NO → All good! Monitor and maintain
```

## Resource Links

- Full guide: [PERFORMANCE_TUNING_GUIDE.md](PERFORMANCE_TUNING_GUIDE.md)
- Load testing: [load-tests/README.md](load-tests/README.md)
- Deployment: [PRODUCTION_DEPLOYMENT_GUIDE.md](PRODUCTION_DEPLOYMENT_GUIDE.md)
- Monitoring: [PROMETHEUS_GRAFANA_GUIDE.md](PROMETHEUS_GRAFANA_GUIDE.md)
