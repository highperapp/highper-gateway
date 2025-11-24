//! TCP Proxy Server
//!
//! High-performance TCP server with SO_REUSEPORT for multi-threaded accept

use super::{TcpConfig, TcpProxy};
use crate::state::ProxyState;
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// TCP Proxy Server
pub struct TcpProxyServer {
    config: Arc<TcpConfig>,
    proxy: Arc<TcpProxy>,
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

impl TcpProxyServer {
    /// Create a new TCP proxy server
    pub fn new(config: TcpConfig) -> Self {
        let proxy = Arc::new(TcpProxy::new(config.clone()));

        Self {
            config: Arc::new(config),
            proxy,
            shutdown_tx: None,
        }
    }

    /// Create TCP proxy server with ProxyState
    pub fn with_state(config: TcpConfig, proxy_state: Arc<ProxyState>) -> Self {
        let proxy = Arc::new(TcpProxy::with_state(config.clone(), proxy_state));

        Self {
            config: Arc::new(config),
            proxy,
            shutdown_tx: None,
        }
    }

    /// Start the TCP proxy server
    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Starting TCP proxy server on {} (protocol: {})",
            self.config.bind,
            self.config.protocol.as_str()
        );

        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Bind listener
        let listener = self.create_listener().await?;

        info!(
            "TCP proxy server listening on {} (reuseport: {})",
            self.config.bind, self.config.reuseport
        );

        // Accept loop
        self.accept_loop(listener, shutdown_tx).await
    }

    /// Create TCP listener with optional SO_REUSEPORT
    async fn create_listener(&self) -> Result<TcpListener> {
        if self.config.reuseport {
            // Use socket2 for SO_REUSEPORT
            self.create_reuseport_listener().await
        } else {
            // Standard Tokio listener
            TcpListener::bind(self.config.bind)
                .await
                .context("Failed to bind TCP listener")
        }
    }

    /// Create listener with SO_REUSEPORT for multi-threaded accept
    async fn create_reuseport_listener(&self) -> Result<TcpListener> {
        use socket2::{Domain, Protocol, Socket, Type};
        use std::net::SocketAddr;

        let addr: SocketAddr = self.config.bind;

        // Create socket with SO_REUSEPORT
        let socket = Socket::new(
            if addr.is_ipv4() {
                Domain::IPV4
            } else {
                Domain::IPV6
            },
            Type::STREAM,
            Some(Protocol::TCP),
        )?;

        // Set SO_REUSEPORT (Linux) or SO_REUSEADDR (other platforms)
        #[cfg(target_os = "linux")]
        socket.set_reuse_port(true)?;

        #[cfg(not(target_os = "linux"))]
        socket.set_reuse_address(true)?;

        // Bind and listen
        socket.bind(&addr.into())?;
        socket.listen(1024)?; // Backlog of 1024

        // Convert to Tokio TcpListener
        socket.set_nonblocking(true)?;
        let std_listener: std::net::TcpListener = socket.into();
        TcpListener::from_std(std_listener).context("Failed to create Tokio listener")
    }

    /// Main accept loop
    async fn accept_loop(
        &self,
        listener: TcpListener,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Result<()> {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let proxy = self.proxy.clone();
        let max_concurrent = self.config.max_concurrent_connections;

        // Semaphore to limit concurrent connections
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            // Acquire permit for connection
                            let permit = match semaphore.clone().try_acquire_owned() {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!("Max concurrent connections reached ({}), rejecting connection from {}", max_concurrent, addr);
                                    continue;
                                }
                            };

                            // Spawn handler task
                            let proxy_clone = proxy.clone();
                            tokio::spawn(async move {
                                if let Err(e) = proxy_clone.handle_connection(stream, addr).await {
                                    error!("Connection handler error for {}: {}", addr, e);
                                }
                                // Permit is automatically released when dropped
                                drop(permit);
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }

                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping TCP proxy server");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
            info!("TCP proxy server shutdown initiated");
        }
    }

    /// Get server statistics
    pub fn stats(&self) -> super::TcpStats {
        self.proxy.stats()
    }

    /// Spawn server in background
    pub fn spawn(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move { self.start().await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_server_creation() {
        let config = TcpConfig {
            bind: "127.0.0.1:0".parse().unwrap(), // Random port
            upstreams: vec![],
            load_balancing: super::super::LoadBalancingAlgorithm::RoundRobin,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            enable_pooling: false,
            max_connections_per_backend: 1000,
            min_idle_connections: 10,
            max_idle_connections: 100,
            connection_lifetime: Duration::from_secs(3600),
            enable_health_checks: false,
            health_check_interval: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(5),
            protocol: super::super::Protocol::Generic,
            reuseport: false,
            buffer_size: 8192,
            nodelay: true,
            keepalive: true,
            keepalive_time: 60,
            max_concurrent_connections: 10000,
        };

        let server = TcpProxyServer::new(config);
        assert!(server.shutdown_tx.is_none());
    }

    #[tokio::test]
    async fn test_server_stats() {
        let config = TcpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            upstreams: vec![],
            load_balancing: super::super::LoadBalancingAlgorithm::RoundRobin,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            enable_pooling: false,
            max_connections_per_backend: 1000,
            min_idle_connections: 10,
            max_idle_connections: 100,
            connection_lifetime: Duration::from_secs(3600),
            enable_health_checks: false,
            health_check_interval: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(5),
            protocol: super::super::Protocol::Generic,
            reuseport: false,
            buffer_size: 8192,
            nodelay: true,
            keepalive: true,
            keepalive_time: 60,
            max_concurrent_connections: 10000,
        };

        let server = TcpProxyServer::new(config);
        let stats = server.stats();

        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.active_connections, 0);
    }
}
