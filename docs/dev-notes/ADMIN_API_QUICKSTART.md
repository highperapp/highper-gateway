# Admin API - Quick Start Guide

Get started with the Admin API in 5 minutes.

## Table of Contents

- [Enable the Admin API](#enable-the-admin-api)
- [Basic Configuration](#basic-configuration)
- [First API Calls](#first-api-calls)
- [Common Operations](#common-operations)
- [Authentication](#authentication)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

---

## Enable the Admin API

Add this to your `config.yaml`:

```yaml
admin:
  enabled: true
  bind: "127.0.0.1:9090"
  auth:
    enabled: true
    api_keys:
      - "your-secret-api-key"
  cors:
    enabled: true
```

**Security Note:** The Admin API is bound to `127.0.0.1` by default for security. For remote access, use SSH tunneling or a VPN.

---

## Basic Configuration

### Minimal Configuration

```yaml
admin:
  enabled: true
  bind: "127.0.0.1:9090"
  auth:
    enabled: false  # Disable for local testing only!
```

### Production Configuration

```yaml
admin:
  enabled: true
  bind: "127.0.0.1:9090"
  auth:
    enabled: true
    api_keys:
      - "prod-api-key-change-me"
    jwt_secret: "your-jwt-secret-256-bit"
    jwt_expiration: "24h"
  cors:
    enabled: true
    allowed_origins:
      - "https://admin.yourdomain.com"
```

---

## First API Calls

### 1. Health Check

Verify the Admin API is running:

```bash
curl http://localhost:9090/health
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-11-02T10:30:00Z"
}
```

### 2. List Backends

See all backend servers:

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/backends
```

**Response:**
```json
{
  "backends": [
    {
      "id": "api_backend_0",
      "upstream": "api_backend",
      "url": "http://backend1:8080",
      "health_status": "healthy",
      "enabled": true,
      "active_connections": 45
    }
  ],
  "total": 1
}
```

### 3. Get Statistics

View real-time proxy statistics:

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/stats
```

**Response:**
```json
{
  "timestamp": "2025-11-02T10:30:00Z",
  "requests": {
    "total": 152341,
    "per_second": 42.5,
    "avg_response_time_ms": 125.3,
    "success_rate": 99.2
  },
  "connections": {
    "active": 145,
    "total": 15234
  }
}
```

---

## Common Operations

### Backend Maintenance

**Take a backend offline:**

```bash
# 1. Drain connections (allow existing requests to complete)
curl -X POST -H "X-API-Key: your-secret-api-key" \
  -H "Content-Type: application/json" \
  -d '{"drain_timeout_seconds": 60}' \
  http://localhost:9090/api/backends/api_backend_0/drain

# 2. Disable the backend
curl -X POST -H "X-API-Key: your-secret-api-key" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Scheduled maintenance"}' \
  http://localhost:9090/api/backends/api_backend_0/disable
```

**Bring it back online:**

```bash
# 1. Enable the backend
curl -X POST -H "X-API-Key: your-secret-api-key" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Maintenance completed"}' \
  http://localhost:9090/api/backends/api_backend_0/enable

# 2. Verify health
curl -X POST -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/backends/api_backend_0/health-check
```

### Cache Management

**Clear cache after deployment:**

```bash
# Clear all cache
curl -X POST -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/cache/clear

# Clear specific routes
curl -X POST -H "X-API-Key: your-secret-api-key" \
  -H "Content-Type: application/json" \
  -d '{"pattern": "/api/v2/*"}' \
  http://localhost:9090/api/cache/clear
```

**Invalidate specific keys:**

```bash
curl -X POST -H "X-API-Key: your-secret-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "keys": ["GET:/api/users", "GET:/api/products/123"]
  }' \
  http://localhost:9090/api/cache/invalidate
```

### Configuration Reload

**Hot-reload configuration without restart:**

```bash
curl -X POST -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/config/reload
```

---

## Authentication

### API Key Authentication

Set the API key in the header:

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/backends
```

### JWT Authentication

Use a Bearer token:

```bash
curl -H "Authorization: Bearer your-jwt-token" \
  http://localhost:9090/api/backends
```

### Generate JWT Token (Example)

```bash
# Using a JWT library or online tool
# Payload example:
{
  "sub": "admin",
  "exp": 1735814400,
  "iat": 1735728000
}

# Sign with your jwt_secret from config
```

---

## Monitoring

### Prometheus Integration

The Admin API exposes metrics in Prometheus format at `/metrics`:

```bash
curl http://localhost:9090/metrics
```

**Add to Prometheus configuration:**

```yaml
scrape_configs:
  - job_name: 'reverse-proxy'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
```

### Health Monitoring

**Set up health check monitoring:**

```bash
# Check health every 10 seconds
while true; do
  curl -s http://localhost:9090/health | jq .
  sleep 10
done
```

**Monitor backend health:**

```bash
# Get health check history
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/metrics/health
```

### Real-Time Metrics

**Per-route metrics:**

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/metrics/routes | jq '.routes[] | {
    path: .path_pattern,
    requests: .total_requests,
    rps: .requests_per_second,
    latency_p95: .p95_latency_ms,
    success_rate: .success_rate
  }'
```

**Per-backend metrics:**

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/metrics/backends | jq '.backends[] | {
    id: .backend_id,
    url: .url,
    health: .health_status,
    requests: .total_requests,
    latency_avg: .avg_response_time_ms,
    success_rate: .success_rate
  }'
```

---

## Troubleshooting

### Admin API Not Responding

**Check if it's running:**

```bash
curl http://localhost:9090/health
```

**Check the bind address:**

```bash
netstat -tlnp | grep 9090
# or
ss -tlnp | grep 9090
```

**Check logs:**

```bash
# Look for Admin API startup messages
tail -f /var/log/highper-gateway/proxy.log | grep -i admin
```

### Authentication Issues

**401 Unauthorized:**

- Verify the API key is correct
- Check that auth is enabled in config
- Ensure the header is `X-API-Key` (case-sensitive)

**Test without authentication (if safe):**

```yaml
admin:
  auth:
    enabled: false  # Temporary for testing
```

### Backend Not Found

**404 Error on `/api/backends/{id}`:**

- Check the backend ID format: `upstream_name_index`
- List all backends to see available IDs:
  ```bash
  curl -H "X-API-Key: key" http://localhost:9090/api/backends
  ```

### Cache Operations Not Working

**Check cache configuration:**

```bash
curl -H "X-API-Key: key" http://localhost:9090/api/cache/stats
```

**Verify cache is enabled in main config:**

```yaml
cache:
  enabled: true
  local:
    enabled: true
  distributed:
    enabled: true
    redis_url: "redis://localhost:6379"
```

---

## Shell Alias for Quick Access

Add to your `.bashrc` or `.zshrc`:

```bash
# Admin API aliases
export ADMIN_API_KEY="your-secret-api-key"
export ADMIN_URL="http://localhost:9090"

alias admin-health='curl -s $ADMIN_URL/health | jq .'
alias admin-backends='curl -s -H "X-API-Key: $ADMIN_API_KEY" $ADMIN_URL/api/backends | jq .'
alias admin-stats='curl -s -H "X-API-Key: $ADMIN_API_KEY" $ADMIN_URL/api/stats | jq .'
alias admin-cache-stats='curl -s -H "X-API-Key: $ADMIN_API_KEY" $ADMIN_URL/api/cache/stats | jq .'
```

**Usage:**

```bash
admin-health
admin-backends
admin-stats
```

---

## Next Steps

1. **Explore the full API:** See [ADMIN_API_REFERENCE.md](ADMIN_API_REFERENCE.md)
2. **Run examples:** Execute `examples/admin_api_examples.sh`
3. **Set up monitoring:** Integrate with Prometheus/Grafana
4. **Automate operations:** Create scripts for common tasks

---

## Quick Reference Card

| Operation | Endpoint | Method |
|-----------|----------|--------|
| Health check | `/health` | GET |
| List backends | `/api/backends` | GET |
| Get backend | `/api/backends/{id}` | GET |
| Disable backend | `/api/backends/{id}/disable` | POST |
| Enable backend | `/api/backends/{id}/enable` | POST |
| Drain backend | `/api/backends/{id}/drain` | POST |
| Cache stats | `/api/cache/stats` | GET |
| Clear cache | `/api/cache/clear` | POST |
| Invalidate keys | `/api/cache/invalidate` | POST |
| Route metrics | `/api/metrics/routes` | GET |
| Backend metrics | `/api/metrics/backends` | GET |
| Prometheus | `/metrics` | GET |
| Statistics | `/api/stats` | GET |
| Config reload | `/api/config/reload` | POST |

---

**Need Help?**

- Full API Reference: [ADMIN_API_REFERENCE.md](ADMIN_API_REFERENCE.md)
- Examples Script: `examples/admin_api_examples.sh`
- Implementation Details: [ADMIN_API_COMPLETION_SUMMARY.md](ADMIN_API_COMPLETION_SUMMARY.md)
