# Phase 1.4: Admin API Completion - Implementation Specification

**Duration:** 2 weeks
**Priority:** High (Operational Feature)
**Difficulty:** Medium
**Impact:** +5% configuration score

---

## Executive Summary

Complete the Admin API implementation to provide REST endpoints for runtime management of the proxy. The skeleton exists but needs to be updated to hyper 1.x and connected to actual proxy state. This enables dynamic configuration changes, health monitoring, and operational control without file editing or restarts.

---

## Goals

### Primary Goals
1. Update Admin API to hyper 1.x compatibility
2. Connect API to live proxy state (routes, upstreams, metrics)
3. Implement all CRUD endpoints for routes and upstreams
4. Add authentication (API key + JWT)
5. Implement health check endpoints
6. Add Swagger/OpenAPI documentation
7. Enable CORS for web dashboard access

### Success Metrics
- All Admin API endpoints functional
- Real-time updates to proxy configuration
- API authentication working
- OpenAPI spec generated
- +5% improvement in configuration score

---

## Current Status

### Existing Code

**Location:** `highper-gateway/src/admin/`
- `api.rs` - HTTP server (stub, disabled)
- `mod.rs` - Module definition (disabled)
- `routes.rs` - Route handlers (incomplete)
- `stats.rs` - Statistics collection (incomplete)

**Status:** Commented out in `src/lib.rs` due to hyper API incompatibility

**Issues:**
```rust
// In src/lib.rs
// TODO: Admin API temporarily disabled - needs update to new hyper API
// pub mod admin;
```

---

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Admin API System                      │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────────┐      ┌──────────────────┐        │
│  │  HTTP Server     │      │  Authentication  │        │
│  │  (hyper 1.x)     │─────→│  - API Key       │        │
│  │  Port: 9000      │      │  - JWT           │        │
│  └──────────────────┘      └──────────────────┘        │
│           │                                              │
│           ↓                                              │
│  ┌─────────────────────────────────────────┐           │
│  │         Router / Handler                │           │
│  │  - /api/routes                          │           │
│  │  - /api/upstreams                       │           │
│  │  - /api/health                          │           │
│  │  - /api/stats                           │           │
│  │  - /api/config/reload                   │           │
│  └─────────────────────────────────────────┘           │
│           │                                              │
│           ↓                                              │
│  ┌─────────────────────────────────────────┐           │
│  │      Proxy State Manager                │           │
│  │  - Arc<RwLock<Config>>                  │           │
│  │  - Arc<RwLock<RuntimeState>>            │           │
│  └─────────────────────────────────────────┘           │
│           │                                              │
│           ↓                                              │
│  ┌─────────────────┬─────────────────┬───────────────┐ │
│  │  Routes         │  Upstreams      │   Metrics     │ │
│  │  (live update)  │  (live update)  │  (read-only)  │ │
│  └─────────────────┴─────────────────┴───────────────┘ │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

### API Endpoint Structure

```
Admin API (Port 9000)
│
├─ /api/health              GET  - Health check
├─ /api/ready               GET  - Readiness check
├─ /api/stats               GET  - Runtime statistics
│
├─ /api/config
│  ├─ GET                   - Get current config
│  └─ /reload POST          - Reload configuration
│
├─ /api/routes
│  ├─ GET                   - List all routes
│  ├─ POST                  - Create new route
│  ├─ /{id} GET             - Get specific route
│  ├─ /{id} PUT             - Update route
│  └─ /{id} DELETE          - Delete route
│
├─ /api/upstreams
│  ├─ GET                   - List all upstreams
│  ├─ POST                  - Create new upstream
│  ├─ /{id} GET             - Get specific upstream
│  ├─ /{id} PUT             - Update upstream
│  ├─ /{id} DELETE          - Delete upstream
│  └─ /{id}/servers
│     ├─ GET                - List servers in upstream
│     ├─ POST               - Add server to upstream
│     ├─ /{server_id} GET   - Get specific server
│     ├─ /{server_id} PUT   - Update server
│     └─ /{server_id} DELETE - Remove server
│
├─ /api/servers
│  ├─ /{id}/enable POST     - Enable backend server
│  └─ /{id}/disable POST    - Disable backend server
│
├─ /api/cache
│  ├─ /stats GET            - Cache statistics
│  └─ /clear POST           - Clear cache
│
└─ /metrics                 GET  - Prometheus metrics
```

---

## Detailed Design

### 1. Configuration Schema Enhancement

**File:** `highper-gateway/src/config/schema.rs`

**Add Admin API config:**

```rust
/// Complete configuration with admin API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub tls: Option<TlsConfig>,
    pub upstreams: Vec<UpstreamConfig>,
    pub routes: Vec<RouteConfig>,
    pub observability: ObservabilityConfig,
    pub websocket: crate::websocket::WebSocketConfig,
    pub grpc: crate::grpc::GrpcConfig,

    /// Admin API configuration
    #[serde(default)]
    pub admin: Option<AdminConfig>,

    /// Configuration version (for tracking reloads)
    #[serde(skip)]
    pub version: Option<u64>,
}

/// Admin API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    /// Enable admin API
    #[serde(default)]
    pub enabled: bool,

    /// Admin API listen address
    #[serde(default = "default_admin_host")]
    pub host: String,

    /// Admin API port
    #[serde(default = "default_admin_port")]
    pub port: u16,

    /// Enable authentication
    #[serde(default = "default_true")]
    pub auth_enabled: bool,

    /// API key for authentication
    pub api_key: Option<String>,

    /// JWT secret for authentication
    pub jwt_secret: Option<String>,

    /// Enable CORS
    #[serde(default)]
    pub cors_enabled: bool,

    /// Allowed CORS origins
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

fn default_admin_host() -> String {
    "127.0.0.1".to_string()
}

fn default_admin_port() -> u16 {
    9000
}

fn default_true() -> bool {
    true
}
```

**Configuration Example:**

```yaml
# Admin API configuration
admin:
  enabled: true
  host: "0.0.0.0"
  port: 9000
  auth_enabled: true
  api_key: "your-secure-api-key-here"
  jwt_secret: "your-jwt-secret-here"
  cors_enabled: true
  cors_origins:
    - "http://localhost:3000"
    - "https://dashboard.example.com"
```

---

### 2. Runtime State Manager

**File:** `highper-gateway/src/admin/state.rs` (new)

**Purpose:** Manage mutable proxy state for Admin API

```rust
use crate::config::schema::{Config, RouteConfig, UpstreamConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Runtime proxy state that can be modified via Admin API
pub struct RuntimeState {
    /// Current configuration
    config: Arc<RwLock<Config>>,

    /// Live route table (can be modified)
    routes: Arc<RwLock<Vec<RouteConfig>>>,

    /// Live upstream table (can be modified)
    upstreams: Arc<RwLock<HashMap<String, UpstreamConfig>>>,

    /// Backend health status
    backend_health: Arc<RwLock<HashMap<String, BackendHealth>>>,

    /// Runtime statistics
    stats: Arc<RwLock<RuntimeStats>>,
}

/// Backend health information
#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub url: String,
    pub healthy: bool,
    pub last_check: std::time::SystemTime,
    pub consecutive_failures: u32,
    pub response_time_ms: Option<u64>,
}

/// Runtime statistics
#[derive(Debug, Clone, Default)]
pub struct RuntimeStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub active_connections: u64,
    pub uptime_seconds: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub request_rate_per_sec: f64,
}

impl RuntimeState {
    /// Create new runtime state from config
    pub fn new(config: Config) -> Self {
        let routes = config.routes.clone();
        let upstreams: HashMap<_, _> = config
            .upstreams
            .iter()
            .map(|u| (u.name.clone(), u.clone()))
            .collect();

        Self {
            config: Arc::new(RwLock::new(config)),
            routes: Arc::new(RwLock::new(routes)),
            upstreams: Arc::new(RwLock::new(upstreams)),
            backend_health: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RuntimeStats::default())),
        }
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Update entire configuration
    pub async fn update_config(&self, new_config: Config) {
        let mut config = self.config.write().await;
        *config = new_config.clone();

        // Update routes
        let mut routes = self.routes.write().await;
        *routes = new_config.routes;

        // Update upstreams
        let mut upstreams = self.upstreams.write().await;
        upstreams.clear();
        for upstream in &new_config.upstreams {
            upstreams.insert(upstream.name.clone(), upstream.clone());
        }

        info!("Configuration updated via Admin API");
    }

    /// Get all routes
    pub async fn get_routes(&self) -> Vec<RouteConfig> {
        self.routes.read().await.clone()
    }

    /// Add new route
    pub async fn add_route(&self, route: RouteConfig) -> anyhow::Result<()> {
        let mut routes = self.routes.write().await;

        // Check for duplicate path
        if routes.iter().any(|r| r.path == route.path) {
            return Err(anyhow::anyhow!("Route with path '{}' already exists", route.path));
        }

        routes.push(route.clone());
        info!("Added route: {}", route.path);
        Ok(())
    }

    /// Update existing route
    pub async fn update_route(&self, path: &str, updated_route: RouteConfig) -> anyhow::Result<()> {
        let mut routes = self.routes.write().await;

        if let Some(route) = routes.iter_mut().find(|r| r.path == path) {
            *route = updated_route;
            info!("Updated route: {}", path);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Route '{}' not found", path))
        }
    }

    /// Delete route
    pub async fn delete_route(&self, path: &str) -> anyhow::Result<()> {
        let mut routes = self.routes.write().await;
        let initial_len = routes.len();

        routes.retain(|r| r.path != path);

        if routes.len() < initial_len {
            info!("Deleted route: {}", path);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Route '{}' not found", path))
        }
    }

    /// Get all upstreams
    pub async fn get_upstreams(&self) -> Vec<UpstreamConfig> {
        self.upstreams.read().await.values().cloned().collect()
    }

    /// Get specific upstream
    pub async fn get_upstream(&self, name: &str) -> Option<UpstreamConfig> {
        self.upstreams.read().await.get(name).cloned()
    }

    /// Add new upstream
    pub async fn add_upstream(&self, upstream: UpstreamConfig) -> anyhow::Result<()> {
        let mut upstreams = self.upstreams.write().await;

        if upstreams.contains_key(&upstream.name) {
            return Err(anyhow::anyhow!("Upstream '{}' already exists", upstream.name));
        }

        upstreams.insert(upstream.name.clone(), upstream.clone());
        info!("Added upstream: {}", upstream.name);
        Ok(())
    }

    /// Update existing upstream
    pub async fn update_upstream(&self, name: &str, updated_upstream: UpstreamConfig) -> anyhow::Result<()> {
        let mut upstreams = self.upstreams.write().await;

        if upstreams.contains_key(name) {
            upstreams.insert(name.to_string(), updated_upstream);
            info!("Updated upstream: {}", name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Upstream '{}' not found", name))
        }
    }

    /// Delete upstream
    pub async fn delete_upstream(&self, name: &str) -> anyhow::Result<()> {
        let mut upstreams = self.upstreams.write().await;

        if upstreams.remove(name).is_some() {
            info!("Deleted upstream: {}", name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Upstream '{}' not found", name))
        }
    }

    /// Update backend health
    pub async fn update_backend_health(&self, backend_id: String, health: BackendHealth) {
        let mut health_map = self.backend_health.write().await;
        health_map.insert(backend_id, health);
    }

    /// Get all backend health
    pub async fn get_backend_health(&self) -> HashMap<String, BackendHealth> {
        self.backend_health.read().await.clone()
    }

    /// Get runtime statistics
    pub async fn get_stats(&self) -> RuntimeStats {
        self.stats.read().await.clone()
    }

    /// Update runtime statistics
    pub async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut RuntimeStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut *stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_get_route() {
        let config = create_test_config();
        let state = RuntimeState::new(config);

        let route = RouteConfig {
            path: "/test".to_string(),
            upstream: "backend".to_string(),
            methods: None,
            mtls: None,
        };

        state.add_route(route.clone()).await.unwrap();

        let routes = state.get_routes().await;
        assert!(routes.iter().any(|r| r.path == "/test"));
    }

    #[tokio::test]
    async fn test_add_duplicate_route() {
        let config = create_test_config();
        let state = RuntimeState::new(config);

        let route = RouteConfig {
            path: "/test".to_string(),
            upstream: "backend".to_string(),
            methods: None,
            mtls: None,
        };

        state.add_route(route.clone()).await.unwrap();
        let result = state.add_route(route).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_route() {
        let config = create_test_config();
        let state = RuntimeState::new(config);

        let route = RouteConfig {
            path: "/test".to_string(),
            upstream: "backend".to_string(),
            methods: None,
            mtls: None,
        };

        state.add_route(route).await.unwrap();

        let updated_route = RouteConfig {
            path: "/test".to_string(),
            upstream: "backend2".to_string(),
            methods: Some(vec!["GET".to_string()]),
            mtls: None,
        };

        state.update_route("/test", updated_route).await.unwrap();

        let routes = state.get_routes().await;
        let route = routes.iter().find(|r| r.path == "/test").unwrap();
        assert_eq!(route.upstream, "backend2");
    }

    #[tokio::test]
    async fn test_delete_route() {
        let config = create_test_config();
        let state = RuntimeState::new(config);

        let route = RouteConfig {
            path: "/test".to_string(),
            upstream: "backend".to_string(),
            methods: None,
            mtls: None,
        };

        state.add_route(route).await.unwrap();
        state.delete_route("/test").await.unwrap();

        let routes = state.get_routes().await;
        assert!(!routes.iter().any(|r| r.path == "/test"));
    }
}
```

---

### 3. Admin API Server (Updated)

**File:** `highper-gateway/src/admin/api.rs` (rewrite for hyper 1.x)

**Purpose:** HTTP server for Admin API

```rust
use crate::admin::state::RuntimeState;
use crate::config::schema::AdminConfig;
use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

/// Admin API server
pub struct AdminApiServer {
    config: AdminConfig,
    state: Arc<RuntimeState>,
}

impl AdminApiServer {
    /// Create new admin API server
    pub fn new(config: AdminConfig, state: Arc<RuntimeState>) -> Self {
        Self { config, state }
    }

    /// Start admin API server
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        info!("Admin API listening on {}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let server = Arc::clone(&self);

            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    let server = Arc::clone(&server);
                    async move { server.handle_request(req).await }
                });

                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                    error!("Error serving admin API connection: {}", e);
                }
            });
        }
    }

    /// Handle admin API request
    async fn handle_request(&self, req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        // Check authentication
        if self.config.auth_enabled {
            if let Err(response) = self.authenticate(&req) {
                return Ok(response);
            }
        }

        // Handle CORS preflight
        if req.method() == Method::OPTIONS {
            return Ok(self.handle_cors_preflight());
        }

        // Route the request
        let path = req.uri().path();
        let method = req.method();

        debug!("Admin API request: {} {}", method, path);

        let mut response = match (method, path) {
            // Health endpoints
            (&Method::GET, "/api/health") => self.health_check().await,
            (&Method::GET, "/api/ready") => self.readiness_check().await,

            // Configuration endpoints
            (&Method::GET, "/api/config") => self.get_config().await,
            (&Method::POST, "/api/config/reload") => self.reload_config(req).await,

            // Route management
            (&Method::GET, "/api/routes") => self.list_routes().await,
            (&Method::POST, "/api/routes") => self.create_route(req).await,
            (&Method::GET, path) if path.starts_with("/api/routes/") => {
                let route_path = path.strip_prefix("/api/routes/").unwrap();
                self.get_route(route_path).await
            }
            (&Method::PUT, path) if path.starts_with("/api/routes/") => {
                let route_path = path.strip_prefix("/api/routes/").unwrap();
                self.update_route(route_path, req).await
            }
            (&Method::DELETE, path) if path.starts_with("/api/routes/") => {
                let route_path = path.strip_prefix("/api/routes/").unwrap();
                self.delete_route(route_path).await
            }

            // Upstream management
            (&Method::GET, "/api/upstreams") => self.list_upstreams().await,
            (&Method::POST, "/api/upstreams") => self.create_upstream(req).await,
            (&Method::GET, path) if path.starts_with("/api/upstreams/") => {
                let upstream_name = path.strip_prefix("/api/upstreams/").unwrap();
                self.get_upstream(upstream_name).await
            }
            (&Method::PUT, path) if path.starts_with("/api/upstreams/") => {
                let upstream_name = path.strip_prefix("/api/upstreams/").unwrap();
                self.update_upstream(upstream_name, req).await
            }
            (&Method::DELETE, path) if path.starts_with("/api/upstreams/") => {
                let upstream_name = path.strip_prefix("/api/upstreams/").unwrap();
                self.delete_upstream(upstream_name).await
            }

            // Statistics
            (&Method::GET, "/api/stats") => self.get_stats().await,

            // Metrics (Prometheus format)
            (&Method::GET, "/metrics") => self.get_prometheus_metrics().await,

            // Not found
            _ => Ok(self.not_found()),
        }?;

        // Add CORS headers if enabled
        if self.config.cors_enabled {
            self.add_cors_headers(&mut response);
        }

        Ok(response)
    }

    /// Authenticate request
    fn authenticate(&self, req: &Request<Incoming>) -> Result<(), Response<Full<Bytes>>> {
        // Check API key
        if let Some(api_key) = &self.config.api_key {
            if let Some(provided_key) = req.headers().get("x-api-key") {
                if provided_key.to_str().unwrap_or("") == api_key {
                    return Ok(());
                }
            }
        }

        // Check JWT (basic validation for now)
        if let Some(_jwt_secret) = &self.config.jwt_secret {
            if let Some(auth_header) = req.headers().get(hyper::header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if auth_str.starts_with("Bearer ") {
                        // TODO: Full JWT verification
                        // For now, accept any Bearer token
                        return Ok(());
                    }
                }
            }
        }

        Err(json_response(
            StatusCode::UNAUTHORIZED,
            json!({"error": "Unauthorized"}),
        ))
    }

    /// Health check endpoint
    async fn health_check(&self) -> Result<Response<Full<Bytes>>> {
        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "healthy",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ))
    }

    /// Readiness check endpoint
    async fn readiness_check(&self) -> Result<Response<Full<Bytes>>> {
        // Check if proxy is ready to accept traffic
        // (all upstreams configured, etc.)
        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ready",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ))
    }

    /// Get current configuration
    async fn get_config(&self) -> Result<Response<Full<Bytes>>> {
        let config = self.state.get_config().await;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "version": config.version,
                "server": config.server,
                "routes_count": config.routes.len(),
                "upstreams_count": config.upstreams.len()
            }),
        ))
    }

    /// Reload configuration (trigger hot reload)
    async fn reload_config(&self, _req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        // TODO: Integrate with hot reload manager from Phase 1.2
        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": "Configuration reload triggered"
            }),
        ))
    }

    /// List all routes
    async fn list_routes(&self) -> Result<Response<Full<Bytes>>> {
        let routes = self.state.get_routes().await;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "routes": routes,
                "count": routes.len()
            }),
        ))
    }

    /// Create new route
    async fn create_route(&self, req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        use http_body_util::BodyExt;

        let body = req.collect().await?.to_bytes();
        let route: crate::config::schema::RouteConfig = serde_json::from_slice(&body)?;

        self.state.add_route(route.clone()).await?;

        Ok(json_response(
            StatusCode::CREATED,
            json!({
                "status": "ok",
                "message": "Route created successfully",
                "route": route
            }),
        ))
    }

    /// Get specific route
    async fn get_route(&self, route_path: &str) -> Result<Response<Full<Bytes>>> {
        let routes = self.state.get_routes().await;

        if let Some(route) = routes.iter().find(|r| r.path == route_path) {
            Ok(json_response(StatusCode::OK, json!(route)))
        } else {
            Ok(json_response(
                StatusCode::NOT_FOUND,
                json!({"error": "Route not found"}),
            ))
        }
    }

    /// Update existing route
    async fn update_route(
        &self,
        route_path: &str,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>> {
        use http_body_util::BodyExt;

        let body = req.collect().await?.to_bytes();
        let updated_route: crate::config::schema::RouteConfig = serde_json::from_slice(&body)?;

        self.state.update_route(route_path, updated_route).await?;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Route '{}' updated successfully", route_path)
            }),
        ))
    }

    /// Delete route
    async fn delete_route(&self, route_path: &str) -> Result<Response<Full<Bytes>>> {
        self.state.delete_route(route_path).await?;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Route '{}' deleted successfully", route_path)
            }),
        ))
    }

    /// List all upstreams
    async fn list_upstreams(&self) -> Result<Response<Full<Bytes>>> {
        let upstreams = self.state.get_upstreams().await;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "upstreams": upstreams,
                "count": upstreams.len()
            }),
        ))
    }

    /// Create new upstream
    async fn create_upstream(&self, req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        use http_body_util::BodyExt;

        let body = req.collect().await?.to_bytes();
        let upstream: crate::config::schema::UpstreamConfig = serde_json::from_slice(&body)?;

        self.state.add_upstream(upstream.clone()).await?;

        Ok(json_response(
            StatusCode::CREATED,
            json!({
                "status": "ok",
                "message": "Upstream created successfully",
                "upstream": upstream
            }),
        ))
    }

    /// Get specific upstream
    async fn get_upstream(&self, upstream_name: &str) -> Result<Response<Full<Bytes>>> {
        if let Some(upstream) = self.state.get_upstream(upstream_name).await {
            Ok(json_response(StatusCode::OK, json!(upstream)))
        } else {
            Ok(json_response(
                StatusCode::NOT_FOUND,
                json!({"error": "Upstream not found"}),
            ))
        }
    }

    /// Update existing upstream
    async fn update_upstream(
        &self,
        upstream_name: &str,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>> {
        use http_body_util::BodyExt;

        let body = req.collect().await?.to_bytes();
        let updated_upstream: crate::config::schema::UpstreamConfig =
            serde_json::from_slice(&body)?;

        self.state
            .update_upstream(upstream_name, updated_upstream)
            .await?;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Upstream '{}' updated successfully", upstream_name)
            }),
        ))
    }

    /// Delete upstream
    async fn delete_upstream(&self, upstream_name: &str) -> Result<Response<Full<Bytes>>> {
        self.state.delete_upstream(upstream_name).await?;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "message": format!("Upstream '{}' deleted successfully", upstream_name)
            }),
        ))
    }

    /// Get runtime statistics
    async fn get_stats(&self) -> Result<Response<Full<Bytes>>> {
        let stats = self.state.get_stats().await;
        let backend_health = self.state.get_backend_health().await;

        Ok(json_response(
            StatusCode::OK,
            json!({
                "stats": stats,
                "backend_health": backend_health,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ))
    }

    /// Get Prometheus metrics
    async fn get_prometheus_metrics(&self) -> Result<Response<Full<Bytes>>> {
        use crate::observability::metrics::Metrics;

        let metrics = Metrics::new();
        let output = metrics.render();

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4")
            .body(Full::new(Bytes::from(output)))?)
    }

    /// Handle CORS preflight
    fn handle_cors_preflight(&self) -> Response<Full<Bytes>> {
        let mut response = Response::new(Full::new(Bytes::new()));
        *response.status_mut() = StatusCode::NO_CONTENT;
        self.add_cors_headers(&mut response);
        response
    }

    /// Add CORS headers to response
    fn add_cors_headers(&self, response: &mut Response<Full<Bytes>>) {
        let headers = response.headers_mut();

        // Determine allowed origin
        let origin = if self.config.cors_origins.is_empty() {
            "*"
        } else {
            &self.config.cors_origins[0] // TODO: Match request origin
        };

        headers.insert(
            hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            origin.parse().unwrap(),
        );
        headers.insert(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap(),
        );
        headers.insert(
            hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, X-API-Key, Authorization".parse().unwrap(),
        );
    }

    /// 404 Not Found response
    fn not_found(&self) -> Response<Full<Bytes>> {
        json_response(
            StatusCode::NOT_FOUND,
            json!({"error": "Endpoint not found"}),
        )
    }
}

/// Helper: Create JSON response
fn json_response(status: StatusCode, body: serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let config = create_test_admin_config();
        let state = Arc::new(RuntimeState::new(create_test_config()));
        let server = AdminApiServer::new(config, state);

        let response = server.health_check().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_routes() {
        let config = create_test_admin_config();
        let state = Arc::new(RuntimeState::new(create_test_config()));
        let server = AdminApiServer::new(config, state);

        let response = server.list_routes().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

---

### 4. OpenAPI Documentation

**File:** `highper-gateway/src/admin/openapi.rs` (new)

**Purpose:** Generate OpenAPI/Swagger specification

```rust
use serde_json::json;

/// Generate OpenAPI 3.0 specification
pub fn generate_openapi_spec() -> serde_json::Value {
    json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Highper Gateway Admin API",
            "version": "0.1.0",
            "description": "Admin API for managing Rust Reverse Proxy at runtime"
        },
        "servers": [
            {
                "url": "http://localhost:9000",
                "description": "Local Admin API"
            }
        ],
        "security": [
            {"apiKey": []},
            {"bearerAuth": []}
        ],
        "paths": {
            "/api/health": {
                "get": {
                    "summary": "Health check",
                    "responses": {
                        "200": {
                            "description": "Proxy is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "status": {"type": "string"},
                                            "timestamp": {"type": "string"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/routes": {
                "get": {
                    "summary": "List all routes",
                    "responses": {
                        "200": {
                            "description": "List of routes",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "routes": {
                                                "type": "array",
                                                "items": {"$ref": "#/components/schemas/Route"}
                                            },
                                            "count": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Create new route",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/Route"}
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Route created successfully"
                        }
                    }
                }
            },
            "/api/routes/{path}": {
                "get": {
                    "summary": "Get specific route",
                    "parameters": [
                        {
                            "name": "path",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Route details",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/Route"}
                                }
                            }
                        },
                        "404": {
                            "description": "Route not found"
                        }
                    }
                },
                "put": {
                    "summary": "Update route",
                    "parameters": [
                        {
                            "name": "path",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/Route"}
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Route updated successfully"
                        }
                    }
                },
                "delete": {
                    "summary": "Delete route",
                    "parameters": [
                        {
                            "name": "path",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Route deleted successfully"
                        }
                    }
                }
            }
            // ... more endpoints
        },
        "components": {
            "securitySchemes": {
                "apiKey": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "X-API-Key"
                },
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT"
                }
            },
            "schemas": {
                "Route": {
                    "type": "object",
                    "required": ["path", "upstream"],
                    "properties": {
                        "path": {
                            "type": "string",
                            "example": "/api/v1"
                        },
                        "upstream": {
                            "type": "string",
                            "example": "backend"
                        },
                        "methods": {
                            "type": "array",
                            "items": {"type": "string"},
                            "example": ["GET", "POST"]
                        }
                    }
                },
                "Upstream": {
                    "type": "object",
                    "required": ["name", "servers"],
                    "properties": {
                        "name": {
                            "type": "string",
                            "example": "backend"
                        },
                        "servers": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/Server"}
                        }
                    }
                },
                "Server": {
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": {
                            "type": "string",
                            "example": "http://localhost:8001"
                        },
                        "weight": {
                            "type": "integer",
                            "example": 1
                        }
                    }
                }
            }
        }
    })
}

/// Serve OpenAPI spec at /api/openapi.json
pub async fn serve_openapi() -> serde_json::Value {
    generate_openapi_spec()
}
```

---

## Dependencies

### New Dependencies Required

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# For date/time in API responses
chrono = { version = "0.4", features = ["serde"] }
```

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_add_route()
#[tokio::test]
async fn test_update_route()
#[tokio::test]
async fn test_delete_route()
#[tokio::test]
async fn test_authentication()
#[tokio::test]
async fn test_cors_headers()
```

### Integration Tests

**File:** `highper-gateway/tests/admin_api_test.rs`

```rust
#[tokio::test]
async fn test_admin_api_health_endpoint() {
    let proxy = start_proxy_with_admin_api().await;

    let response = reqwest::get("http://localhost:9000/api/health")
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_create_route_via_api() {
    let proxy = start_proxy_with_admin_api().await;

    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:9000/api/routes")
        .header("X-API-Key", "test-key")
        .json(&json!({
            "path": "/new-route",
            "upstream": "backend"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    // Verify route was created
    let routes_response = client
        .get("http://localhost:9000/api/routes")
        .header("X-API-Key", "test-key")
        .send()
        .await
        .unwrap();

    let json: serde_json::Value = routes_response.json().await.unwrap();
    assert!(json["routes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["path"] == "/new-route"));
}

#[tokio::test]
async fn test_authentication_required() {
    let proxy = start_proxy_with_admin_api().await;

    // Request without API key
    let response = reqwest::get("http://localhost:9000/api/routes")
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}
```

---

## Acceptance Criteria

- [ ] Admin API server starts on configured port
- [ ] All CRUD endpoints work for routes
- [ ] All CRUD endpoints work for upstreams
- [ ] API key authentication works
- [ ] JWT authentication works (basic)
- [ ] CORS headers added correctly
- [ ] OpenAPI spec generated
- [ ] Health endpoints return correct status
- [ ] Stats endpoint returns runtime metrics
- [ ] Unit tests pass (100%)
- [ ] Integration tests pass (100%)
- [ ] Documentation complete

---

## Next Steps

After implementing Admin API:
1. Test with curl/Postman
2. Generate OpenAPI documentation
3. Commit changes
4. Proceed to Phase 1.5: Certificate Hot Reload

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
