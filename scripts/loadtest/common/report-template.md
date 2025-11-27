# Load Test Report: {{SCENARIO_NAME}}

**Test ID**: `{{TEST_ID}}`
**Date**: {{TEST_DATE}}
**Duration**: {{TEST_DURATION}} seconds
**Provider**: {{PROVIDER}} ({{REGION}})

---

## Executive Summary

{{SCENARIO_DESCRIPTION}}

| Metric | Target | Achieved | Status |
|--------|--------|----------|--------|
| **Requests/sec** | {{TARGET_RPS}} | {{ACTUAL_RPS}} | {{RPS_STATUS}} |
| **P99 Latency** | < {{TARGET_P99}}ms | {{ACTUAL_P99}}ms | {{P99_STATUS}} |
| **Success Rate** | > 99.99% | {{SUCCESS_RATE}}% | {{SUCCESS_STATUS}} |
| **Connections** | {{TARGET_CONNECTIONS}} | {{ACTUAL_CONNECTIONS}} | {{CONN_STATUS}} |

**Overall Result**: {{OVERALL_STATUS}}

---

## Test Configuration

### Infrastructure

#### Proxy Server (Gateway)
- **Plan**: {{PROXY_PLAN}} - {{PROXY_CPU}} cores, {{PROXY_MEMORY}}GB RAM
- **IP**: {{PROXY_IP}}
- **Cost**: ${{PROXY_COST}}/hour

#### Backend Servers (×{{BACKEND_COUNT}})
- **Plan**: {{BACKEND_PLAN}} - {{BACKEND_CPU}} cores, {{BACKEND_MEMORY}}GB RAM
- **IPs**: {{BACKEND_IPS}}
- **Total Cost**: ${{BACKEND_COST}}/hour

#### Load Generators (×{{GENERATOR_COUNT}})
- **Plan**: {{GENERATOR_PLAN}} - {{GENERATOR_CPU}} cores, {{GENERATOR_MEMORY}}GB RAM
- **IPs**: {{GENERATOR_IPS}}
- **Tool**: {{LOAD_TEST_TOOL}}
- **Total Cost**: ${{GENERATOR_COST}}/hour

**Total Infrastructure**: {{TOTAL_INSTANCES}} instances, {{TOTAL_CORES}} cores, {{TOTAL_MEMORY}}GB RAM
**Test Cost**: ${{TEST_COST}} ({{TEST_DURATION_MINUTES}} minutes × ${{TOTAL_COST_PER_HOUR}}/hour)

### Software Versions

| Component | Version | Details |
|-----------|---------|---------|
| **Highper Gateway** | {{GATEWAY_VERSION}} | Commit: `{{GIT_COMMIT}}` |
| **Backend** | {{BACKEND_VERSION}} | {{BACKEND_FRAMEWORK}} |
| **Load Test Tool** | {{LOAD_TEST_TOOL_VERSION}} | Primary tool |
| **Prometheus** | {{PROMETHEUS_VERSION}} | Metrics collection |
| **Grafana** | {{GRAFANA_VERSION}} | Visualization |

### Gateway Configuration

```yaml
max_connections: {{MAX_CONNECTIONS}}
connection_timeout: {{CONNECTION_TIMEOUT}}
keepalive_timeout: {{KEEPALIVE_TIMEOUT}}
load_balancer: {{LB_ALGORITHM}}
rate_limit: {{RATE_LIMIT_RPS}} RPS (burst: {{RATE_LIMIT_BURST}})
buffer_pool:
  enabled: {{BUFFER_POOL_ENABLED}}
  buffer_size: {{BUFFER_SIZE}} bytes
  pool_size: {{POOL_SIZE}} buffers
connection_pool:
  min_idle: {{POOL_MIN_IDLE}}
  max_idle: {{POOL_MAX_IDLE}}
  max_open: {{POOL_MAX_OPEN}}
```

### System Optimizations Applied

```bash
# Network tuning
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_max_tw_buckets = 2000000
net.core.somaxconn = 65535
net.ipv4.tcp_congestion_control = bbr
net.ipv4.ip_local_port_range = 1024-65535

# Resource limits
fs.file-max = 10000000
ulimit -n 10000000

# Memory tuning
vm.swappiness = 10
transparent_hugepages = disabled
```

---

## Results

### Throughput

| Metric | Value |
|--------|-------|
| **Total Requests** | {{TOTAL_REQUESTS}} |
| **Successful Requests** | {{SUCCESSFUL_REQUESTS}} |
| **Failed Requests** | {{FAILED_REQUESTS}} |
| **Actual RPS** | {{ACTUAL_RPS}} |
| **Success Rate** | {{SUCCESS_RATE}}% |
| **Data Transferred** | {{DATA_TRANSFERRED}}GB |
| **Throughput** | {{THROUGHPUT_GBPS}}Gbps |

**Achievement**: {{RPS_ACHIEVEMENT}}% of target ({{TARGET_RPS}} RPS)

### Latency Distribution

| Percentile | Latency | Status vs Target |
|------------|---------|------------------|
| **Min** | {{MIN_LATENCY}}ms | - |
| **P50 (Median)** | {{P50_LATENCY}}ms | {{P50_STATUS}} |
| **P95** | {{P95_LATENCY}}ms | {{P95_STATUS}} |
| **P99** | {{P99_LATENCY}}ms | {{P99_STATUS}} |
| **P99.9** | {{P999_LATENCY}}ms | {{P999_STATUS}} |
| **Max** | {{MAX_LATENCY}}ms | - |
| **Mean** | {{MEAN_LATENCY}}ms | - |
| **Std Dev** | {{STDDEV_LATENCY}}ms | - |

```
Latency Histogram:
{{LATENCY_HISTOGRAM}}
```

### Error Analysis

| Error Type | Count | Percentage |
|------------|-------|------------|
| **Connection Errors** | {{CONNECTION_ERRORS}} | {{CONNECTION_ERROR_PCT}}% |
| **Timeout Errors** | {{TIMEOUT_ERRORS}} | {{TIMEOUT_ERROR_PCT}}% |
| **HTTP 4xx** | {{HTTP_4XX_ERRORS}} | {{HTTP_4XX_PCT}}% |
| **HTTP 5xx** | {{HTTP_5XX_ERRORS}} | {{HTTP_5XX_PCT}}% |
| **Other Errors** | {{OTHER_ERRORS}} | {{OTHER_ERROR_PCT}}% |
| **Total Errors** | {{TOTAL_ERRORS}} | {{TOTAL_ERROR_PCT}}% |

{{#if ERRORS_PRESENT}}
**Error Rate**: {{ERROR_RATE}}% (Target: <0.01%)
**Status**: {{ERROR_STATUS}}
{{/if}}

### Resource Utilization

#### Proxy Server (Gateway)

| Resource | Average | Peak | Status |
|----------|---------|------|--------|
| **CPU** | {{PROXY_CPU_AVG}}% | {{PROXY_CPU_PEAK}}% | {{PROXY_CPU_STATUS}} |
| **Memory** | {{PROXY_MEM_AVG}}% | {{PROXY_MEM_PEAK}}% | {{PROXY_MEM_STATUS}} |
| **Network** | {{PROXY_NET_AVG}}Gbps | {{PROXY_NET_PEAK}}Gbps | {{PROXY_NET_STATUS}} |

**Sizing Recommendation**: {{PROXY_SIZING_REC}}

#### Backend Servers (Average across {{BACKEND_COUNT}} servers)

| Resource | Average | Peak |
|----------|---------|------|
| **CPU** | {{BACKEND_CPU_AVG}}% | {{BACKEND_CPU_PEAK}}% |
| **Memory** | {{BACKEND_MEM_AVG}}% | {{BACKEND_MEM_PEAK}}% |
| **Requests/sec** | {{BACKEND_RPS_AVG}} | {{BACKEND_RPS_PEAK}} |

**Load Distribution**: {{LOAD_DISTRIBUTION_STATUS}}

---

## Visualizations

### Grafana Dashboards

#### Load Test Overview
![Load Test Overview](metrics/grafana/load-test-overview.png)

#### Gateway Performance
![Gateway Performance](metrics/grafana/gateway-performance.png)

#### Backend Health
![Backend Health](metrics/grafana/backend-health.png)

#### Latency Analysis
![Latency Analysis](metrics/grafana/latency-analysis.png)

#### Resource Utilization
![Resource Utilization](metrics/grafana/resource-utilization.png)

---

## Detailed Analysis

### Performance Observations

{{OBSERVATIONS}}

### Issues Encountered

{{ISSUES}}

### Optimizations Applied

{{OPTIMIZATIONS}}

### Comparison with Previous Tests

{{#if PREVIOUS_TEST_ID}}
| Metric | Previous Test | This Test | Change |
|--------|---------------|-----------|--------|
| **RPS** | {{PREV_RPS}} | {{ACTUAL_RPS}} | {{RPS_CHANGE}}% |
| **P99 Latency** | {{PREV_P99}}ms | {{ACTUAL_P99}}ms | {{P99_CHANGE}}% |
| **Success Rate** | {{PREV_SUCCESS}}% | {{SUCCESS_RATE}}% | {{SUCCESS_CHANGE}}% |

**Previous Test**: [{{PREVIOUS_TEST_ID}}](../{{PREVIOUS_TEST_ID}}/report.md)
{{else}}
*No previous test for comparison*
{{/if}}

---

## Recommendations

### Immediate Actions

{{IMMEDIATE_RECOMMENDATIONS}}

### Future Optimizations

{{FUTURE_RECOMMENDATIONS}}

### Resource Sizing

{{SIZING_RECOMMENDATIONS}}

---

## Test Artifacts

### Files Generated

- **Metadata**: [`metadata.json`](metadata.json)
- **Summary**: [`summary.txt`](summary.txt)
- **Vegeta Results**:
  {{#each VEGETA_RESULTS}}
  - [`{{this}}`]({{this}})
  {{/each}}
- **Prometheus Snapshot**: [`metrics/prometheus-snapshot.tar.gz`](metrics/prometheus-snapshot.tar.gz)
- **Grafana Dashboards**: See [metrics/grafana/](metrics/grafana/) directory
- **Logs**: See [logs/](logs/) directory

### Quick Access

- **Gateway Log**: [`logs/gateway.log`](logs/gateway.log)
- **Backend Logs**:
  {{#each BACKEND_LOGS}}
  - [`{{this}}`]({{this}})
  {{/each}}
- **Deployment Log**: [`logs/deployment.log`](logs/deployment.log)

---

## Conclusion

{{CONCLUSION}}

**Test Status**: {{OVERALL_STATUS}}
**Recommendation**: {{OVERALL_RECOMMENDATION}}

---

## Appendix

### Test Command

```bash
{{TEST_COMMAND}}
```

### Scenario Configuration

```yaml
{{SCENARIO_CONFIG}}
```

### System Information

```bash
# Proxy Server
{{PROXY_SYSTEM_INFO}}

# Backend Servers
{{BACKEND_SYSTEM_INFO}}

# Load Generators
{{GENERATOR_SYSTEM_INFO}}
```

---

**Generated**: {{REPORT_GENERATION_TIME}}
**Generator**: Highper Gateway Load Testing Framework v{{FRAMEWORK_VERSION}}
**Report Format**: Markdown

*For questions or issues, see [Load Testing Documentation](../../scripts/loadtest/README.md)*
