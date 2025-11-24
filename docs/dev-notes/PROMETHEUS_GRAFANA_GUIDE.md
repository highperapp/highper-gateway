# Prometheus & Grafana Integration Guide
## Rust Reverse Proxy - Monitoring & Observability

**Date:** November 17, 2025
**Version:** 1.0
**Status:** ✅ **METRICS ALREADY IMPLEMENTED**

---

## Executive Summary

The Rust proxy **already has comprehensive Prometheus metrics** built-in:

✅ **20+ metrics implemented** (requests, latency, errors, connections, upstreams)
✅ **Histogram buckets configured** for latency tracking
✅ **Per-route metrics** with p50/p95/p99 latency
✅ **Per-backend metrics** with health status
✅ **Admin API** for metrics exposure (`/metrics` endpoint)
✅ **Production-ready** metrics exporter

This guide shows how to integrate with Prometheus and visualize with Grafana.

---

## Table of Contents

1. [Metrics Overview](#metrics-overview)
2. [Enabling Metrics](#enabling-metrics)
3. [Prometheus Configuration](#prometheus-configuration)
4. [Grafana Dashboard Setup](#grafana-dashboard-setup)
5. [Key Metrics Reference](#key-metrics-reference)
6. [Alerting Rules](#alerting-rules)
7. [Troubleshooting](#troubleshooting)

---

## Metrics Overview

### Built-in Metrics

The proxy exposes metrics via the **Admin API** on the `/metrics` endpoint.

**Metrics Categories:**

1. **HTTP Request Metrics** - Total requests, latency, status codes
2. **Connection Metrics** - Active connections, connection pool
3. **Upstream Metrics** - Backend health, request latency
4. **TLS Metrics** - Handshakes, errors
5. **Certificate Metrics** - Certificate count, ACME renewals
6. **Route Metrics** - Per-route latency and throughput
7. **Load Balancer Metrics** - Selection counts
8. **Circuit Breaker Metrics** - State, trip count (if implemented)

### Histogram Buckets

Latency metrics use these histogram buckets (in seconds):
```
[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
```

This allows calculating:
- p50 (median)
- p95
- p99
- p99.9

---

## Enabling Metrics

### Configuration

Add to your `config.toml`:

```toml
[observability]
# Enable metrics collection
metrics_enabled = true

# Admin API configuration
[admin]
# Bind admin API (includes /metrics endpoint)
bind = "127.0.0.1:9090"  # localhost only for security

# Or bind to all interfaces (less secure, use firewall)
# bind = "0.0.0.0:9090"
```

### Starting the Proxy

```bash
./highper-gateway start --config config.toml
```

### Verifying Metrics Endpoint

```bash
# Check metrics are available
curl http://localhost:9090/metrics

# Expected output (sample):
# http_requests_total{method="GET",status="200"} 1523
# http_request_duration_seconds_bucket{method="GET",status="200",le="0.001"} 1200
# http_request_duration_seconds_sum{method="GET",status="200"} 1.245
# ...
```

---

## Prometheus Configuration

### Installing Prometheus

```bash
# Download Prometheus
wget https://github.com/prometheus/prometheus/releases/download/v2.45.0/prometheus-2.45.0.linux-amd64.tar.gz
tar xvfz prometheus-*.tar.gz
cd prometheus-*
```

### Prometheus Configuration File

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s      # Scrape targets every 15 seconds
  evaluation_interval: 15s  # Evaluate rules every 15 seconds

  # Attach these labels to all time series
  external_labels:
    cluster: 'production'
    environment: 'prod'

# Alertmanager configuration
alerting:
  alertmanagers:
    - static_configs:
        - targets:
            - localhost:9093

# Load rules once and periodically evaluate them
rule_files:
  - "highper_gateway_alerts.yml"

# Scrape configurations
scrape_configs:
  # Highper Gateway metrics
  - job_name: 'highper-gateway'
    static_configs:
      - targets: ['localhost:9090']
        labels:
          instance: 'proxy-01'
          datacenter: 'us-east-1'

    # Scrape metrics from /metrics endpoint
    metrics_path: '/metrics'

    # Optional: Add basic auth if needed
    # basic_auth:
    #   username: 'prometheus'
    #   password: 'secret'

  # Prometheus self-monitoring
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
```

### Starting Prometheus

```bash
./prometheus --config.file=prometheus.yml
```

**Access Prometheus UI:**
- Open browser: `http://localhost:9090`
- Navigate to **Status > Targets** to verify proxy is being scraped
- Navigate to **Graph** to query metrics

---

## Grafana Dashboard Setup

### Installing Grafana

```bash
# Ubuntu/Debian
sudo apt-get install -y apt-transport-https software-properties-common
sudo wget -q -O /usr/share/keyrings/grafana.key https://apt.grafana.com/gpg.key
echo "deb [signed-by=/usr/share/keyrings/grafana.key] https://apt.grafana.com stable main" | sudo tee -a /etc/apt/sources.list.d/grafana.list
sudo apt-get update
sudo apt-get install grafana

# Start Grafana
sudo systemctl start grafana-server
sudo systemctl enable grafana-server
```

**Access Grafana:**
- URL: `http://localhost:3000`
- Default credentials: `admin` / `admin`

### Adding Prometheus Data Source

1. **Login to Grafana** (`http://localhost:3000`)
2. **Navigate** to Configuration > Data Sources
3. **Click** "Add data source"
4. **Select** "Prometheus"
5. **Configure:**
   - URL: `http://localhost:9090`
   - Access: Server (default)
6. **Click** "Save & Test"

### Importing Highper Gateway Dashboard

Save this as `highper-gateway-dashboard.json`:

```json
{
  "dashboard": {
    "title": "Rust Reverse Proxy - Overview",
    "tags": ["highper-gateway", "performance"],
    "timezone": "browser",
    "schemaVersion": 16,
    "version": 1,
    "refresh": "10s",

    "panels": [
      {
        "id": 1,
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "{{method}} {{status}}"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0}
      },
      {
        "id": 2,
        "title": "Request Latency (p50, p95, p99)",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p50"
          },
          {
            "expr": "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p95"
          },
          {
            "expr": "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p99"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0}
      },
      {
        "id": 3,
        "title": "Error Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_errors_total[5m])",
            "legendFormat": "{{error}}"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 8}
      },
      {
        "id": 4,
        "title": "Active Connections",
        "type": "graph",
        "targets": [
          {
            "expr": "http_connections_active",
            "legendFormat": "Active Connections"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 8}
      },
      {
        "id": 5,
        "title": "Upstream Request Latency",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.99, rate(upstream_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p99 {{upstream}}"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 16}
      },
      {
        "id": 6,
        "title": "Status Code Distribution",
        "type": "piechart",
        "targets": [
          {
            "expr": "sum by (status) (rate(http_requests_total[5m]))",
            "legendFormat": "{{status}}"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 16}
      }
    ]
  }
}
```

**Import Dashboard:**
1. In Grafana, click **+** > **Import**
2. Upload `highper-gateway-dashboard.json`
3. Select Prometheus data source
4. Click **Import**

---

## Key Metrics Reference

### HTTP Request Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `http_requests_total` | Counter | Total HTTP requests | `method`, `status` |
| `http_requests_errors_total` | Counter | Total HTTP errors | `method`, `error` |
| `http_request_duration_seconds` | Histogram | Request latency | `method`, `status` |
| `http_requests_bytes_total` | Counter | Total bytes received | - |
| `http_responses_bytes_total` | Counter | Total bytes sent | - |

**Example Queries:**

```promql
# Request rate (per second) over last 5 minutes
rate(http_requests_total[5m])

# P99 latency in milliseconds
histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) * 1000

# Error rate percentage
rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m]) * 100

# Total throughput (bytes/sec)
rate(http_responses_bytes_total[5m])
```

### Connection Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `http_connections_active` | Gauge | Current active connections |
| `http_connections_total` | Counter | Total connections (lifetime) |

**Example Queries:**

```promql
# Current active connections
http_connections_active

# Connection rate (connections/sec)
rate(http_connections_total[5m])
```

### Upstream Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `upstream_requests_total` | Counter | Total upstream requests | `upstream` |
| `upstream_requests_errors_total` | Counter | Total upstream errors | `upstream`, `error` |
| `upstream_request_duration_seconds` | Histogram | Upstream latency | `upstream` |

**Example Queries:**

```promql
# Upstream request rate per backend
rate(upstream_requests_total[5m])

# Upstream error rate
rate(upstream_requests_errors_total[5m])

# P95 upstream latency by backend
histogram_quantile(0.95, rate(upstream_request_duration_seconds_bucket[5m]))
```

### TLS Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `tls_handshakes_total` | Counter | Total TLS handshakes |
| `tls_handshakes_errors_total` | Counter | TLS handshake errors |

**Example Queries:**

```promql
# TLS handshake rate
rate(tls_handshakes_total[5m])

# TLS error rate
rate(tls_handshakes_errors_total[5m])
```

### Certificate Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `certificates_count` | Gauge | Number of loaded certificates |
| `acme_requests_total` | Counter | Total ACME certificate requests |
| `acme_renewals_total` | Counter | Total certificate renewals |

### Route Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `route_requests_total` | Counter | Requests per route | `route` |
| `route_request_duration_seconds` | Histogram | Latency per route | `route` |
| `route_requests_bytes_total` | Counter | Request bytes per route | `route` |
| `route_responses_bytes_total` | Counter | Response bytes per route | `route` |

**Example Queries:**

```promql
# Top 5 routes by request rate
topk(5, rate(route_requests_total[5m]))

# P99 latency per route
histogram_quantile(0.99, rate(route_request_duration_seconds_bucket[5m]))
```

---

## Alerting Rules

Create `highper_gateway_alerts.yml`:

```yaml
groups:
  - name: highper_gateway_alerts
    interval: 30s
    rules:
      # High error rate
      - alert: HighErrorRate
        expr: |
          (rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | humanizePercentage }} (threshold: 5%)"

      # High P99 latency
      - alert: HighP99Latency
        expr: |
          histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) > 0.5
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High P99 latency"
          description: "P99 latency is {{ $value | humanizeDuration }} (threshold: 500ms)"

      # Very high P99 latency (critical)
      - alert: CriticalP99Latency
        expr: |
          histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) > 2.0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Critical P99 latency"
          description: "P99 latency is {{ $value | humanizeDuration }} (threshold: 2s)"

      # High connection count
      - alert: HighConnectionCount
        expr: http_connections_active > 15000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High number of active connections"
          description: "{{ $value }} active connections (threshold: 15000)"

      # Upstream errors
      - alert: UpstreamErrors
        expr: rate(upstream_requests_errors_total[5m]) > 10
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High upstream error rate"
          description: "Upstream {{ $labels.upstream }} error rate: {{ $value }} errors/sec"

      # TLS errors
      - alert: TLSHandshakeErrors
        expr: rate(tls_handshakes_errors_total[5m]) > 5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High TLS handshake error rate"
          description: "TLS error rate: {{ $value }} errors/sec (threshold: 5/sec)"

      # Certificate expiration (placeholder - requires custom metric)
      # - alert: CertificateExpiringSoon
      #   expr: certificate_expiry_days < 7
      #   labels:
      #     severity: critical
      #   annotations:
      #     summary: "Certificate expiring soon"
      #     description: "Certificate {{ $labels.domain }} expires in {{ $value }} days"

      # Low request rate (possible outage)
      - alert: LowRequestRate
        expr: rate(http_requests_total[5m]) < 1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Unusually low request rate"
          description: "Request rate is {{ $value }} req/sec (may indicate outage)"

      # High 5xx error rate
      - alert: High5xxRate
        expr: |
          sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m])) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High 5xx error rate"
          description: "5xx error rate is {{ $value | humanizePercentage }} (threshold: 1%)"
```

### Testing Alerts

```bash
# Check alert rules syntax
promtool check rules highper_gateway_alerts.yml

# Query active alerts
curl http://localhost:9090/api/v1/alerts | jq
```

---

## Troubleshooting

### Metrics Not Showing Up

**Problem:** Prometheus shows no data for proxy metrics

**Solutions:**

1. **Check proxy is running:**
   ```bash
   curl http://localhost:9090/metrics
   ```

2. **Verify Prometheus scrape config:**
   ```bash
   # Check Prometheus targets
   curl http://localhost:9090/api/v1/targets | jq
   ```

3. **Check firewall rules:**
   ```bash
   # Ensure port 9090 is accessible
   sudo ufw allow 9090
   ```

4. **Check Prometheus logs:**
   ```bash
   journalctl -u prometheus -f
   ```

### High Cardinality Warnings

**Problem:** Prometheus warns about high cardinality metrics

**Cause:** Too many unique label combinations

**Solution:** Limit label values or use recording rules:

```yaml
# Recording rule example
groups:
  - name: highper_gateway_recording
    interval: 30s
    rules:
      - record: job:http_requests_total:rate5m
        expr: rate(http_requests_total[5m])
```

### Missing Latency Percentiles

**Problem:** P95/P99 queries return no data

**Cause:** Not enough data points or incorrect query

**Solution:**

```promql
# Correct query for P99
histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))

# Ensure at least 5 minutes of data collected
```

---

## Production Deployment Checklist

### Before Going Live

- [ ] **Enable metrics** in configuration
- [ ] **Secure admin API** (bind to localhost or add auth)
- [ ] **Install Prometheus** and configure scraping
- [ ] **Set up Grafana** dashboards
- [ ] **Configure alerting** rules
- [ ] **Test alert notifications** (Slack, PagerDuty, etc.)
- [ ] **Document runbooks** for common alerts
- [ ] **Set up log aggregation** (ELK, Loki)
- [ ] **Configure retention** policies (Prometheus data retention)

### Recommended Retention

```yaml
# prometheus.yml
global:
  # Prometheus data retention
  storage.tsdb.retention.time: 15d
  storage.tsdb.retention.size: 50GB
```

---

## Advanced Monitoring

### Service Level Objectives (SLOs)

Define SLOs for your proxy:

```yaml
# Example SLOs
- name: availability
  target: 99.9%  # 3 nines
  metric: |
    sum(rate(http_requests_total{status!~"5.."}[30d])) /
    sum(rate(http_requests_total[30d]))

- name: latency_p99
  target: 100ms
  metric: |
    histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))
```

### Distributed Tracing Integration

Add tracing for detailed request flow:

```toml
[observability.tracing]
enabled = true
endpoint = "http://jaeger:14268/api/traces"
sampling_rate = 0.1  # 10% of requests
```

---

## Conclusion

**Status:** ✅ **PROMETHEUS METRICS READY**

The Rust proxy has **comprehensive Prometheus metrics** built-in:
- 20+ metrics covering all critical aspects
- Histogram buckets for accurate latency percentiles
- Per-route and per-backend metrics
- Production-ready metrics exporter

**Use this guide to:**
1. Enable metrics in your configuration
2. Set up Prometheus scraping
3. Create Grafana dashboards
4. Configure alerting rules
5. Monitor proxy health and performance

**Next Steps:**
- Set up Prometheus (5 minutes)
- Import Grafana dashboard (2 minutes)
- Configure alerts (10 minutes)
- Test end-to-end monitoring (5 minutes)

**For more information:**
- Source code: `highper-gateway/src/observability/metrics.rs`
- Admin API: `highper-gateway/src/admin/metrics.rs`
- Week 2 Summary: `WEEK2_SECURITY_FEATURES_SUMMARY.md`

---

**Last Updated:** November 17, 2025
**Version:** 1.0
**Status:** Production Ready
