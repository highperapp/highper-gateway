//! High-performance TCP proxy with zero-copy forwarding
//!
//! Implements bidirectional TCP forwarding with minimal overhead.
//! Target: < 0.5ms P99 latency overhead

use super::{LoadBalancingAlgorithm, Protocol, TcpBackend, TcpConfig, TcpStats};
use crate::state::ProxyState;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, error};

/// TCP proxy instance
pub struct TcpProxy {
    config: Arc<TcpConfig>,
    stats: Arc<TcpProxyStats>,
    backend_selector: Arc<BackendSelector>,
    _proxy_state: Option<Arc<ProxyState>>,
}

/// TCP proxy statistics
#[derive(Debug)]
pub struct TcpProxyStats {
    /// Total connections accepted
    pub total_connections: AtomicU64,

    /// Active connections
    pub active_connections: AtomicU64,

    /// Total bytes received from clients
    pub bytes_received: AtomicU64,

    /// Total bytes sent to clients
    pub bytes_sent: AtomicU64,

    /// Connection errors
    pub connection_errors: AtomicU64,

    /// Backend connection failures
    pub backend_failures: AtomicU64,

    /// Total requests proxied
    pub requests_proxied: AtomicU64,
}

impl TcpProxyStats {
    pub fn new() -> Self {
        Self {
            total_connections: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            backend_failures: AtomicU64::new(0),
            requests_proxied: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> TcpStats {
        TcpStats {
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            connection_errors: self.connection_errors.load(Ordering::Relaxed),
            backend_failures: self.backend_failures.load(Ordering::Relaxed),
            requests_proxied: self.requests_proxied.load(Ordering::Relaxed),
            avg_connection_duration_ms: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
        }
    }
}

impl Default for TcpProxyStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Backend selector for load balancing
pub struct BackendSelector {
    backends: Vec<TcpBackend>,
    algorithm: LoadBalancingAlgorithm,
    round_robin_index: AtomicU64,
}

impl BackendSelector {
    pub fn new(backends: Vec<TcpBackend>, algorithm: LoadBalancingAlgorithm) -> Self {
        Self {
            backends,
            algorithm,
            round_robin_index: AtomicU64::new(0),
        }
    }

    /// Select a backend based on load balancing algorithm
    pub fn select(&self, client_addr: Option<SocketAddr>) -> Option<&TcpBackend> {
        if self.backends.is_empty() {
            return None;
        }

        match self.algorithm {
            LoadBalancingAlgorithm::RoundRobin => {
                let index = self.round_robin_index.fetch_add(1, Ordering::Relaxed);
                Some(&self.backends[index as usize % self.backends.len()])
            }
            LoadBalancingAlgorithm::Random => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let index = rng.gen_range(0..self.backends.len());
                Some(&self.backends[index])
            }
            LoadBalancingAlgorithm::IpHash => {
                if let Some(addr) = client_addr {
                    let hash = Self::hash_addr(&addr);
                    let index = hash % self.backends.len() as u64;
                    Some(&self.backends[index as usize])
                } else {
                    Some(&self.backends[0])
                }
            }
            LoadBalancingAlgorithm::WeightedRoundRobin => {
                // Simple weighted selection based on weight
                let total_weight: u32 = self.backends.iter().map(|b| b.weight).sum();
                if total_weight == 0 {
                    return Some(&self.backends[0]);
                }

                let index = self.round_robin_index.fetch_add(1, Ordering::Relaxed);
                let mut cumulative = 0u32;
                let target = (index % total_weight as u64) as u32;

                for backend in &self.backends {
                    cumulative += backend.weight;
                    if target < cumulative {
                        return Some(backend);
                    }
                }

                Some(&self.backends[0])
            }
            _ => Some(&self.backends[0]), // Fallback for other algorithms
        }
    }

    /// Hash socket address for consistent hashing
    fn hash_addr(addr: &SocketAddr) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        addr.ip().hash(&mut hasher);
        hasher.finish()
    }
}

impl TcpProxy {
    /// Create a new TCP proxy
    pub fn new(config: TcpConfig) -> Self {
        let backends: Vec<TcpBackend> = config
            .upstreams
            .iter()
            .flat_map(|u| u.backends.clone())
            .collect();

        let backend_selector = Arc::new(BackendSelector::new(
            backends,
            config.load_balancing,
        ));

        Self {
            config: Arc::new(config),
            stats: Arc::new(TcpProxyStats::new()),
            backend_selector,
            _proxy_state: None,
        }
    }

    /// Create TCP proxy with ProxyState integration
    pub fn with_state(config: TcpConfig, proxy_state: Arc<ProxyState>) -> Self {
        let mut proxy = Self::new(config);
        proxy._proxy_state = Some(proxy_state);
        proxy
    }

    /// Get proxy statistics
    pub fn stats(&self) -> TcpStats {
        self.stats.snapshot()
    }

    /// Handle a client connection
    pub async fn handle_connection(&self, mut client: TcpStream, client_addr: SocketAddr) -> Result<()> {
        let start = Instant::now();

        // Update stats
        self.stats.total_connections.fetch_add(1, Ordering::Relaxed);
        self.stats.active_connections.fetch_add(1, Ordering::Relaxed);

        debug!("Accepted TCP connection from {}", client_addr);

        // Select backend
        let backend = self
            .backend_selector
            .select(Some(client_addr))
            .context("No available backends")?;

        // Connect to backend with timeout
        let backend_result = timeout(
            self.config.connect_timeout,
            TcpStream::connect(backend.addr),
        )
        .await;

        let mut backend_stream = match backend_result {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                self.stats.backend_failures.fetch_add(1, Ordering::Relaxed);
                error!("Failed to connect to backend {}: {}", backend.addr, e);
                return Err(e.into());
            }
            Err(_) => {
                self.stats.backend_failures.fetch_add(1, Ordering::Relaxed);
                error!("Backend connection timeout to {}", backend.addr);
                return Err(anyhow::anyhow!("Backend connection timeout"));
            }
        };

        debug!("Connected to backend {} for client {}", backend.addr, client_addr);

        // Configure TCP options for both streams
        self.configure_stream(&client)?;
        self.configure_stream(&backend_stream)?;

        // Perform bidirectional forwarding
        let result = self
            .forward_bidirectional(&mut client, &mut backend_stream)
            .await;

        // Update stats
        self.stats.active_connections.fetch_sub(1, Ordering::Relaxed);
        self.stats.requests_proxied.fetch_add(1, Ordering::Relaxed);

        let duration = start.elapsed();
        debug!(
            "Connection from {} closed after {:?}",
            client_addr, duration
        );

        result
    }

    /// Configure TCP stream options
    fn configure_stream(&self, stream: &TcpStream) -> Result<()> {
        use socket2::SockRef;

        let sock_ref = SockRef::from(stream);

        // Set TCP_NODELAY (disable Nagle's algorithm)
        if self.config.nodelay {
            sock_ref.set_nodelay(true)?;
        }

        // Set SO_KEEPALIVE
        if self.config.keepalive {
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(Duration::from_secs(self.config.keepalive_time));

            sock_ref.set_tcp_keepalive(&keepalive)?;
        }

        Ok(())
    }

    /// Bidirectional forwarding with zero-copy optimization
    async fn forward_bidirectional(
        &self,
        client: &mut TcpStream,
        backend: &mut TcpStream,
    ) -> Result<()> {
        // Use tokio::io::copy_bidirectional for zero-copy forwarding
        let (client_to_backend, backend_to_client) = tokio::io::copy_bidirectional(client, backend)
            .await
            .context("Bidirectional copy failed")?;

        // Update byte counters
        self.stats
            .bytes_received
            .fetch_add(client_to_backend, Ordering::Relaxed);
        self.stats
            .bytes_sent
            .fetch_add(backend_to_client, Ordering::Relaxed);

        debug!(
            "Forwarded {} bytes client->backend, {} bytes backend->client",
            client_to_backend, backend_to_client
        );

        Ok(())
    }

    /// Protocol-aware forwarding (for future use)
    #[allow(dead_code)]
    async fn forward_protocol_aware(
        &self,
        client: &mut TcpStream,
        backend: &mut TcpStream,
        protocol: Protocol,
    ) -> Result<()> {
        match protocol {
            Protocol::Mysql => self.forward_mysql(client, backend).await,
            Protocol::Postgresql => self.forward_postgresql(client, backend).await,
            Protocol::Redis => self.forward_redis(client, backend).await,
            Protocol::Generic => self.forward_bidirectional(client, backend).await,
        }
    }

    /// MySQL-aware forwarding with query logging (future enhancement)
    #[allow(dead_code)]
    async fn forward_mysql(
        &self,
        client: &mut TcpStream,
        backend: &mut TcpStream,
    ) -> Result<()> {
        // For now, just do bidirectional forwarding
        // Future: Parse MySQL protocol, log queries, handle connection pooling
        self.forward_bidirectional(client, backend).await
    }

    /// PostgreSQL-aware forwarding (future enhancement)
    #[allow(dead_code)]
    async fn forward_postgresql(
        &self,
        client: &mut TcpStream,
        backend: &mut TcpStream,
    ) -> Result<()> {
        // For now, just do bidirectional forwarding
        // Future: Parse PostgreSQL protocol, log queries
        self.forward_bidirectional(client, backend).await
    }

    /// Redis-aware forwarding (future enhancement)
    #[allow(dead_code)]
    async fn forward_redis(
        &self,
        client: &mut TcpStream,
        backend: &mut TcpStream,
    ) -> Result<()> {
        // For now, just do bidirectional forwarding
        // Future: Parse RESP protocol, handle pipelining
        self.forward_bidirectional(client, backend).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_selector_round_robin() {
        let backends = vec![
            TcpBackend {
                addr: "127.0.0.1:3306".parse().unwrap(),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
            TcpBackend {
                addr: "127.0.0.1:3307".parse().unwrap(),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
        ];

        let selector = BackendSelector::new(backends, LoadBalancingAlgorithm::RoundRobin);

        let b1 = selector.select(None).unwrap();
        let b2 = selector.select(None).unwrap();
        let b3 = selector.select(None).unwrap();

        assert_eq!(b1.addr.port(), 3306);
        assert_eq!(b2.addr.port(), 3307);
        assert_eq!(b3.addr.port(), 3306);
    }

    #[test]
    fn test_backend_selector_ip_hash() {
        let backends = vec![
            TcpBackend {
                addr: "127.0.0.1:3306".parse().unwrap(),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
            TcpBackend {
                addr: "127.0.0.1:3307".parse().unwrap(),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
        ];

        let selector = BackendSelector::new(backends, LoadBalancingAlgorithm::IpHash);

        let client1: SocketAddr = "192.168.1.100:12345".parse().unwrap();
        let client2: SocketAddr = "192.168.1.101:12346".parse().unwrap();

        let b1 = selector.select(Some(client1)).unwrap();
        let b2 = selector.select(Some(client1)).unwrap();
        let b3 = selector.select(Some(client2)).unwrap();

        // Same client should get same backend
        assert_eq!(b1.addr, b2.addr);

        // Different clients may get different backends (not guaranteed, but likely)
        // We can't assert this as hash collision is possible
    }

    #[test]
    fn test_backend_selector_weighted() {
        let backends = vec![
            TcpBackend {
                addr: "127.0.0.1:3306".parse().unwrap(),
                weight: 3,
                max_conns: None,
                health_check: None,
            },
            TcpBackend {
                addr: "127.0.0.1:3307".parse().unwrap(),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
        ];

        let selector = BackendSelector::new(backends, LoadBalancingAlgorithm::WeightedRoundRobin);

        let mut counts = std::collections::HashMap::new();

        for _ in 0..100 {
            let backend = selector.select(None).unwrap();
            *counts.entry(backend.addr.port()).or_insert(0) += 1;
        }

        // Backend 1 (weight 3) should get roughly 3x more connections than backend 2 (weight 1)
        let count1 = counts.get(&3306).unwrap_or(&0);
        let count2 = counts.get(&3307).unwrap_or(&0);

        // Allow for some variance
        assert!(*count1 > *count2);
    }

    #[test]
    fn test_tcp_proxy_stats() {
        let stats = TcpProxyStats::new();

        stats.total_connections.store(100, Ordering::Relaxed);
        stats.active_connections.store(10, Ordering::Relaxed);
        stats.bytes_received.store(1024000, Ordering::Relaxed);
        stats.bytes_sent.store(2048000, Ordering::Relaxed);

        let snapshot = stats.snapshot();

        assert_eq!(snapshot.total_connections, 100);
        assert_eq!(snapshot.active_connections, 10);
        assert_eq!(snapshot.bytes_received, 1024000);
        assert_eq!(snapshot.bytes_sent, 2048000);
    }
}
