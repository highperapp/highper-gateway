//! Request handler

use crate::config::{Config, RouteConfig, UpstreamConfig};
use crate::gateway::routing::HostnameRouter;
use crate::http::{alt_svc, CollectedBody, collect_body_validated, ResponseBody};
use crate::middleware::{MiddlewareChain, compression_middleware::CompressionMiddleware};
use crate::middleware::waf::WafMiddleware;
use crate::observability::metrics::{record_request, record_upstream_request};
use crate::proxy::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
use crate::proxy::{Client, LoadBalancer};
use crate::state::ProxyState;
use crate::tls::ChallengeStore;
use crate::websocket::handler as ws_handler;
use crate::grpc::detector as grpc_detector;
use crate::grpc::handler as grpc_handler;
use crate::webserver::StaticFileHandler;
use crate::webserver::PhpFpmPool;
use crate::webserver::security::{PathValidator, validate_php_script, sanitize_fastcgi_param};
use crate::Result;
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Handle incoming requests
pub struct Handler {
    config: Arc<Config>,
    client: Client,
    upstreams: HashMap<String, Arc<Upstream>>,
    challenge_store: Option<ChallengeStore>,
    proxy_state: Option<Arc<ProxyState>>,
    middleware_chain: Arc<MiddlewareChain>,
    /// Hostname-based API Gateway router (optional, enabled via config)
    hostname_router: Option<Arc<HostnameRouter>>,
    /// Static file handler for web server features (optional)
    static_file_handler: Option<Arc<StaticFileHandler>>,
    /// PHP-FPM pool for PHP processing (optional)
    php_fpm_pool: Option<Arc<PhpFpmPool>>,
    /// WebSocket session manager (optional, enabled via config)
    ws_session_manager: Option<Arc<crate::websocket::SessionManager>>,
    /// WebSocket connection tracker (optional, enabled via config)
    ws_connection_tracker: Option<Arc<crate::websocket::ConnectionTracker>>,
    /// WebSocket keep-alive manager (optional, enabled via config)
    ws_keepalive_manager: Option<Arc<crate::websocket::KeepAliveManager>>,
    /// WebSocket recovery manager (optional, enabled via config)
    ws_recovery_manager: Option<Arc<crate::websocket::RecoveryManager>>,
    /// WebSocket shutdown coordinator (optional, enabled via config)
    ws_shutdown_coordinator: Option<Arc<crate::websocket::ShutdownCoordinator>>,
    /// Response cache for CDN features (optional, enabled via config)
    cache: Option<Arc<crate::gateway::cache::LocalCache>>,
    /// GraphQL gateway for schema stitching and federation (optional, enabled via config)
    graphql_gateway: Option<Arc<crate::gateway::graphql::GraphQLGateway>>,
}

/// Upstream server group with load balancing and circuit breaker
struct Upstream {
    #[allow(dead_code)]
    config: UpstreamConfig,
    load_balancer: LoadBalancer,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl Upstream {
    fn new(config: UpstreamConfig) -> Self {
        let algorithm = config.load_balancing.algorithm;
        let servers = config.servers.clone();
        let geoip_provider = config.load_balancing.geoip_provider;
        let geoip_path = config.load_balancing.geoip_db_path.as_deref();
        let load_balancer = LoadBalancer::with_geoip_config(algorithm, servers, geoip_provider, geoip_path);

        // Create circuit breaker for this upstream
        let cb_config = CircuitBreakerConfig::default();
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            config.name.clone(),
            cb_config,
        ));

        Self {
            config,
            load_balancer,
            circuit_breaker,
        }
    }

    /// Select next backend server using configured algorithm
    fn select_backend(&self, client_ip: Option<&str>) -> Option<String> {
        self.load_balancer
            .select(client_ip, None)
            .map(|backend| backend.server.url.clone())
    }

    /// Select backend for gRPC request using gRPC-specific load balancing
    fn select_backend_grpc(
        &self,
        policy: crate::grpc::GrpcLoadBalancingPolicy,
        metadata: &[(String, String)],
        affinity_key: Option<&str>,
    ) -> Option<String> {
        // Convert Vec<(String, String)> to HashMap for select_grpc
        let metadata_map: std::collections::HashMap<String, String> =
            metadata.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        self.load_balancer
            .select_grpc(policy, Some(&metadata_map), affinity_key)
            .map(|backend| backend.server.url.clone())
    }

    /// Get circuit breaker for this upstream
    fn circuit_breaker(&self) -> &Arc<CircuitBreaker> {
        &self.circuit_breaker
    }
}

impl Handler {
    /// Create a new handler
    pub fn new(config: Arc<Config>) -> Self {
        Self::with_challenge_store(config, None)
    }

    /// Create a new handler with challenge store
    pub fn with_challenge_store(config: Arc<Config>, challenge_store: Option<ChallengeStore>) -> Self {
        // Use connection pool config from server.performance settings
        let pool_config = config.server.performance.connection_pool.clone();
        let client = Client::with_config(None, Some(pool_config));

        // Build upstream map
        let mut upstreams = HashMap::new();
        for upstream_config in &config.upstreams {
            info!("Registered upstream: {}", upstream_config.name);
            upstreams.insert(
                upstream_config.name.clone(),
                Arc::new(Upstream::new(upstream_config.clone())),
            );
        }

        // Build middleware chain
        let mut middleware_chain = MiddlewareChain::new();

        // Add WAF middleware if enabled
        if let Some(schema_waf_config) = &config.waf {
            if schema_waf_config.enabled {
                // Convert schema::WafConfig to waf::WafConfig
                // Note: Schema and middleware have different config structures, so we use defaults
                // for engine-specific configs. A proper conversion layer should be added in the future.
                let waf_config = crate::middleware::waf::WafConfig {
                    enabled: schema_waf_config.enabled,
                    mode: schema_waf_config.mode,
                    block_mode: schema_waf_config.block_mode,
                    custom: schema_waf_config.custom.clone(),
                    coraza: None, // TODO: Convert schema::CorazaConfig to waf::CorazaConfig
                    modsecurity: None, // TODO: Convert schema::ModSecurityConfig to waf::ModSecurityConfig
                    aws: None, // TODO: Convert schema::AwsWafConfig to waf::AwsWafConfig
                    max_body_size: schema_waf_config.max_body_size,
                };

                match WafMiddleware::new(waf_config) {
                    Ok(waf) => {
                        info!("WAF middleware enabled (mode: {:?}, block_mode: {})",
                            schema_waf_config.mode, schema_waf_config.block_mode);
                        middleware_chain.add(waf);
                    }
                    Err(e) => {
                        warn!("Failed to initialize WAF middleware: {}. WAF will be disabled.", e);
                    }
                }
            }
        }

        // Add compression middleware
        middleware_chain.add(CompressionMiddleware::with_defaults());

        info!("Initialized middleware chain with {} middlewares: {:?}",
            middleware_chain.len(),
            middleware_chain.middleware_names()
        );

        // Initialize WebSocket managers if enabled
        let (ws_session_manager, ws_connection_tracker, ws_keepalive_manager,
             ws_recovery_manager, ws_shutdown_coordinator) =
            if config.websocket.enabled && config.websocket.track_connections {
                use std::time::Duration;

                let tracker = Arc::new(crate::websocket::ConnectionTracker::new(
                    Duration::from_secs(config.websocket.idle_timeout)
                ));

                let session_manager = if config.websocket.sticky_sessions {
                    Some(Arc::new(crate::websocket::SessionManager::new(
                        Duration::from_secs(config.websocket.session_timeout)
                    )))
                } else {
                    None
                };

                let keepalive = Some(Arc::new(crate::websocket::KeepAliveManager::new(
                    crate::websocket::KeepAliveConfig {
                        ping_interval: Duration::from_secs(config.websocket.ping_interval),
                        pong_timeout: Duration::from_secs(5),
                        max_missed_pongs: 3,
                        enabled: true,
                    },
                    tracker.clone()
                )));

                let recovery = Some(Arc::new(crate::websocket::RecoveryManager::new(
                    crate::websocket::RecoveryConfig::default(),
                    tracker.clone()
                )));

                let shutdown = Some(Arc::new(crate::websocket::ShutdownCoordinator::new(
                    tracker.clone(),
                    Duration::from_secs(30), // graceful timeout
                    Duration::from_secs(5),  // force timeout
                )));

                info!("Initialized WebSocket managers (sticky_sessions: {}, track_connections: {})",
                    config.websocket.sticky_sessions,
                    config.websocket.track_connections
                );

                (session_manager, Some(tracker), keepalive, recovery, shutdown)
            } else {
                (None, None, None, None, None)
            };

        // Initialize cache if enabled
        let cache = if let Some(cache_config) = &config.cache {
            if cache_config.enabled {
                use std::time::Duration;
                let local_cache = Arc::new(crate::gateway::cache::LocalCache::new(cache_config.default_ttl));

                // Start cleanup task (runs every 60 seconds)
                local_cache.clone().start_cleanup_task(Duration::from_secs(60));

                info!("Initialized response cache (TTL: {:?})", cache_config.default_ttl);
                Some(local_cache)
            } else {
                None
            }
        } else {
            None
        };

        Self {
            config,
            client,
            upstreams,
            challenge_store,
            proxy_state: None,
            middleware_chain: Arc::new(middleware_chain),
            hostname_router: None,
            static_file_handler: None,
            php_fpm_pool: None,
            ws_session_manager,
            ws_connection_tracker,
            ws_keepalive_manager,
            ws_recovery_manager,
            ws_shutdown_coordinator,
            cache,
            graphql_gateway: None,  // Initialize in with_graphql_gateway method
        }
    }

    /// Create a new handler with ProxyState
    pub fn with_state(
        config: Arc<Config>,
        challenge_store: Option<ChallengeStore>,
        proxy_state: Arc<ProxyState>,
    ) -> Self {
        // Use connection pool config from server.performance settings
        let pool_config = config.server.performance.connection_pool.clone();
        let client = Client::with_config(None, Some(pool_config));

        // Build upstream map
        let mut upstreams = HashMap::new();
        for upstream_config in &config.upstreams {
            info!("Registered upstream: {}", upstream_config.name);
            upstreams.insert(
                upstream_config.name.clone(),
                Arc::new(Upstream::new(upstream_config.clone())),
            );
        }

        // Build middleware chain
        let mut middleware_chain = MiddlewareChain::new();

        // Add WAF middleware if enabled
        if let Some(schema_waf_config) = &config.waf {
            if schema_waf_config.enabled {
                // Convert schema::WafConfig to waf::WafConfig
                // Note: Schema and middleware have different config structures, so we use defaults
                // for engine-specific configs. A proper conversion layer should be added in the future.
                let waf_config = crate::middleware::waf::WafConfig {
                    enabled: schema_waf_config.enabled,
                    mode: schema_waf_config.mode,
                    block_mode: schema_waf_config.block_mode,
                    custom: schema_waf_config.custom.clone(),
                    coraza: None, // TODO: Convert schema::CorazaConfig to waf::CorazaConfig
                    modsecurity: None, // TODO: Convert schema::ModSecurityConfig to waf::ModSecurityConfig
                    aws: None, // TODO: Convert schema::AwsWafConfig to waf::AwsWafConfig
                    max_body_size: schema_waf_config.max_body_size,
                };

                match WafMiddleware::new(waf_config) {
                    Ok(waf) => {
                        info!("WAF middleware enabled (mode: {:?}, block_mode: {})",
                            schema_waf_config.mode, schema_waf_config.block_mode);
                        middleware_chain.add(waf);
                    }
                    Err(e) => {
                        warn!("Failed to initialize WAF middleware: {}. WAF will be disabled.", e);
                    }
                }
            }
        }

        // Add compression middleware
        middleware_chain.add(CompressionMiddleware::with_defaults());

        info!("Initialized middleware chain with {} middlewares: {:?}",
            middleware_chain.len(),
            middleware_chain.middleware_names()
        );

        // Initialize WebSocket managers if enabled
        let (ws_session_manager, ws_connection_tracker, ws_keepalive_manager,
             ws_recovery_manager, ws_shutdown_coordinator) =
            if config.websocket.enabled && config.websocket.track_connections {
                use std::time::Duration;

                let tracker = Arc::new(crate::websocket::ConnectionTracker::new(
                    Duration::from_secs(config.websocket.idle_timeout)
                ));

                let session_manager = if config.websocket.sticky_sessions {
                    Some(Arc::new(crate::websocket::SessionManager::new(
                        Duration::from_secs(config.websocket.session_timeout)
                    )))
                } else {
                    None
                };

                let keepalive = Some(Arc::new(crate::websocket::KeepAliveManager::new(
                    crate::websocket::KeepAliveConfig {
                        ping_interval: Duration::from_secs(config.websocket.ping_interval),
                        pong_timeout: Duration::from_secs(5),
                        max_missed_pongs: 3,
                        enabled: true,
                    },
                    tracker.clone()
                )));

                let recovery = Some(Arc::new(crate::websocket::RecoveryManager::new(
                    crate::websocket::RecoveryConfig::default(),
                    tracker.clone()
                )));

                let shutdown = Some(Arc::new(crate::websocket::ShutdownCoordinator::new(
                    tracker.clone(),
                    Duration::from_secs(30), // graceful timeout
                    Duration::from_secs(5),  // force timeout
                )));

                info!("Initialized WebSocket managers (sticky_sessions: {}, track_connections: {})",
                    config.websocket.sticky_sessions,
                    config.websocket.track_connections
                );

                (session_manager, Some(tracker), keepalive, recovery, shutdown)
            } else {
                (None, None, None, None, None)
            };

        // Initialize cache if enabled
        let cache = if let Some(cache_config) = &config.cache {
            if cache_config.enabled {
                use std::time::Duration;
                let local_cache = Arc::new(crate::gateway::cache::LocalCache::new(cache_config.default_ttl));

                // Start cleanup task (runs every 60 seconds)
                local_cache.clone().start_cleanup_task(Duration::from_secs(60));

                info!("Initialized response cache (TTL: {:?})", cache_config.default_ttl);
                Some(local_cache)
            } else {
                None
            }
        } else {
            None
        };

        Self {
            config,
            client,
            upstreams,
            challenge_store,
            proxy_state: Some(proxy_state),
            middleware_chain: Arc::new(middleware_chain),
            hostname_router: None,
            static_file_handler: None,
            php_fpm_pool: None,
            ws_session_manager,
            ws_connection_tracker,
            ws_keepalive_manager,
            ws_recovery_manager,
            ws_shutdown_coordinator,
            cache,
            graphql_gateway: None,  // Initialize in with_graphql_gateway method
        }
    }

    /// Set hostname router for API Gateway features
    pub fn with_hostname_router(mut self, router: Arc<HostnameRouter>) -> Self {
        self.hostname_router = Some(router);
        self
    }

    /// Set GraphQL gateway for schema stitching and federation
    pub fn with_graphql_gateway(mut self, gateway: Arc<crate::gateway::graphql::GraphQLGateway>) -> Self {
        self.graphql_gateway = Some(gateway);
        self
    }

    /// Set static file handler for web server features
    pub fn with_static_file_handler(mut self, handler: Arc<StaticFileHandler>) -> Self {
        self.static_file_handler = Some(handler);
        self
    }

    /// Set PHP-FPM pool for PHP processing
    pub fn with_php_fpm_pool(mut self, pool: Arc<PhpFpmPool>) -> Self {
        self.php_fpm_pool = Some(pool);
        self
    }

    /// Handle an incoming HTTP request
    pub async fn handle(&self, req: Request<Incoming>) -> Result<Response<ResponseBody>> {
        let start = Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let path = uri.path();

        // Get host from either :authority (HTTP/2) or Host header (HTTP/1.1)
        // HTTP/2 uses :authority pseudo-header, HTTP/1.1 uses Host header
        let host = uri.authority()
            .map(|a| a.as_str())
            .or_else(|| req.headers()
                .get("host")
                .and_then(|h| h.to_str().ok()))
            .unwrap_or("")
            .to_string(); // Convert to owned String to allow moving req later

        // Extract client IP from headers (X-Forwarded-For or X-Real-IP)
        let client_ip = req.headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split(',').next())
            .or_else(|| {
                req.headers()
                    .get("x-real-ip")
                    .and_then(|h| h.to_str().ok())
            })
            .map(|s| s.to_string()); // Convert to owned String

        // Extract trace context from incoming headers for distributed tracing
        let _trace_context = crate::observability::tracing::extract_trace_context(req.headers());

        // Record HTTP request in tracing span
        crate::observability::tracing::record_http_request(
            method.as_str(),
            path,
            Some(&host),
            client_ip.as_deref(),
        );

        debug!("Received {} request for {} (Host: {}, Client IP: {:?})", method, path, host, client_ip);

        // Check for ACME HTTP-01 challenge
        if method == Method::GET && path.starts_with("/.well-known/acme-challenge/") {
            if let Some(challenge_store) = &self.challenge_store {
                let token = &path[28..]; // Skip "/.well-known/acme-challenge/"
                if let Some(key_auth) = challenge_store.get(token) {
                    debug!("Serving ACME challenge for token: {}", token);
                    let duration = start.elapsed().as_secs_f64();
                    record_request(method.as_str(), StatusCode::OK.as_u16(), duration);

                    // Record response in tracing
                    crate::observability::tracing::record_http_response(
                        StatusCode::OK,
                        duration * 1000.0,
                    );

                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/plain")
                        .body(ResponseBody::buffered(Bytes::from(key_auth)))?);
                } else {
                    debug!("No ACME challenge found for token: {}", token);
                }
            }
        }

        // Check for GraphQL request
        if let Some(graphql_gateway) = &self.graphql_gateway {
            // GraphQL requests are typically POST to /graphql
            if (path == "/graphql" || path.starts_with("/graphql/")) && method == Method::POST {
                debug!("Detected GraphQL request to {}", path);

                // Split request to access body
                let (parts, incoming_body) = req.into_parts();

                // Extract content length
                let content_length = parts.headers
                    .get("content-length")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());

                match collect_body_validated(incoming_body, content_length, 10 * 1024 * 1024).await {
                    Ok(body) => {
                        // Parse GraphQL request
                        match serde_json::from_slice::<crate::gateway::graphql::GraphQLRequest>(&body.bytes) {
                            Ok(graphql_req) => {
                                debug!("GraphQL query: {}", graphql_req.query);

                                // Handle GraphQL request
                                match graphql_gateway.handle_request(graphql_req).await {
                                    Ok(graphql_response) => {
                                        let response_json = serde_json::to_vec(&graphql_response)
                                            .unwrap_or_else(|_| b"{}".to_vec());

                                        let duration = start.elapsed().as_secs_f64();
                                        let status = if graphql_response.errors.is_some() {
                                            StatusCode::OK // GraphQL errors still return 200
                                        } else {
                                            StatusCode::OK
                                        };
                                        record_request(method.as_str(), status.as_u16(), duration);

                                        return Ok(Response::builder()
                                            .status(status)
                                            .header("content-type", "application/json")
                                            .body(ResponseBody::buffered(Bytes::from(response_json)))?);
                                    }
                                    Err(e) => {
                                        error!("GraphQL request failed: {}", e);
                                        let duration = start.elapsed().as_secs_f64();
                                        record_request(method.as_str(), StatusCode::INTERNAL_SERVER_ERROR.as_u16(), duration);

                                        return self.error_response(
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            "GraphQL request failed",
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse GraphQL request: {}", e);
                                let duration = start.elapsed().as_secs_f64();
                                record_request(method.as_str(), StatusCode::BAD_REQUEST.as_u16(), duration);

                                return self.error_response(
                                    StatusCode::BAD_REQUEST,
                                    "Invalid GraphQL request",
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read GraphQL request body: {}", e);
                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), StatusCode::BAD_REQUEST.as_u16(), duration);

                        return self.error_response(
                            StatusCode::BAD_REQUEST,
                            "Failed to read request body",
                        );
                    }
                }
            }

            // Handle GraphQL introspection query (GET to /graphql)
            if path == "/graphql" && method == Method::GET {
                debug!("Detected GraphQL introspection request");

                match graphql_gateway.handle_introspection().await {
                    Ok(graphql_response) => {
                        let response_json = serde_json::to_vec(&graphql_response)
                            .unwrap_or_else(|_| b"{}".to_vec());

                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), StatusCode::OK.as_u16(), duration);

                        return Ok(Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", "application/json")
                            .body(ResponseBody::buffered(Bytes::from(response_json)))?);
                    }
                    Err(e) => {
                        error!("GraphQL introspection failed: {}", e);
                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), StatusCode::INTERNAL_SERVER_ERROR.as_u16(), duration);

                        return self.error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "GraphQL introspection failed",
                        );
                    }
                }
            }
        }

        // Check for WebSocket upgrade request (BEFORE consuming req)
        if self.config.websocket.enabled && ws_handler::is_websocket_upgrade(&req) {
            debug!("Detected WebSocket upgrade request");
            info!("WebSocket upgrade detected for path: {}", path);

            // Extract session ID from cookie if sticky sessions enabled
            let existing_session_id = if self.ws_session_manager.is_some() {
                ws_handler::extract_session_id_from_cookie(&req, &self.config.websocket.session_cookie_name)
            } else {
                None
            };

            // Find matching route (with hostname router support)
            let upstream_name = self.find_route_async(&method, &host, path).await;

            match upstream_name {
                Some(upstream_name) => {
                    // Get upstream
                    if let Some(upstream) = self.upstreams.get(&upstream_name) {
                        // Use session ID as request key for consistent hashing (sticky sessions)
                        let request_key = existing_session_id.as_ref().map(|sid| sid.to_string());

                        // Select backend using session ID for consistency
                        let backend_selection = upstream.load_balancer.select(
                            client_ip.as_deref(),
                            request_key.as_deref()
                        );

                        if let Some(selected_backend) = backend_selection {
                            let backend_url = &selected_backend.server.url;

                            debug!("WebSocket backend selected: url={}", backend_url);

                            // Use hash of backend URL as index for tracking (since servers list is private)
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            backend_url.hash(&mut hasher);
                            let backend_idx = (hasher.finish() % 1000) as usize;

                            // Create or update session
                            let session_for_cookie = if let Some(session_mgr) = &self.ws_session_manager {
                                if existing_session_id.is_none() {
                                    // Create new session
                                    Some(session_mgr.create_session(backend_idx, client_ip.clone()))
                                } else {
                                    // Get existing session (updates last_activity)
                                    existing_session_id.and_then(|sid| session_mgr.get_session(&sid))
                                }
                            } else {
                                None
                            };

                            // Parse backend URL for WebSocket connection
                            let backend_ws_url = backend_url
                                .replace("http://", "ws://")
                                .replace("https://", "wss://");
                            let backend_ws_url = format!("{}{}", backend_ws_url.trim_end_matches('/'), path);

                            info!("Establishing WebSocket connection to backend: {}", backend_ws_url);

                            // Create WebSocket upgrade response with session cookie
                            let upgrade_response = if let Some(session) = session_for_cookie.as_ref() {
                                ws_handler::create_upgrade_response_with_session(
                                    &req,
                                    Some(&session.id),
                                    Some(&self.config.websocket.session_cookie_name),
                                    self.config.websocket.session_timeout,
                                )
                            } else {
                                ws_handler::create_upgrade_response(&req)
                            };

                            match upgrade_response {
                                Ok(response) => {
                                    info!("WebSocket upgrade response created for {}", path);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::SWITCHING_PROTOCOLS.as_u16(), duration);

                                    // Register connection if tracking enabled
                                    let conn_id = if let Some(tracker) = &self.ws_connection_tracker {
                                        let session_id = session_for_cookie.as_ref().map(|s| s.id);
                                        Some(tracker.register(backend_idx, session_id, client_ip.clone()))
                                    } else {
                                        None
                                    };

                                    // Clone managers for async task
                                    let tracker_clone = self.ws_connection_tracker.clone();
                                    let recovery_clone = self.ws_recovery_manager.clone();

                                    // Spawn async task for WebSocket proxying after upgrade
                                    tokio::spawn(async move {
                                        // Update connection state to Connecting
                                        if let (Some(cid), Some(tracker)) = (conn_id.as_ref(), tracker_clone.as_ref()) {
                                            tracker.update_state(cid, crate::websocket::ConnectionState::Connecting);
                                        }

                                        // Wait for the upgrade to complete
                                        match hyper::upgrade::on(req).await {
                                            Ok(upgraded) => {
                                                info!("Client WebSocket connection upgraded, connecting to backend");

                                                // Update connection state to Connected
                                                if let (Some(cid), Some(tracker)) = (conn_id.as_ref(), tracker_clone.as_ref()) {
                                                    tracker.update_state(cid, crate::websocket::ConnectionState::Connected);
                                                }

                                                // Wrap upgraded connection with TokioIo for AsyncRead/AsyncWrite
                                                use hyper_util::rt::TokioIo;
                                                let upgraded_io = TokioIo::new(upgraded);

                                                // Connect to backend WebSocket
                                                match tokio_tungstenite::connect_async(&backend_ws_url).await {
                                                    Ok((backend_ws, _)) => {
                                                        info!("Connected to backend WebSocket: {}", backend_ws_url);

                                                        // Record successful connection
                                                        if let Some(recovery) = recovery_clone.as_ref() {
                                                            recovery.record_success(backend_idx);
                                                        }

                                                        // Convert upgraded connection to WebSocket frames
                                                        let client_ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
                                                            upgraded_io,
                                                            tokio_tungstenite::tungstenite::protocol::Role::Server,
                                                            None,
                                                        ).await;

                                                        // Proxy bidirectionally with tracking
                                                        if let Err(e) = Self::proxy_websocket_streams(client_ws, backend_ws).await {
                                                            error!("WebSocket proxy error: {}", e);
                                                            if let (Some(cid), Some(tracker)) = (conn_id.as_ref(), tracker_clone.as_ref()) {
                                                                tracker.record_error(cid, format!("Proxy error: {}", e));
                                                            }
                                                        } else {
                                                            info!("WebSocket connection closed cleanly");
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to connect to backend WebSocket: {}", e);
                                                        if let Some(recovery) = recovery_clone.as_ref() {
                                                            recovery.record_failure(backend_idx, crate::websocket::WebSocketError::ConnectionRefused);
                                                        }
                                                    }
                                                }

                                                // Update connection state to Closing
                                                if let (Some(cid), Some(tracker)) = (conn_id.as_ref(), tracker_clone.as_ref()) {
                                                    tracker.update_state(cid, crate::websocket::ConnectionState::Closing);
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to upgrade client connection: {}", e);
                                                if let (Some(cid), Some(tracker)) = (conn_id.as_ref(), tracker_clone.as_ref()) {
                                                    tracker.record_error(cid, format!("Upgrade error: {}", e));
                                                }
                                            }
                                        }

                                        // Unregister connection
                                        if let (Some(cid), Some(tracker)) = (conn_id, tracker_clone.as_ref()) {
                                            tracker.unregister(&cid);
                                        }
                                    });

                                    return Ok(response);
                                }
                                Err(e) => {
                                    error!("Failed to create WebSocket upgrade response: {}", e);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::BAD_REQUEST.as_u16(), duration);
                                    return self.error_response(
                                        StatusCode::BAD_REQUEST,
                                        "Invalid WebSocket upgrade request",
                                    );
                                }
                            }
                        }
                    }
                }
                None => {
                    warn!("No route matched for WebSocket: {}", path);
                }
            }

            // If we get here, no backend was found
            let duration = start.elapsed().as_secs_f64();
            record_request(method.as_str(), StatusCode::SERVICE_UNAVAILABLE.as_u16(), duration);
            return self.error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "No backend available for WebSocket connection",
            );
        }

        // Check for gRPC request and store info for later use (BEFORE consuming req)
        let grpc_request_info = if self.config.grpc.enabled && grpc_detector::is_grpc_request(&req) {
            debug!("Detected gRPC request");
            if let Some(grpc_req) = grpc_detector::parse_grpc_request(&req) {
                info!("gRPC request: {} (service: {:?})", grpc_req.path,
                    grpc_detector::extract_service_name(&grpc_req.path));
                Some(grpc_req)
            } else {
                None
            }
        } else {
            None
        };

        // Check for webserver route (static file or PHP-FPM) BEFORE proxy routing
        // Try to find matching route to check for webserver configuration
        if let Some(route) = self.find_route(&method, &host, path) {
            // Check if this route has webserver configuration
            let has_webserver_config = route.static_files
                || route.php_fpm.is_some()
                || route.root.is_some();

            if has_webserver_config {
                debug!("Route {} has webserver configuration, processing as webserver request", route.name);

                // Handle as webserver request using route configuration
                // NOTE: This consumes req, so we can't fall through to proxy
                return self.handle_webserver_request(req, route, &method, &host, path, start).await;
            }
        }

        // Find matching route (with hostname router support)
        let upstream_name = self.find_route_async(&method, &host, path).await;

        match upstream_name {
            Some(upstream_name) => {
                debug!("Matched route to upstream: {}", upstream_name);

                // Check cache for GET requests (only cache safe, idempotent requests)
                if method == Method::GET && self.cache.is_some() {
                    let cache = self.cache.as_ref().unwrap();

                    // Generate cache key from method and URI
                    let cache_key = crate::gateway::cache::LocalCache::generate_key(
                        method.as_str(),
                        &uri.to_string(),
                        &[]
                    );

                    // Try to get from cache
                    if let Some(cached_entry) = cache.get(&cache_key) {
                        debug!("Cache HIT for {}", uri);

                        // Build response from cache
                        let mut response_builder = Response::builder()
                            .status(StatusCode::from_u16(cached_entry.status).unwrap_or(StatusCode::OK))
                            .header("x-cache", "HIT");

                        // Add cached headers
                        for (name, value) in cached_entry.headers {
                            response_builder = response_builder.header(name, value);
                        }

                        // Add Age header (how old is this cached entry)
                        let age_secs = cached_entry.created_at.elapsed().as_secs();
                        response_builder = response_builder.header("age", age_secs.to_string());

                        let cached_response = response_builder
                            .body(ResponseBody::buffered(cached_entry.body))?;

                        // Record metrics
                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), cached_entry.status, duration);

                        return Ok(cached_response);
                    } else {
                        debug!("Cache MISS for {}", uri);
                    }
                }

                // Get upstream
                if let Some(upstream) = self.upstreams.get(&upstream_name) {
                    // Check circuit breaker
                    let circuit_breaker = upstream.circuit_breaker();
                    if !circuit_breaker.allow_request() {
                        warn!("Circuit breaker OPEN for upstream: {}", upstream_name);
                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), StatusCode::SERVICE_UNAVAILABLE.as_u16(), duration);
                        return self.error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "Circuit breaker is open",
                        );
                    }

                    // Select backend server using load balancing algorithm
                    // Use gRPC-specific load balancing for gRPC requests
                    let backend_url = if let Some(ref grpc_req) = grpc_request_info {
                        // Use gRPC-specific load balancing
                        let grpc_config = &self.config.grpc.load_balancing;
                        let affinity_key = grpc_config.affinity_key.as_deref();
                        upstream.select_backend_grpc(
                            grpc_config.policy,
                            &grpc_req.metadata,
                            affinity_key,
                        )
                    } else {
                        // Use standard load balancing
                        upstream.select_backend(client_ip.as_deref())
                    };

                    if let Some(backend_url) = backend_url {
                        debug!("Selected backend: {}", backend_url);

                        // Check if this is a gRPC request that needs special handling
                        if let Some(ref grpc_req) = grpc_request_info {
                            // gRPC-specific proxying with streaming support
                            info!("Using gRPC-aware proxying for {}", grpc_req.path);

                            let grpc_req_clone = grpc_req.clone();
                            let backend_url_clone = backend_url.clone();

                            let result = circuit_breaker.execute(|| async move {
                                grpc_handler::proxy_grpc_request(&grpc_req_clone, &backend_url_clone, req).await
                            }).await;

                            match result {
                                Ok(response) => {
                                    // For gRPC, return streaming response directly (preserve trailers)
                                    let status = response.status();

                                    // Extract gRPC status from trailers if available
                                    let grpc_status = grpc_handler::extract_grpc_status(&response);
                                    if let Some(grpc_status) = grpc_status {
                                        debug!("gRPC response status: {:?}", grpc_status);
                                    }

                                    // Record metrics
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), status.as_u16(), duration);
                                    record_upstream_request(&upstream_name, status.as_u16(), duration);

                                    // Convert Incoming body to ResponseBody for return type compatibility
                                    // Incoming needs to be boxed and error type converted
                                    let (parts, body) = response.into_parts();
                                    use http_body_util::BodyExt as _;
                                    let boxed_body = body
                                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                                        .boxed_unsync();
                                    let response = Response::from_parts(parts, ResponseBody::Stream(boxed_body));
                                    Ok(response)
                                }
                                Err(CircuitBreakerError::Open) => {
                                    warn!("Circuit breaker opened for upstream: {}", upstream_name);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::SERVICE_UNAVAILABLE.as_u16(), duration);

                                    let grpc_error = grpc_handler::create_grpc_error_response(
                                        crate::grpc::GrpcStatusCode::Unavailable,
                                        "Circuit breaker is open",
                                    );
                                    // Convert Full<Bytes> body to ResponseBody
                                    let (parts, body) = grpc_error.into_parts();
                                    use http_body_util::BodyExt as _;
                                    let bytes = body.collect().await.unwrap().to_bytes();
                                    Ok(Response::from_parts(parts, ResponseBody::buffered(bytes)))
                                }
                                Err(CircuitBreakerError::Failure(e)) => {
                                    error!("gRPC backend request failed: {}", e);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::BAD_GATEWAY.as_u16(), duration);
                                    record_upstream_request(&upstream_name, StatusCode::BAD_GATEWAY.as_u16(), duration);

                                    let grpc_error = grpc_handler::create_grpc_error_response(
                                        crate::grpc::GrpcStatusCode::Unavailable,
                                        &format!("Backend request failed: {}", e),
                                    );
                                    // Convert Full<Bytes> body to ResponseBody
                                    let (parts, body) = grpc_error.into_parts();
                                    use http_body_util::BodyExt as _;
                                    let bytes = body.collect().await.unwrap().to_bytes();
                                    Ok(Response::from_parts(parts, ResponseBody::buffered(bytes)))
                                }
                            }
                        } else {
                            // Regular HTTP/HTTPS proxying
                            let client = self.client.clone();
                            let backend_url_clone = backend_url.clone();
                            let method_clone = method.clone();
                            let path_str = path.to_string();
                            let headers = req.headers().clone();

                            let result = circuit_breaker.execute(|| async {
                                client.forward(&backend_url_clone, method_clone.clone(), &path_str, headers.clone()).await
                            }).await;

                        match result {
                            Ok(response) => {
                                // Circuit breaker records success automatically
                                // Convert response body using BufferPool for efficient memory reuse
                                let status = response.status();
                                let headers = response.headers().clone();

                                // Use global buffer pool to avoid allocations
                                use crate::runtime::GLOBAL_BUFFER_POOL;
                                let mut buffer = GLOBAL_BUFFER_POOL.get(16384); // 16KB initial size

                                // Stream response body into pooled buffer
                                let mut body = response.into_body();
                                let body_result = async {
                                    while let Some(frame) = body.frame().await {
                                        match frame {
                                            Ok(frame) => {
                                                if let Some(chunk) = frame.data_ref() {
                                                    buffer.extend_from_slice(chunk);
                                                }
                                            }
                                            Err(e) => return Err(e),
                                        }
                                    }
                                    Ok(())
                                }.await;

                                match body_result {
                                    Ok(()) => {
                                        // Convert buffer to Bytes (zero-copy view)
                                        let body_bytes = buffer.freeze();

                                        let mut resp = Response::builder()
                                            .status(status);

                                        // Copy headers
                                        for (key, value) in headers.iter() {
                                            resp = resp.header(key, value);
                                        }

                                        let response = resp.body(ResponseBody::buffered(body_bytes.clone()))?;

                                        // Store in cache for GET requests with successful status
                                        if method == Method::GET && self.cache.is_some() && status.is_success() {
                                            let cache = self.cache.as_ref().unwrap();

                                            // Check Cache-Control header to respect caching directives
                                            let should_cache = if let Some(cache_control) = headers.get("cache-control") {
                                                if let Ok(cc_str) = cache_control.to_str() {
                                                    let cc_lower = cc_str.to_lowercase();
                                                    // Don't cache if no-store or no-cache
                                                    !cc_lower.contains("no-store") && !cc_lower.contains("no-cache")
                                                } else {
                                                    true
                                                }
                                            } else {
                                                true // No cache-control header, safe to cache
                                            };

                                            if should_cache {
                                                // Generate cache key
                                                let cache_key = crate::gateway::cache::LocalCache::generate_key(
                                                    method.as_str(),
                                                    &uri.to_string(),
                                                    &[]
                                                );

                                                // Convert headers to Vec<(String, String)>
                                                let header_vec: Vec<(String, String)> = headers
                                                    .iter()
                                                    .filter_map(|(name, value)| {
                                                        value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
                                                    })
                                                    .collect();

                                                // Create cache entry (use TTL from config)
                                                let ttl = if let Some(cache_config) = &self.config.cache {
                                                    cache_config.default_ttl
                                                } else {
                                                    std::time::Duration::from_secs(300) // Default 5 minutes
                                                };

                                                let cache_entry = crate::gateway::cache::CacheEntry {
                                                    body: body_bytes.clone(),
                                                    status: status.as_u16(),
                                                    headers: header_vec,
                                                    created_at: std::time::Instant::now(),
                                                    ttl,
                                                };

                                                cache.set(cache_key.clone(), cache_entry);
                                                debug!("Cached response for {} (key: {})", uri, cache_key);
                                            }
                                        }

                                        // Apply middleware chain (compression, etc.)
                                        let mut response = match self.middleware_chain.process_response(response).await {
                                            Ok(resp) => resp,
                                            Err(e) => {
                                                error!("Middleware processing failed: {}", e);
                                                // Fall through to original response if middleware fails
                                                return self.error_response(
                                                    StatusCode::INTERNAL_SERVER_ERROR,
                                                    "Middleware processing failed",
                                                );
                                            }
                                        };

                                        // Add Alt-Svc header to advertise HTTP/3 if enabled
                                        if self.config.server.http3.enabled {
                                            alt_svc::add_alt_svc_header(&mut response, self.config.server.http3.port);
                                        }

                                        // Record metrics
                                        let duration = start.elapsed().as_secs_f64();
                                        record_request(method.as_str(), status.as_u16(), duration);
                                        record_upstream_request(&upstream_name, status.as_u16(), duration);

                                        Ok(response)
                                    }
                                    Err(e) => {
                                        error!("Failed to read response body: {}", e);
                                        circuit_breaker.record_failure();
                                        let duration = start.elapsed().as_secs_f64();
                                        record_request(method.as_str(), StatusCode::BAD_GATEWAY.as_u16(), duration);
                                        record_upstream_request(&upstream_name, StatusCode::BAD_GATEWAY.as_u16(), duration);
                                        self.error_response(
                                            StatusCode::BAD_GATEWAY,
                                            "Failed to read upstream response",
                                        )
                                    }
                                }
                            }
                            Err(CircuitBreakerError::Open) => {
                                warn!("Circuit breaker opened for upstream: {}", upstream_name);
                                let duration = start.elapsed().as_secs_f64();
                                record_request(method.as_str(), StatusCode::SERVICE_UNAVAILABLE.as_u16(), duration);
                                self.error_response(
                                    StatusCode::SERVICE_UNAVAILABLE,
                                    "Circuit breaker is open",
                                )
                            }
                            Err(CircuitBreakerError::Failure(e)) => {
                                error!("Failed to forward request: {}", e);
                                let duration = start.elapsed().as_secs_f64();
                                record_request(method.as_str(), StatusCode::BAD_GATEWAY.as_u16(), duration);
                                record_upstream_request(&upstream_name, StatusCode::BAD_GATEWAY.as_u16(), duration);
                                self.error_response(
                                    StatusCode::BAD_GATEWAY,
                                    "Failed to connect to upstream",
                                )
                            }
                        }
                        }
                    } else {
                        warn!("No healthy backends available for upstream: {}", upstream_name);
                        let duration = start.elapsed().as_secs_f64();
                        record_request(method.as_str(), StatusCode::SERVICE_UNAVAILABLE.as_u16(), duration);
                        self.error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "No healthy backends available",
                        )
                    }
                } else {
                    warn!("Upstream not found: {}", upstream_name);
                    let duration = start.elapsed().as_secs_f64();
                    record_request(method.as_str(), StatusCode::BAD_GATEWAY.as_u16(), duration);
                    self.error_response(StatusCode::BAD_GATEWAY, "Upstream not found")
                }
            }
            None => {
                warn!("No route matched for {} {}", method, path);
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), StatusCode::NOT_FOUND.as_u16(), duration);
                self.error_response(StatusCode::NOT_FOUND, "No matching route found")
            }
        }
    }

    /// Find a matching route for the request (with hostname router support)
    async fn find_route_async(&self, method: &Method, host: &str, path: &str) -> Option<String> {
        // Try hostname router first (API Gateway)
        if let Some(router) = &self.hostname_router {
            if let Some(matched) = router.match_request_wildcard(host, path, method.as_str()).await {
                info!(
                    "Hostname router matched: route={}, upstream={}, match_type={:?}",
                    matched.route.name, matched.route.upstream, matched.match_type
                );
                return Some(matched.route.upstream);
            }
        }

        // Fallback to config-based routing
        self.find_route(method, host, path).map(|r| r.upstream.clone())
    }

    /// Find a matching route for the request (legacy config-based)
    fn find_route(&self, method: &Method, host: &str, path: &str) -> Option<&RouteConfig> {
        // Normalize host by stripping port (e.g., "localhost:8443" -> "localhost")
        // This allows route patterns to match without requiring port specification
        let host_without_port = host.split(':').next().unwrap_or(host);

        debug!("Route matching: method={}, host={}, host_without_port={}, path={}",
               method, host, host_without_port, path);
        debug!("Available routes: {}", self.config.routes.len());

        for route in &self.config.routes {
            debug!("Checking route: name={}, hosts={:?}, paths={:?}",
                   route.name, route.match_rules.hosts, route.match_rules.paths);

            // Check host match (if specified)
            if !route.match_rules.hosts.is_empty() {
                let host_matches = route
                    .match_rules
                    .hosts
                    .iter()
                    .any(|pattern| {
                        let match_result = self.matches_pattern(host, pattern) ||
                                         self.matches_pattern(host_without_port, pattern);
                        debug!("  Host pattern '{}' vs '{}' (without port: '{}'): {}",
                               pattern, host, host_without_port, match_result);
                        match_result
                    });

                if !host_matches {
                    debug!("  Route {} rejected: host mismatch", route.name);
                    continue;
                }
                debug!("  Route {} passed host match", route.name);
            }

            // Check path match (if specified)
            if !route.match_rules.paths.is_empty() {
                let path_matches = route
                    .match_rules
                    .paths
                    .iter()
                    .any(|pattern| {
                        let match_result = self.matches_pattern(path, pattern);
                        debug!("  Path pattern '{}' vs '{}': {}", pattern, path, match_result);
                        match_result
                    });

                if !path_matches {
                    debug!("  Route {} rejected: path mismatch", route.name);
                    continue;
                }
                debug!("  Route {} passed path match", route.name);
            }

            // Check method match (if specified)
            if !route.match_rules.methods.is_empty() {
                let method_matches = route
                    .match_rules
                    .methods
                    .iter()
                    .any(|m| m == method.as_str());

                if !method_matches {
                    continue;
                }
            }

            // All checks passed
            return Some(route);
        }

        None
    }

    /// Simple pattern matching (supports * wildcard)
    fn matches_pattern(&self, value: &str, pattern: &str) -> bool {
        if pattern == "*" || pattern == "/*" {
            return true;
        }

        if pattern.contains('*') {
            // Simple wildcard matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        }

        value == pattern
    }

    /// Proxy WebSocket frames bidirectionally between client and backend
    async fn proxy_websocket_streams<S>(
        client: tokio_tungstenite::WebSocketStream<S>,
        backend: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        
        use futures_util::{SinkExt, StreamExt};

        info!("Starting bidirectional WebSocket proxy");

        let (mut client_sink, mut client_stream) = client.split();
        let (mut backend_sink, mut backend_stream) = backend.split();

        // Client to backend task
        let client_to_backend = async {
            while let Some(msg_result) = client_stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        debug!("Client→Backend: {:?}", msg);
                        if backend_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Client WebSocket error: {}", e);
                        break;
                    }
                }
            }
        };

        // Backend to client task
        let backend_to_client = async {
            while let Some(msg_result) = backend_stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        debug!("Backend→Client: {:?}", msg);
                        if client_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Backend WebSocket error: {}", e);
                        break;
                    }
                }
            }
        };

        // Run both directions concurrently
        tokio::select! {
            _ = client_to_backend => {
                info!("Client to backend stream closed");
            }
            _ = backend_to_client => {
                info!("Backend to client stream closed");
            }
        }

        info!("WebSocket proxy completed");
        Ok(())
    }

    /// Handle webserver request (static files or PHP-FPM) using route configuration
    async fn handle_webserver_request(
        &self,
        req: Request<Incoming>,
        route: &RouteConfig,
        method: &Method,
        host: &str,
        path: &str,
        start: Instant,
    ) -> Result<Response<ResponseBody>> {
        // Get static file handler
        let static_handler = self.static_file_handler.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Static file handler not configured"))?;

        // Get document root from route or use default
        let document_root = route.root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No document root configured for route"))?;

        debug!("Webserver request: path={}, root={}", path, document_root);

        // Initialize path validator for security checks
        let path_validator = PathValidator::new(document_root);

        // Validate request path for security issues (path traversal, null bytes, etc.)
        let file_path = match path_validator.validate_path(path) {
            Ok(validated_path) => validated_path,
            Err(e) => {
                warn!("Path validation failed for {}: {}", path, e);
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), StatusCode::FORBIDDEN.as_u16(), duration);
                return self.webserver_error_response(
                    StatusCode::FORBIDDEN,
                    "Access denied",
                    route,
                );
            }
        };

        // Try to resolve the file using the validated path
        let mut resolved_path = None;

        // Check if the path exists
        if file_path.exists() && file_path.is_file() {
            resolved_path = Some(file_path);
        } else if file_path.is_dir() {
            // Try index files
            for index_file in &route.index {
                let index_path = file_path.join(index_file);
                if index_path.exists() && index_path.is_file() {
                    resolved_path = Some(index_path);
                    break;
                }
            }

            // If no index file found and directory listing is enabled, serve directory listing
            if resolved_path.is_none() && route.directory_listing {
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), StatusCode::OK.as_u16(), duration);
                return self.serve_directory_listing(&file_path, path);
            }
        }

        // If not found, try try_files patterns
        if resolved_path.is_none() && !route.try_files.is_empty() {
            for pattern in &route.try_files {
                let test_path = if pattern == "$uri" {
                    // Try original path
                    std::path::Path::new(document_root).join(path.trim_start_matches('/'))
                } else if pattern == "$uri/" {
                    // Try as directory with trailing slash
                    let mut p = std::path::Path::new(document_root).join(path.trim_start_matches('/'));
                    if p.is_dir() {
                        // Try index files in this directory
                        for index_file in &route.index {
                            let index_path = p.join(index_file);
                            if index_path.exists() && index_path.is_file() {
                                resolved_path = Some(index_path);
                                break;
                            }
                        }
                    }
                    continue;
                } else if pattern.starts_with('=') {
                    // Status code fallback (e.g., =404)
                    if let Ok(code) = pattern[1..].parse::<u16>() {
                        if let Ok(status) = StatusCode::from_u16(code) {
                            let duration = start.elapsed().as_secs_f64();
                            record_request(method.as_str(), status.as_u16(), duration);
                            return self.webserver_error_response(status, &format!("File not found: {}", path), route);
                        }
                    }
                    continue;
                } else if pattern.starts_with('/') {
                    // Absolute path fallback
                    std::path::Path::new(document_root).join(pattern.trim_start_matches('/'))
                } else {
                    // Treat as relative path
                    std::path::Path::new(document_root).join(pattern)
                };

                if test_path.exists() && test_path.is_file() {
                    resolved_path = Some(test_path);
                    break;
                }
            }
        }

        // If still not found, return 404
        let final_path = resolved_path.ok_or_else(|| {
            anyhow::anyhow!("File not found: {}", path)
        })?;

        debug!("Resolved path: {:?}", final_path);

        // Validate file is allowed to be served (check for sensitive files, dangerous extensions, hidden files)
        if let Err(e) = path_validator.is_file_allowed(&final_path) {
            warn!("File access denied for {:?}: {}", final_path, e);
            let duration = start.elapsed().as_secs_f64();
            record_request(method.as_str(), StatusCode::FORBIDDEN.as_u16(), duration);
            return self.webserver_error_response(
                StatusCode::FORBIDDEN,
                "Access denied",
                route,
            );
        }

        // Get file info
        let file_info = static_handler.get_file_info(&final_path)?;

        // Validate file size
        if let Err(e) = path_validator.validate_file_size(file_info.metadata.len()) {
            warn!("File too large for {:?}: {}", final_path, e);
            let duration = start.elapsed().as_secs_f64();
            record_request(method.as_str(), StatusCode::PAYLOAD_TOO_LARGE.as_u16(), duration);
            return self.webserver_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "File too large",
                route,
            );
        }

        // Check if this is a PHP file and PHP-FPM is configured
        if file_info.is_php {
            // Validate PHP script before processing
            if let Err(e) = validate_php_script(&final_path) {
                warn!("PHP script validation failed for {:?}: {}", final_path, e);
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), StatusCode::FORBIDDEN.as_u16(), duration);
                return self.webserver_error_response(
                    StatusCode::FORBIDDEN,
                    "Invalid PHP script",
                    route,
                );
            }

            if let Some(php_config) = &route.php_fpm {
                if php_config.enabled {
                    if let Some(php_pool) = &self.php_fpm_pool {
                        debug!("Processing PHP request: {:?}", final_path);

                        // Split request to access parts
                        let (parts, incoming_body) = req.into_parts();

                        // Extract Content-Length header
                        let content_length = parts.headers
                            .get("content-length")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok());

                        // Collect body for POST/PUT/PATCH requests
                        let body = if matches!(method, &Method::POST | &Method::PUT | &Method::PATCH) {
                            match collect_body_validated(incoming_body, content_length, 10 * 1024 * 1024).await {
                                Ok(collected) => {
                                    debug!("Collected {} bytes for PHP request", collected.len());

                                    // Validate request body size
                                    if let Err(e) = path_validator.validate_request_body_size(collected.len()) {
                                        warn!("Request body too large: {}", e);
                                        let duration = start.elapsed().as_secs_f64();
                                        record_request(method.as_str(), StatusCode::PAYLOAD_TOO_LARGE.as_u16(), duration);
                                        return self.webserver_error_response(
                                            StatusCode::PAYLOAD_TOO_LARGE,
                                            "Request body too large",
                                            route,
                                        );
                                    }

                                    collected
                                }
                                Err(e) => {
                                    warn!("Failed to collect request body: {}", e);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::BAD_REQUEST.as_u16(), duration);
                                    return self.webserver_error_response(
                                        StatusCode::BAD_REQUEST,
                                        "Invalid request body",
                                        route,
                                    );
                                }
                            }
                        } else {
                            // For GET, HEAD, etc., use empty body
                            CollectedBody::empty()
                        };

                        // Reconstruct request with empty body
                        let req_empty = Request::from_parts(parts, Empty::<Bytes>::new());

                        match self.serve_php_file(&file_info, php_pool, &req_empty, host, path, body).await {
                            Ok(response) => {
                                let duration = start.elapsed().as_secs_f64();
                                record_request(method.as_str(), response.status().as_u16(), duration);
                                return Ok(response);
                            }
                            Err(e) => {
                                error!("PHP-FPM processing failed for {:?}: {}", final_path, e);
                                let duration = start.elapsed().as_secs_f64();
                                record_request(method.as_str(), StatusCode::INTERNAL_SERVER_ERROR.as_u16(), duration);
                                return self.webserver_error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "PHP processing failed",
                                    route,
                                );
                            }
                        }
                    }
                }
            }
            // If PHP-FPM not configured, treat as static file (will likely fail with wrong MIME type)
            warn!("PHP file detected but PHP-FPM not configured for route: {}", path);
        }

        // Serve as static file
        debug!("Serving static file: {:?}", final_path);

        // Reconstruct request for static file serving
        match self.serve_static_file(&file_info, static_handler, &req).await {
            Ok(response) => {
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), response.status().as_u16(), duration);
                Ok(response)
            }
            Err(e) => {
                error!("Static file serving failed for {:?}: {}", final_path, e);
                let duration = start.elapsed().as_secs_f64();
                record_request(method.as_str(), StatusCode::INTERNAL_SERVER_ERROR.as_u16(), duration);
                self.webserver_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to serve file",
                    route,
                )
            }
        }
    }

    /// Serve a static file
    async fn serve_static_file(
        &self,
        file_info: &crate::webserver::FileInfo,
        static_handler: &crate::webserver::StaticFileHandler,
        req: &Request<Incoming>,
    ) -> Result<Response<ResponseBody>> {
        

        // Get file metadata
        let metadata = &file_info.metadata;
        let file_size = metadata.len();

        // Generate ETag
        let etag = static_handler.generate_etag(metadata);

        // Check If-None-Match header for ETag-based caching
        if let Some(if_none_match) = req.headers().get("if-none-match") {
            if let Ok(client_etag) = if_none_match.to_str() {
                if client_etag == etag {
                    debug!("ETag match, returning 304 Not Modified");
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header("etag", etag)
                        .body(ResponseBody::empty())?);
                }
            }
        }

        // Check If-Modified-Since header
        if let Some(if_modified_since) = req.headers().get("if-modified-since") {
            if let Ok(since_str) = if_modified_since.to_str() {
                if let Ok(since_time) = httpdate::parse_http_date(since_str) {
                    if let Ok(modified_time) = metadata.modified() {
                        if modified_time <= since_time {
                            debug!("File not modified since {}, returning 304", since_str);
                            return Ok(Response::builder()
                                .status(StatusCode::NOT_MODIFIED)
                                .header("etag", etag)
                                .header("last-modified", httpdate::fmt_http_date(modified_time))
                                .body(ResponseBody::empty())?);
                        }
                    }
                }
            }
        }

        // Check Range header for partial content requests
        let range = req.headers().get("range")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| self.parse_range_header(s, file_size));

        // Open file async (non-blocking)
        let file = tokio::fs::File::open(&file_info.path).await?;

        // Get MIME type
        let mime_type = static_handler.get_mime_type(&file_info.path);

        // Handle range request
        if let Some((start, end)) = range {
            debug!("Range request: bytes {}-{}/{}", start, end, file_size);

            // Seek to start position
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let mut file = file;
            file.seek(std::io::SeekFrom::Start(start)).await?;

            // Read the requested range
            let length = end - start + 1;
            let mut buffer = vec![0u8; length as usize];
            file.read_exact(&mut buffer).await?;

            // Build 206 Partial Content response
            let mut response = Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header("content-type", mime_type)
                .header("content-length", length.to_string())
                .header("content-range", format!("bytes {}-{}/{}", start, end, file_size))
                .header("accept-ranges", "bytes")
                .header("etag", &etag);

            // Add Last-Modified header
            if let Ok(modified_time) = metadata.modified() {
                response = response.header("last-modified", httpdate::fmt_http_date(modified_time));
            }

            // Add cache-control
            response = response.header("cache-control", "public, max-age=3600");

            return Ok(response.body(ResponseBody::buffered(Bytes::from(buffer)))?);
        }

        // Build response for full file
        let mut response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", mime_type)
            .header("content-length", file_size.to_string())
            .header("accept-ranges", "bytes")
            .header("etag", etag);

        // Add Last-Modified header
        if let Ok(modified_time) = metadata.modified() {
            response = response.header("last-modified", httpdate::fmt_http_date(modified_time));
        }

        // Add cache-control
        response = response.header("cache-control", "public, max-age=3600");

        Ok(response.body(ResponseBody::from_file(file))?)
    }

    /// Serve a PHP file via PHP-FPM
    async fn serve_php_file(
        &self,
        file_info: &crate::webserver::FileInfo,
        php_pool: &crate::webserver::PhpFpmPool,
        req: &Request<Empty<Bytes>>,
        host: &str,
        path: &str,
        body: CollectedBody,
    ) -> Result<Response<ResponseBody>> {
        // Get connection from pool
        let mut conn = php_pool.get_connection()
            .map_err(|e| anyhow::anyhow!("Failed to get PHP-FPM connection: {}", e))?;

        // Build FastCGI parameters
        let script_filename = file_info.path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid script path"))?;

        let mut params = vec![
            ("REQUEST_METHOD".to_string(), req.method().as_str().to_string()),
            ("SCRIPT_FILENAME".to_string(), script_filename.to_string()),
            ("REQUEST_URI".to_string(), sanitize_fastcgi_param(req.uri().path())),
            ("DOCUMENT_URI".to_string(), sanitize_fastcgi_param(path)),
            ("SERVER_PROTOCOL".to_string(), format!("{:?}", req.version())),
            ("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string()),
            ("SERVER_SOFTWARE".to_string(), "highper-gateway".to_string()),
            ("REMOTE_ADDR".to_string(), "127.0.0.1".to_string()),
            ("SERVER_NAME".to_string(), sanitize_fastcgi_param(host)),
            ("SERVER_PORT".to_string(), "80".to_string()),
        ];

        // Add query string if present
        if let Some(query) = req.uri().query() {
            params.push(("QUERY_STRING".to_string(), sanitize_fastcgi_param(query)));
        }

        // Add content length and type for POST/PUT
        if let Some(content_type) = req.headers().get("content-type") {
            if let Ok(ct) = content_type.to_str() {
                params.push(("CONTENT_TYPE".to_string(), sanitize_fastcgi_param(ct)));
            }
        }

        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(cl) = content_length.to_str() {
                params.push(("CONTENT_LENGTH".to_string(), sanitize_fastcgi_param(cl)));
            }
        }

        // Add HTTP headers as CGI variables (sanitize to prevent injection)
        for (name, value) in req.headers() {
            if let Ok(value_str) = value.to_str() {
                let header_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', "_"));
                params.push((header_name, sanitize_fastcgi_param(value_str)));
            }
        }

        // Use the collected request body (already extracted before calling this function)
        debug!("Sending {} bytes to PHP-FPM", body.len());

        // Execute FastCGI request with actual body data
        let output = conn.execute(&params, &body.bytes)
            .map_err(|e| anyhow::anyhow!("PHP-FPM execution failed: {}", e))?;

        // Parse CGI response (headers + body)
        let response = self.parse_cgi_response(&output)?;

        Ok(response)
    }

    /// Parse CGI response into HTTP response
    fn parse_cgi_response(&self, output: &[u8]) -> Result<Response<ResponseBody>> {
        // Find end of headers (double newline)
        let mut header_end = 0;
        for i in 0..output.len().saturating_sub(3) {
            if &output[i..i+4] == b"\r\n\r\n" {
                header_end = i + 4;
                break;
            } else if &output[i..i+2] == b"\n\n" {
                header_end = i + 2;
                break;
            }
        }

        if header_end == 0 {
            // No headers found, treat entire output as body
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .body(ResponseBody::buffered(Bytes::from(output.to_vec())))?);
        }

        // Parse headers
        let header_section = &output[0..header_end];
        let body = &output[header_end..];

        let mut response_builder = Response::builder();
        let mut status_code = StatusCode::OK;

        // Parse each header line
        for line in header_section.split(|&b| b == b'\n') {
            let line = std::str::from_utf8(line).unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            if let Some(colon_pos) = line.find(':') {
                let name = &line[0..colon_pos].trim();
                let value = &line[colon_pos+1..].trim();

                if name.eq_ignore_ascii_case("status") {
                    // Parse status code
                    if let Some(code_str) = value.split_whitespace().next() {
                        if let Ok(code) = code_str.parse::<u16>() {
                            if let Ok(sc) = StatusCode::from_u16(code) {
                                status_code = sc;
                            }
                        }
                    }
                } else {
                    response_builder = response_builder.header(*name, *value);
                }
            }
        }

        Ok(response_builder
            .status(status_code)
            .body(ResponseBody::buffered(Bytes::from(body.to_vec())))?)
    }

    /// Create an error response
    fn error_response(
        &self,
        status: StatusCode,
        message: &str,
    ) -> Result<Response<ResponseBody>> {
        Ok(Response::builder()
            .status(status)
            .header("content-type", "text/plain")
            .body(ResponseBody::buffered(Bytes::from(message.to_string())))?)
    }

    /// Create an error response with custom error page support (for webserver routes)
    fn webserver_error_response(
        &self,
        status: StatusCode,
        message: &str,
        route: &RouteConfig,
    ) -> Result<Response<ResponseBody>> {
        // Check if there's a custom error page for this status code
        if let Some(error_page_path) = route.error_pages.get(&status.as_u16()) {
            // Get document root
            if let Some(root) = &route.root {
                let error_file_path = std::path::Path::new(root).join(error_page_path.trim_start_matches('/'));

                // Try to read the custom error page
                if let Ok(contents) = std::fs::read(&error_file_path) {
                    debug!("Serving custom error page: {:?} for status {}", error_file_path, status);

                    // Detect content type
                    let content_type = if error_page_path.ends_with(".html") || error_page_path.ends_with(".htm") {
                        "text/html; charset=utf-8"
                    } else if error_page_path.ends_with(".json") {
                        "application/json"
                    } else {
                        "text/plain"
                    };

                    return Ok(Response::builder()
                        .status(status)
                        .header("content-type", content_type)
                        .body(ResponseBody::buffered(Bytes::from(contents)))?);
                } else {
                    warn!("Custom error page not found: {:?}", error_file_path);
                }
            }
        }

        // Fall back to default plain text error
        self.error_response(status, message)
    }

    /// Generate HTML directory listing
    fn serve_directory_listing(
        &self,
        dir_path: &std::path::Path,
        request_path: &str,
    ) -> Result<Response<ResponseBody>> {
        use std::fs;

        // Read directory entries
        let entries = fs::read_dir(dir_path)
            .map_err(|e| anyhow::anyhow!("Failed to read directory: {}", e))?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            let size = if metadata.is_dir() {
                "-".to_string()
            } else {
                format_file_size(metadata.len())
            };

            let modified = metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    let secs = d.as_secs();
                    chrono::DateTime::from_timestamp(secs as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|| "Unknown".to_string());

            if metadata.is_dir() {
                dirs.push((name, size, modified));
            } else {
                files.push((name, size, modified));
            }
        }

        // Sort alphabetically
        dirs.sort_by(|a, b| a.0.cmp(&b.0));
        files.sort_by(|a, b| a.0.cmp(&b.0));

        // Generate HTML
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Index of {path}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 2rem;
            background: #f5f5f5;
        }}
        h1 {{
            color: #333;
            border-bottom: 2px solid #667eea;
            padding-bottom: 0.5rem;
        }}
        table {{
            width: 100%;
            background: white;
            border-radius: 8px;
            overflow: hidden;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        th {{
            background: #667eea;
            color: white;
            text-align: left;
            padding: 1rem;
            font-weight: 600;
        }}
        td {{
            padding: 0.75rem 1rem;
            border-bottom: 1px solid #eee;
        }}
        tr:hover td {{
            background: #f8f9ff;
        }}
        a {{
            color: #667eea;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
        .dir {{
            font-weight: 500;
        }}
        .dir::before {{
            content: "📁 ";
        }}
        .file::before {{
            content: "📄 ";
        }}
        .size {{
            text-align: right;
            color: #666;
        }}
        .modified {{
            color: #888;
            font-size: 0.9em;
        }}
    </style>
</head>
<body>
    <h1>Index of {path}</h1>
    <table>
        <thead>
            <tr>
                <th>Name</th>
                <th style="text-align: right">Size</th>
                <th>Modified</th>
            </tr>
        </thead>
        <tbody>
            {parent_link}
            {directory_rows}
            {file_rows}
        </tbody>
    </table>
</body>
</html>"#,
            path = request_path,
            parent_link = if request_path != "/" {
                r#"<tr>
                <td><a href=".." class="dir">Parent Directory</a></td>
                <td class="size">-</td>
                <td class="modified">-</td>
            </tr>"#
            } else {
                ""
            },
            directory_rows = dirs.iter()
                .map(|(name, size, modified)| format!(
                    r#"<tr>
                <td><a href="{}/{}" class="dir">{}/</a></td>
                <td class="size">{}</td>
                <td class="modified">{}</td>
            </tr>"#,
                    request_path.trim_end_matches('/'), name, name, size, modified
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            file_rows = files.iter()
                .map(|(name, size, modified)| format!(
                    r#"<tr>
                <td><a href="{}/{}" class="file">{}</a></td>
                <td class="size">{}</td>
                <td class="modified">{}</td>
            </tr>"#,
                    request_path.trim_end_matches('/'), name, name, size, modified
                ))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "no-cache")
            .body(ResponseBody::buffered(Bytes::from(html)))?)
    }

    /// Parse Range header and return (start, end) byte positions
    /// Supports formats like: "bytes=0-1023", "bytes=1024-", "bytes=-500"
    /// Returns None if range is invalid or not satisfiable
    fn parse_range_header(&self, range_str: &str, file_size: u64) -> Option<(u64, u64)> {
        // Must start with "bytes="
        if !range_str.starts_with("bytes=") {
            return None;
        }

        let range_spec = &range_str[6..]; // Skip "bytes="

        // Split on comma (we only support single range, not multipart)
        let range_part = range_spec.split(',').next()?;

        // Parse start-end format
        if let Some((start_str, end_str)) = range_part.split_once('-') {
            match (start_str.trim(), end_str.trim()) {
                // "bytes=100-200" - explicit start and end
                (start, end) if !start.is_empty() && !end.is_empty() => {
                    let start: u64 = start.parse().ok()?;
                    let end: u64 = end.parse().ok()?;

                    // Validate range
                    if start > end || start >= file_size {
                        return None;
                    }

                    // Clamp end to file size
                    let end = std::cmp::min(end, file_size - 1);
                    Some((start, end))
                }

                // "bytes=100-" - start to end of file
                (start, "") if !start.is_empty() => {
                    let start: u64 = start.parse().ok()?;

                    if start >= file_size {
                        return None;
                    }

                    Some((start, file_size - 1))
                }

                // "bytes=-500" - last N bytes
                ("", end) if !end.is_empty() => {
                    let suffix_len: u64 = end.parse().ok()?;

                    if suffix_len == 0 || suffix_len > file_size {
                        return None;
                    }

                    let start = file_size - suffix_len;
                    Some((start, file_size - 1))
                }

                _ => None,
            }
        } else {
            None
        }
    }

    /// Track metrics in ProxyState
    fn track_metrics(&self, status_code: u16) {
        if let Some(ref state) = self.proxy_state {
            let metrics = state.metrics();
            metrics.increment_requests();
            metrics.record_status(status_code);
        }
    }
}

/// Format file size in human-readable format
fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self::new(Arc::new(Config {
            server: Default::default(),
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: Default::default(),
            websocket: Default::default(),
            grpc: Default::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
        }))
    }
}
