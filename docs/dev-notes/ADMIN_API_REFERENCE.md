# Admin API Reference

Comprehensive API reference for the embedded Admin API.

## Table of Contents

- [Overview](#overview)
- [Authentication](#authentication)
- [Health & Status](#health--status)
- [Configuration Management](#configuration-management)
- [Backend Control](#backend-control)
- [Cache Management](#cache-management)
- [Enhanced Metrics](#enhanced-metrics)
- [Statistics](#statistics)
- [Error Responses](#error-responses)

---

## Overview

The Admin API provides REST endpoints for managing and monitoring the reverse proxy at runtime.

**Base URL:** `http://localhost:9090` (default admin port)

**Content-Type:** `application/json`

**Authentication:** API Key or JWT Bearer Token (configurable)

---

## Authentication

### API Key Authentication

Include the API key in the `X-API-Key` header:

```bash
curl -H "X-API-Key: your-secret-api-key" \
  http://localhost:9090/api/backends
```

### JWT Authentication

Include the JWT token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer your-jwt-token" \
  http://localhost:9090/api/backends
```

### Configuration

```yaml
admin_api:
  enabled: true
  bind: "127.0.0.1:9090"
  auth:
    enabled: true
    api_keys:
      - "your-secret-api-key"
    jwt_secret: "your-jwt-secret"
```

---

## Health & Status

### Health Check

Check if the Admin API is healthy.

**Endpoint:** `GET /health` or `GET /api/health`

**Authentication:** Not required

**Example:**

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

---

### Readiness Check

Check if the proxy is ready to serve traffic.

**Endpoint:** `GET /ready` or `GET /api/ready`

**Authentication:** Not required

**Example:**

```bash
curl http://localhost:9090/ready
```

**Response:**

```json
{
  "status": "ready",
  "timestamp": "2025-11-02T10:30:00Z"
}
```

---

## Configuration Management

### View Current Configuration

Get the current proxy configuration.

**Endpoint:** `GET /api/config`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" http://localhost:9090/api/config
```

**Response:**

```json
{
  "upstreams": [
    {
      "name": "api_backend",
      "servers": [
        {
          "url": "http://backend1:8080",
          "weight": 100,
          "max_conns": 1000
        }
      ],
      "load_balancing": {
        "algorithm": "round_robin"
      }
    }
  ],
  "routes": [...],
  "tls": {...}
}
```

---

### Reload Configuration

Hot-reload configuration from the config file without restarting.

**Endpoint:** `POST /api/config/reload`

**Authentication:** Required

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  http://localhost:9090/api/config/reload
```

**Response:**

```json
{
  "success": true,
  "message": "Configuration reloaded successfully",
  "timestamp": "2025-11-02T10:30:00Z"
}
```

---

## Backend Control

### List All Backends

Get a list of all backend servers across all upstreams.

**Endpoint:** `GET /api/backends`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" http://localhost:9090/api/backends
```

**Response:**

```json
{
  "backends": [
    {
      "id": "api_backend_0",
      "upstream": "api_backend",
      "url": "http://backend1:8080",
      "weight": 100,
      "max_connections": 1000,
      "active_connections": 45,
      "health_status": "healthy",
      "enabled": true,
      "draining": false,
      "location": "37.7749, -122.4194",
      "region": "us-west-1"
    },
    {
      "id": "api_backend_1",
      "upstream": "api_backend",
      "url": "http://backend2:8080",
      "weight": 50,
      "max_connections": 500,
      "active_connections": 12,
      "health_status": "healthy",
      "enabled": true,
      "draining": false,
      "region": "us-west-2"
    }
  ],
  "total": 2
}
```

---

### Get Backend Details

Get details for a specific backend.

**Endpoint:** `GET /api/backends/{id}`

**Authentication:** Required

**Parameters:**
- `id` (path): Backend ID in format `upstream_index` (e.g., `api_backend_0`)

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/backends/api_backend_0
```

**Response:**

```json
{
  "id": "api_backend_0",
  "upstream": "api_backend",
  "url": "http://backend1:8080",
  "weight": 100,
  "max_connections": 1000,
  "active_connections": 45,
  "health_status": "healthy",
  "enabled": true,
  "draining": false,
  "location": "37.7749, -122.4194",
  "region": "us-west-1"
}
```

---

### Enable Backend

Enable a previously disabled backend.

**Endpoint:** `POST /api/backends/{id}/enable`

**Authentication:** Required

**Request Body (optional):**

```json
{
  "reason": "Maintenance completed"
}
```

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"reason":"Maintenance completed"}' \
  http://localhost:9090/api/backends/api_backend_0/enable
```

**Response:**

```json
{
  "success": true,
  "message": "Backend 'api_backend_0' enabled successfully",
  "backend": null
}
```

---

### Disable Backend

Disable a backend to stop routing traffic to it.

**Endpoint:** `POST /api/backends/{id}/disable`

**Authentication:** Required

**Request Body (optional):**

```json
{
  "reason": "Server maintenance",
  "drain_timeout_seconds": 30
}
```

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"reason":"Server maintenance","drain_timeout_seconds":30}' \
  http://localhost:9090/api/backends/api_backend_0/disable
```

**Response:**

```json
{
  "success": true,
  "message": "Backend 'api_backend_0' disabled successfully. Reason: Server maintenance. Drain timeout: 30s",
  "backend": null
}
```

---

### Drain Backend

Gracefully drain connections from a backend.

**Endpoint:** `POST /api/backends/{id}/drain`

**Authentication:** Required

**Request Body (optional):**

```json
{
  "drain_timeout_seconds": 60
}
```

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"drain_timeout_seconds":60}' \
  http://localhost:9090/api/backends/api_backend_0/drain
```

**Response:**

```json
{
  "success": true,
  "message": "Backend 'api_backend_0' is draining. Timeout: 60s",
  "backend": null
}
```

---

### Force Health Check

Trigger an immediate health check on a backend.

**Endpoint:** `POST /api/backends/{id}/health-check`

**Authentication:** Required

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  http://localhost:9090/api/backends/api_backend_0/health-check
```

**Response:**

```json
{
  "success": true,
  "message": "Health check triggered for backend 'api_backend_0'",
  "health_status": "unknown"
}
```

---

## Cache Management

### Get Cache Statistics

Retrieve statistics about local and distributed caches.

**Endpoint:** `GET /api/cache/stats`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/cache/stats
```

**Response:**

```json
{
  "local": {
    "entries": 1523,
    "enabled": true
  },
  "distributed": {
    "entries": 45231,
    "connected": true,
    "enabled": true
  },
  "total_entries": 46754
}
```

---

### List Cache Keys

List all cache keys, optionally filtered by pattern.

**Endpoint:** `GET /api/cache/keys`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/cache/keys
```

**Response:**

```json
{
  "keys": [
    "GET:/api/users",
    "GET:/api/products/123",
    "POST:/api/search"
  ],
  "total": 3,
  "pattern": ""
}
```

---

### Clear Cache

Clear all cache entries or entries matching a pattern.

**Endpoint:** `POST /api/cache/clear`

**Authentication:** Required

**Request Body (optional):**

```json
{
  "pattern": "/api/users/*",
  "clear_local": true,
  "clear_distributed": true
}
```

**Example 1: Clear all cache**

```bash
curl -X POST -H "X-API-Key: secret" \
  http://localhost:9090/api/cache/clear
```

**Example 2: Clear with pattern**

```bash
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"pattern":"/api/users/*","clear_local":true}' \
  http://localhost:9090/api/cache/clear
```

**Response:**

```json
{
  "success": true,
  "message": "Cache cleared successfully matching pattern '/api/users/*' from: local",
  "keys_affected": 0
}
```

---

### Invalidate Cache Keys

Invalidate specific cache keys.

**Endpoint:** `POST /api/cache/invalidate`

**Authentication:** Required

**Request Body:**

```json
{
  "keys": [
    "GET:/api/users",
    "GET:/api/products/123"
  ],
  "invalidate_local": true,
  "invalidate_distributed": true
}
```

**Example:**

```bash
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"keys":["GET:/api/users","GET:/api/products/123"]}' \
  http://localhost:9090/api/cache/invalidate
```

**Response:**

```json
{
  "success": true,
  "message": "Invalidated 2 cache keys from: local, distributed",
  "keys_affected": 2
}
```

---

## Enhanced Metrics

### Per-Route Metrics

Get detailed metrics for each route.

**Endpoint:** `GET /api/metrics/routes`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/metrics/routes
```

**Response:**

```json
{
  "routes": [
    {
      "route_id": "api_route",
      "path_pattern": "/api/*",
      "methods": ["GET", "POST"],
      "total_requests": 152341,
      "requests_per_second": 42.5,
      "avg_response_time_ms": 125.3,
      "p50_latency_ms": 98.2,
      "p95_latency_ms": 256.7,
      "p99_latency_ms": 512.1,
      "success_rate": 99.2,
      "error_rate": 0.8,
      "status_codes": {
        "status_2xx": 151120,
        "status_3xx": 0,
        "status_4xx": 1021,
        "status_5xx": 200
      },
      "upstream": "api_backend"
    }
  ],
  "total_routes": 1
}
```

---

### Per-Backend Metrics

Get detailed metrics for each backend server.

**Endpoint:** `GET /api/metrics/backends`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/metrics/backends
```

**Response:**

```json
{
  "backends": [
    {
      "backend_id": "api_backend_0",
      "upstream": "api_backend",
      "url": "http://backend1:8080",
      "health_status": "healthy",
      "active_connections": 45,
      "total_requests": 98234,
      "requests_per_second": 27.3,
      "avg_response_time_ms": 118.5,
      "p50_latency_ms": 95.1,
      "p95_latency_ms": 243.2,
      "p99_latency_ms": 498.7,
      "success_rate": 99.5,
      "error_rate": 0.5,
      "status_codes": {
        "status_2xx": 97742,
        "status_3xx": 0,
        "status_4xx": 412,
        "status_5xx": 80
      },
      "bytes_sent": 524288000,
      "bytes_received": 12582912
    }
  ],
  "total_backends": 1
}
```

---

### Health Check History

Get historical health check results.

**Endpoint:** `GET /api/metrics/health`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/metrics/health
```

**Response:**

```json
{
  "history": [
    {
      "backend_id": "api_backend_0",
      "upstream": "api_backend",
      "url": "http://backend1:8080",
      "timestamp": "2025-11-02T10:30:00Z",
      "status": "healthy",
      "response_time_ms": 12.5,
      "status_code": 200,
      "error": null
    },
    {
      "backend_id": "api_backend_0",
      "upstream": "api_backend",
      "url": "http://backend1:8080",
      "timestamp": "2025-11-02T10:29:50Z",
      "status": "unhealthy",
      "response_time_ms": null,
      "status_code": null,
      "error": "Connection timeout"
    }
  ],
  "total": 2,
  "limit": 100
}
```

---

### Prometheus Metrics Export

Export metrics in Prometheus format for scraping.

**Endpoint:** `GET /metrics`

**Authentication:** Not required (typically)

**Example:**

```bash
curl http://localhost:9090/metrics
```

**Response:**

```
# HELP proxy_requests_total Total number of requests
# TYPE proxy_requests_total counter
proxy_requests_total 152341

# HELP proxy_requests_duration_seconds Request duration
# TYPE proxy_requests_duration_seconds histogram
proxy_requests_duration_seconds_bucket{le="0.005"} 12340
proxy_requests_duration_seconds_bucket{le="0.01"} 45231
proxy_requests_duration_seconds_bucket{le="0.025"} 98234
proxy_requests_duration_seconds_bucket{le="0.05"} 125341
proxy_requests_duration_seconds_bucket{le="0.1"} 145231
proxy_requests_duration_seconds_bucket{le="0.25"} 150123
proxy_requests_duration_seconds_bucket{le="0.5"} 151892
proxy_requests_duration_seconds_bucket{le="1"} 152234
proxy_requests_duration_seconds_bucket{le="+Inf"} 152341
proxy_requests_duration_seconds_sum 19125.43
proxy_requests_duration_seconds_count 152341

# HELP proxy_backend_up Backend health status (1=up, 0=down)
# TYPE proxy_backend_up gauge
proxy_backend_up{backend="api_backend_0",upstream="api_backend"} 1

# HELP proxy_backend_connections_active Active backend connections
# TYPE proxy_backend_connections_active gauge
proxy_backend_connections_active{backend="api_backend_0",upstream="api_backend"} 45
```

---

## Statistics

### Real-Time Statistics

Get real-time statistics snapshot.

**Endpoint:** `GET /api/stats`

**Authentication:** Required

**Example:**

```bash
curl -H "X-API-Key: secret" \
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
    "p50_latency_ms": 98.2,
    "p95_latency_ms": 256.7,
    "p99_latency_ms": 512.1,
    "success_rate": 99.2,
    "error_rate": 0.8
  },
  "connections": {
    "active": 145,
    "total": 15234,
    "websocket": 23,
    "grpc": 12,
    "http1": 67,
    "http2": 43
  },
  "backends": [...],
  "cache": {
    "total_entries": 46754,
    "hits": 98234,
    "misses": 12341,
    "hit_rate": 88.8,
    "memory_usage": 524288000
  },
  "system": {
    "cpu_usage": 23.5,
    "memory_usage": 2147483648,
    "uptime_seconds": 86400,
    "workers": 8
  }
}
```

---

## Error Responses

All error responses follow this format:

```json
{
  "error": "Error type",
  "message": "Detailed error message",
  "status": 400
}
```

### Common Error Codes

| Status Code | Error Type | Description |
|-------------|-----------|-------------|
| 400 | Bad Request | Invalid request parameters or body |
| 401 | Unauthorized | Missing or invalid authentication |
| 404 | Not Found | Endpoint or resource not found |
| 500 | Internal Server Error | Server-side error |

**Example Error:**

```json
{
  "error": "Invalid backend ID format",
  "message": "Expected: upstream_index",
  "status": 400
}
```

---

## Complete Example: Backend Maintenance Workflow

Here's a complete workflow for taking a backend offline for maintenance:

```bash
# 1. Check backend status
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/backends/api_backend_0

# 2. Drain connections (allow existing requests to complete)
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"drain_timeout_seconds":60}' \
  http://localhost:9090/api/backends/api_backend_0/drain

# 3. Disable backend
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"reason":"Scheduled maintenance"}' \
  http://localhost:9090/api/backends/api_backend_0/disable

# Perform maintenance...

# 4. Enable backend
curl -X POST -H "X-API-Key: secret" \
  -H "Content-Type: application/json" \
  -d '{"reason":"Maintenance completed"}' \
  http://localhost:9090/api/backends/api_backend_0/enable

# 5. Force health check
curl -X POST -H "X-API-Key: secret" \
  http://localhost:9090/api/backends/api_backend_0/health-check

# 6. Verify backend is healthy
curl -H "X-API-Key: secret" \
  http://localhost:9090/api/backends/api_backend_0
```

---

## Next Steps

- See [ADMIN_API_STATUS.md](ADMIN_API_STATUS.md) for implementation status
- See proxy configuration documentation for setting up the Admin API
- Integration with actual cache, load balancer, and health checker components is pending

**Note:** Some endpoints currently return stub data and require integration with the runtime proxy components for full functionality.
