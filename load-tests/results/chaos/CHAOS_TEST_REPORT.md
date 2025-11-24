# Chaos Testing Report - Toxiproxy

## Test Environment
- Proxy: Rust Reverse Proxy (optimized config)
- Chaos Tool: Toxiproxy v2.9.0
- Backend: Node.js simple HTTP server
- Load: 100 req/s for 10 seconds per test

## Test Scenarios

### Test: 1_baseline
```
Requests      [total, rate, throughput]         1000, 100.10, 100.08
Duration      [total, attack, wait]             9.992s, 9.99s, 1.62ms
Latencies     [min, mean, 50, 90, 95, 99, max]  687.883µs, 1.438ms, 1.332ms, 2.05ms, 2.376ms, 2.822ms, 3.272ms
Bytes In      [total, mean]                     73000, 73.00
```

### Test: 2_latency_100ms
```
Requests      [total, rate, throughput]         1000, 100.10, 99.08
Duration      [total, attack, wait]             10.093s, 9.99s, 102.563ms
Latencies     [min, mean, 50, 90, 95, 99, max]  100.804ms, 101.921ms, 101.872ms, 102.65ms, 102.922ms, 103.486ms, 105.31ms
Bytes In      [total, mean]                     73000, 73.00
```

### Test: 3_latency_500ms
```
Requests      [total, rate, throughput]         1000, 100.10, 95.31
Duration      [total, attack, wait]             10.492s, 9.99s, 501.969ms
Latencies     [min, mean, 50, 90, 95, 99, max]  500.723ms, 501.827ms, 501.772ms, 502.544ms, 502.785ms, 503.513ms, 505.673ms
Bytes In      [total, mean]                     73000, 73.00
```

### Test: 4_latency_jitter
```
Requests      [total, rate, throughput]         1000, 100.10, 97.76
Duration      [total, attack, wait]             10.229s, 9.99s, 238.411ms
Latencies     [min, mean, 50, 90, 95, 99, max]  101.003ms, 200.266ms, 201.663ms, 281.198ms, 290.507ms, 299.971ms, 301.524ms
Bytes In      [total, mean]                     73000, 73.00
```

### Test: 5_bandwidth_1mbps
```
Requests      [total, rate, throughput]         1000, 100.11, 100.10
Duration      [total, attack, wait]             9.99s, 9.989s, 1.33ms
Latencies     [min, mean, 50, 90, 95, 99, max]  530.344µs, 1.561ms, 1.371ms, 2.486ms, 2.72ms, 3.35ms, 4.335ms
Bytes In      [total, mean]                     73000, 73.00
```

### Test: 6_timeout
```
Requests      [total, rate, throughput]         1000, 100.09, 0.00
Duration      [total, attack, wait]             9.991s, 9.991s, 338.663µs
Latencies     [min, mean, 50, 90, 95, 99, max]  180.505µs, 105.85ms, 574.759µs, 845.745ms, 1.003s, 1.004s, 1.005s
Bytes In      [total, mean]                     23630, 23.63
```

### Test: 7_slow_close
```
Requests      [total, rate, throughput]         1000, 100.10, 0.00
Duration      [total, attack, wait]             9.992s, 9.99s, 1.142ms
Latencies     [min, mean, 50, 90, 95, 99, max]  175.187µs, 563.078µs, 528.773µs, 834.532µs, 987.848µs, 1.453ms, 1.959ms
Bytes In      [total, mean]                     23000, 23.00
```

### Test: 8_packet_loss_10pct
```
Requests      [total, rate, throughput]         1000, 100.10, 0.00
Duration      [total, attack, wait]             9.991s, 9.99s, 852.246µs
Latencies     [min, mean, 50, 90, 95, 99, max]  169.961µs, 617.059µs, 554.348µs, 866.517µs, 1.033ms, 1.887ms, 10.854ms
Bytes In      [total, mean]                     23000, 23.00
```


## Analysis

### Resilience Metrics

| Test | Success Rate | p99 Latency | Circuit Breaker | Failures |
|------|--------------|-------------|-----------------|----------|
| Baseline | [from results] | [from results] | No | 0 |
| 100ms Latency | [from results] | [from results] | No | [count] |
| 500ms Latency | [from results] | [from results] | Possible | [count] |
| Latency + Jitter | [from results] | [from results] | Possible | [count] |
| 1 MB/s Bandwidth | [from results] | [from results] | No | [count] |
| Timeout | [from results] | [from results] | Yes | [count] |
| Slow Close | [from results] | [from results] | Possible | [count] |
| 10% Packet Loss | [from results] | [from results] | Possible | [count] |

### Key Findings

1. **Circuit Breaker Behavior**: [To be analyzed from logs]
2. **Connection Pool Under Stress**: [To be analyzed]
3. **Timeout Handling**: [To be analyzed]
4. **Error Recovery**: [To be analyzed]

### Recommendations

1. [Based on test results]
2. [Based on test results]
3. [Based on test results]

