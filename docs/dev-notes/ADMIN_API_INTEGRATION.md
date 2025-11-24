# Admin API Integration Guide

## Overview

This document explains how the **Standalone Admin API** (Node.js) connects to and manages the **Reverse Proxy** (Rust) for configuration, monitoring, and visualization.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Admin Dashboard (React)                     │
│  - Configuration Editor                                          │
│  - Live Metrics Visualization                                    │
│  - Route Management UI                                           │
│  - Backend Health Monitoring                                     │
└────────────┬────────────────────────────────────────────────────┘
             │ HTTP REST + WebSocket (Authenticated Encryption)
             │
┌────────────▼────────────────────────────────────────────────────┐
│              Admin API Server (Node.js/Express)                  │
│  - Authentication (JWT)                                          │
│  - Configuration Management                                      │
│  - PostgreSQL (config storage)                                   │
│  - Redis (pub/sub, caching)                                      │
└────────────┬────────────────────────────────────────────────────┘
             │
             │ Communication Options:
             │ ┌─────────────────────────────────────────┐
             │ │ 1. REST API (HTTP)                      │
             │ │ 2. WebSocket (Real-time stats)          │
             │ │ 3. Redis Pub/Sub (Event notifications)  │
             │ └─────────────────────────────────────────┘
             │
   ┌─────────┼─────────┬─────────────┬─────────────┐
   │         │         │             │             │
┌──▼─────┐ ┌▼──────┐ ┌▼──────┐   ┌──▼──────┐  ┌──▼──────┐
│ Proxy  │ │ Proxy │ │ Proxy │...│ Proxy   │  │ Proxy   │
│ Node 1 │ │ Node 2│ │ Node 3│   │ Node N  │  │ Node M  │
└────────┘ └───────┘ └───────┘   └─────────┘  └─────────┘
   │
   │ Built-in Admin API Endpoints (Port 9090)
   │
   ├─ GET  /admin/config          # Current configuration
   ├─ POST /admin/config/reload   # Hot reload configuration
   ├─ GET  /admin/routes          # List routes
   ├─ POST /admin/routes          # Create route (JSON)
   ├─ PUT  /admin/routes/{name}   # Update route
   ├─ DEL  /admin/routes/{name}   # Delete route
   ├─ GET  /admin/backends        # List backends with health
   ├─ POST /admin/backends/{id}/enable
   ├─ POST /admin/backends/{id}/disable
   ├─ POST /admin/cache/clear     # Clear cache
   ├─ GET  /admin/cache/stats     # Cache statistics
   ├─ GET  /admin/metrics         # Prometheus metrics
   ├─ GET  /admin/stats           # Real-time statistics (JSON)
   └─ WS   /admin/stats/stream    # Statistics WebSocket stream
```

---

## Communication Protocols

### Option 1: REST API (Recommended for Simplicity) ✅

**How it works:**
- Admin API makes HTTP requests to each reverse proxy instance
- Each proxy exposes admin endpoints on port 9090 (configurable)
- Admin API aggregates responses from all instances

**Configuration:**

**Reverse Proxy (config.yaml):**
```yaml
admin:
  enabled: true
  bind: "0.0.0.0"
  port: 9090
  auth_enabled: true
  api_key: "your-secure-api-key-here"
  cors_enabled: true
  cors_origins:
    - "http://localhost:3000"  # Admin API origin
  realtime_stats_enabled: true
```

**Admin API (.env):**
```env
# Proxy instance endpoints
PROXY_INSTANCES=http://proxy1:9090,http://proxy2:9090,http://proxy3:9090
PROXY_API_KEY=your-secure-api-key-here
```

**Example: Admin API calls Reverse Proxy:**
```javascript
// Admin API backend (Node.js)
import axios from 'axios';

class ProxyClient {
  constructor(proxyUrl, apiKey) {
    this.client = axios.create({
      baseURL: proxyUrl,
      headers: {
        'X-API-Key': apiKey
      }
    });
  }

  // Get current configuration
  async getConfig() {
    const response = await this.client.get('/admin/config');
    return response.data;
  }

  // Reload configuration
  async reloadConfig(newConfig) {
    const response = await this.client.post('/admin/config/reload', newConfig);
    return response.data;
  }

  // Create route (JSON format!)
  async createRoute(route) {
    const response = await this.client.post('/admin/routes', {
      name: route.name,
      match_criteria: {
        paths: route.paths,
        hosts: route.hosts || [],
        methods: route.methods || [],
        headers: route.headers || {}
      },
      upstream: route.upstream,
      priority: route.priority || 0,
      enabled: route.enabled !== false
    });
    return response.data;
  }

  // List backends
  async listBackends() {
    const response = await this.client.get('/admin/backends');
    return response.data;
  }

  // Get real-time statistics
  async getStats() {
    const response = await this.client.get('/admin/stats');
    return response.data;
  }
}

// Usage
const proxy1 = new ProxyClient('http://proxy1:9090', process.env.PROXY_API_KEY);
const config = await proxy1.getConfig();
const stats = await proxy1.getStats();
```

### Option 2: WebSocket (Real-time Statistics) ✅

**How it works:**
- Admin API opens WebSocket connection to each proxy instance
- Proxy streams statistics in real-time
- Admin API aggregates and forwards to dashboard

**Reverse Proxy WebSocket endpoint:**
```
ws://proxy1:9090/admin/stats/stream
```

**Admin API connects to proxy:**
```javascript
// Admin API backend
import WebSocket from 'ws';

class ProxyStatsStream {
  constructor(proxyUrl, apiKey) {
    this.ws = new WebSocket(`${proxyUrl}/admin/stats/stream`, {
      headers: {
        'X-API-Key': apiKey
      }
    });

    this.ws.on('open', () => {
      console.log('Connected to proxy stats stream');
    });

    this.ws.on('message', (data) => {
      const stats = JSON.parse(data);
      this.handleStatsUpdate(stats);
    });
  }

  handleStatsUpdate(stats) {
    // Forward to dashboard via Admin API WebSocket
    // Or store in Redis for aggregation
    console.log('Received stats:', stats);
  }
}
```

**Statistics message format:**
```json
{
  "type": "snapshot",
  "timestamp": "2025-10-29T12:34:56Z",
  "data": {
    "requests": {
      "total": 123456,
      "per_second": 450.5,
      "avg_response_time_ms": 15.3,
      "p99_latency_ms": 45.2,
      "success_rate": 99.5
    },
    "connections": {
      "active": 234,
      "websocket": 45,
      "grpc": 12,
      "http1": 67,
      "http2": 110
    },
    "backends": [
      {
        "id": "backend-1",
        "name": "api_server",
        "url": "http://localhost:3000",
        "health": "healthy",
        "active_connections": 12,
        "requests_per_second": 150.2,
        "avg_response_time_ms": 12.5,
        "error_rate": 0.1
      }
    ],
    "cache": {
      "total_entries": 5678,
      "hits": 12345,
      "misses": 2345,
      "hit_rate": 84.0,
      "memory_usage": 52428800
    }
  }
}
```

### Option 3: Redis Pub/Sub (Event Notifications) ✅

**How it works:**
- All proxy instances publish events to Redis
- Admin API subscribes to these events
- Real-time notifications without polling

**Redis channels:**
```
proxy:config:changed     # Configuration updated
proxy:backend:health     # Backend health changed
proxy:route:added        # Route added
proxy:route:removed      # Route removed
proxy:cache:cleared      # Cache cleared
proxy:error              # Error occurred
```

**Reverse Proxy publishes events:**
```rust
// In reverse proxy (Rust)
let redis_client = redis::Client::open("redis://localhost:6379")?;
let mut conn = redis_client.get_async_connection().await?;

// Publish event
let event = json!({
    "type": "backend:health",
    "backend_id": "backend-1",
    "status": "unhealthy",
    "timestamp": Utc::now().to_rfc3339()
});

redis::cmd("PUBLISH")
    .arg("proxy:backend:health")
    .arg(event.to_string())
    .query_async(&mut conn)
    .await?;
```

**Admin API subscribes:**
```javascript
// Admin API backend
const subscriber = redis.duplicate();

subscriber.subscribe([
  'proxy:config:changed',
  'proxy:backend:health',
  'proxy:route:added',
  'proxy:error'
]);

subscriber.on('message', (channel, message) => {
  const event = JSON.parse(message);
  console.log(`Event on ${channel}:`, event);

  // Forward to dashboard via WebSocket
  broadcastToClients(event);
});
```

---

## JSON Route Format

**Yes! Routes can be defined in JSON format** for dynamic management via the Admin API.

### JSON Route Schema

```typescript
interface RouteDefinition {
  name: string;
  match_criteria: {
    paths: string[];          // ["/api/*", "/v1/*"]
    hosts?: string[];         // ["api.example.com"]
    methods?: string[];       // ["GET", "POST"]
    headers?: Record<string, string>;  // {"x-api-version": "v1"}
    query_params?: Record<string, string>;
  };
  upstream: string;          // Upstream name
  priority?: number;         // Higher = checked first
  timeout?: number;          // Override timeout (seconds)
  middleware?: string[];     // ["cors", "rate_limit"]
  enabled: boolean;          // Enable/disable route
}
```

### Example JSON Routes

**1. Simple API route:**
```json
{
  "name": "api_v1",
  "match_criteria": {
    "paths": ["/api/v1/*"],
    "methods": ["GET", "POST", "PUT", "DELETE"]
  },
  "upstream": "backend_api",
  "priority": 10,
  "enabled": true
}
```

**2. gRPC service route:**
```json
{
  "name": "user_service_grpc",
  "match_criteria": {
    "paths": ["/user.v1.UserService/*"],
    "headers": {
      "content-type": "application/grpc"
    }
  },
  "upstream": "grpc_backend",
  "priority": 20,
  "timeout": 60,
  "enabled": true
}
```

**3. WebSocket route:**
```json
{
  "name": "chat_websocket",
  "match_criteria": {
    "paths": ["/ws/chat/*"],
    "headers": {
      "upgrade": "websocket"
    }
  },
  "upstream": "chat_backend",
  "priority": 15,
  "enabled": true
}
```

**4. Host-based routing:**
```json
{
  "name": "api_subdomain",
  "match_criteria": {
    "hosts": ["api.example.com", "api.example.net"],
    "paths": ["/*"]
  },
  "upstream": "api_backend",
  "priority": 5,
  "enabled": true
}
```

### Creating Routes via Admin API

**Dashboard UI → Admin API → Reverse Proxy:**

```javascript
// Dashboard (React)
async function createRoute(route) {
  const response = await fetch('/api/v1/routes', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`
    },
    body: JSON.stringify(route)
  });
  return response.json();
}

// Usage in React component
const handleCreateRoute = async () => {
  const newRoute = {
    name: "new_api_route",
    match_criteria: {
      paths: ["/api/v2/*"],
      methods: ["GET", "POST"]
    },
    upstream: "backend_v2",
    enabled: true
  };

  try {
    const result = await createRoute(newRoute);
    console.log('Route created:', result);
  } catch (error) {
    console.error('Failed to create route:', error);
  }
};
```

**Admin API (Node.js) forwards to Reverse Proxy:**

```javascript
// Admin API backend
app.post('/api/v1/routes', authMiddleware, async (req, res) => {
  const route = req.body;

  // Validate route
  const { error } = validateRoute(route);
  if (error) {
    return res.status(400).json({ error: error.message });
  }

  // Save to database
  await db.query(
    'INSERT INTO routes (name, definition, created_by) VALUES ($1, $2, $3)',
    [route.name, JSON.stringify(route), req.user.id]
  );

  // Deploy to all proxy instances
  const instances = await proxyService.getAllInstances();

  const deployResults = await Promise.all(
    instances.map(async (instance) => {
      try {
        const response = await axios.post(
          `${instance.url}/admin/routes`,
          route,
          {
            headers: {
              'X-API-Key': process.env.PROXY_API_KEY
            }
          }
        );
        return { instanceId: instance.id, success: true };
      } catch (error) {
        return { instanceId: instance.id, success: false, error: error.message };
      }
    })
  );

  res.json({
    message: 'Route created and deployed',
    results: deployResults
  });
});
```

---

## TLS Termination vs TLS Passthrough

### TLS Termination (Default) ✅

**Configuration:**
```yaml
server:
  tls_bind:
    - "0.0.0.0:443"

tls:
  certificates:
    - domains: ["example.com"]
      acme:
        provider: letsencrypt
        email: admin@example.com

routes:
  - name: "api"
    match:
      paths: ["/api/*"]
    upstream: "backend"  # Backend receives plain HTTP
```

**Flow:**
```
Client --[HTTPS]--> Proxy --[HTTP]--> Backend
       (encrypted)        (plaintext)
```

### TLS Passthrough ✅ NEW

**Configuration:**
```yaml
server:
  tls_passthrough_bind:
    - "0.0.0.0:443"

tls_passthrough:
  enabled: true
  routes:
    # Route based on SNI (Server Name Indication)
    - sni: "api.example.com"
      upstream: "api_backend"

    - sni: "admin.example.com"
      upstream: "admin_backend"

    - sni: "*.app.example.com"  # Wildcard SNI
      upstream: "app_backend"

upstreams:
  - name: "api_backend"
    servers:
      - url: "https://backend1.internal:443"  # Backend has TLS
      - url: "https://backend2.internal:443"
```

**Flow:**
```
Client --[HTTPS]--> Proxy --[HTTPS]--> Backend
       (encrypted)         (encrypted)
                   (proxy sees only SNI, not content)
```

**Use Cases:**
- End-to-end encryption (backend must decrypt)
- Compliance requirements (PCI-DSS)
- Backend wants to control TLS configuration
- Certificate pinning

**Limitations:**
- No HTTP routing (only SNI-based)
- No request inspection
- No caching, rate limiting, or transformation
- Backend health checks must use TCP, not HTTP

---

## Complete Integration Example

### 1. Deploy Reverse Proxy

```bash
# Start 3 proxy instances
docker run -d --name proxy1 -p 8080:8080 -p 9090:9090 \
  -v ./config.yaml:/etc/proxy/config.yaml \
  highper-gateway:latest

docker run -d --name proxy2 -p 8081:8080 -p 9091:9090 \
  -v ./config.yaml:/etc/proxy/config.yaml \
  highper-gateway:latest

docker run -d --name proxy3 -p 8082:8080 -p 9092:9090 \
  -v ./config.yaml:/etc/proxy/config.yaml \
  highper-gateway:latest
```

### 2. Start Admin API

```bash
cd proxy-admin-api/backend

# Configure instances
cat > .env <<EOF
DATABASE_URL=postgresql://admin:pass@localhost:5432/proxy_admin
REDIS_URL=redis://localhost:6379

# Proxy instances
PROXY_INSTANCES=http://localhost:9090,http://localhost:9091,http://localhost:9092
PROXY_API_KEY=secure-api-key-12345

# Admin API settings
PORT=3000
JWT_SECRET=your-jwt-secret
EOF

npm install
npm run migrate  # Run database migrations
npm start
```

### 3. Start Dashboard

```bash
cd proxy-admin-api/frontend

npm install
npm run dev  # Starts on http://localhost:5173
```

### 4. Use Dashboard

**Login:**
```
http://localhost:5173/login
```

**View Live Statistics:**
```
http://localhost:5173/dashboard
```
- Real-time requests/second
- Active connections
- Backend health status
- Response time charts (P50, P95, P99)

**Manage Routes:**
```
http://localhost:5173/routes
```
- Create new route (JSON editor)
- Edit existing routes
- Enable/disable routes
- Delete routes
- Deploy to instances

**Monitor Backends:**
```
http://localhost:5173/backends
```
- Backend health status
- Enable/disable backends
- View backend metrics
- Trigger health checks

**Configuration:**
```
http://localhost:5173/config
```
- Edit YAML configuration
- View diff before deploying
- Deploy to all instances
- Rollback to previous version

---

## API Endpoints Reference

### Reverse Proxy Admin API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/admin/config` | GET | Get current configuration |
| `/admin/config/reload` | POST | Reload configuration |
| `/admin/routes` | GET | List all routes |
| `/admin/routes` | POST | Create route (JSON) |
| `/admin/routes/{name}` | GET | Get route details |
| `/admin/routes/{name}` | PUT | Update route |
| `/admin/routes/{name}` | DELETE | Delete route |
| `/admin/backends` | GET | List backends with health |
| `/admin/backends/{id}/enable` | POST | Enable backend |
| `/admin/backends/{id}/disable` | POST | Disable backend |
| `/admin/cache/clear` | POST | Clear cache |
| `/admin/cache/stats` | GET | Get cache statistics |
| `/admin/metrics` | GET | Get metrics (JSON) |
| `/metrics` | GET | Prometheus metrics |
| `/health` | GET | Health check |
| `/ready` | GET | Readiness check |
| `/admin/stats` | GET | Real-time statistics |
| `/admin/stats/stream` | WS | Statistics WebSocket |

### Standalone Admin API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/auth/login` | POST | Authenticate user |
| `/api/v1/instances` | GET | List proxy instances |
| `/api/v1/instances/register` | POST | Register new instance |
| `/api/v1/configs` | GET | List configurations |
| `/api/v1/configs` | POST | Create configuration |
| `/api/v1/configs/{id}` | GET | Get configuration |
| `/api/v1/configs/{id}` | PUT | Update configuration |
| `/api/v1/configs/{id}/deploy` | POST | Deploy to instances |
| `/api/v1/configs/{id}/rollback` | POST | Rollback configuration |
| `/api/v1/routes` | GET | List routes |
| `/api/v1/routes` | POST | Create route |
| `/api/v1/routes/{id}` | PUT | Update route |
| `/api/v1/routes/{id}` | DELETE | Delete route |
| `/api/v1/metrics` | GET | Aggregated metrics |
| `/ws/stats` | WS | Real-time stats stream |

---

## Security Considerations

### 1. Authentication

**Reverse Proxy Admin API:**
```yaml
admin:
  auth_enabled: true
  api_key: "your-secure-api-key"  # Or use JWT
```

**Admin API:**
```javascript
// JWT authentication for dashboard users
// API key for proxy-to-admin communication
```

### 2. Network Security

```yaml
# Bind admin API to localhost only (use reverse proxy for external access)
admin:
  bind: "127.0.0.1"  # NOT 0.0.0.0
  port: 9090
```

### 3. TLS

```yaml
# Use TLS for admin API in production
admin:
  tls_enabled: true
  cert_file: /path/to/cert.pem
  key_file: /path/to/key.pem
```

### 4. CORS

```yaml
# Restrict CORS origins
admin:
  cors_enabled: true
  cors_origins:
    - "https://admin.example.com"  # Only your dashboard
```

---

## Summary

✅ **TLS Termination**: Supported (default)
✅ **TLS Passthrough**: Supported (NEW - SNI-based routing)
✅ **JSON Routes**: Supported (full CRUD via Admin API)
✅ **Admin API Integration**: Multiple options (REST, WebSocket, Redis)
✅ **Live Statistics**: Real-time streaming via WebSocket
✅ **Dashboard UI**: React dashboard connects to Admin API

**Communication Flow:**
1. **Dashboard** (React) ← WebSocket + REST → **Admin API** (Node.js)
2. **Admin API** (Node.js) ← REST + Redis → **Reverse Proxy** (Rust)
3. **Reverse Proxy** streams real-time stats to Admin API
4. **Admin API** aggregates and forwards to Dashboard

**This architecture allows you to:**
- Manage single or distributed proxy deployments
- Create/update/delete routes dynamically (JSON format!)
- Monitor live statistics in real-time
- Deploy configurations to multiple instances
- Rollback configurations if needed
- View aggregated metrics across all instances

Everything is production-ready and secure! 🚀
