# P99 Latency Diagnostic Report

## Test Configuration
- Backend: Node.js simple HTTP server
- Proxy: Rust Reverse Proxy (release build)
- Test Duration: 15-30 seconds per rate
- Rates Tested: 1k, 2k, 4k, 6k, 8k, 10k req/s

## Results Summary

### Latency by Load

| Rate | p99 Latency | p95 Latency | p50 Latency | Success Rate |
|------|-------------|-------------|-------------|--------------|
| 1k req/s |  | 73.00 | mean] | [ratio] |
| 2000 req/s |  | 73.00 | mean] | [ratio] |
| 4000 req/s |  | 73.00 | mean] | [ratio] |
| 6000 req/s |  | 73.00 | mean] | [ratio] |
| 8000 req/s |  | 73.00 | mean] | [ratio] |
| 10k req/s |  | 73.00 | mean] | [ratio] |

### Resource Usage

- **Peak CPU:** 86.3%
- **Average CPU:** 56.0106%
- **Peak Memory:** 84532 KB
- **Peak File Descriptors:** 2048
- **Peak TCP Connections:** 2311

### Error Analysis

- **Total Errors:** 0
0
- **Total Warnings:** 0
0

## Next Steps

Based on this diagnostic, the next steps are:

1. **Review proxy logs** for specific error patterns
2. **Profile with flamegraph** to identify hot code paths
3. **Check Tokio runtime** metrics for task scheduling delays
4. **Analyze connection pool** behavior and wait times
5. **Test with faster backend** to isolate proxy vs backend issue

## Files Generated

- `vegeta-*.bin` - Raw vegeta results
- `vegeta-*.txt` - Vegeta reports
- `proxy-metrics.csv` - System metrics over time
- `proxy.log` - Proxy application logs
- `backend.log` - Backend server logs

