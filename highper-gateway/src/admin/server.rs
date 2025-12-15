//! Admin API HTTP server implementation
//!
//! Provides REST API endpoints for managing the reverse proxy runtime

use crate::config::{AdminConfig, Config, ReloadTrigger};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{header, Method, Request, Response, StatusCode};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

/// JWT claims for admin API authentication
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    /// Subject (user ID or name)
    sub: String,
    /// Issued at (Unix timestamp)
    iat: usize,
    /// Expiration time (Unix timestamp)
    exp: usize,
}

/// Admin API server
pub struct AdminServer {
    config: AdminConfig,
    proxy_config: Arc<RwLock<Config>>,
    proxy_state: Option<Arc<crate::state::ProxyState>>,
    reload_tx: Option<mpsc::UnboundedSender<ReloadTrigger>>,
    hostname_router: Option<Arc<crate::gateway::routing::HostnameRouter>>,
    auth_db: Option<Arc<crate::admin::auth::AuthDb>>,
    route_manager: Arc<crate::admin::RouteManager>,
}

impl AdminServer {
    /// Create new admin server
    pub fn new(config: AdminConfig, proxy_config: Arc<RwLock<Config>>) -> Self {
        Self {
            config,
            proxy_config,
            proxy_state: None,
            reload_tx: None,
            hostname_router: None,
            auth_db: None,
            route_manager: Arc::new(crate::admin::RouteManager::new()),
        }
    }

    /// Create new admin server with state
    pub fn with_state(
        config: AdminConfig,
        proxy_config: Arc<RwLock<Config>>,
        proxy_state: Arc<crate::state::ProxyState>,
    ) -> Self {
        Self {
            config,
            proxy_config,
            proxy_state: Some(proxy_state),
            reload_tx: None,
            hostname_router: None,
            auth_db: None,
            route_manager: Arc::new(crate::admin::RouteManager::new()),
        }
    }

    /// Create new admin server with reload trigger
    pub fn with_reload_trigger(
        config: AdminConfig,
        proxy_config: Arc<RwLock<Config>>,
        reload_tx: mpsc::UnboundedSender<ReloadTrigger>,
    ) -> Self {
        Self {
            config,
            proxy_config,
            proxy_state: None,
            reload_tx: Some(reload_tx),
            hostname_router: None,
            auth_db: None,
            route_manager: Arc::new(crate::admin::RouteManager::new()),
        }
    }

    /// Create new admin server with state and reload trigger
    pub fn with_state_and_reload(
        config: AdminConfig,
        proxy_config: Arc<RwLock<Config>>,
        proxy_state: Arc<crate::state::ProxyState>,
        reload_tx: mpsc::UnboundedSender<ReloadTrigger>,
    ) -> Self {
        Self {
            config,
            proxy_config,
            proxy_state: Some(proxy_state),
            reload_tx: Some(reload_tx),
            hostname_router: None,
            auth_db: None,
            route_manager: Arc::new(crate::admin::RouteManager::new()),
        }
    }

    /// Set the hostname router (builder pattern)
    pub fn with_hostname_router(mut self, router: Arc<crate::gateway::routing::HostnameRouter>) -> Self {
        self.hostname_router = Some(router);
        self
    }

    /// Set the authentication database (builder pattern)
    pub fn with_auth_db(mut self, auth_db: Arc<crate::admin::auth::AuthDb>) -> Self {
        self.auth_db = Some(auth_db);
        self
    }

    /// Run the admin API server
    pub async fn run(self) -> crate::Result<()> {
        let addr: SocketAddr = self.config.bind.parse()?;
        let listener = TcpListener::bind(addr).await?;

        info!("Admin API listening on {}", addr);
        info!("Admin API authentication: {}", self.config.auth_enabled);

        let server = Arc::new(self);

        loop {
            let (stream, remote_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            let server = Arc::clone(&server);

            tokio::spawn(async move {
                let io = hyper_util::rt::TokioIo::new(stream);

                let service = hyper::service::service_fn(move |req| {
                    let server = Arc::clone(&server);
                    async move { server.handle_request(req).await }
                });

                if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .serve_connection(io, service)
                .await
                {
                    error!("Error serving admin connection from {}: {}", remote_addr, e);
                }
            });
        }
    }

    /// Handle admin API request
    pub async fn handle_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        // Route the request first (before auth check)
        let path = req.uri().path().to_string();
        let method = req.method().clone();

        debug!("Admin API request: {} {}", method, path);

        // Handle CORS preflight
        if method == Method::OPTIONS {
            return Ok(self.handle_cors_preflight());
        }

        // Skip authentication for login endpoint
        let is_login_endpoint = method == Method::POST && path == "/api/auth/login";

        // Check authentication (skip for login endpoint)
        if self.config.auth_enabled && !is_login_endpoint {
            if let Err(response) = self.authenticate(&req) {
                return Ok(response);
            }
        }

        let response = match (&method, path.as_str()) {
            // Authentication endpoints
            (&Method::POST, "/api/auth/login") => self.handle_login(req).await,

            // User management endpoints
            (&Method::GET, "/api/users") => self.list_users().await,
            (&Method::POST, "/api/users") => self.create_user(req).await,

            // Health checks
            (&Method::GET, "/health") | (&Method::GET, "/api/health") => self.health_check().await,
            (&Method::GET, "/ready") | (&Method::GET, "/api/ready") => self.readiness_check().await,

            // Configuration endpoints
            (&Method::GET, "/api/config") => self.get_config().await,
            (&Method::POST, "/api/config/reload") => self.reload_config().await,

            // Stats
            (&Method::GET, "/api/stats") => self.get_stats().await,

            // Backend control
            (&Method::GET, "/api/backends") => self.list_backends().await,

            // Routes management
            (&Method::GET, "/api/routes") => self.list_routes().await,
            (&Method::POST, "/api/routes") => self.create_route(req).await,
            _ if method == Method::GET && path.starts_with("/api/routes/") && !path.contains("/metrics") => {
                let route_name = path.strip_prefix("/api/routes/").unwrap();
                self.get_route(route_name).await
            }
            _ if method == Method::PUT && path.starts_with("/api/routes/") => {
                let route_name = path.strip_prefix("/api/routes/").unwrap();
                self.update_route(route_name, req).await
            }
            _ if method == Method::DELETE && path.starts_with("/api/routes/") => {
                let route_name = path.strip_prefix("/api/routes/").unwrap();
                self.delete_route(route_name).await
            }

            // Upstreams (stub for now)
            (&Method::GET, "/api/upstreams") => self.list_upstreams().await,

            // Upstream health status (match paths with names)
            _ if method == Method::GET && path.starts_with("/api/upstreams/") && path.ends_with("/health") => {
                self.handle_upstream_health(&path).await
            }

            // Backend operations (match paths with IDs)
            _ if method == Method::GET && path.starts_with("/api/backends/") => {
                self.handle_backend_get(&path).await
            }
            _ if method == Method::POST && path.starts_with("/api/backends/") => {
                self.handle_backend_post(&path, req).await
            }

            // Cache management
            (&Method::GET, "/api/cache/stats") => self.get_cache_stats().await,
            (&Method::GET, "/api/cache/keys") => self.list_cache_keys(None).await,
            (&Method::POST, "/api/cache/clear") => self.clear_cache(req).await,
            (&Method::POST, "/api/cache/invalidate") => self.invalidate_cache(req).await,

            // Enhanced metrics
            (&Method::GET, "/api/metrics/routes") => self.get_route_metrics().await,
            (&Method::GET, "/api/metrics/backends") => self.get_backend_metrics().await,
            (&Method::GET, "/api/metrics/health") => self.get_health_history().await,
            (&Method::GET, "/metrics") => self.export_prometheus_metrics().await,

            // Compression endpoints
            (&Method::GET, "/api/compression/stats") => self.get_compression_stats().await,
            (&Method::GET, "/api/compression/compressors") => self.list_compressors().await,

            // Connection pool metrics
            (&Method::GET, "/api/pool/metrics") => self.get_pool_metrics().await,
            (&Method::POST, "/api/pool/reset") => self.reset_pool_metrics().await,

            // Connection pool metrics with query params
            _ if method == Method::GET && path.starts_with("/api/pool/host") => {
                self.get_host_pool_metrics(req).await
            }

            // Request metrics
            (&Method::GET, "/api/request-metrics/routes") => self.get_request_route_metrics().await,
            (&Method::GET, "/api/request-metrics/backends") => self.get_backend_metrics_api().await,
            (&Method::GET, "/api/metrics/response-time") => self.get_global_response_time().await,
            (&Method::POST, "/api/metrics/reset") => self.reset_request_metrics().await,

            // Request metrics with query params
            _ if method == Method::GET && path.starts_with("/api/request-metrics/route?") => {
                self.get_route_metric(req).await
            }
            _ if method == Method::GET && path.starts_with("/api/request-metrics/backend?") => {
                self.get_backend_metric(req).await
            }

            // User deletion with path parameter
            _ if method == Method::DELETE && path.starts_with("/api/users/") => {
                self.delete_user(&path).await
            }

            // Not found
            _ => self.not_found(),
        };

        // Add CORS headers if enabled
        let response = if self.config.cors_enabled {
            self.add_cors_headers(response)
        } else {
            response
        };

        Ok(response)
    }

    /// Authenticate request
    fn authenticate(&self, req: &Request<Incoming>) -> Result<(), Response<Full<Bytes>>> {
        // Check API keys
        if !self.config.api_keys.is_empty() {
            if let Some(provided_key) = req.headers().get("x-api-key") {
                if let Ok(key_str) = provided_key.to_str() {
                    if self.config.api_keys.iter().any(|k| k == key_str) {
                        return Ok(());
                    }
                }
            }
        }

        // Check JWT (if configured)
        if let Some(jwt_secret) = &self.config.jwt_secret {
            if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if auth_str.starts_with("Bearer ") {
                        let token = &auth_str[7..]; // Remove "Bearer " prefix

                        // Verify JWT token
                        let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());
                        let mut validation = Validation::new(Algorithm::HS256);
                        validation.validate_exp = true;

                        match decode::<JwtClaims>(token, &decoding_key, &validation) {
                            Ok(token_data) => {
                                debug!("JWT authentication successful for user: {}", token_data.claims.sub);
                                return Ok(());
                            }
                            Err(e) => {
                                debug!("JWT verification failed: {}", e);
                                // Continue to unauthorized response
                            }
                        }
                    }
                }
            }
        }

        Err(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(
                json!({"error": "Unauthorized"}).to_string(),
            )))
            .unwrap())
    }

    /// Health check endpoint
    async fn health_check(&self) -> Response<Full<Bytes>> {
        json_response(
            StatusCode::OK,
            json!({
                "status": "healthy",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        )
    }

    /// Readiness check endpoint
    async fn readiness_check(&self) -> Response<Full<Bytes>> {
        // Check if proxy config is loaded
        let config = self.proxy_config.read().await;
        let ready = !config.routes.is_empty() || !config.upstreams.is_empty();

        let status = if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };

        json_response(
            status,
            json!({
                "ready": ready,
                "routes_count": config.routes.len(),
                "upstreams_count": config.upstreams.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        )
    }

    /// Get current configuration
    async fn get_config(&self) -> Response<Full<Bytes>> {
        let config = self.proxy_config.read().await;

        json_response(
            StatusCode::OK,
            json!({
                "server": {
                    "bind": config.server.bind,
                    "workers": config.server.workers,
                },
                "routes_count": config.routes.len(),
                "upstreams_count": config.upstreams.len(),
                "tls_enabled": config.tls.is_some(),
                "observability": {
                    "metrics_enabled": config.observability.metrics.enabled,
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        )
    }

    /// Reload configuration
    async fn reload_config(&self) -> Response<Full<Bytes>> {
        // Check read-only mode
        if self.config.read_only {
            return json_response(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "Admin API is in read-only mode"
                }),
            );
        }

        // Trigger configuration reload if available
        if let Some(reload_tx) = &self.reload_tx {
            match reload_tx.send(ReloadTrigger::Manual) {
                Ok(_) => {
                    info!("Configuration reload triggered via Admin API");
                    json_response(
                        StatusCode::OK,
                        json!({
                            "status": "ok",
                            "message": "Configuration reload triggered successfully",
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    )
                }
                Err(e) => {
                    error!("Failed to trigger reload: {}", e);
                    json_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json!({
                            "error": "Failed to trigger configuration reload",
                            "message": e.to_string()
                        }),
                    )
                }
            }
        } else {
            // Hot reload not enabled
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Hot reload not enabled",
                    "message": "Configuration hot reload is not available. Restart the server to apply configuration changes."
                }),
            )
        }
    }

    /// Get runtime statistics
    async fn get_stats(&self) -> Response<Full<Bytes>> {
        let config = self.proxy_config.read().await;

        json_response(
            StatusCode::OK,
            json!({
                "routes": config.routes.len(),
                "upstreams": config.upstreams.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
                // TODO: Add more stats (requests, errors, latency, etc.)
            }),
        )
    }

    /// List all routes
    async fn list_routes(&self) -> Response<Full<Bytes>> {
        // If hostname router is available, use it (new routing system)
        if let Some(router) = &self.hostname_router {
            let routes_data = router.export_routes_summary();
            return json_response(
                StatusCode::OK,
                json!({
                    "routes": routes_data,
                    "source": "hostname_router"
                })
            );
        }

        // Fallback to old config-based routing
        let config = self.proxy_config.read().await;

        let routes: Vec<_> = config
            .routes
            .iter()
            .map(|route| {
                json!({
                    "name": route.name,
                    "upstream": route.upstream,
                })
            })
            .collect();

        json_response(
            StatusCode::OK,
            json!({
                "routes": routes,
                "source": "config"
            })
        )
    }

    /// Create a new route
    async fn create_route(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        use http_body_util::BodyExt;

        // Check read-only mode
        if self.config.read_only {
            return json_response(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "Server is in read-only mode"
                }),
            );
        }

        // Parse request body
        let body_bytes = match req.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": format!("Failed to read request body: {}", e)
                    }),
                );
            }
        };

        let route: crate::admin::RouteDefinition = match serde_json::from_slice(&body_bytes) {
            Ok(route) => route,
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": format!("Invalid route definition: {}", e)
                    }),
                );
            }
        };

        // Add route to manager
        match self.route_manager.add_route(route.clone()).await {
            Ok(()) => json_response(
                StatusCode::CREATED,
                json!({
                    "status": "ok",
                    "message": format!("Route '{}' created successfully", route.name),
                    "route": route
                }),
            ),
            Err(e) => json_response(
                StatusCode::CONFLICT,
                json!({
                    "error": e
                }),
            ),
        }
    }

    /// Get a specific route by name
    async fn get_route(&self, route_name: &str) -> Response<Full<Bytes>> {
        match self.route_manager.get_route(route_name).await {
            Some(route) => json_response(StatusCode::OK, json!(route)),
            None => json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "error": format!("Route '{}' not found", route_name)
                }),
            ),
        }
    }

    /// Update an existing route
    async fn update_route(&self, route_name: &str, req: Request<Incoming>) -> Response<Full<Bytes>> {
        use http_body_util::BodyExt;

        // Check read-only mode
        if self.config.read_only {
            return json_response(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "Server is in read-only mode"
                }),
            );
        }

        // Parse request body
        let body_bytes = match req.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": format!("Failed to read request body: {}", e)
                    }),
                );
            }
        };

        let route: crate::admin::RouteDefinition = match serde_json::from_slice(&body_bytes) {
            Ok(route) => route,
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": format!("Invalid route definition: {}", e)
                    }),
                );
            }
        };

        // Update route in manager
        match self.route_manager.update_route(route_name, route.clone()).await {
            Ok(()) => json_response(
                StatusCode::OK,
                json!({
                    "status": "ok",
                    "message": format!("Route '{}' updated successfully", route_name),
                    "route": route
                }),
            ),
            Err(e) => json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "error": e
                }),
            ),
        }
    }

    /// Delete a route
    async fn delete_route(&self, route_name: &str) -> Response<Full<Bytes>> {
        // Check read-only mode
        if self.config.read_only {
            return json_response(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "Server is in read-only mode"
                }),
            );
        }

        match self.route_manager.remove_route(route_name).await {
            Ok(()) => json_response(
                StatusCode::OK,
                json!({
                    "status": "ok",
                    "message": format!("Route '{}' deleted successfully", route_name)
                }),
            ),
            Err(e) => json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "error": e
                }),
            ),
        }
    }

    /// List all upstreams
    async fn list_upstreams(&self) -> Response<Full<Bytes>> {
        // If hostname router is available, use it (new routing system with health)
        if let Some(router) = &self.hostname_router {
            let upstreams_data = router.export_upstreams_summary();
            return json_response(
                StatusCode::OK,
                json!({
                    "upstreams": upstreams_data,
                    "source": "hostname_router"
                })
            );
        }

        // Fallback to old config-based upstreams
        let config = self.proxy_config.read().await;

        let upstreams: Vec<_> = config
            .upstreams
            .iter()
            .map(|upstream| {
                json!({
                    "name": upstream.name,
                    "servers_count": upstream.servers.len(),
                    "health_check_enabled": upstream.health_check.active.enabled,
                })
            })
            .collect();

        json_response(
            StatusCode::OK,
            json!({
                "upstreams": upstreams,
                "source": "config"
            })
        )
    }

    /// List all backends
    async fn list_backends(&self) -> Response<Full<Bytes>> {
        crate::admin::backends::list_backends(
            self.proxy_config.clone(),
            self.proxy_state.clone(),
        )
        .await
    }

    /// Handle backend GET requests (get specific backend)
    async fn handle_backend_get(&self, path: &str) -> Response<Full<Bytes>> {
        // Parse path: /api/backends/{id}
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 4 {
            return self.not_found();
        }

        let backend_id = parts[3];

        // Check for sub-operations
        if parts.len() > 4 {
            // Handle sub-paths if needed in future
            return self.not_found();
        }

        // Get backend details
        crate::admin::backends::get_backend(self.proxy_config.clone(), backend_id).await
    }

    /// Handle backend POST requests (enable, disable, drain, health-check)
    async fn handle_backend_post(
        &self,
        path: &str,
        req: Request<Incoming>,
    ) -> Response<Full<Bytes>> {
        // Parse path: /api/backends/{id}/{operation}
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 5 {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "Invalid request",
                    "message": "Operation required. Use: /api/backends/{id}/{enable|disable|drain|health-check}"
                }),
            );
        }

        let backend_id = parts[3];
        let operation = parts[4];

        // Read request body
        let body_bytes = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Failed to read request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Parse request body (optional)
        let control_request = if body_bytes.is_empty() {
            crate::admin::backends::BackendControlRequest {
                reason: None,
                drain_timeout_seconds: None,
            }
        } else {
            match serde_json::from_slice::<crate::admin::backends::BackendControlRequest>(&body_bytes) {
                Ok(req) => req,
                Err(e) => {
                    return json_response(
                        StatusCode::BAD_REQUEST,
                        json!({
                            "error": "Invalid JSON",
                            "message": e.to_string()
                        }),
                    );
                }
            }
        };

        // Execute operation
        match operation {
            "enable" => {
                crate::admin::backends::enable_backend(
                    self.proxy_config.clone(),
                    self.proxy_state.clone(),
                    backend_id,
                    control_request,
                )
                .await
            }
            "disable" => {
                crate::admin::backends::disable_backend(
                    self.proxy_config.clone(),
                    self.proxy_state.clone(),
                    backend_id,
                    control_request,
                )
                .await
            }
            "drain" => {
                crate::admin::backends::drain_backend(
                    self.proxy_config.clone(),
                    self.proxy_state.clone(),
                    backend_id,
                    control_request,
                )
                .await
            }
            "health-check" => {
                crate::admin::backends::force_health_check(self.proxy_config.clone(), backend_id)
                    .await
            }
            _ => json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "Invalid operation",
                    "message": format!("Unknown operation: {}. Valid operations: enable, disable, drain, health-check", operation)
                }),
            ),
        }
    }

    /// Get cache statistics
    async fn get_cache_stats(&self) -> Response<Full<Bytes>> {
        crate::admin::cache::get_cache_stats(self.proxy_state.clone()).await
    }

    /// List cache keys
    async fn list_cache_keys(&self, pattern: Option<String>) -> Response<Full<Bytes>> {
        crate::admin::cache::list_cache_keys(self.proxy_state.clone(), pattern).await
    }

    /// Clear cache
    async fn clear_cache(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        // Read request body
        let body_bytes = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Failed to read request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Parse request body (optional)
        let clear_request = if body_bytes.is_empty() {
            crate::admin::cache::ClearCacheRequest {
                pattern: None,
                clear_local: true,
                clear_distributed: true,
            }
        } else {
            match serde_json::from_slice::<crate::admin::cache::ClearCacheRequest>(&body_bytes) {
                Ok(req) => req,
                Err(e) => {
                    return json_response(
                        StatusCode::BAD_REQUEST,
                        json!({
                            "error": "Invalid request body",
                            "message": e.to_string()
                        }),
                    );
                }
            }
        };

        crate::admin::cache::clear_cache(self.proxy_state.clone(), clear_request).await
    }

    /// Invalidate cache keys
    async fn invalidate_cache(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        // Read request body
        let body_bytes = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Failed to read request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Parse request body
        let invalidate_request = match serde_json::from_slice::<crate::admin::cache::InvalidateCacheRequest>(&body_bytes) {
            Ok(req) => req,
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Invalid request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        crate::admin::cache::invalidate_cache_keys(self.proxy_state.clone(), invalidate_request).await
    }

    /// Get route metrics
    async fn get_route_metrics(&self) -> Response<Full<Bytes>> {
        crate::admin::metrics::get_route_metrics(self.proxy_state.clone()).await
    }

    /// Get backend metrics
    async fn get_backend_metrics(&self) -> Response<Full<Bytes>> {
        crate::admin::metrics::get_backend_metrics(self.proxy_state.clone()).await
    }

    /// Get health check history
    async fn get_health_history(&self) -> Response<Full<Bytes>> {
        crate::admin::metrics::get_health_history(None).await
    }

    /// Export Prometheus metrics
    async fn export_prometheus_metrics(&self) -> Response<Full<Bytes>> {
        crate::admin::metrics::export_prometheus_metrics(self.proxy_state.clone()).await
    }

    /// Handle login
    async fn handle_login(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        // Check if auth database is available
        let auth_db = match &self.auth_db {
            Some(db) => db,
            None => {
                return json_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    json!({
                        "error": "Authentication not configured",
                        "message": "Authentication database is not available"
                    }),
                );
            }
        };

        // Read request body
        let body_bytes = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Failed to read request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Parse login request
        let login_request = match serde_json::from_slice::<crate::admin::auth::LoginRequest>(&body_bytes) {
            Ok(req) => req,
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Invalid request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Attempt login
        match auth_db.login(login_request).await {
            Ok(response) => json_response(StatusCode::OK, serde_json::to_value(response).unwrap()),
            Err(e) => e.into(),
        }
    }

    /// List all users
    async fn list_users(&self) -> Response<Full<Bytes>> {
        let auth_db = match &self.auth_db {
            Some(db) => db,
            None => {
                return json_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    json!({
                        "error": "Authentication not configured"
                    }),
                );
            }
        };

        match auth_db.list_users().await {
            Ok(users) => json_response(
                StatusCode::OK,
                json!({
                    "users": users,
                    "count": users.len()
                }),
            ),
            Err(e) => e.into(),
        }
    }

    /// Create a new user
    async fn create_user(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        let auth_db = match &self.auth_db {
            Some(db) => db,
            None => {
                return json_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    json!({
                        "error": "Authentication not configured"
                    }),
                );
            }
        };

        // Read request body
        let body_bytes = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Failed to read request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        // Parse create user request
        let create_request = match serde_json::from_slice::<crate::admin::auth::CreateUserRequest>(&body_bytes) {
            Ok(req) => req,
            Err(e) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "Invalid request body",
                        "message": e.to_string()
                    }),
                );
            }
        };

        match auth_db.create_user(create_request).await {
            Ok(user) => json_response(StatusCode::CREATED, serde_json::to_value(user).unwrap()),
            Err(e) => e.into(),
        }
    }

    /// Delete a user
    async fn delete_user(&self, path: &str) -> Response<Full<Bytes>> {
        let auth_db = match &self.auth_db {
            Some(db) => db,
            None => {
                return json_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    json!({
                        "error": "Authentication not configured"
                    }),
                );
            }
        };

        // Parse path: /api/users/{username}
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 4 {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "Invalid request",
                    "message": "Username required in path"
                }),
            );
        }

        let username = parts[3];

        match auth_db.delete_user(username).await {
            Ok(_) => json_response(
                StatusCode::OK,
                json!({
                    "message": "User deleted successfully",
                    "username": username
                }),
            ),
            Err(e) => e.into(),
        }
    }

    /// 404 Not Found
    fn not_found(&self) -> Response<Full<Bytes>> {
        json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": "Not found",
                "message": "The requested endpoint does not exist"
            }),
        )
    }

    /// Handle CORS preflight
    fn handle_cors_preflight(&self) -> Response<Full<Bytes>> {
        let mut response = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Full::new(Bytes::new()))
            .unwrap();

        if self.config.cors_enabled {
            let headers = response.headers_mut();
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse()
                    .unwrap(),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                "Content-Type, Authorization, X-API-Key"
                    .parse()
                    .unwrap(),
            );
            headers.insert(header::ACCESS_CONTROL_MAX_AGE, "86400".parse().unwrap());

            if !self.config.cors_origins.is_empty() {
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    self.config.cors_origins[0].parse().unwrap(),
                );
            } else {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
            }
        }

        response
    }

    /// Get compression statistics
    async fn get_compression_stats(&self) -> Response<Full<Bytes>> {
        use crate::middleware::compression::GLOBAL_COMPRESSOR_REGISTRY;

        let compressors = GLOBAL_COMPRESSOR_REGISTRY.list_detailed();
        let mut stats_map = serde_json::Map::new();

        for compressor_info in compressors {
            if let Some(compressor) = GLOBAL_COMPRESSOR_REGISTRY.get(&compressor_info.encoding) {
                let stats = compressor.stats();
                stats_map.insert(
                    compressor_info.name.clone(),
                    json!({
                        "encoding": compressor_info.encoding,
                        "quality": compressor_info.quality,
                        "available": compressor_info.available,
                        "total_compressions": stats.total_compressions,
                        "total_bytes_in": stats.total_bytes_in,
                        "total_bytes_out": stats.total_bytes_out,
                        "compression_ratio": stats.calculate_ratio(),
                        "percentage_saved": (1.0 - stats.calculate_ratio()) * 100.0,
                        "errors": stats.errors,
                    }),
                );
            }
        }

        json_response(
            StatusCode::OK,
            json!({
                "compression_stats": stats_map,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        )
    }

    /// List available compressors
    async fn list_compressors(&self) -> Response<Full<Bytes>> {
        use crate::middleware::compression::GLOBAL_COMPRESSOR_REGISTRY;

        let compressors = GLOBAL_COMPRESSOR_REGISTRY.list_detailed();
        let compressor_list: Vec<_> = compressors
            .into_iter()
            .map(|info| {
                json!({
                    "name": info.name,
                    "encoding": info.encoding,
                    "quality": info.quality,
                    "available": info.available,
                })
            })
            .collect();

        json_response(
            StatusCode::OK,
            json!({
                "compressors": compressor_list,
                "count": compressor_list.len(),
            }),
        )
    }

    /// Get global connection pool metrics
    async fn get_pool_metrics(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(pool_metrics) = state.pool_metrics() {
                match crate::admin::pool::get_pool_metrics(pool_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        error!("Failed to get pool metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get pool metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Pool metrics not initialized",
                        "message": "Connection pool metrics tracking is not enabled"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available",
                    "message": "Admin server running without proxy state"
                }),
            )
        }
    }

    /// Get connection pool metrics for a specific host
    async fn get_host_pool_metrics(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(pool_metrics) = state.pool_metrics() {
                match crate::admin::pool::get_host_pool_metrics(pool_metrics, req).await {
                    Ok(response) => response,
                    Err(e) => {
                        error!("Failed to get host pool metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get host pool metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Pool metrics not initialized",
                        "message": "Connection pool metrics tracking is not enabled"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available",
                    "message": "Admin server running without proxy state"
                }),
            )
        }
    }

    /// Reset connection pool metrics
    async fn reset_pool_metrics(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(pool_metrics) = state.pool_metrics() {
                match crate::admin::pool::reset_pool_metrics(pool_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        error!("Failed to reset pool metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to reset pool metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Pool metrics not initialized",
                        "message": "Connection pool metrics tracking is not enabled"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available",
                    "message": "Admin server running without proxy state"
                }),
            )
        }
    }

    /// Get all route request metrics
    async fn get_request_route_metrics(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::get_route_metrics(request_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to get route metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get route metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized",
                        "message": "Request metrics tracking is not enabled"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available",
                    "message": "Admin server running without proxy state"
                }),
            )
        }
    }

    /// Get specific route metric
    async fn get_route_metric(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::get_route_metric(request_metrics, req).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to get route metric: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get route metric",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available"
                }),
            )
        }
    }

    /// Get all backend metrics from request metrics
    async fn get_backend_metrics_api(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::get_backend_metrics(request_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to get backend metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get backend metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available"
                }),
            )
        }
    }

    /// Get specific backend metric from request metrics
    async fn get_backend_metric(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::get_backend_metric(request_metrics, req).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to get backend metric: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get backend metric",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available"
                }),
            )
        }
    }

    /// Get global response time histogram
    async fn get_global_response_time(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::get_global_response_time(request_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to get global response time: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to get global response time",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available"
                }),
            )
        }
    }

    /// Reset all request metrics
    async fn reset_request_metrics(&self) -> Response<Full<Bytes>> {
        if let Some(state) = &self.proxy_state {
            if let Some(request_metrics) = state.request_metrics() {
                match crate::admin::req_metrics::reset_request_metrics(request_metrics).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("Failed to reset request metrics: {}", e);
                        json_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "Failed to reset request metrics",
                                "details": e.to_string(),
                            }),
                        )
                    }
                }
            } else {
                json_response(
                    StatusCode::NOT_FOUND,
                    json!({
                        "error": "Request metrics not initialized"
                    }),
                )
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Proxy state not available"
                }),
            )
        }
    }

    /// Handle upstream health status request
    async fn handle_upstream_health(&self, path: &str) -> Response<Full<Bytes>> {
        // Parse path: /api/upstreams/{name}/health
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 5 || parts[4] != "health" {
            return self.not_found();
        }

        let upstream_name = parts[3];

        // Check if hostname router is available
        if let Some(router) = &self.hostname_router {
            match router.get_upstream_health_status(upstream_name) {
                Some(health_data) => {
                    json_response(
                        StatusCode::OK,
                        json!({
                            "upstream": upstream_name,
                            "health": health_data,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        })
                    )
                }
                None => {
                    json_response(
                        StatusCode::NOT_FOUND,
                        json!({
                            "error": "Upstream not found",
                            "upstream": upstream_name
                        })
                    )
                }
            }
        } else {
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "error": "Hostname router not available",
                    "message": "Health status tracking requires hostname router to be enabled"
                })
            )
        }
    }

    /// Add CORS headers to response
    fn add_cors_headers(&self, mut response: Response<Full<Bytes>>) -> Response<Full<Bytes>> {
        let headers = response.headers_mut();

        if !self.config.cors_origins.is_empty() {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                self.config.cors_origins[0].parse().unwrap(),
            );
        } else {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
        }

        headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            "true".parse().unwrap(),
        );

        response
    }
}

/// Helper function to create JSON response
fn json_response(status: StatusCode, body: serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_response() {
        let response = json_response(StatusCode::OK, json!({"test": "value"}));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }
}
