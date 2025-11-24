//! Load balancing algorithms

use crate::config::{LoadBalancingAlgorithm, ServerDef};
use crate::proxy::geographic::{GeoLoadBalancer, GeoServer};
use crate::state::ProxyState;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;

/// Load balancer for selecting backend servers
pub struct LoadBalancer {
    algorithm: LoadBalancingAlgorithm,
    servers: Vec<Arc<BackendServer>>,
    round_robin_counter: AtomicUsize,
    consistent_hash_ring: RwLock<Vec<(u64, usize)>>,
    maglev_table: RwLock<Vec<usize>>,
    geo_lb: Option<GeoLoadBalancer>,
    geo_servers: Vec<GeoServer>,
    upstream_name: String,
    proxy_state: Option<Arc<ProxyState>>,
}

/// Backend server with connection tracking
pub struct BackendServer {
    pub server: ServerDef,
    active_connections: AtomicUsize,
}

impl BackendServer {
    pub fn new(server: ServerDef) -> Self {
        Self {
            server,
            active_connections: AtomicUsize::new(0),
        }
    }

    /// Increment active connections
    pub fn acquire(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections
    pub fn release(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current connection count
    pub fn connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(algorithm: LoadBalancingAlgorithm, servers: Vec<ServerDef>) -> Self {
        Self::with_geoip_config(algorithm, servers, crate::config::GeoIpProvider::MaxMind, None::<String>)
    }

    /// Create a new load balancer with GeoIP database path
    pub fn with_geoip<P: AsRef<std::path::Path>>(
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        geoip_db_path: Option<P>,
    ) -> Self {
        Self::with_geoip_config(algorithm, servers, crate::config::GeoIpProvider::MaxMind, geoip_db_path)
    }

    /// Create a new load balancer with full GeoIP configuration
    pub fn with_geoip_config<P: AsRef<std::path::Path>>(
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        geoip_provider: crate::config::GeoIpProvider,
        geoip_db_path: Option<P>,
    ) -> Self {
        let backend_servers: Vec<Arc<BackendServer>> = servers
            .into_iter()
            .map(|s| Arc::new(BackendServer::new(s)))
            .collect();

        // Build consistent hash ring if needed
        let mut hash_ring = Vec::new();
        if algorithm == LoadBalancingAlgorithm::ConsistentHash {
            hash_ring = Self::build_hash_ring(&backend_servers);
        }

        // Build Maglev lookup table if needed
        let mut maglev_table = Vec::new();
        if algorithm == LoadBalancingAlgorithm::Maglev {
            maglev_table = Self::build_maglev_table(&backend_servers);
        }

        // Build geographic load balancer if needed
        let (geo_lb, geo_servers) = if algorithm == LoadBalancingAlgorithm::Geographic {
            match GeoLoadBalancer::new(geoip_provider, geoip_db_path) {
                Ok(lb) => {
                    let geo_servers: Vec<GeoServer> = backend_servers
                        .iter()
                        .enumerate()
                        .filter_map(|(index, server)| {
                            server.server.location.as_ref().map(|loc| GeoServer {
                                index,
                                location: loc.clone(),
                                region: server.server.region.clone(),
                            })
                        })
                        .collect();

                    (Some(lb), geo_servers)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize geographic load balancer: {}", e);
                    (None, Vec::new())
                }
            }
        } else {
            (None, Vec::new())
        };

        Self {
            algorithm,
            servers: backend_servers,
            round_robin_counter: AtomicUsize::new(0),
            consistent_hash_ring: RwLock::new(hash_ring),
            maglev_table: RwLock::new(maglev_table),
            geo_lb,
            geo_servers,
            upstream_name: "default".to_string(),
            proxy_state: None,
        }
    }

    /// Create a new load balancer with ProxyState integration
    pub fn with_state(
        upstream_name: String,
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        proxy_state: Arc<ProxyState>,
    ) -> Self {
        let mut lb = Self::new(algorithm, servers);
        lb.upstream_name = upstream_name;
        lb.proxy_state = Some(proxy_state);
        lb
    }

    /// Create a new load balancer with full configuration including state
    pub fn with_full_config<P: AsRef<std::path::Path>>(
        upstream_name: String,
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        geoip_provider: crate::config::GeoIpProvider,
        geoip_db_path: Option<P>,
        proxy_state: Option<Arc<ProxyState>>,
    ) -> Self {
        let mut lb = Self::with_geoip_config(algorithm, servers, geoip_provider, geoip_db_path);
        lb.upstream_name = upstream_name;
        lb.proxy_state = proxy_state;
        lb
    }

    /// Check if a backend is available (enabled and not draining)
    async fn is_backend_available(&self, index: usize) -> bool {
        if let Some(ref state) = self.proxy_state {
            let backend_id = format!("{}_{}", self.upstream_name, index);
            if let Some(backend_state) = state.get_backend(&backend_id).await {
                return backend_state.enabled && !backend_state.draining;
            }
        }
        true // If no state, assume available
    }

    /// Select a backend server based on the configured algorithm (async version)
    pub async fn select_async(&self, client_ip: Option<&str>, request_key: Option<&str>) -> Option<Arc<BackendServer>> {
        if self.servers.is_empty() {
            return None;
        }

        let selected = match self.algorithm {
            LoadBalancingAlgorithm::RoundRobin => self.round_robin(),
            LoadBalancingAlgorithm::LeastConn => self.least_connections(),
            LoadBalancingAlgorithm::Random => self.random(),
            LoadBalancingAlgorithm::IpHash => {
                if let Some(ip) = client_ip {
                    self.ip_hash(ip)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::ConsistentHash => {
                if let Some(key) = request_key {
                    self.consistent_hash(key)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::Maglev => {
                if let Some(key) = request_key.or(client_ip) {
                    self.maglev(key)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::PowerOfTwo => self.power_of_two(),
            LoadBalancingAlgorithm::Geographic => {
                if let Some(lb) = &self.geo_lb {
                    if let Some(index) = lb.select_nearest(client_ip, &self.geo_servers) {
                        if self.is_backend_available(index).await {
                            return Some(self.servers[index].clone());
                        }
                    }
                }
                // Fallback to round-robin if geographic selection fails
                self.round_robin()
            }
        };

        // Verify the selected backend is available
        // If not, try to find an available one
        if let Some(ref backend) = selected {
            let index = self.servers.iter().position(|s| Arc::ptr_eq(s, backend))?;
            if !self.is_backend_available(index).await {
                // Try to find another available backend
                return self.find_available_backend().await;
            }
        }

        selected
    }

    /// Select a backend server (synchronous version for backward compatibility)
    pub fn select(&self, client_ip: Option<&str>, request_key: Option<&str>) -> Option<Arc<BackendServer>> {
        if self.servers.is_empty() {
            return None;
        }

        // Simple selection without state checking for sync context
        match self.algorithm {
            LoadBalancingAlgorithm::RoundRobin => self.round_robin(),
            LoadBalancingAlgorithm::LeastConn => self.least_connections(),
            LoadBalancingAlgorithm::Random => self.random(),
            LoadBalancingAlgorithm::IpHash => {
                if let Some(ip) = client_ip {
                    self.ip_hash(ip)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::ConsistentHash => {
                if let Some(key) = request_key {
                    self.consistent_hash(key)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::Maglev => {
                if let Some(key) = request_key.or(client_ip) {
                    self.maglev(key)
                } else {
                    self.round_robin()
                }
            }
            LoadBalancingAlgorithm::PowerOfTwo => self.power_of_two(),
            LoadBalancingAlgorithm::Geographic => {
                if let Some(lb) = &self.geo_lb {
                    if let Some(index) = lb.select_nearest(client_ip, &self.geo_servers) {
                        return Some(self.servers[index].clone());
                    }
                }
                self.round_robin()
            }
        }
    }

    /// Find any available backend (not disabled/draining)
    async fn find_available_backend(&self) -> Option<Arc<BackendServer>> {
        for (index, server) in self.servers.iter().enumerate() {
            if self.is_backend_available(index).await {
                return Some(server.clone());
            }
        }
        None
    }

    /// Update ProxyState with current connection counts
    pub async fn sync_connection_counts(&self) {
        if let Some(ref state) = self.proxy_state {
            for (index, server) in self.servers.iter().enumerate() {
                let backend_id = format!("{}_{}", self.upstream_name, index);
                let connections = server.connections();
                let _ = state.set_backend_connections(&backend_id, connections).await;
            }
        }
    }

    /// Get upstream name
    pub fn upstream_name(&self) -> &str {
        &self.upstream_name
    }

    /// Get number of backends
    pub fn backend_count(&self) -> usize {
        self.servers.len()
    }

    /// Round-robin load balancing
    fn round_robin(&self) -> Option<Arc<BackendServer>> {
        let idx = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % self.servers.len();
        Some(self.servers[idx].clone())
    }

    /// Least connections load balancing
    fn least_connections(&self) -> Option<Arc<BackendServer>> {
        let mut min_conn = usize::MAX;
        let mut selected_idx = 0;

        for (idx, server) in self.servers.iter().enumerate() {
            let conns = server.connections();
            if conns < min_conn {
                min_conn = conns;
                selected_idx = idx;
            }
        }

        Some(self.servers[selected_idx].clone())
    }

    /// Random load balancing
    fn random(&self) -> Option<Arc<BackendServer>> {
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as usize;
        let idx = seed % self.servers.len();
        Some(self.servers[idx].clone())
    }

    /// IP hash load balancing
    fn ip_hash(&self, client_ip: &str) -> Option<Arc<BackendServer>> {
        let mut hasher = DefaultHasher::new();
        client_ip.hash(&mut hasher);
        let hash = hasher.finish();
        let idx = (hash as usize) % self.servers.len();
        Some(self.servers[idx].clone())
    }

    /// Consistent hash load balancing
    fn consistent_hash(&self, key: &str) -> Option<Arc<BackendServer>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let ring = self.consistent_hash_ring.read();

        // Binary search to find the first hash >= target
        let idx = ring.binary_search_by_key(&hash, |(h, _)| *h)
            .unwrap_or_else(|i| if i == ring.len() { 0 } else { i });

        let server_idx = ring[idx].1;
        Some(self.servers[server_idx].clone())
    }

    /// Power of two choices load balancing
    fn power_of_two(&self) -> Option<Arc<BackendServer>> {
        use std::time::SystemTime;

        if self.servers.len() <= 1 {
            return self.servers.first().cloned();
        }

        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as usize;

        // Pick two random servers
        let idx1 = seed % self.servers.len();
        let idx2 = (seed / self.servers.len()) % self.servers.len();

        let server1 = &self.servers[idx1];
        let server2 = &self.servers[idx2];

        // Choose the one with fewer connections
        if server1.connections() <= server2.connections() {
            Some(server1.clone())
        } else {
            Some(server2.clone())
        }
    }

    /// Build consistent hash ring with virtual nodes
    fn build_hash_ring(servers: &[Arc<BackendServer>]) -> Vec<(u64, usize)> {
        const VIRTUAL_NODES: usize = 150; // Virtual nodes per server

        let mut ring = Vec::new();

        for (server_idx, server) in servers.iter().enumerate() {
            for vnode in 0..VIRTUAL_NODES {
                let key = format!("{}:{}", server.server.url, vnode);
                let mut hasher = DefaultHasher::new();
                key.hash(&mut hasher);
                let hash = hasher.finish();
                ring.push((hash, server_idx));
            }
        }

        // Sort by hash value
        ring.sort_by_key(|(hash, _)| *hash);
        ring
    }

    /// Maglev consistent hashing
    ///
    /// Google's Maglev algorithm provides:
    /// - Minimal disruption when backends change (only K/N entries reassigned)
    /// - Excellent load distribution
    /// - Fast lookup (O(1) using lookup table)
    ///
    /// Reference: https://static.googleusercontent.com/media/research.google.com/en//pubs/archive/44824.pdf
    fn maglev(&self, key: &str) -> Option<Arc<BackendServer>> {
        let table = self.maglev_table.read();

        if table.is_empty() || self.servers.is_empty() {
            return None;
        }

        // Hash the key
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Lookup in Maglev table
        let idx = (hash as usize) % table.len();
        let server_idx = table[idx];

        Some(self.servers[server_idx].clone())
    }

    /// Build Maglev lookup table
    ///
    /// The Maglev algorithm builds a lookup table of size M (typically a prime number)
    /// where each entry points to a backend server. The table is populated using a
    /// permutation-based approach that ensures minimal disruption when backends change.
    fn build_maglev_table(servers: &[Arc<BackendServer>]) -> Vec<usize> {
        const TABLE_SIZE: usize = 65537; // Prime number for better distribution

        if servers.is_empty() {
            return Vec::new();
        }

        let n = servers.len();
        let mut table = vec![None; TABLE_SIZE];
        let mut next = vec![0usize; n];

        // Generate permutation for each backend
        let permutations: Vec<Vec<usize>> = servers
            .iter()
            .enumerate()
            .map(|(i, server)| {
                Self::generate_maglev_permutation(&server.server.url, i, TABLE_SIZE)
            })
            .collect();

        // Populate the table
        let mut filled = 0;
        while filled < TABLE_SIZE {
            for backend_idx in 0..n {
                let mut offset = next[backend_idx];

                while filled < TABLE_SIZE {
                    let candidate = permutations[backend_idx][offset % TABLE_SIZE];

                    if table[candidate].is_none() {
                        table[candidate] = Some(backend_idx);
                        next[backend_idx] = offset + 1;
                        filled += 1;
                        break;
                    }

                    offset += 1;
                    next[backend_idx] = offset;
                }
            }
        }

        // Convert Option<usize> to usize (unwrap is safe because table is fully populated)
        table.into_iter().map(|x| x.unwrap()).collect()
    }

    /// Generate Maglev permutation for a backend
    fn generate_maglev_permutation(backend_key: &str, backend_idx: usize, size: usize) -> Vec<usize> {
        // Generate offset and skip values using double hashing
        let key1 = format!("{}:offset", backend_key);
        let key2 = format!("{}:skip:{}", backend_key, backend_idx);

        let mut hasher1 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        let offset = (hasher1.finish() % size as u64) as usize;

        let mut hasher2 = DefaultHasher::new();
        key2.hash(&mut hasher2);
        let skip = ((hasher2.finish() % (size as u64 - 1)) + 1) as usize; // Skip must be > 0

        // Generate permutation using offset and skip
        let mut permutation = Vec::with_capacity(size);
        for i in 0..size {
            permutation.push((offset + i * skip) % size);
        }

        permutation
    }

    /// Get all backend servers
    pub fn servers(&self) -> &[Arc<BackendServer>] {
        &self.servers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_servers(count: usize) -> Vec<ServerDef> {
        (0..count)
            .map(|i| ServerDef {
                url: format!("http://backend-{}", i),
                weight: 1,
                max_conns: 100,
                location: None,
                region: None,
            })
            .collect()
    }

    #[test]
    fn test_round_robin() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let s1 = lb.select(None, None).unwrap();
        let s2 = lb.select(None, None).unwrap();
        let s3 = lb.select(None, None).unwrap();
        let s4 = lb.select(None, None).unwrap();

        assert_eq!(s1.server.url, "http://backend-0");
        assert_eq!(s2.server.url, "http://backend-1");
        assert_eq!(s3.server.url, "http://backend-2");
        assert_eq!(s4.server.url, "http://backend-0"); // Wraps around
    }

    #[test]
    fn test_least_connections() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastConn, servers);

        let s1 = lb.select(None, None).unwrap();
        s1.acquire(); // 1 connection

        let s2 = lb.select(None, None).unwrap();
        s2.acquire(); // 1 connection
        s2.acquire(); // 2 connections

        // Should select server with fewest connections (backend-2 with 0)
        let s3 = lb.select(None, None).unwrap();
        assert_eq!(s3.server.url, "http://backend-2");
    }

    #[test]
    fn test_ip_hash() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::IpHash, servers);

        let s1 = lb.select(Some("192.168.1.1"), None).unwrap();
        let s2 = lb.select(Some("192.168.1.1"), None).unwrap();
        let s3 = lb.select(Some("192.168.1.2"), None).unwrap();

        // Same IP should go to same server
        assert_eq!(s1.server.url, s2.server.url);

        // Different IPs might go to different servers
        // (not guaranteed due to hash collisions, but likely)
    }

    #[test]
    fn test_consistent_hash() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::ConsistentHash, servers);

        let s1 = lb.select(None, Some("user123")).unwrap();
        let s2 = lb.select(None, Some("user123")).unwrap();
        let s3 = lb.select(None, Some("user456")).unwrap();

        // Same key should go to same server
        assert_eq!(s1.server.url, s2.server.url);
    }

    #[test]
    fn test_connection_tracking() {
        let servers = create_test_servers(1);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let server = lb.select(None, None).unwrap();
        assert_eq!(server.connections(), 0);

        server.acquire();
        assert_eq!(server.connections(), 1);

        server.acquire();
        assert_eq!(server.connections(), 2);

        server.release();
        assert_eq!(server.connections(), 1);

        server.release();
        assert_eq!(server.connections(), 0);
    }

    #[test]
    fn test_maglev() {
        let servers = create_test_servers(5);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::Maglev, servers);

        // Same key should consistently go to same server
        let s1 = lb.select(None, Some("user123")).unwrap();
        let s2 = lb.select(None, Some("user123")).unwrap();
        let s3 = lb.select(None, Some("user123")).unwrap();

        assert_eq!(s1.server.url, s2.server.url);
        assert_eq!(s2.server.url, s3.server.url);

        // Different keys should distribute across servers
        let mut server_distribution = std::collections::HashMap::new();
        for i in 0..1000 {
            let key = format!("user{}", i);
            let server = lb.select(None, Some(&key)).unwrap();
            *server_distribution.entry(server.server.url.clone()).or_insert(0) += 1;
        }

        // All servers should get some traffic (with 5 servers and 1000 requests, each should get ~200)
        // Allow for variance, but each should get at least 100
        assert_eq!(server_distribution.len(), 5);
        for (_, count) in server_distribution.iter() {
            assert!(*count >= 100, "Server got too few requests: {}", count);
            assert!(*count <= 300, "Server got too many requests: {}", count);
        }
    }

    #[test]
    fn test_maglev_consistency() {
        // Test that Maglev maintains consistency when backends are available
        let servers = create_test_servers(10);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::Maglev, servers);

        // Build a map of keys to servers
        let mut key_to_server = std::collections::HashMap::new();
        for i in 0..500 {
            let key = format!("session{}", i);
            let server = lb.select(None, Some(&key)).unwrap();
            key_to_server.insert(key.clone(), server.server.url.clone());
        }

        // Verify consistency - same keys should map to same servers
        for (key, expected_server) in key_to_server.iter() {
            let server = lb.select(None, Some(key)).unwrap();
            assert_eq!(
                &server.server.url,
                expected_server,
                "Key {} should consistently map to server {}",
                key,
                expected_server
            );
        }
    }

    #[test]
    fn test_maglev_with_client_ip() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::Maglev, servers);

        // When no request_key provided, Maglev should use client_ip
        let s1 = lb.select(Some("192.168.1.100"), None).unwrap();
        let s2 = lb.select(Some("192.168.1.100"), None).unwrap();

        // Same IP should go to same server
        assert_eq!(s1.server.url, s2.server.url);

        // Different IPs may go to different servers
        let s3 = lb.select(Some("192.168.1.101"), None).unwrap();
        // We can't assert they're different (hash collision possible), but they should be consistent
        let s4 = lb.select(Some("192.168.1.101"), None).unwrap();
        assert_eq!(s3.server.url, s4.server.url);
    }

    #[test]
    fn test_maglev_table_size() {
        // Verify Maglev table is built correctly
        let servers = create_test_servers(7);
        let table = LoadBalancer::build_maglev_table(
            &servers.into_iter().map(|s| Arc::new(BackendServer::new(s))).collect::<Vec<_>>()
        );

        // Table size should be 65537 (prime number)
        assert_eq!(table.len(), 65537);

        // All entries should be valid backend indices (0-6 for 7 servers)
        for &idx in table.iter() {
            assert!(idx < 7, "Invalid backend index in table: {}", idx);
        }

        // Verify distribution - all backends should appear in the table
        let mut backend_counts = vec![0; 7];
        for &idx in table.iter() {
            backend_counts[idx] += 1;
        }

        for (backend_idx, count) in backend_counts.iter().enumerate() {
            assert!(
                *count > 0,
                "Backend {} not present in Maglev table",
                backend_idx
            );
            // Each backend should get roughly equal share (65537 / 7 ≈ 9362)
            // Allow for variance (+/- 20%)
            assert!(
                *count >= 7490 && *count <= 11234,
                "Backend {} has unbalanced distribution: {} entries",
                backend_idx,
                count
            );
        }
    }
}
