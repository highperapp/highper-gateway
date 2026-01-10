# Prometheus + Grafana Observability Stack

Complete monitoring solution for Highper Gateway load testing.

## Architecture

```
┌─────────────────┐
│  Load Testing   │
│   Proxy Server  │
├─────────────────┤
│                 │
│  Highper        │◄──┐
│  Gateway        │   │
│  :8080          │   │
│                 │   │
├─────────────────┤   │
│                 │   │  Scrapes metrics
│  Prometheus     │───┤  every 5-15s
│  :9091          │   │
│                 │   │
├─────────────────┤   │
│                 │   │
│  Grafana        │   │
│  :3000          │───┘
│                 │
├─────────────────┤
│  Node Exporter  │
│  :9100          │
│  (System)       │
├─────────────────┤
│  cAdvisor       │
│  :8080          │
│  (Containers)   │
└─────────────────┘
        ▲
        │ Scrapes
        │
┌───────┴────────┐
│   Backends     │
│ BACKEND_1:9100 │
│ BACKEND_2:9100 │
│ BACKEND_3:9100 │
└────────────────┘
```

## Quick Start

### 1. Deploy Observability Stack on Proxy Server

```bash
# SSH to proxy server
ssh root@<proxy-ip>

# Copy observability configs
scp -r configs/observability root@<proxy-ip>:/opt/highper/

# Start the stack
cd /opt/highper/observability
docker-compose up -d

# Verify services
docker-compose ps
docker-compose logs -f
```

### 2. Access Dashboards

- **Grafana**: http://<proxy-ip>:3000
  - Username: `admin`
  - Password: `highper2025`
- **Prometheus**: http://<proxy-ip>:9091
- **Node Exporter**: http://<proxy-ip>:9100/metrics
- **cAdvisor**: http://<proxy-ip>:8080

### 3. Configure Backend IPs

Before starting, update backend IPs in `prometheus/prometheus.yml`:

```bash
# Replace BACKEND_1, BACKEND_2, BACKEND_3 with actual IPs
sed -i "s/BACKEND_1/$BACKEND_IP_1/g" prometheus/prometheus.yml
sed -i "s/BACKEND_2/$BACKEND_IP_2/g" prometheus/prometheus.yml
sed -i "s/BACKEND_3/$BACKEND_IP_3/g" prometheus/prometheus.yml
sed -i "s/PROXY_IP/localhost/g" prometheus/prometheus.yml

# Reload Prometheus configuration
curl -X POST http://localhost:9091/-/reload
```

## Components

### Prometheus

**Purpose**: Time-series metrics collection and storage

**Configuration**:
- Scrape interval: 5-15s depending on target
- Retention: 7 days
- Storage: Local volume (`prometheus-data`)
- Port: 9091 (offset to avoid conflict with gateway :9090)

**Metrics Sources**:
1. **Highper Gateway** (:9090)
   - Request rate, latency (P50/P95/P99/P99.9)
   - Connection pool stats
   - Backend health
   - Buffer pool utilization
   - Error rates

2. **Rust Backends** (:9100)
   - Request count
   - Health status
   - Response times

3. **Node Exporter** (:9100)
   - CPU, memory, disk, network
   - File descriptors, connections
   - System load

4. **cAdvisor** (:8080)
   - Container CPU/memory
   - Network I/O
   - Filesystem usage

### Grafana

**Purpose**: Visualization and alerting

**Configuration**:
- Port: 3000
- Admin credentials: `admin` / `highper2025`
- Data source: Prometheus (auto-provisioned)
- Dashboards: Auto-loaded from `/var/lib/grafana/dashboards`

**Pre-built Dashboards** (to be created):
1. **Load Test Overview** - High-level summary
2. **Gateway Performance** - Detailed gateway metrics
3. **Backend Health** - Backend server monitoring
4. **Latency Analysis** - Histogram and percentiles
5. **Resource Utilization** - CPU, memory, network

### Node Exporter

**Purpose**: System-level metrics collection

**Metrics**:
- CPU usage per core
- Memory usage and swap
- Disk I/O and space
- Network traffic and errors
- File descriptors and processes

### cAdvisor

**Purpose**: Container metrics collection

**Metrics**:
- Container CPU/memory limits and usage
- Network I/O per container
- Filesystem usage per container

## Grafana Dashboards

### Dashboard 1: Load Test Overview

**Panels**:
- Current RPS (single stat)
- Target vs Actual RPS (gauge)
- P99 Latency (graph + single stat)
- Error Rate (graph)
- Active Connections (graph)
- Backend Health Status (stat)

### Dashboard 2: Gateway Performance

**Panels**:
- Request Rate by Status Code (graph)
- Latency Histogram (heatmap)
- P50/P95/P99/P99.9 Latencies (graph)
- Connection Pool Utilization (graph)
- Buffer Pool Stats (graph)
- Upstream Response Times (graph)

### Dashboard 3: Backend Health

**Panels**:
- Backend Availability (table)
- Backend Request Distribution (pie chart)
- Backend Response Times (graph)
- Backend Error Rates (graph)
- Backend Connections (graph)

### Dashboard 4: Latency Analysis

**Panels**:
- Latency Distribution (histogram)
- Percentile Comparison (graph)
- Latency by Backend (graph)
- Request Duration Buckets (heatmap)
- Slow Request Traces (table)

### Dashboard 5: Resource Utilization

**Panels**:
- CPU Usage (graph)
- Memory Usage (graph)
- Network Throughput (graph)
- Disk I/O (graph)
- File Descriptors (graph)
- TCP Connection States (graph)

## Dashboard Export

### Manual Export (PNG)

```bash
# Install Grafana Image Renderer plugin
docker exec -it highper-grafana grafana-cli plugins install grafana-image-renderer

# Restart Grafana
docker-compose restart grafana

# Export dashboard to PNG (via Grafana API)
curl -H "Authorization: Bearer <api-key>" \
  "http://localhost:3000/render/d/<dashboard-uid>?orgId=1&width=1920&height=1080" \
  -o dashboard.png
```

### Automated Export Script

See `scripts/export-dashboards.sh` for automated export of all dashboards.

## Metrics to Prometheus Transformer

For vegeta/wrk2 results that don't natively export to Prometheus:

```bash
# Transform vegeta JSON to Prometheus format
./scripts/vegeta-to-prometheus.sh results/vegeta-results.json > metrics.prom

# Push to Prometheus (Pushgateway required)
curl -X POST http://localhost:9091/metrics/job/load-test < metrics.prom
```

## Alert Rules

Prometheus alert rules are defined in `prometheus/rules/highper-alerts.yml`:

**Performance Alerts**:
- High error rate (>1%)
- High P99 latency (>10ms)
- Low throughput (<400K RPS)
- Connection pool exhaustion (>90%)

**System Alerts**:
- High CPU usage (>85%)
- High memory usage (>85%)
- High network errors
- File descriptor exhaustion (>80%)

**Backend Alerts**:
- Backend down
- Backend high latency

Alerts can be routed to Alertmanager for notifications (Slack, PagerDuty, email, etc.).

## Data Retention

- **Prometheus**: 7 days (configurable via `--storage.tsdb.retention.time`)
- **Grafana**: Persistent dashboards and settings

For long-term storage, configure Prometheus remote write to external TSDB (Thanos, Cortex, M3DB, etc.).

## Resource Requirements

| Component | CPU | Memory | Disk |
|-----------|-----|--------|------|
| Prometheus | 2 cores | 4GB | 50GB (7 days) |
| Grafana | 1 core | 2GB | 5GB |
| Node Exporter | 0.1 core | 100MB | - |
| cAdvisor | 0.2 core | 200MB | - |
| **Total** | **3.3 cores** | **6.3GB** | **55GB** |

## Troubleshooting

### Prometheus Not Scraping Targets

```bash
# Check Prometheus targets
curl http://localhost:9091/api/v1/targets

# Check Prometheus logs
docker-compose logs prometheus

# Verify target is reachable
curl http://BACKEND_1:9100/metrics
```

### Grafana Dashboard Not Loading

```bash
# Check Grafana logs
docker-compose logs grafana

# Verify Prometheus datasource
curl -u admin:highper2025 http://localhost:3000/api/datasources

# Test Prometheus query
curl -u admin:highper2025 \
  "http://localhost:3000/api/datasources/proxy/1/api/v1/query?query=up"
```

### High Memory Usage

```bash
# Check Prometheus memory
docker stats highper-prometheus

# Reduce retention time
# Edit docker-compose.yml: --storage.tsdb.retention.time=3d

# Restart Prometheus
docker-compose restart prometheus
```

## Integration with Load Testing

The deployment script (`scripts/loadtest/vultr/deploy.sh`) will:

1. Copy observability configs to proxy server
2. Replace BACKEND_* placeholders with actual IPs
3. Start Docker Compose stack
4. Wait for Prometheus and Grafana to be healthy
5. Run load tests
6. Export Grafana dashboards to PNG
7. Download all metrics and dashboard images to WSL
8. Cleanup infrastructure

## Security Considerations

**Production Deployment**:
- Change default Grafana password
- Enable HTTPS for Grafana
- Configure authentication (OAuth, LDAP, etc.)
- Restrict Prometheus scraping to trusted networks
- Use network policies to isolate monitoring stack
- Enable Prometheus admin API authentication
- Rotate API tokens regularly

**Load Testing**:
- Default credentials are acceptable (ephemeral infrastructure)
- No sensitive data in metrics
- Firewall rules prevent external access to monitoring ports

## Backup and Recovery

```bash
# Backup Prometheus data
docker run --rm -v prometheus-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/prometheus-backup.tar.gz /data

# Backup Grafana data
docker run --rm -v grafana-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/grafana-backup.tar.gz /data

# Restore Prometheus data
docker run --rm -v prometheus-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/prometheus-backup.tar.gz -C /

# Restore Grafana data
docker run --rm -v grafana-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/grafana-backup.tar.gz -C /
```

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Node Exporter](https://github.com/prometheus/node_exporter)
- [cAdvisor](https://github.com/google/cadvisor)
- [PromQL Query Language](https://prometheus.io/docs/prometheus/latest/querying/basics/)
