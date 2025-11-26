# Admin API Documentation

The Rust Proxy Admin API provides REST endpoints for runtime management, monitoring, and configuration of the reverse proxy.

## Table of Contents

- [Quick Start](#quick-start)
- [Authentication](#authentication)
- [Endpoints](#endpoints)
- [Configuration](#configuration)
- [Examples](#examples)
- [Error Responses](#error-responses)

## Quick Start

### Enable the Admin API

Add to your `config.yaml`:

```yaml
admin:
  enabled: true
  bind: "127.0.0.1:9000"  # Bind address with port
  auth_enabled: true       # Enable authentication
  api_keys:
    - "your-secret-api-key-here"
  cors_enabled: true
  cors_origins:
    - "http://localhost:3000"
```

### Test the API

```bash
# Health check (no auth required if auth_enabled: false)
curl http://127.0.0.1:9000/health

# With API key
curl -H "X-API-Key: your-secret-api-key-here" \
  http://127.0.0.1:9000/api/config
```

## Authentication

The Admin API supports two authentication methods:

### 1. API Key Authentication

Pass API key in the `X-API-Key` header:

```bash
curl -H "X-API-Key: your-secret-api-key-here" \
  http://127.0.0.1:9000/api/routes
```

**Configuration:**
```yaml
admin:
  auth_enabled: true
  api_keys:
    - "key-for-monitoring"
    - "key-for-admin-ops"
    - "key-for-dashboard"
```

### 2. JWT Token Authentication

Pass JWT token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer <jwt-token>" \
  http://127.0.0.1:9000/api/config
```

**Token Requirements:**
- Algorithm: HS256
- Required claims:
  - `sub` - Subject (user/service identifier)
  - `iat` - Issued at (Unix timestamp)
  - `exp` - Expiration time (Unix timestamp)

**Configuration:**
```yaml
admin:
  auth_enabled: true
  jwt_secret: "your-jwt-secret-change-in-production"
  jwt_expiration: "24h"
```

**Generating JWT Tokens (Python example):**
```python
import jwt
import time

secret = "your-jwt-secret-change-in-production"
payload = {
    "sub": "admin-user",
    "iat": int(time.time()),
    "exp": int(time.time()) + 86400  # 24 hours
}

token = jwt.encode(payload, secret, algorithm="HS256")
print(f"JWT Token: {token}")
```

### Authentication Behavior

- Both API key and JWT authentication work independently
- If auth is disabled, no authentication is required
- Invalid/expired credentials return `401 Unauthorized`

## Endpoints

### Health & Readiness

#### GET /health

Returns basic health status.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-10-31T02:17:45.194821866+00:00"
}
```

**Status Codes:**
- `200 OK` - Service is healthy

---

#### GET /ready

Returns readiness status with metrics.

**Response:**
```json
{
  "ready": true,
  "routes_count": 5,
  "upstreams_count": 3,
  "timestamp": "2025-10-31T02:17:50.870101276+00:00"
}
```

**Status Codes:**
- `200 OK` - Service is ready
- `503 Service Unavailable` - Service not ready (no routes/upstreams configured)

---

### Configuration Management

#### GET /api/config

Get current proxy configuration summary.

**Response:**
```json
{
  "server": {
    "bind": ["0.0.0.0:8080"],
    "workers": "auto"
  },
  "routes_count": 5,
  "upstreams_count": 3,
  "tls_enabled": true,
  "observability": {
    "metrics_enabled": true
  },
  "timestamp": "2025-10-31T02:18:21.399554123+00:00"
}
```

**Status Codes:**
- `200 OK` - Configuration retrieved

---

#### POST /api/config/reload

Trigger configuration hot reload.

**Requirements:**
- Hot reload must be enabled (start with `--hot-reload` flag)
- Admin API must not be in read-only mode

**Response (Success):**
```json
{
  "status": "ok",
  "message": "Configuration reload triggered successfully",
  "timestamp": "2025-10-31T02:26:01.497264164+00:00"
}
```

**Response (Read-Only Mode):**
```json
{
  "error": "Admin API is in read-only mode"
}
```

**Response (Hot Reload Not Available):**
```json
{
  "error": "Hot reload not enabled",
  "message": "Configuration hot reload is not available. Restart the server to apply configuration changes."
}
```

**Status Codes:**
- `200 OK` - Reload triggered
- `403 Forbidden` - Read-only mode
- `503 Service Unavailable` - Hot reload not enabled

**Example:**
```bash
curl -X POST \
  -H "X-API-Key: your-api-key" \
  http://127.0.0.1:9000/api/config/reload
```

---

### Routes

#### GET /api/routes

List all configured routes.

**Response:**
```json
{
  "routes": [
    {
      "name": "api_v1",
      "upstream": "backend_api"
    },
    {
      "name": "static_content",
      "upstream": "cdn_servers"
    }
  ]
}
```

**Status Codes:**
- `200 OK` - Routes retrieved

---

### Upstreams

#### GET /api/upstreams

List all configured upstreams with health status.

**Response:**
```json
{
  "upstreams": [
    {
      "name": "backend_api",
      "servers_count": 3,
      "health_check_enabled": true
    },
    {
      "name": "cdn_servers",
      "servers_count": 5,
      "health_check_enabled": false
    }
  ]
}
```

**Status Codes:**
- `200 OK` - Upstreams retrieved

---

### Statistics

#### GET /api/stats

Get runtime statistics.

**Response:**
```json
{
  "routes": 5,
  "upstreams": 3,
  "timestamp": "2025-10-31T02:18:59.308899983+00:00"
}
```

**Status Codes:**
- `200 OK` - Statistics retrieved

---

## Configuration

### Full Admin API Configuration

```yaml
admin:
  # Enable/disable Admin API
  enabled: true

  # Bind address with port
  # Use 127.0.0.1 for localhost only (recommended)
  # Use 0.0.0.0 to allow external access
  bind: "127.0.0.1:9000"

  # Authentication
  auth_enabled: true

  # API Keys (can have multiple for different clients)
  api_keys:
    - "key-for-monitoring-system"
    - "key-for-dashboard"
    - "key-for-admin-scripts"

  # JWT Authentication
  jwt_secret: "your-jwt-secret-here-change-in-production"
  jwt_expiration: "24h"

  # CORS (for web dashboards)
  cors_enabled: true
  cors_origins:
    - "http://localhost:3000"
    - "https://admin.example.com"

  # Read-only mode (only allow GET requests)
  read_only: false
```

### Security Best Practices

1. **Use Strong API Keys**
   ```bash
   # Generate secure random API keys
   openssl rand -hex 32
   ```

2. **Use Strong JWT Secrets**
   ```bash
   # Generate secure JWT secret
   openssl rand -base64 64
   ```

3. **Bind to Localhost in Production**
   ```yaml
   admin:
     bind: "127.0.0.1:9000"  # Not accessible from outside
   ```

4. **Use TLS for External Access**
   - Put Admin API behind reverse proxy with TLS
   - Or use SSH tunneling: `ssh -L 9000:localhost:9000 server`

5. **Enable Read-Only Mode for Monitoring**
   ```yaml
   admin:
     read_only: true  # Monitoring can read but not modify
   ```

6. **Limit CORS Origins**
   ```yaml
   admin:
     cors_origins:
       - "https://admin.example.com"  # Specific domain, not "*"
   ```

## Examples

### Monitoring Script

```bash
#!/bin/bash

API_KEY="your-api-key-here"
ADMIN_URL="http://127.0.0.1:9000"

# Check health
health=$(curl -s -H "X-API-Key: $API_KEY" "$ADMIN_URL/health")
echo "Health: $health"

# Check readiness
ready=$(curl -s -H "X-API-Key: $API_KEY" "$ADMIN_URL/ready")
echo "Ready: $ready"

# Get stats
stats=$(curl -s -H "X-API-Key: $API_KEY" "$ADMIN_URL/api/stats")
echo "Stats: $stats"
```

### Configuration Reload

```bash
#!/bin/bash

# Trigger config reload after updating config file
curl -X POST \
  -H "X-API-Key: your-api-key" \
  http://127.0.0.1:9000/api/config/reload

# Check if reload was successful
if [ $? -eq 0 ]; then
  echo "Configuration reloaded successfully"
else
  echo "Configuration reload failed"
  exit 1
fi
```

### Python Client

```python
import requests

class ProxyAdminClient:
    def __init__(self, base_url, api_key):
        self.base_url = base_url
        self.headers = {"X-API-Key": api_key}

    def health_check(self):
        response = requests.get(
            f"{self.base_url}/health",
            headers=self.headers
        )
        return response.json()

    def get_routes(self):
        response = requests.get(
            f"{self.base_url}/api/routes",
            headers=self.headers
        )
        return response.json()

    def reload_config(self):
        response = requests.post(
            f"{self.base_url}/api/config/reload",
            headers=self.headers
        )
        return response.json()

# Usage
client = ProxyAdminClient(
    base_url="http://127.0.0.1:9000",
    api_key="your-api-key-here"
)

print(client.health_check())
print(client.get_routes())
```

## Error Responses

All error responses follow this format:

```json
{
  "error": "Error type",
  "message": "Detailed error message (optional)"
}
```

### Common Error Codes

#### 401 Unauthorized
Missing or invalid authentication credentials.

```json
{
  "error": "Unauthorized"
}
```

#### 403 Forbidden
Operation not allowed (e.g., read-only mode).

```json
{
  "error": "Admin API is in read-only mode"
}
```

#### 404 Not Found
Endpoint doesn't exist.

```json
{
  "error": "Not found",
  "message": "The requested endpoint does not exist"
}
```

#### 500 Internal Server Error
Server error during request processing.

```json
{
  "error": "Failed to trigger configuration reload",
  "message": "channel closed"
}
```

#### 503 Service Unavailable
Service not ready or feature not available.

```json
{
  "error": "Hot reload not enabled",
  "message": "Configuration hot reload is not available. Restart the server to apply configuration changes."
}
```

## CORS Support

When CORS is enabled, the Admin API includes appropriate headers:

**Preflight Response (OPTIONS):**
```
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization, X-API-Key
Access-Control-Max-Age: 86400
Access-Control-Allow-Origin: http://localhost:3000
```

**Normal Response:**
```
Access-Control-Allow-Origin: http://localhost:3000
Access-Control-Allow-Credentials: true
```

## Future Endpoints (Planned)

The following endpoints are planned for future releases:

### Route Management (CRUD)
- `POST /api/routes` - Create new route
- `PUT /api/routes/:name` - Update route
- `DELETE /api/routes/:name` - Delete route

### Upstream Management (CRUD)
- `POST /api/upstreams` - Create new upstream
- `PUT /api/upstreams/:name` - Update upstream
- `DELETE /api/upstreams/:name` - Delete upstream

### Real-time Statistics
- `GET /api/stats/realtime` - WebSocket for live stats
- `GET /api/stats/history` - Historical statistics

### Backend Control
- `POST /api/upstreams/:name/servers/:id/enable` - Enable backend
- `POST /api/upstreams/:name/servers/:id/disable` - Disable backend

### Cache Control
- `DELETE /api/cache` - Clear cache
- `GET /api/cache/stats` - Cache statistics

## Support

For issues or questions about the Admin API:
- GitHub Issues: https://github.com/anthropics/rust-proxy/issues
- Documentation: https://docs.rust-proxy.dev
