# Grafana Dashboards for Highper Gateway

This directory contains pre-configured Grafana dashboards for monitoring the Highper Gateway.

## Available Dashboards

### 1. Connection Pool Metrics Dashboard
**File**: `connection-pool-metrics.json`

Complete observability for HTTP connection pool health and performance.

**Panels**:
1. **Connection Pool Overview** - Active vs Idle connections over time
2. **Connection Reuse Ratio** - Gauge showing pool efficiency (target >80%)
3. **Total Connections Created** - Lifetime counter
4. **Total Connections Reused** - Pool reuse counter
5. **Pool Exhaustion Events** - Alert when pool runs out of idle connections
6. **Per-Host Active Connections** - Multi-series graph per upstream
7. **Per-Host Idle Connections** - Pool availability per host
8. **Per-Host Reuse Ratio** - Efficiency metrics per upstream
9. **Pool Utilization Heatmap** - Visual representation of pool usage
10. **Connection Errors** - Error rates with alerting
11. **Average Connection Lifetime** - Connection longevity tracking
12. **Connection Pool Statistics Table** - Comprehensive per-host metrics

**Alerts**:
- High connection error rate (>0.1 errors/sec)
- Pool exhaustion events
- Low reuse ratio (<50%)

**Metrics Used**:
- `proxy_pool_connections_active`
- `proxy_pool_connections_idle`
- `proxy_pool_connections_created_total`
- `proxy_pool_connections_reused_total`
- `proxy_pool_reuse_ratio`
- `proxy_pool_errors_total`
- `proxy_pool_exhausted_total`
- `proxy_pool_tracked_hosts`
- `proxy_pool_host_connections_active`
- `proxy_pool_host_connections_idle`
- `proxy_pool_host_reuse_ratio`
- `proxy_pool_host_utilization`
- `proxy_pool_host_errors_total`
- `proxy_pool_host_exhausted_total`
- `proxy_pool_host_avg_lifetime_ms`

---

## Installation

### Prerequisites:
1. **Prometheus** - Scraping metrics from `/metrics` endpoint
2. **Grafana** - Dashboard visualization
3. **Highper Gateway** - Running with Admin API enabled

### Setup Steps:

#### 1. Configure Prometheus

Add to `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'highper-gateway'
    scrape_interval: 5s
    static_configs:
      - targets: ['localhost:9090']  # Admin API port
    metrics_path: '/metrics'
```

#### 2. Import Dashboard to Grafana

**Option A: UI Import**
1. Open Grafana (http://localhost:3000)
2. Navigate to Dashboards → Import
3. Upload `connection-pool-metrics.json`
4. Select Prometheus datasource
5. Click Import

**Option B: API Import**
```bash
curl -X POST http://admin:admin@localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @connection-pool-metrics.json
```

**Option C: Provisioning**
```yaml
# /etc/grafana/provisioning/dashboards/highper-gateway.yml
apiVersion: 1

providers:
  - name: 'Highper Gateway'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /path/to/highper-gateway/dashboards
```

#### 3. Configure Alerts (Optional)

Grafana will automatically create alerts based on panel configurations. To customize:

1. Navigate to Alert Rules
2. Edit "Connection Pool Errors" rule
3. Configure notification channels (Slack, PagerDuty, etc.)

---

## Dashboard Usage

### Understanding Metrics:

**Connection Reuse Ratio**:
- **Green (>80%)**: Excellent - pool is working efficiently
- **Yellow (50-80%)**: Good - room for improvement
- **Red (<50%)**: Poor - check pool configuration

**Pool Utilization**:
- **0.0**: Pool empty
- **0.5**: 50% capacity used
- **1.0**: Pool at max capacity
- **>1.0**: Pool exhausted (creating connections beyond limit)

**Pool Exhaustion Events**:
- **0**: Healthy - pool has spare capacity
- **>0**: Warning - pool running out of idle connections
- **>10**: Critical - pool consistently exhausted, increase size

**Average Connection Lifetime**:
- **Too short (<1s)**: Connections closing prematurely
- **Healthy (10s-90s)**: Normal pool behavior
- **Too long (>90s)**: Connections not being recycled (check idle timeout)

### Common Scenarios:

#### Low Reuse Ratio (<50%)
**Symptoms**: High connection creation rate, low reuse
**Causes**:
- Pool timeout too aggressive
- Backend closing connections
- High request rate with many unique hosts
**Solutions**:
- Increase `pool_idle_timeout`
- Check backend keep-alive settings
- Increase `pool_max_idle_per_host`

#### Pool Exhaustion
**Symptoms**: `pool_exhausted_total` increasing
**Causes**:
- `pool_max_idle_per_host` too small
- High concurrent request rate
- Slow backend response times
**Solutions**:
- Increase pool size (100 → 200)
- Add more backend instances
- Optimize backend performance

#### High Error Rate
**Symptoms**: `proxy_pool_errors_total` increasing
**Causes**:
- Backend unavailable
- Network issues
- Connection timeouts
**Solutions**:
- Check backend health
- Review network connectivity
- Adjust connection timeouts

---

## Customization

### Adding Custom Panels:

1. Click "Add Panel" in Grafana
2. Select metric from Prometheus datasource
3. Configure visualization (Graph, Gauge, Stat, etc.)
4. Save dashboard

### Modifying Thresholds:

Edit panel JSON and adjust threshold values:
```json
"thresholds": {
  "mode": "absolute",
  "steps": [
    {"value": 0, "color": "red"},
    {"value": 0.8, "color": "green"}
  ]
}
```

### Adding Variables:

Template variables for filtering by host:
```json
"templating": {
  "list": [
    {
      "name": "host",
      "type": "query",
      "datasource": "Prometheus",
      "query": "label_values(proxy_pool_host_connections_active, host)"
    }
  ]
}
```

---

## Troubleshooting

### Dashboard shows "No data"
1. Verify Prometheus is scraping metrics: http://localhost:9090/targets
2. Check Admin API is running: http://localhost:9090/metrics
3. Verify datasource configuration in Grafana
4. Check time range selection (last 15 minutes by default)

### Metrics not updating
1. Check Prometheus scrape interval (default 5s)
2. Verify proxy has active traffic
3. Check ProxyState has pool_metrics initialized
4. Review Prometheus logs for scrape errors

### Missing per-host metrics
1. Ensure connections have been created to hosts
2. Check that pool_metrics is tracking hosts
3. Verify host labels match Prometheus query

---

## Performance Recommendations

Based on dashboard metrics:

### Optimal Values:
- **Reuse Ratio**: >80%
- **Pool Utilization**: 30-70%
- **Avg Lifetime**: 10-90 seconds
- **Error Rate**: <0.01 errors/sec
- **Exhaustion Events**: 0

### Tuning Pool Size:
```yaml
# If utilization >90% and exhaustion events >0
pool_max_idle_per_host: 200  # Increase from 100

# If reuse ratio <50%
pool_idle_timeout: 120s  # Increase from 90s

# If avg lifetime <5s
pool_idle_timeout: 60s  # Decrease timeout
```

---

## Support

For issues or questions:
- Documentation: See `WEEK2_CONNECTION_POOL_METRICS_COMPLETE.md`
- Admin API: See `ADMIN_API_REFERENCE.md`
- Metrics: See `src/proxy/pool_metrics.rs`

---

## Version History

- **v1.0** (2025-11-09): Initial connection pool metrics dashboard
  - 12 panels covering all pool metrics
  - Per-host and global views
  - Alerting for errors and exhaustion
  - Heatmap for utilization tracking
  - Comprehensive statistics table
