# Standalone Admin API & Dashboard Design

**Project:** Highper Gateway Admin API & Dashboard
**Type:** Standalone microservice
**Language:** Rust (Backend) + TypeScript/React (Frontend)
**Communication:** REST API + WebSocket + gRPC (optional)

---

## 🎯 Executive Summary

A standalone Admin API service that provides:
1. **Configuration Management** - CRUD operations for proxy config
2. **Real-time Monitoring** - Live metrics and statistics
3. **Control Plane** - Backend enable/disable, cache control
4. **Web Dashboard** - Modern UI for visualization and management
5. **Multi-instance Support** - Manage multiple proxy instances
6. **Security** - Authentication, authorization, audit logging

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Admin Dashboard                       │
│              (React/TypeScript/Vite)                     │
│   ┌──────────┬──────────┬──────────┬──────────┐        │
│   │ Config   │ Metrics  │ Backends │ Logs     │        │
│   │ Editor   │ Dashboard│ Status   │ Viewer   │        │
│   └──────────┴──────────┴──────────┴──────────┘        │
└────────────────┬────────────────────────────────────────┘
                 │ HTTP/HTTPS + WebSocket
                 ▼
┌─────────────────────────────────────────────────────────┐
│              Admin API Server (Rust)                     │
│  ┌──────────────────────────────────────────────────┐  │
│  │  REST API    WebSocket    gRPC (optional)        │  │
│  │  (Axum)      (tokio-tungstenite)  (tonic)        │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │  Service Layer                                    │  │
│  │  • Config Manager  • Metrics Aggregator          │  │
│  │  • Instance Manager • Cache Controller           │  │
│  │  • Health Monitor   • Auth Service               │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │  Data Layer                                       │  │
│  │  • PostgreSQL/SQLite • Redis Cache               │  │
│  │  • Config Storage    • Metrics Storage           │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────┬────────────────────────────────────────┘
                 │ gRPC / HTTP / Redis Pub/Sub
                 ▼
┌─────────────────────────────────────────────────────────┐
│           Highper Gateway Instances (1..N)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Instance 1  │  │  Instance 2  │  │  Instance N  │  │
│  │  :8080       │  │  :8081       │  │  :808N       │  │
│  │              │  │              │  │              │  │
│  │ • Admin      │  │ • Admin      │  │ • Admin      │  │
│  │   Agent      │  │   Agent      │  │   Agent      │  │
│  │ • Metrics    │  │ • Metrics    │  │ • Metrics    │  │
│  │   Endpoint   │  │   Endpoint   │  │   Endpoint   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 📂 Project Structure

```
admin-api/
├── Cargo.toml
├── README.md
├── .env.example
│
├── src/
│   ├── main.rs                    # Entry point
│   ├── lib.rs                     # Library root
│   │
│   ├── api/                       # API layer
│   │   ├── mod.rs
│   │   ├── rest/                  # REST API endpoints
│   │   │   ├── mod.rs
│   │   │   ├── config.rs          # Config endpoints
│   │   │   ├── instances.rs       # Instance management
│   │   │   ├── backends.rs        # Backend control
│   │   │   ├── metrics.rs         # Metrics endpoints
│   │   │   ├── cache.rs           # Cache control
│   │   │   ├── health.rs          # Health checks
│   │   │   └── auth.rs            # Authentication
│   │   │
│   │   ├── websocket/             # WebSocket handlers
│   │   │   ├── mod.rs
│   │   │   ├── metrics_stream.rs  # Real-time metrics
│   │   │   ├── logs_stream.rs     # Log streaming
│   │   │   └── events.rs          # Event notifications
│   │   │
│   │   └── grpc/                  # gRPC services (optional)
│   │       ├── mod.rs
│   │       ├── admin.proto
│   │       └── admin.rs
│   │
│   ├── services/                  # Business logic
│   │   ├── mod.rs
│   │   ├── config_manager.rs      # Configuration management
│   │   ├── instance_manager.rs    # Proxy instance management
│   │   ├── metrics_aggregator.rs  # Metrics collection
│   │   ├── health_monitor.rs      # Health monitoring
│   │   ├── cache_controller.rs    # Cache operations
│   │   └── auth_service.rs        # Authentication/authorization
│   │
│   ├── models/                    # Data models
│   │   ├── mod.rs
│   │   ├── config.rs              # Configuration models
│   │   ├── instance.rs            # Instance models
│   │   ├── metrics.rs             # Metrics models
│   │   ├── user.rs                # User/auth models
│   │   └── audit.rs               # Audit log models
│   │
│   ├── db/                        # Database layer
│   │   ├── mod.rs
│   │   ├── schema.rs              # Database schema
│   │   ├── migrations/            # SQL migrations
│   │   ├── postgres.rs            # PostgreSQL implementation
│   │   ├── sqlite.rs              # SQLite implementation
│   │   └── redis.rs               # Redis client
│   │
│   ├── client/                    # Proxy client
│   │   ├── mod.rs
│   │   ├── http_client.rs         # HTTP client for proxy
│   │   ├── grpc_client.rs         # gRPC client
│   │   └── metrics_scraper.rs     # Prometheus scraper
│   │
│   ├── middleware/                # API middleware
│   │   ├── mod.rs
│   │   ├── auth.rs                # Authentication middleware
│   │   ├── rate_limit.rs          # Rate limiting
│   │   ├── cors.rs                # CORS
│   │   └── audit.rs               # Audit logging
│   │
│   ├── config/                    # Admin API config
│   │   ├── mod.rs
│   │   └── settings.rs            # Settings management
│   │
│   └── utils/
│       ├── mod.rs
│       ├── crypto.rs              # Encryption/hashing
│       └── validation.rs          # Input validation
│
├── dashboard/                     # Web dashboard (separate)
│   ├── package.json
│   ├── vite.config.ts
│   ├── index.html
│   │
│   ├── src/
│   │   ├── main.tsx               # Entry point
│   │   ├── App.tsx                # Root component
│   │   │
│   │   ├── pages/                 # Page components
│   │   │   ├── Dashboard.tsx      # Main dashboard
│   │   │   ├── Instances.tsx      # Instance list
│   │   │   ├── Config.tsx         # Config editor
│   │   │   ├── Backends.tsx       # Backend status
│   │   │   ├── Metrics.tsx        # Metrics charts
│   │   │   ├── Logs.tsx           # Log viewer
│   │   │   └── Settings.tsx       # Settings
│   │   │
│   │   ├── components/            # Reusable components
│   │   │   ├── charts/            # Chart components
│   │   │   ├── forms/             # Form components
│   │   │   ├── tables/            # Table components
│   │   │   └── layout/            # Layout components
│   │   │
│   │   ├── hooks/                 # Custom React hooks
│   │   │   ├── useWebSocket.ts    # WebSocket hook
│   │   │   ├── useMetrics.ts      # Metrics fetching
│   │   │   └── useAuth.ts         # Authentication
│   │   │
│   │   ├── services/              # API services
│   │   │   ├── api.ts             # API client
│   │   │   ├── websocket.ts       # WebSocket client
│   │   │   └── auth.ts            # Auth service
│   │   │
│   │   ├── stores/                # State management
│   │   │   ├── authStore.ts       # Auth state
│   │   │   ├── metricsStore.ts    # Metrics state
│   │   │   └── configStore.ts     # Config state
│   │   │
│   │   └── types/                 # TypeScript types
│   │       ├── api.ts             # API types
│   │       ├── config.ts          # Config types
│   │       └── metrics.ts         # Metrics types
│   │
│   └── public/                    # Static assets
│
├── migrations/                    # Database migrations
│   ├── 001_initial.sql
│   ├── 002_add_audit_logs.sql
│   └── 003_add_instances.sql
│
├── config/
│   ├── admin-api.yaml             # Example config
│   └── docker-compose.yml         # Dev environment
│
├── deploy/
│   ├── kubernetes/                # K8s manifests
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   └── ingress.yaml
│   │
│   └── docker/
│       ├── Dockerfile.api         # API container
│       └── Dockerfile.dashboard   # Dashboard container
│
└── docs/
    ├── API.md                     # API reference
    ├── DEPLOYMENT.md              # Deployment guide
    └── INTEGRATION.md             # Integration guide
```

---

## 🔌 Communication Protocols

### 1. Admin API → Proxy Instances

#### Option A: HTTP REST API (Recommended)
**Proxy exposes admin endpoints:**
```
GET  /admin/health           # Health status
GET  /admin/metrics          # Prometheus metrics
GET  /admin/config           # Current config
POST /admin/config/reload    # Reload config
POST /admin/backends/{id}/enable
POST /admin/backends/{id}/disable
POST /admin/cache/clear
GET  /admin/stats            # Real-time statistics
```

**Pros:**
- Simple to implement
- No additional dependencies
- Works with existing HTTP server
- Easy to secure (API keys, JWT)

**Cons:**
- Polling required for real-time updates
- Higher latency for frequent updates

#### Option B: gRPC (High Performance)
**Bidirectional streaming for real-time:**
```protobuf
service ProxyAdmin {
  rpc GetConfig(Empty) returns (ConfigResponse);
  rpc UpdateConfig(ConfigRequest) returns (StatusResponse);
  rpc StreamMetrics(Empty) returns (stream MetricsData);
  rpc ControlBackend(BackendControl) returns (StatusResponse);
  rpc ClearCache(CacheRequest) returns (StatusResponse);
}
```

**Pros:**
- Better performance
- Bidirectional streaming
- Type-safe with Protocol Buffers
- Built-in health checks

**Cons:**
- More complex setup
- Additional dependency
- Requires protobuf compilation

#### Option C: Redis Pub/Sub (Distributed)
**For multi-instance coordination:**
```
Channels:
- admin:commands:{instance_id}  # Commands to instance
- admin:metrics:{instance_id}   # Metrics from instance
- admin:events                  # Global events
```

**Pros:**
- Distributed architecture
- Pub/sub pattern
- Works well with existing Redis

**Cons:**
- Redis dependency
- No request/response guarantee
- Complex error handling

**Recommendation:** Start with HTTP REST (Option A), add gRPC (Option B) later for performance, use Redis Pub/Sub (Option C) for distributed coordination.

---

## 📡 REST API Specification

### Configuration Management

```http
# List all configurations
GET /api/v1/configs
Response: {
  "configs": [
    {
      "id": "uuid",
      "name": "production",
      "version": 5,
      "created_at": "2025-10-29T...",
      "updated_at": "2025-10-29T...",
      "active": true
    }
  ]
}

# Get configuration
GET /api/v1/configs/{id}
Response: {
  "id": "uuid",
  "name": "production",
  "content": { /* YAML config as JSON */ },
  "version": 5,
  "instances": ["instance-1", "instance-2"]
}

# Create configuration
POST /api/v1/configs
Body: {
  "name": "staging",
  "content": { /* config */ }
}

# Update configuration
PUT /api/v1/configs/{id}
Body: {
  "content": { /* updated config */ }
}

# Deploy configuration to instances
POST /api/v1/configs/{id}/deploy
Body: {
  "instance_ids": ["instance-1", "instance-2"],
  "strategy": "rolling" | "all_at_once"
}

# Rollback configuration
POST /api/v1/configs/{id}/rollback
Body: {
  "to_version": 4
}
```

### Instance Management

```http
# List all proxy instances
GET /api/v1/instances
Response: {
  "instances": [
    {
      "id": "instance-1",
      "name": "proxy-01",
      "host": "10.0.1.10:8080",
      "version": "0.9.0",
      "status": "healthy" | "unhealthy" | "unknown",
      "uptime_seconds": 86400,
      "config_version": 5,
      "last_seen": "2025-10-29T..."
    }
  ]
}

# Register new instance
POST /api/v1/instances/register
Body: {
  "id": "instance-3",
  "name": "proxy-03",
  "host": "10.0.1.12:8080",
  "admin_endpoint": "http://10.0.1.12:9090"
}

# Get instance details
GET /api/v1/instances/{id}
Response: {
  "id": "instance-1",
  "name": "proxy-01",
  "host": "10.0.1.10:8080",
  "status": "healthy",
  "metrics": {
    "requests_per_second": 1250,
    "active_connections": 543,
    "cpu_usage_percent": 35.2,
    "memory_mb": 156
  },
  "backends": [
    {
      "id": "backend-1",
      "url": "http://backend:8080",
      "status": "healthy",
      "active_connections": 45
    }
  ]
}

# Update instance
PUT /api/v1/instances/{id}
Body: {
  "name": "proxy-01-updated"
}

# Remove instance
DELETE /api/v1/instances/{id}
```

### Backend Control

```http
# List backends (across all instances or specific instance)
GET /api/v1/backends?instance_id=instance-1
Response: {
  "backends": [
    {
      "id": "backend-1",
      "name": "api_backend",
      "instance_id": "instance-1",
      "url": "http://backend:8080",
      "status": "healthy" | "unhealthy",
      "enabled": true,
      "health_check": {
        "consecutive_successes": 10,
        "consecutive_failures": 0,
        "last_check": "2025-10-29T..."
      },
      "metrics": {
        "requests_per_second": 234,
        "active_connections": 23,
        "avg_latency_ms": 12.5
      }
    }
  ]
}

# Enable backend
POST /api/v1/backends/{id}/enable
Response: {
  "success": true,
  "message": "Backend enabled successfully"
}

# Disable backend
POST /api/v1/backends/{id}/disable
Body: {
  "reason": "Maintenance",
  "drain_timeout_seconds": 30
}

# Force health check
POST /api/v1/backends/{id}/health-check
```

### Cache Control

```http
# Get cache statistics
GET /api/v1/cache/stats?instance_id=instance-1
Response: {
  "local_cache": {
    "entries": 1234,
    "size_mb": 45,
    "hit_rate": 0.78
  },
  "distributed_cache": {
    "entries": 5678,
    "size_mb": 234,
    "hit_rate": 0.85
  }
}

# Clear cache
POST /api/v1/cache/clear
Body: {
  "instance_ids": ["instance-1"],
  "cache_type": "local" | "distributed" | "all",
  "pattern": "user:*" // optional key pattern
}

# Invalidate specific keys
POST /api/v1/cache/invalidate
Body: {
  "keys": ["cache:key1", "cache:key2"]
}
```

### Metrics & Monitoring

```http
# Get aggregated metrics
GET /api/v1/metrics?instance_id=instance-1&range=1h
Response: {
  "timestamp": "2025-10-29T...",
  "metrics": {
    "requests_total": 1000000,
    "requests_per_second": 1250,
    "avg_latency_ms": 15.3,
    "p50_latency_ms": 12.1,
    "p95_latency_ms": 28.5,
    "p99_latency_ms": 45.2,
    "error_rate": 0.002,
    "active_connections": 543
  },
  "by_route": [
    {
      "route": "/api/users",
      "requests_per_second": 450,
      "avg_latency_ms": 18.2
    }
  ]
}

# Get health status
GET /api/v1/health
Response: {
  "status": "healthy",
  "instances": {
    "total": 3,
    "healthy": 3,
    "unhealthy": 0
  },
  "backends": {
    "total": 9,
    "healthy": 8,
    "unhealthy": 1
  }
}
```

### Logs

```http
# Query logs
GET /api/v1/logs?instance_id=instance-1&level=error&limit=100
Response: {
  "logs": [
    {
      "timestamp": "2025-10-29T...",
      "level": "error",
      "message": "Backend connection failed",
      "instance_id": "instance-1",
      "fields": {
        "backend": "backend-1",
        "error": "connection timeout"
      }
    }
  ],
  "total": 245,
  "has_more": true
}
```

### Rate Limiting

```http
# Get rate limit status
GET /api/v1/rate-limits?key=user:123
Response: {
  "key": "user:123",
  "limit": 1000,
  "remaining": 234,
  "reset_at": "2025-10-29T..."
}

# Reset rate limit
POST /api/v1/rate-limits/reset
Body: {
  "keys": ["user:123", "user:456"]
}
```

---

## 🌐 WebSocket API

### Real-time Metrics Stream

```javascript
// Connect to WebSocket
const ws = new WebSocket('ws://admin-api:3000/ws/metrics');

// Subscribe to metrics
ws.send(JSON.stringify({
  type: 'subscribe',
  channels: ['metrics', 'health', 'logs'],
  instance_ids: ['instance-1', 'instance-2']
}));

// Receive updates
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);

  // Metrics update
  if (data.type === 'metrics') {
    console.log('RPS:', data.payload.requests_per_second);
  }

  // Health update
  if (data.type === 'health') {
    console.log('Backend status:', data.payload.backend_id, data.payload.status);
  }

  // Log message
  if (data.type === 'log') {
    console.log('Log:', data.payload.message);
  }
};
```

### Message Format

```json
{
  "type": "metrics" | "health" | "log" | "event",
  "timestamp": "2025-10-29T...",
  "instance_id": "instance-1",
  "payload": {
    // Type-specific data
  }
}
```

---

## 🎨 Dashboard Features

### 1. Main Dashboard
- **Overview Cards**
  - Total requests/sec
  - Active connections
  - Error rate
  - Average latency
- **Real-time Charts**
  - Requests per second (line chart)
  - Latency distribution (histogram)
  - Error rate (area chart)
  - Active connections (line chart)
- **Instance Status**
  - Grid/list of all instances
  - Health indicators
  - Quick actions (restart, drain)

### 2. Instances Page
- **Instance List Table**
  - ID, Name, Host, Status, Version
  - Uptime, Config Version, Last Seen
  - Actions: View, Edit, Delete
- **Instance Details Modal**
  - Full metrics
  - Backend status
  - Recent logs
  - Configuration

### 3. Configuration Editor
- **Config List** (versions, active status)
- **YAML Editor** with syntax highlighting
- **Validation** (live config validation)
- **Diff Viewer** (compare versions)
- **Deploy Controls** (deploy, rollback)
- **Preview** (show affected instances)

### 4. Backends Status
- **Backend Grid/Table**
  - URL, Status, Instance
  - Health metrics
  - Active connections
- **Actions**
  - Enable/Disable
  - Force health check
  - View metrics
- **Health History Chart**

### 5. Metrics & Analytics
- **Time Range Selector** (1h, 6h, 24h, 7d, 30d)
- **Metric Selector** (requests, latency, errors, etc.)
- **Charts**
  - Time series
  - Heatmaps
  - Percentile charts
- **Export** (CSV, JSON, PNG)

### 6. Logs Viewer
- **Real-time Log Stream**
- **Filters**
  - By instance
  - By level (error, warn, info, debug)
  - By time range
  - Full-text search
- **Log Details Modal**
- **Download Logs**

### 7. Cache Management
- **Cache Statistics**
  - Hit rate
  - Size
  - Entry count
- **Actions**
  - Clear cache
  - Invalidate keys
  - View cache keys
- **Cache Performance Chart**

### 8. Settings
- **User Management** (if multi-user)
- **Authentication Settings**
- **Notification Settings**
- **Theme Settings** (dark/light mode)
- **API Keys Management**

---

## 🔐 Security & Authentication

### Authentication Methods

#### 1. API Key Authentication (Simple)
```http
GET /api/v1/instances
Authorization: Bearer admin-api-key-12345678
```

#### 2. JWT Authentication (Recommended)
```http
POST /api/v1/auth/login
Body: {
  "username": "admin",
  "password": "secure-password"
}
Response: {
  "access_token": "eyJhbGc...",
  "refresh_token": "dGhpcyB...",
  "expires_in": 3600
}

# Use token
GET /api/v1/instances
Authorization: Bearer eyJhbGc...
```

#### 3. OAuth2/OIDC (Enterprise)
- Google/GitHub/Azure AD integration
- Single Sign-On (SSO)
- Multi-factor authentication (MFA)

### Authorization

#### Role-Based Access Control (RBAC)
```
Roles:
- admin:      Full access
- operator:   Read + Control (enable/disable backends)
- viewer:     Read-only access
- developer:  Config edit + Deploy

Permissions:
- config:read, config:write, config:deploy
- instances:read, instances:write
- backends:read, backends:control
- cache:read, cache:clear
- metrics:read
- logs:read
```

### Audit Logging
```
Track all operations:
- Who: user_id
- What: operation (e.g., "config:deploy")
- When: timestamp
- Where: IP address
- Result: success/failure
- Details: changed values
```

---

## 💾 Database Schema

### PostgreSQL Schema

```sql
-- Users and authentication
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- API keys
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    key_hash VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    permissions JSONB NOT NULL,
    expires_at TIMESTAMP,
    last_used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Proxy instances
CREATE TABLE instances (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    host VARCHAR(255) NOT NULL,
    admin_endpoint VARCHAR(255) NOT NULL,
    version VARCHAR(50),
    status VARCHAR(50) NOT NULL,
    config_version INTEGER,
    metadata JSONB,
    last_seen_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Configurations
CREATE TABLE configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    content JSONB NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN DEFAULT FALSE,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Configuration history
CREATE TABLE configuration_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id UUID REFERENCES configurations(id),
    version INTEGER NOT NULL,
    content JSONB NOT NULL,
    changed_by UUID REFERENCES users(id),
    change_reason TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Deployments
CREATE TABLE deployments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id UUID REFERENCES configurations(id),
    config_version INTEGER NOT NULL,
    instance_ids TEXT[] NOT NULL,
    strategy VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL, -- pending, in_progress, completed, failed
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    deployed_by UUID REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Audit logs
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    action VARCHAR(255) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,
    resource_id VARCHAR(255),
    details JSONB,
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Metrics snapshots (for historical data)
CREATE TABLE metrics_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id VARCHAR(255) REFERENCES instances(id),
    metrics JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_metrics_snapshots_instance_time
ON metrics_snapshots(instance_id, created_at DESC);

-- Indexes
CREATE INDEX idx_instances_status ON instances(status);
CREATE INDEX idx_configurations_active ON configurations(is_active);
CREATE INDEX idx_audit_logs_user_time ON audit_logs(user_id, created_at DESC);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
```

---

## 🛠️ Technology Stack

### Backend (Rust)
```toml
[dependencies]
# Web framework
axum = "0.7"                    # Modern web framework
tower = "0.4"                   # Middleware
tower-http = "0.5"              # HTTP middleware

# WebSocket
tokio-tungstenite = "0.21"      # WebSocket implementation

# gRPC (optional)
tonic = "0.11"                  # gRPC framework
prost = "0.12"                  # Protocol Buffers

# Database
sqlx = { version = "0.7", features = ["postgres", "sqlite", "runtime-tokio", "json"] }
sea-orm = "0.12"                # ORM (optional)

# Redis
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }

# Authentication
jsonwebtoken = "9.3"            # JWT
argon2 = "0.5"                  # Password hashing
uuid = { version = "1.6", features = ["v4", "serde"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# HTTP client (to communicate with proxy instances)
reqwest = { version = "0.11", features = ["json"] }

# Observability
tracing = "0.1"
tracing-subscriber = "0.3"

# Configuration
config = "0.14"
dotenv = "0.15"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Validation
validator = { version = "0.18", features = ["derive"] }
```

### Frontend (Dashboard)
```json
{
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "react-router-dom": "^6.20.0",

    "@tanstack/react-query": "^5.0.0",
    "axios": "^1.6.0",

    "@radix-ui/react-*": "^1.0.0",
    "lucide-react": "^0.300.0",

    "recharts": "^2.10.0",
    "react-ace": "^10.1.0",
    "js-yaml": "^4.1.0",

    "zustand": "^4.4.0",
    "react-hook-form": "^7.48.0",

    "tailwindcss": "^3.3.0",
    "class-variance-authority": "^0.7.0",

    "date-fns": "^2.30.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/node": "^20.10.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.2.0",
    "eslint": "^8.55.0",
    "prettier": "^3.1.0"
  }
}
```

---

## 📦 Deployment Options

### 1. Docker Compose (Development)
```yaml
version: '3.8'
services:
  admin-api:
    build: ./admin-api
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://user:pass@postgres:5432/admin
      REDIS_URL: redis://redis:6379
    depends_on:
      - postgres
      - redis

  dashboard:
    build: ./dashboard
    ports:
      - "8080:80"
    environment:
      API_URL: http://admin-api:3000

  postgres:
    image: postgres:16
    volumes:
      - postgres-data:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    volumes:
      - redis-data:/data

volumes:
  postgres-data:
  redis-data:
```

### 2. Kubernetes
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: admin-api
spec:
  replicas: 2
  selector:
    matchLabels:
      app: admin-api
  template:
    metadata:
      labels:
        app: admin-api
    spec:
      containers:
      - name: admin-api
        image: highper-gateway/admin-api:latest
        ports:
        - containerPort: 3000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: admin-api-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            configMapKeyRef:
              name: admin-api-config
              key: redis-url
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 5
```

---

## 🔄 Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Project structure setup
- [ ] Database schema and migrations
- [ ] Basic REST API with Axum
- [ ] Authentication (JWT)
- [ ] Configuration CRUD endpoints
- [ ] Instance registration endpoint

### Phase 2: Core Features (Week 3-4)
- [ ] Proxy client implementation
- [ ] Metrics scraping and aggregation
- [ ] Backend control endpoints
- [ ] Cache management endpoints
- [ ] WebSocket server for real-time updates
- [ ] Audit logging

### Phase 3: Dashboard (Week 5-6)
- [ ] React app setup with Vite
- [ ] Main dashboard page
- [ ] Instances page
- [ ] Configuration editor
- [ ] Backends status page
- [ ] Authentication UI

### Phase 4: Advanced Features (Week 7-8)
- [ ] Real-time metrics charts
- [ ] Log viewer with search
- [ ] Deployment management
- [ ] Rollback functionality
- [ ] Multi-instance coordination
- [ ] Alert notifications

### Phase 5: Polish & Deploy (Week 9-10)
- [ ] Unit and integration tests
- [ ] API documentation (OpenAPI)
- [ ] User documentation
- [ ] Docker images
- [ ] Kubernetes manifests
- [ ] Performance optimization

---

## 🎯 Next Steps

1. **Review and approve this design**
2. **Choose communication protocol** (HTTP, gRPC, or both)
3. **Select database** (PostgreSQL or SQLite for simplicity)
4. **Start Phase 1 implementation**

Would you like me to:
1. Start implementing the Admin API backend?
2. Create the proxy-side admin agent/endpoints?
3. Build the React dashboard?
4. Set up the project structure first?

---

**Document Version:** 1.0
**Date:** October 29, 2025
**Status:** Design Complete - Ready for Implementation
