//! Request handler

use crate::config::{Config, RouteConfig, UpstreamConfig};
use crate::gateway::routing::HostnameRouter;
use crate::http::{CollectedBody, collect_body_validated, ResponseBody};
use crate::middleware::{MiddlewareChain, compression_middleware::CompressionMiddleware};
use crate::observability::metrics::{record_request, record_upstream_request};
use crate::proxy::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
use crate::proxy::{Client, LoadBalancer};
use crate::state::ProxyState;
use crate::tls::ChallengeStore;
use crate::websocket::handler as ws_handler;
use crate::grpc::detector as grpc_detector;
use crate::webserver::StaticFileHandler;
use crate::webserver::PhpFpmPool;
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

        // Add compression middleware
        middleware_chain.add(CompressionMiddleware::with_defaults());

        info!("Initialized middleware chain with {} middlewares: {:?}",
            middleware_chain.len(),
            middleware_chain.middleware_names()
        );

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

        // Add compression middleware
        middleware_chain.add(CompressionMiddleware::with_defaults());

        info!("Initialized middleware chain with {} middlewares: {:?}",
            middleware_chain.len(),
            middleware_chain.middleware_names()
        );

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
        }
    }

    /// Set hostname router for API Gateway features
    pub fn with_hostname_router(mut self, router: Arc<HostnameRouter>) -> Self {
        self.hostname_router = Some(router);
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

        // Check for static file or PHP-FPM request (web server features)
        if let Some(static_handler) = &self.static_file_handler {
            // Try to resolve the path
            match static_handler.resolve_path(path) {
                Ok(file_path) => {
                    match static_handler.get_file_info(&file_path) {
                        Ok(file_info) => {
                            // Check if this is a PHP file
                            if file_info.is_php {
                                if let Some(php_pool) = &self.php_fpm_pool {
                                    debug!("Processing PHP request: {}", path);

                                    // Split request to access parts
                                    let (parts, incoming_body) = req.into_parts();

                                    // Extract host from parts (owned copy)
                                    let php_host = parts.headers
                                        .get("host")
                                        .and_then(|h| h.to_str().ok())
                                        .unwrap_or("localhost")
                                        .to_string();

                                    // Extract Content-Length header
                                    let content_length = parts.headers
                                        .get("content-length")
                                        .and_then(|h| h.to_str().ok())
                                        .and_then(|s| s.parse::<u64>().ok());

                                    // Collect body for POST/PUT/PATCH requests
                                    let body = if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
                                        match collect_body_validated(incoming_body, content_length, 10 * 1024 * 1024).await {
                                            Ok(collected) => {
                                                debug!("Collected {} bytes for PHP request", collected.len());
                                                collected
                                            }
                                            Err(e) => {
                                                warn!("Failed to collect request body: {}", e);
                                                let duration = start.elapsed().as_secs_f64();
                                                record_request(method.as_str(), StatusCode::BAD_REQUEST.as_u16(), duration);
                                                return self.error_response(
                                                    StatusCode::BAD_REQUEST,
                                                    "Invalid request body",
                                                );
                                            }
                                        }
                                    } else {
                                        // For GET, HEAD, etc., use empty body
                                        CollectedBody::empty()
                                    };

                                    // Reconstruct request with empty body
                                    let req_empty = Request::from_parts(parts, Empty::<Bytes>::new());

                                    match self.serve_php_file(&file_info, php_pool, &req_empty, &php_host, path, body).await {
                                        Ok(response) => {
                                            let duration = start.elapsed().as_secs_f64();
                                            record_request(method.as_str(), response.status().as_u16(), duration);
                                            return Ok(response);
                                        }
                                        Err(e) => {
                                            error!("PHP-FPM processing failed for {:?}: {}", file_path, e);
                                            let duration = start.elapsed().as_secs_f64();
                                            record_request(method.as_str(), StatusCode::INTERNAL_SERVER_ERROR.as_u16(), duration);
                                            return self.error_response(
                                                StatusCode::INTERNAL_SERVER_ERROR,
                                                "PHP processing failed",
                                            );
                                        }
                                    }
                                } else {
                                    // PHP file but no PHP-FPM pool configured
                                    debug!("PHP file detected but no PHP-FPM pool configured: {}", path);
                                }
                            } else {
                                // Serve static file
                                debug!("Serving static file: {:?}", file_path);

                                match self.serve_static_file(&file_info, &static_handler, &req).await {
                                    Ok(response) => {
                                        let duration = start.elapsed().as_secs_f64();
                                        record_request(method.as_str(), response.status().as_u16(), duration);
                                        return Ok(response);
                                    }
                                    Err(e) => {
                                        warn!("Error serving static file {:?}: {}", file_path, e);
                                        // Fall through to normal routing
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // File not found or directory listing disabled
                            debug!("Static file not found or directory: {} - {}", path, e);
                            // Fall through to normal routing
                        }
                    }
                }
                Err(e) => {
                    // Path resolution failed (likely path traversal or invalid path)
                    debug!("Path resolution failed for {}: {}", path, e);
                    // Fall through to normal routing
                }
            }
        }

        // Check for WebSocket upgrade request
        if self.config.websocket.enabled && ws_handler::is_websocket_upgrade(&req) {
            debug!("Detected WebSocket upgrade request");
            info!("WebSocket upgrade detected for path: {}", path);

            // Find matching route (with hostname router support)
            let upstream_name = self.find_route_async(&method, &host, path).await;

            match upstream_name {
                Some(upstream_name) => {
                    // Get upstream
                    if let Some(upstream) = self.upstreams.get(&upstream_name) {
                        // Select backend server
                        if let Some(backend_url) = upstream.select_backend(client_ip.as_deref()) {
                            debug!("WebSocket backend selected: {}", backend_url);

                            // Parse backend URL for WebSocket connection
                            let backend_ws_url = backend_url
                                .replace("http://", "ws://")
                                .replace("https://", "wss://");
                            let backend_ws_url = format!("{}{}", backend_ws_url.trim_end_matches('/'), path);

                            info!("Establishing WebSocket connection to backend: {}", backend_ws_url);

                            // Create WebSocket upgrade response
                            match ws_handler::create_upgrade_response(&req) {
                                Ok(response) => {
                                    info!("WebSocket upgrade response created for {}", path);
                                    let duration = start.elapsed().as_secs_f64();
                                    record_request(method.as_str(), StatusCode::SWITCHING_PROTOCOLS.as_u16(), duration);

                                    // Spawn async task for WebSocket proxying after upgrade
                                    tokio::spawn(async move {
                                        // Wait for the upgrade to complete
                                        match hyper::upgrade::on(req).await {
                                            Ok(upgraded) => {
                                                info!("Client WebSocket connection upgraded, connecting to backend");

                                                // Wrap upgraded connection with TokioIo for AsyncRead/AsyncWrite
                                                use hyper_util::rt::TokioIo;
                                                let upgraded_io = TokioIo::new(upgraded);

                                                // Connect to backend WebSocket
                                                match tokio_tungstenite::connect_async(&backend_ws_url).await {
                                                    Ok((backend_ws, _)) => {
                                                        info!("Connected to backend WebSocket: {}", backend_ws_url);

                                                        // Convert upgraded connection to WebSocket frames
                                                        let client_ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
                                                            upgraded_io,
                                                            tokio_tungstenite::tungstenite::protocol::Role::Server,
                                                            None,
                                                        ).await;

                                                        // Proxy bidirectionally
                                                        if let Err(e) = Self::proxy_websocket_streams(client_ws, backend_ws).await {
                                                            error!("WebSocket proxy error: {}", e);
                                                        } else {
                                                            info!("WebSocket connection closed cleanly");
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to connect to backend WebSocket: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to upgrade client connection: {}", e);
                                            }
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

        // Check for gRPC request
        if self.config.grpc.enabled && grpc_detector::is_grpc_request(&req) {
            debug!("Detected gRPC request");
            if let Some(grpc_req) = grpc_detector::parse_grpc_request(&req) {
                info!("gRPC request: {} (service: {:?})", grpc_req.path,
                    grpc_detector::extract_service_name(&grpc_req.path));
                // For now, gRPC will be proxied as regular HTTP/2
                // Full gRPC-aware proxying with streaming will be added in a future update
                debug!("Proxying gRPC request as HTTP/2");
            }
        }

        // Find matching route (with hostname router support)
        let upstream_name = self.find_route_async(&method, &host, path).await;

        match upstream_name {
            Some(upstream_name) => {
                debug!("Matched route to upstream: {}", upstream_name);

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
                    if let Some(backend_url) = upstream.select_backend(client_ip.as_deref()) {
                        debug!("Selected backend: {}", backend_url);

                        // Execute request with circuit breaker protection
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

                                        let response = resp.body(ResponseBody::buffered(body_bytes))?;

                                        // Apply middleware chain (compression, etc.)
                                        let response = match self.middleware_chain.process_response(response).await {
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

        // Open file async (non-blocking)
        let file = tokio::fs::File::open(&file_info.path).await?;

        // Get MIME type
        let mime_type = static_handler.get_mime_type(&file_info.path);

        // Build response
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
            ("REQUEST_URI".to_string(), req.uri().path().to_string()),
            ("DOCUMENT_URI".to_string(), path.to_string()),
            ("SERVER_PROTOCOL".to_string(), format!("{:?}", req.version())),
            ("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string()),
            ("SERVER_SOFTWARE".to_string(), "highper-gateway".to_string()),
            ("REMOTE_ADDR".to_string(), "127.0.0.1".to_string()),
            ("SERVER_NAME".to_string(), host.to_string()),
            ("SERVER_PORT".to_string(), "80".to_string()),
        ];

        // Add query string if present
        if let Some(query) = req.uri().query() {
            params.push(("QUERY_STRING".to_string(), query.to_string()));
        }

        // Add content length and type for POST/PUT
        if let Some(content_type) = req.headers().get("content-type") {
            if let Ok(ct) = content_type.to_str() {
                params.push(("CONTENT_TYPE".to_string(), ct.to_string()));
            }
        }

        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(cl) = content_length.to_str() {
                params.push(("CONTENT_LENGTH".to_string(), cl.to_string()));
            }
        }

        // Add HTTP headers as CGI variables
        for (name, value) in req.headers() {
            if let Ok(value_str) = value.to_str() {
                let header_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', "_"));
                params.push((header_name, value_str.to_string()));
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

    /// Track metrics in ProxyState
    fn track_metrics(&self, status_code: u16) {
        if let Some(ref state) = self.proxy_state {
            let metrics = state.metrics();
            metrics.increment_requests();
            metrics.record_status(status_code);
        }
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
        }))
    }
}
