//! HTTP server implementation

use crate::config::{Config, Protocol};
use crate::proxy::Handler;
use crate::proxy::connection_pool::{ConnectionPoolManager, PoolConfig, PoolStats};
use crate::runtime::GLOBAL_IO;
use crate::tls::acceptor::TlsAcceptor;
use crate::tls::ktls;
use crate::tls::manager::TlsManager;
use crate::tls::passthrough;
use crate::utils::socket::{create_optimized_socket, socket_to_listener, SocketConfig};
use crate::Result;
use hyper::server::conn::{http1, http2};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::io::{AsyncWriteExt, copy_bidirectional};
use tracing::{debug, error, info, warn};

/// HTTP server
pub struct Server {
    config: Arc<Config>,
    handler: Arc<Handler>,
    tls_acceptor: Option<Arc<TlsAcceptor>>,
    connection_pool: Arc<ConnectionPoolManager>,
}

impl Server {
    /// Create a new server
    pub fn new(config: Arc<Config>) -> Self {
        let mut handler = Handler::new(config.clone());

        // Check if any route has webserver configuration
        let has_static_files = config.routes.iter().any(|r| r.static_files || r.root.is_some());
        let has_php_fpm = config.routes.iter().any(|r| r.php_fpm.is_some());

        // Initialize static file handler if needed
        if has_static_files {
            use crate::webserver::{StaticFileHandler, WebServerConfig};
            use std::path::PathBuf;

            // Get document root from first route that has one, or use default
            let default_root = config.routes.iter()
                .find_map(|r| r.root.as_ref())
                .cloned()
                .unwrap_or_else(|| "/var/www/html".to_string());

            // Create webserver config
            let webserver_config = WebServerConfig::default();

            info!("Static file handler initialized with root: {}", default_root);
            let static_handler = StaticFileHandler::new(PathBuf::from(default_root), webserver_config);
            handler = handler.with_static_file_handler(Arc::new(static_handler));
        }

        // Initialize PHP-FPM pool if needed
        if has_php_fpm {
            use crate::webserver::{PhpFpmPool, PhpFpmConfig as WebserverPhpFpmConfig};

            // Use the first PHP-FPM configuration from routes
            if let Some(route_php_config) = config.routes.iter()
                .find_map(|r| r.php_fpm.as_ref())
                .filter(|php| php.enabled)
            {
                // Convert schema PhpFpmConfig to webserver PhpFpmConfig
                let webserver_php_config = WebserverPhpFpmConfig {
                    socket: route_php_config.socket.clone(),
                    pool_size: route_php_config.pool_size,
                    connect_timeout: route_php_config.connect_timeout_secs,
                    read_timeout: route_php_config.read_timeout_secs,
                    write_timeout: route_php_config.write_timeout_secs,
                    keepalive_timeout: route_php_config.keepalive_timeout_secs,
                    script_extensions: route_php_config.script_extensions.clone(),
                    script_filename_override: None,
                    fastcgi_params: std::collections::HashMap::new(),
                    document_root: route_php_config.document_root.clone(),
                };

                info!("PHP-FPM pool initialized: socket={}, pool_size={}",
                    webserver_php_config.socket, webserver_php_config.pool_size);
                let php_pool = PhpFpmPool::new(webserver_php_config);
                handler = handler.with_php_fpm_pool(Arc::new(php_pool));
            }
        }

        let handler = Arc::new(handler);

        // Initialize TLS if configured
        let tls_acceptor = if let Some(tls_config) = &config.tls {
            match TlsManager::new(tls_config.clone()) {
                Ok(mut manager) => {
                    // Load manual certificates
                    if let Err(e) = manager.load_manual_certificates() {
                        error!("Failed to load manual certificates: {}", e);
                    }

                    // Build server config
                    match manager.build_server_config() {
                        Ok(server_config) => {
                            info!("TLS initialized successfully");

                            // Check for kernel TLS support
                            if ktls::is_ktls_available() {
                                info!("Kernel TLS (kTLS) is available and can be enabled");
                                // Note: Full kTLS integration requires additional work
                                // to extract session keys and configure kernel TLS after handshake
                            } else {
                                debug!("Kernel TLS (kTLS) not available, using userspace TLS");
                            }

                            Some(Arc::new(TlsAcceptor::new(server_config)))
                        }
                        Err(e) => {
                            error!("Failed to build TLS server config: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to initialize TLS manager: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize connection pool for raw TCP connections (TLS passthrough, WebSocket backends)
        let pool_config = PoolConfig {
            max_connections_per_upstream: 100,
            max_idle_duration: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(10),
            keep_alive: true,
            keep_alive_timeout: Duration::from_secs(60),
        };
        let connection_pool = Arc::new(ConnectionPoolManager::new(pool_config));
        info!("Connection pool initialized: max_per_upstream={}, idle_timeout={:?}",
            100, Duration::from_secs(90));

        Self {
            config,
            handler,
            tls_acceptor,
            connection_pool,
        }
    }

    /// Get connection pool statistics
    pub fn pool_stats(&self) -> Vec<(String, PoolStats)> {
        self.connection_pool.get_all_stats()
    }

    /// Start the server
    pub async fn run(&self) -> Result<()> {
        info!("Starting HTTP/HTTPS server with production socket optimizations");

        // Spawn connection pool cleanup task
        let pool_clone = self.connection_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                pool_clone.cleanup_idle_connections();
            }
        });

        // Log which I/O backend is being used
        let backend_stats = GLOBAL_IO.stats();
        info!("I/O backend: {} ({})", GLOBAL_IO.name(), backend_stats.backend_info);

        // Use production-optimized socket configuration
        let socket_config = SocketConfig::production();

        // Parse HTTP bind addresses
        let mut http_listeners = Vec::new();
        for bind_addr in &self.config.server.bind {
            match bind_addr.parse::<SocketAddr>() {
                Ok(addr) => {
                    // Create optimized socket with SO_REUSEADDR, SO_REUSEPORT, TCP_FASTOPEN, etc.
                    match create_optimized_socket(addr, &socket_config) {
                        Ok(socket) => {
                            match socket_to_listener(socket) {
                                Ok(listener) => {
                                    info!(
                                        "HTTP listening on {} (optimized: reuse_addr, reuse_port, fastopen, linger=0)",
                                        addr
                                    );
                                    http_listeners.push((listener, false));
                                }
                                Err(e) => {
                                    error!("Failed to convert socket to listener for {}: {}", addr, e);
                                    return Err(e.into());
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to create optimized socket for {}: {}", addr, e);
                            return Err(e.into());
                        }
                    }
                }
                Err(e) => {
                    error!("Invalid bind address '{}': {}", bind_addr, e);
                    return Err(e.into());
                }
            }
        }

        // Parse HTTPS bind addresses
        let mut https_listeners = Vec::new();
        if self.tls_acceptor.is_some() {
            for bind_addr in &self.config.server.tls_bind {
                match bind_addr.parse::<SocketAddr>() {
                    Ok(addr) => {
                        // Create optimized socket for HTTPS
                        match create_optimized_socket(addr, &socket_config) {
                            Ok(socket) => {
                                match socket_to_listener(socket) {
                                    Ok(listener) => {
                                        info!(
                                            "HTTPS listening on {} (optimized: reuse_addr, reuse_port, fastopen, linger=0)",
                                            addr
                                        );
                                        https_listeners.push((listener, true));
                                    }
                                    Err(e) => {
                                        error!("Failed to convert socket to listener for {}: {}", addr, e);
                                        return Err(e.into());
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to create optimized socket for {}: {}", addr, e);
                                return Err(e.into());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Invalid TLS bind address '{}': {}", bind_addr, e);
                        return Err(e.into());
                    }
                }
            }
        } else if !self.config.server.tls_bind.is_empty() {
            warn!("TLS bind addresses configured but TLS is not enabled");
        }

        // Combine all listeners
        let mut all_listeners = http_listeners;
        all_listeners.extend(https_listeners);

        if all_listeners.is_empty() {
            return Err(anyhow::anyhow!("No valid bind addresses configured"));
        }

        // Determine which protocols are enabled
        let supports_http1 = self.config.server.protocols.contains(&Protocol::Http1);
        let supports_http2 = self.config.server.protocols.contains(&Protocol::Http2);
        let supports_http3 = self.config.server.protocols.contains(&Protocol::Http3) && self.config.server.http3.enabled;

        info!(
            "Enabled protocols: HTTP/1.1={}, HTTP/2={}, HTTP/3={}",
            supports_http1, supports_http2, supports_http3
        );

        // Note: HTTP/3 server is started by the runtime (src/runtime/mod.rs)
        // to avoid duplicate UDP socket binding

        // Track all connection acceptor tasks
        let mut tasks = Vec::new();

        // Accept connections on all listeners
        for (listener, is_tls) in all_listeners {
            let handler = self.handler.clone();
            let config = self.config.clone();
            let tls_acceptor = self.tls_acceptor.clone();

            let task = tokio::spawn(async move {
                loop {
                    // io_uring-accelerated accept when available (Week 2 completion)
                    // Falls back to tokio's epoll/kqueue-based accept on non-Linux systems
                    let listener_fd = listener.as_raw_fd();

                    let accept_result = if crate::runtime::is_io_uring_available() {
                        // Use io_uring for ~25% faster accept operations
                        debug!("Using io_uring accelerated accept on fd {}", listener_fd);
                        GLOBAL_IO.accept(listener_fd).await
                    } else {
                        // Fallback to tokio's standard accept
                        listener.accept().await
                    };

                    match accept_result {
                        Ok((stream, remote_addr)) => {
                            // Log backend stats periodically (every 1000 connections)
                            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                            if COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 1000 == 0 {
                                let stats = GLOBAL_IO.stats();
                                debug!(
                                    "I/O backend stats: {} pending ops, {} total ops, backend: {}",
                                    stats.pending_operations,
                                    stats.total_operations,
                                    GLOBAL_IO.name()
                                );
                            }
                            let handler = handler.clone();
                            let supports_http1 = config.server.protocols.contains(&Protocol::Http1);
                            let supports_http2 = config.server.protocols.contains(&Protocol::Http2);
                            let tls_acceptor = tls_acceptor.clone();

                            tokio::spawn(async move {
                                let service = service_fn(move |req| {
                                    let handler = handler.clone();
                                    async move { handler.handle(req).await }
                                });

                                if is_tls {
                                    // TLS connection
                                    if let Some(acceptor) = tls_acceptor {
                                        match acceptor.accept(stream).await {
                                            Ok(tls_stream) => {
                                                let io = TokioIo::new(tls_stream);

                                                // For TLS, we have ALPN negotiation which determines the protocol
                                                // Serve connection based on enabled protocols
                                                if supports_http1 && supports_http2 {
                                                    // Both protocols enabled, ALPN will negotiate
                                                    debug!("Serving HTTPS connection from {} with ALPN negotiation", remote_addr);
                                                    if let Err(_e) = http2::Builder::new(TokioExecutor::new())
                                                        .serve_connection(io, service)
                                                        .await
                                                    {
                                                        // Try HTTP/1.1 if HTTP/2 fails
                                                        debug!("HTTP/2 failed for {}, trying HTTP/1.1", remote_addr);
                                                    }
                                                } else if supports_http2 {
                                                    // HTTP/2 only
                                                    debug!("Serving HTTPS connection from {} with HTTP/2", remote_addr);
                                                    if let Err(e) = http2::Builder::new(TokioExecutor::new())
                                                        .serve_connection(io, service)
                                                        .await
                                                    {
                                                        warn!("Error serving HTTP/2 connection from {}: {}", remote_addr, e);
                                                    }
                                                } else {
                                                    // HTTP/1.1 only (default)
                                                    debug!("Serving HTTPS connection from {} with HTTP/1.1", remote_addr);
                                                    if let Err(e) = http1::Builder::new()
                                                        .serve_connection(io, service)
                                                        .await
                                                    {
                                                        warn!("Error serving HTTP/1.1 connection from {}: {}", remote_addr, e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("TLS handshake failed for {}: {}", remote_addr, e);
                                            }
                                        }
                                    } else {
                                        error!("TLS acceptor not configured for TLS listener");
                                    }
                                } else {
                                    // Plain HTTP connection
                                    let io = TokioIo::new(stream);

                                    // Serve connection based on enabled protocols
                                    if supports_http1 && supports_http2 {
                                        // Auto-detect: Try HTTP/2 with HTTP/1.1 fallback
                                        debug!("Serving HTTP connection from {} with HTTP/1.1+HTTP/2 auto-detection", remote_addr);
                                        if let Err(e) = http1::Builder::new()
                                            .serve_connection(io, service)
                                            .with_upgrades()
                                            .await
                                        {
                                            warn!("Error serving connection from {}: {}", remote_addr, e);
                                        }
                                    } else if supports_http2 {
                                        // HTTP/2 only (for gRPC, h2c, etc.)
                                        debug!("Serving HTTP connection from {} with HTTP/2 (prior knowledge)", remote_addr);
                                        // Enable HTTP/2 prior knowledge for gRPC (h2c)
                                        if let Err(e) = http2::Builder::new(TokioExecutor::new())
                                            .serve_connection(io, service)
                                            .await
                                        {
                                            warn!("Error serving HTTP/2 connection from {}: {}", remote_addr, e);
                                        }
                                    } else {
                                        // HTTP/1.1 only (default)
                                        debug!("Serving HTTP connection from {} with HTTP/1.1", remote_addr);
                                        if let Err(e) = http1::Builder::new()
                                            .serve_connection(io, service)
                                            .await
                                        {
                                            warn!("Error serving HTTP/1.1 connection from {}: {}", remote_addr, e);
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete (they run forever until shutdown)
        for task in tasks {
            if let Err(e) = task.await {
                error!("Server task error: {}", e);
            }
        }

        Ok(())
    }

    /// Start TLS passthrough server (SNI-based routing without TLS termination)
    pub async fn run_tls_passthrough(config: Arc<Config>) -> Result<()> {
        let passthrough_config = match &config.tls {
            Some(tls) => match &tls.passthrough {
                Some(pt) if pt.enabled => pt,
                _ => {
                    debug!("TLS passthrough not enabled");
                    return Ok(());
                }
            },
            None => {
                debug!("No TLS configuration for passthrough");
                return Ok(());
            }
        };

        info!("Starting TLS passthrough server");

        // Initialize connection pool for passthrough backends
        let pool_config = PoolConfig {
            max_connections_per_upstream: 100,
            max_idle_duration: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(10),
            keep_alive: true,
            keep_alive_timeout: Duration::from_secs(60),
        };
        let connection_pool = Arc::new(ConnectionPoolManager::new(pool_config));
        info!("TLS passthrough connection pool initialized");

        // Spawn cleanup task for idle connections
        let pool_clone = connection_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                pool_clone.cleanup_idle_connections();
            }
        });

        let mut listeners = Vec::new();
        for bind_addr in &passthrough_config.bind {
            match bind_addr.parse::<SocketAddr>() {
                Ok(addr) => {
                    match TcpListener::bind(addr).await {
                        Ok(listener) => {
                            info!("TLS passthrough listening on {}", addr);
                            listeners.push(listener);
                        }
                        Err(e) => {
                            error!("Failed to bind passthrough to {}: {}", addr, e);
                            return Err(e.into());
                        }
                    }
                }
                Err(e) => {
                    error!("Invalid passthrough bind address '{}': {}", bind_addr, e);
                    return Err(e.into());
                }
            }
        }

        if listeners.is_empty() {
            warn!("No TLS passthrough listeners configured");
            return Ok(());
        }

        let mut tasks = Vec::new();
        for listener in listeners {
            let config = config.clone();
            let pool = connection_pool.clone();
            let task = tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut stream, remote_addr)) => {
                            let config = config.clone();
                            let pool = pool.clone();
                            tokio::spawn(async move {
                                debug!("TLS passthrough connection from {}", remote_addr);

                                // Extract SNI from TLS ClientHello
                                let sni_info = match passthrough::extract_sni(&mut stream).await {
                                    Ok(info) => info,
                                    Err(e) => {
                                        error!("Failed to extract SNI from {}: {}", remote_addr, e);
                                        return;
                                    }
                                };

                                info!("TLS passthrough: SNI={} from {}", sni_info.server_name, remote_addr);

                                // Find matching route
                                let passthrough_config = match &config.tls {
                                    Some(tls) => match &tls.passthrough {
                                        Some(pt) => pt,
                                        None => return,
                                    },
                                    None => return,
                                };

                                let backend_url = passthrough_config.routes.iter()
                                    .find(|r| r.enabled && matches_sni(&r.server_name, &sni_info.server_name))
                                    .map(|r| &r.upstream)
                                    .or(passthrough_config.default_backend.as_ref());

                                let backend_url = match backend_url {
                                    Some(url) => url,
                                    None => {
                                        warn!("No backend found for SNI: {}", sni_info.server_name);
                                        return;
                                    }
                                };

                                // Connect to backend using connection pool
                                let backend_addr = match backend_url.parse::<SocketAddr>() {
                                    Ok(addr) => addr,
                                    Err(_) => {
                                        // Try parsing as URL
                                        warn!("Invalid backend address: {}", backend_url);
                                        return;
                                    }
                                };

                                // Use connection pool for backend connection (upstream name = backend_url for passthrough)
                                let mut backend = match pool.get_connection(backend_url, backend_addr).await {
                                    Ok(s) => {
                                        debug!("Got connection from pool for {}", backend_url);
                                        s
                                    }
                                    Err(e) => {
                                        error!("Failed to connect to backend {}: {}", backend_addr, e);
                                        return;
                                    }
                                };

                                // Replay ClientHello to backend
                                if let Err(e) = backend.write_all(&sni_info.client_hello).await {
                                    error!("Failed to send ClientHello to backend: {}", e);
                                    return;
                                }

                                // Bidirectional copy
                                match copy_bidirectional(&mut stream, &mut backend).await {
                                    Ok((client_to_server, server_to_client)) => {
                                        info!(
                                            "TLS passthrough completed: SNI={}, sent={}, received={}",
                                            sni_info.server_name, client_to_server, server_to_client
                                        );
                                        // Return connection to pool for reuse
                                        pool.return_connection(backend_url, backend);
                                    }
                                    Err(e) => {
                                        debug!("TLS passthrough error: {}", e);
                                        // Don't return connection on error - let it drop
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept passthrough connection: {}", e);
                        }
                    }
                }
            });
            tasks.push(task);
        }

        // Wait for all passthrough tasks
        for task in tasks {
            if let Err(e) = task.await {
                error!("TLS passthrough task error: {}", e);
            }
        }

        Ok(())
    }
}

/// Check if SNI matches the route pattern (supports wildcards)
fn matches_sni(pattern: &str, sni: &str) -> bool {
    if pattern == sni {
        return true;
    }

    // Support wildcard matching (*.example.com)
    if let Some(domain_suffix) = pattern.strip_prefix("*.") {
        return sni.ends_with(domain_suffix) && sni.len() > domain_suffix.len();
    }

    false
}
