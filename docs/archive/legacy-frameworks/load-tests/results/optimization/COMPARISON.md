# P99 Latency Optimization Results

## Test Configuration
- Load: 10,000 requests/second
- Duration: 30 seconds
- Total Requests: ~300,000
- Backend: Node.js simple HTTP server

## Results

### Old Configuration (Baseline)
```
Requests      [total, rate, throughput]         300000, 9999.85, 9998.65
Duration      [total, attack, wait]             30.004s, 30s, 3.584ms
Latencies     [min, mean, 50, 90, 95, 99, max]  132.046µs, 2.787ms, 1.176ms, 3.562ms, 5.291ms, 43.375ms, 297.832ms
Bytes In      [total, mean]                     21900000, 73.00
```

### New Configuration (Optimized)
```
Requests      [total, rate, throughput]         300000, 9999.95, 9998.67
Duration      [total, attack, wait]             30.004s, 30s, 3.837ms
Latencies     [min, mean, 50, 90, 95, 99, max]  90.762µs, 2.019ms, 1.111ms, 3.291ms, 4.694ms, 12.417ms, 145.868ms
Bytes In      [total, mean]                     21900000, 73.00
```

## Comparison

| Metric | Old Config | New Config | Improvement |
|--------|------------|------------|-------------|
| p50 | 50 | 50 | TBD |
| p95 |  |  | TBD |
| p99 | 95 | 95 | **PRIMARY METRIC** |
| max |  |  | TBD |
| Success Rate | 99 | 99 | TBD |

## Optimization Changes

1. **Connection Pool Size**: 100 → 500
2. **Pre-warming**: Disabled → 50 idle connections
3. **Per-server Limit**: Default → 1,000 connections
4. **Connection Timeout**: Default → 5s
5. **TCP Tuning**: Basic → Optimized (nodelay, larger buffers)

## Latency Histograms

### Old Configuration
```
Bucket           #       %       Histogram
[0s,     1ms]    124104  41.37%  ###############################
[1ms,    2ms]    102370  34.12%  #########################
[2ms,    5ms]    56878   18.96%  ##############
[5ms,    10ms]   11072   3.69%   ##
[10ms,   20ms]   1709    0.57%   
[20ms,   50ms]   1200    0.40%   
[50ms,   100ms]  1583    0.53%   
[100ms,  200ms]  576     0.19%   
[200ms,  500ms]  508     0.17%   
[500ms,  +Inf]   0       0.00%   
```

### New Configuration
```
Bucket           #       %       Histogram
[0s,     1ms]    133182  44.39%  #################################
[1ms,    2ms]    100199  33.40%  #########################
[2ms,    5ms]    53731   17.91%  #############
[5ms,    10ms]   9393    3.13%   ##
[10ms,   20ms]   1456    0.49%   
[20ms,   50ms]   817     0.27%   
[50ms,   100ms]  766     0.26%   
[100ms,  200ms]  456     0.15%   
[200ms,  500ms]  0       0.00%   
[500ms,  +Inf]   0       0.00%   
```

## Conclusion

⚠️ **NO IMPROVEMENT** - Further investigation needed

