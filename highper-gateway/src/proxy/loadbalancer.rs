//! Load balancing algorithms

use crate::config::{LoadBalancingAlgorithm, ServerDef, SlowStartConfig};
use crate::proxy::geographic::{GeoLoadBalancer, GeoServer};
use crate::state::ProxyState;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// Slow start configuration for traffic ramp-up
    slow_start_config: Option<SlowStartConfig>,
}

/// Maximum number of response times to keep in the rolling window
const RESPONSE_TIME_WINDOW_SIZE: usize = 100;

/// Backend server with connection and response time tracking
pub struct BackendServer {
    pub server: ServerDef,
    active_connections: AtomicUsize,
    /// Rolling window of recent response times in microseconds
    response_times: RwLock<VecDeque<u64>>,
    /// Cached average response time in microseconds (updated on each recording)
    avg_response_time_us: AtomicU64,
    /// Total number of requests processed (for statistics)
    total_requests: AtomicU64,
    /// Time when this backend was added or recovered from unhealthy state
    join_time: RwLock<Instant>,
}

impl BackendServer {
    pub fn new(server: ServerDef) -> Self {
        Self {
            server,
            active_connections: AtomicUsize::new(0),
            response_times: RwLock::new(VecDeque::with_capacity(RESPONSE_TIME_WINDOW_SIZE)),
            avg_response_time_us: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            join_time: RwLock::new(Instant::now()),
        }
    }

    /// Reset join time (called when backend recovers from unhealthy state)
    pub fn reset_join_time(&self) {
        *self.join_time.write() = Instant::now();
    }

    /// Get time since backend was added/recovered
    pub fn time_since_join(&self) -> Duration {
        self.join_time.read().elapsed()
    }

    /// Calculate effective weight based on slow start configuration
    ///
    /// Returns the actual weight to use for load balancing, taking into
    /// account the slow start ramp-up period.
    pub fn effective_weight(&self, slow_start: Option<&SlowStartConfig>) -> u32 {
        let base_weight = self.server.weight;

        // If no slow start config or disabled, return full weight
        let config = match slow_start {
            Some(c) if c.enabled => c,
            _ => return base_weight,
        };

        let elapsed = self.time_since_join();
        let duration = Duration::from_secs(config.duration_secs);

        // If past the slow start duration, return full weight
        if elapsed >= duration {
            return base_weight;
        }

        // Calculate weight multiplier based on elapsed time
        // Linear ramp from initial_weight_percent to 100%
        let progress = elapsed.as_secs_f64() / duration.as_secs_f64();
        let initial = config.initial_weight_percent as f64 / 100.0;
        let multiplier = initial + (1.0 - initial) * progress;

        // Calculate effective weight, minimum of 1
        let effective = (base_weight as f64 * multiplier).round() as u32;
        effective.max(1)
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

    /// Record a response time for this backend
    ///
    /// This method maintains a rolling window of the last N response times
    /// and updates the cached average for efficient selection.
    pub fn record_response_time(&self, duration: Duration) {
        let micros = duration.as_micros() as u64;

        let mut times = self.response_times.write();

        // Remove oldest if at capacity
        if times.len() >= RESPONSE_TIME_WINDOW_SIZE {
            times.pop_front();
        }

        // Add new response time
        times.push_back(micros);

        // Update cached average
        let sum: u64 = times.iter().sum();
        let avg = if times.is_empty() { 0 } else { sum / times.len() as u64 };
        self.avg_response_time_us.store(avg, Ordering::Relaxed);

        // Increment total requests
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the average response time in microseconds
    ///
    /// Returns 0 if no response times have been recorded yet.
    /// New backends with no data will be preferred to allow initial probing.
    pub fn avg_response_time_us(&self) -> u64 {
        self.avg_response_time_us.load(Ordering::Relaxed)
    }

    /// Get the average response time as a Duration
    pub fn avg_response_time(&self) -> Duration {
        Duration::from_micros(self.avg_response_time_us())
    }

    /// Get the total number of requests processed by this backend
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Check if this backend has response time data
    pub fn has_response_data(&self) -> bool {
        self.total_requests() > 0
    }

    /// Get percentile response time (p50, p90, p99, etc.)
    ///
    /// Returns None if no data is available.
    pub fn response_time_percentile(&self, percentile: f64) -> Option<Duration> {
        let times = self.response_times.read();
        if times.is_empty() {
            return None;
        }

        let mut sorted: Vec<u64> = times.iter().copied().collect();
        sorted.sort_unstable();

        let idx = ((percentile / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        let idx = idx.min(sorted.len() - 1);

        Some(Duration::from_micros(sorted[idx]))
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
            slow_start_config: None,
        }
    }

    /// Create a new load balancer with slow start configuration
    pub fn with_slow_start(
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        slow_start: Option<SlowStartConfig>,
    ) -> Self {
        let mut lb = Self::new(algorithm, servers);
        lb.slow_start_config = slow_start;
        lb
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

    /// Create a new load balancer with ProxyState and slow start
    pub fn with_state_and_slow_start(
        upstream_name: String,
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        proxy_state: Arc<ProxyState>,
        slow_start: Option<SlowStartConfig>,
    ) -> Self {
        let mut lb = Self::new(algorithm, servers);
        lb.upstream_name = upstream_name;
        lb.proxy_state = Some(proxy_state);
        lb.slow_start_config = slow_start;
        lb
    }

    /// Set slow start configuration
    pub fn set_slow_start(&mut self, slow_start: Option<SlowStartConfig>) {
        self.slow_start_config = slow_start;
    }

    /// Get slow start configuration
    pub fn slow_start_config(&self) -> Option<&SlowStartConfig> {
        self.slow_start_config.as_ref()
    }

    /// Reset join time for a specific backend (used when backend recovers)
    pub fn reset_backend_join_time(&self, index: usize) {
        if let Some(server) = self.servers.get(index) {
            server.reset_join_time();
            tracing::info!(
                "Backend {} (index {}) join time reset for slow start",
                server.server.url,
                index
            );
        }
    }

    /// Reset join time for all backends
    pub fn reset_all_join_times(&self) {
        for server in &self.servers {
            server.reset_join_time();
        }
    }

    /// Create a new load balancer with full configuration including state
    pub fn with_full_config<P: AsRef<std::path::Path>>(
        upstream_name: String,
        algorithm: LoadBalancingAlgorithm,
        servers: Vec<ServerDef>,
        geoip_provider: crate::config::GeoIpProvider,
        geoip_db_path: Option<P>,
        proxy_state: Option<Arc<ProxyState>>,
        slow_start: Option<SlowStartConfig>,
    ) -> Self {
        let mut lb = Self::with_geoip_config(algorithm, servers, geoip_provider, geoip_db_path);
        lb.upstream_name = upstream_name;
        lb.proxy_state = proxy_state;
        lb.slow_start_config = slow_start;
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
            LoadBalancingAlgorithm::LeastResponseTime => self.least_response_time(),
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
            LoadBalancingAlgorithm::LeastResponseTime => self.least_response_time(),
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

    /// Round-robin load balancing with weighted selection for slow start
    fn round_robin(&self) -> Option<Arc<BackendServer>> {
        // If slow start is enabled, use weighted selection
        if self.slow_start_config.is_some() {
            return self.weighted_selection();
        }

        let idx = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % self.servers.len();
        Some(self.servers[idx].clone())
    }

    /// Weighted selection based on effective weights (considering slow start)
    fn weighted_selection(&self) -> Option<Arc<BackendServer>> {
        if self.servers.is_empty() {
            return None;
        }

        // Calculate total effective weight
        let weights: Vec<u32> = self.servers
            .iter()
            .map(|s| s.effective_weight(self.slow_start_config.as_ref()))
            .collect();

        let total_weight: u64 = weights.iter().map(|&w| w as u64).sum();
        if total_weight == 0 {
            return Some(self.servers[0].clone());
        }

        // Generate a random point in the weight space
        let counter = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) as u64;
        let point = counter % total_weight;

        // Find which backend this point falls into
        let mut cumulative = 0u64;
        for (idx, &weight) in weights.iter().enumerate() {
            cumulative += weight as u64;
            if point < cumulative {
                return Some(self.servers[idx].clone());
            }
        }

        // Fallback to first server
        Some(self.servers[0].clone())
    }

    /// Least connections load balancing with slow start weight consideration
    fn least_connections(&self) -> Option<Arc<BackendServer>> {
        let mut min_ratio = f64::MAX;
        let mut selected_idx = 0;

        for (idx, server) in self.servers.iter().enumerate() {
            let conns = server.connections() as f64;
            let effective_weight = server.effective_weight(self.slow_start_config.as_ref()) as f64;

            // Calculate connection/weight ratio (lower is better)
            // This ensures backends in slow start get proportionally fewer connections
            let ratio = if effective_weight > 0.0 {
                conns / effective_weight
            } else {
                f64::MAX
            };

            if ratio < min_ratio {
                min_ratio = ratio;
                selected_idx = idx;
            }
        }

        Some(self.servers[selected_idx].clone())
    }

    /// Least response time load balancing
    ///
    /// Selects the backend with the lowest average response time.
    /// Backends without response data are preferred initially to allow probing.
    /// Falls back to least connections for tie-breaking.
    fn least_response_time(&self) -> Option<Arc<BackendServer>> {
        let mut selected_idx = 0;
        let mut min_response_time = u64::MAX;
        let mut min_connections = usize::MAX;
        let mut found_unprobed = false;

        for (idx, server) in self.servers.iter().enumerate() {
            // Prefer backends that haven't been probed yet (new backends)
            // This ensures fair initial distribution and discovery of fast backends
            if !server.has_response_data() {
                if !found_unprobed || server.connections() < min_connections {
                    selected_idx = idx;
                    min_connections = server.connections();
                    found_unprobed = true;
                }
                continue;
            }

            // Skip if we already found an unprobed backend
            if found_unprobed {
                continue;
            }

            let response_time = server.avg_response_time_us();
            let connections = server.connections();

            // Select by lowest response time, use connections as tie-breaker
            if response_time < min_response_time
                || (response_time == min_response_time && connections < min_connections)
            {
                min_response_time = response_time;
                min_connections = connections;
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
            .map(|d| d.as_nanos() as usize)
            .unwrap_or_else(|_| {
                // Fallback to thread-local counter if system time fails
                // This should never happen in practice unless clock is before 1970
                tracing::warn!("System time before UNIX_EPOCH, using fallback random seed");
                let _ = metrics::counter!("loadbalancer_time_errors_total", "function" => "random");
                self.round_robin_counter.fetch_add(1, Ordering::Relaxed)
            });
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
            .map(|d| d.as_nanos() as usize)
            .unwrap_or_else(|_| {
                // Fallback to round-robin counter if system time fails
                tracing::warn!("System time before UNIX_EPOCH, using fallback seed for power_of_two");
                let _ = metrics::counter!("loadbalancer_time_errors_total", "function" => "power_of_two");
                self.round_robin_counter.fetch_add(1, Ordering::Relaxed)
            });

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

        // Convert Option<usize> to usize
        // The algorithm should fill all slots, but use defensive programming to avoid panics
        let mut had_none = false;
        let result: Vec<usize> = table.into_iter().enumerate().map(|(idx, x)| {
            match x {
                Some(backend_idx) => backend_idx,
                None => {
                    // This should never happen if the algorithm is correct
                    // But if it does, log an error and use first backend instead of panicking
                    if !had_none {
                        tracing::error!(
                            "Maglev table has unfilled slots - algorithm may have a bug. \
                            Using first backend (0) as fallback."
                        );
                        let _ = metrics::counter!("loadbalancer_maglev_errors_total");
                        had_none = true;
                    }
                    tracing::debug!("Maglev table slot {} was None, using backend 0", idx);
                    0 // Use first backend as fallback
                }
            }
        }).collect();

        if had_none {
            tracing::warn!(
                "Maglev table build completed with {} total slots, but some were unfilled",
                result.len()
            );
        }

        result
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

    /// Select a backend server for gRPC requests using gRPC-specific policies
    ///
    /// This method implements gRPC-specific load balancing strategies:
    /// - RoundRobin: Standard round-robin
    /// - LeastRequest: Select backend with fewest active connections (same as LeastConn)
    /// - Random: Random selection
    /// - PowerOfTwo: Random selection from two choices, pick one with fewer connections
    /// - ConsistentHash: Hash-based selection using gRPC metadata
    ///
    /// # Arguments
    /// * `policy` - The gRPC load balancing policy to use
    /// * `metadata` - Optional gRPC metadata for affinity (e.g., from headers)
    /// * `affinity_key` - Optional header name to use for consistent hashing
    pub fn select_grpc(
        &self,
        policy: crate::grpc::GrpcLoadBalancingPolicy,
        metadata: Option<&std::collections::HashMap<String, String>>,
        affinity_key: Option<&str>,
    ) -> Option<Arc<BackendServer>> {
        if self.servers.is_empty() {
            return None;
        }

        match policy {
            crate::grpc::GrpcLoadBalancingPolicy::RoundRobin => self.round_robin(),
            crate::grpc::GrpcLoadBalancingPolicy::LeastRequest => self.least_connections(),
            crate::grpc::GrpcLoadBalancingPolicy::Random => self.random(),
            crate::grpc::GrpcLoadBalancingPolicy::PowerOfTwo => self.power_of_two(),
            crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash => {
                // Extract affinity value from metadata
                if let Some(affinity_value) = self.extract_affinity_value(metadata, affinity_key) {
                    self.consistent_hash(&affinity_value)
                } else {
                    // Fallback to round-robin if no affinity value
                    self.round_robin()
                }
            }
        }
    }

    /// Async version of select_grpc with backend availability checking
    pub async fn select_grpc_async(
        &self,
        policy: crate::grpc::GrpcLoadBalancingPolicy,
        metadata: Option<&std::collections::HashMap<String, String>>,
        affinity_key: Option<&str>,
    ) -> Option<Arc<BackendServer>> {
        let selected = self.select_grpc(policy, metadata, affinity_key);

        // Verify the selected backend is available
        if let Some(ref backend) = selected {
            let index = self.servers.iter().position(|s| Arc::ptr_eq(s, backend))?;
            if !self.is_backend_available(index).await {
                // Try to find another available backend
                return self.find_available_backend().await;
            }
        }

        selected
    }

    /// Extract affinity value from gRPC metadata for consistent hashing
    ///
    /// If an affinity_key is provided, tries to extract that specific header.
    /// Otherwise, tries common gRPC affinity headers in order:
    /// 1. x-grpc-affinity
    /// 2. x-session-id
    /// 3. x-user-id
    /// 4. authorization (uses full value as key)
    fn extract_affinity_value(
        &self,
        metadata: Option<&std::collections::HashMap<String, String>>,
        affinity_key: Option<&str>,
    ) -> Option<String> {
        let metadata = metadata?;

        // If specific affinity key is provided, use it
        if let Some(key) = affinity_key {
            return metadata.get(key).cloned();
        }

        // Try common affinity headers in order of preference
        let common_keys = [
            "x-grpc-affinity",
            "x-session-id",
            "x-user-id",
            "authorization",
        ];

        for key in &common_keys {
            if let Some(value) = metadata.get(*key) {
                return Some(value.clone());
            }
        }

        None
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
    fn test_least_response_time() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastResponseTime, servers);

        // Get all backends directly to record response times
        // Backend selection for unprobed backends prefers lowest connections
        let s1 = lb.select(None, None).unwrap();
        s1.record_response_time(Duration::from_millis(100)); // 100ms - now backend-0 is probed

        let s2 = lb.select(None, None).unwrap();
        // Second select should get backend-1 (unprobed, same connections as backend-2)
        assert_eq!(s2.server.url, "http://backend-1");
        s2.record_response_time(Duration::from_millis(50));  // 50ms (fastest)

        let s3 = lb.select(None, None).unwrap();
        // Third select should get backend-2 (only unprobed backend left)
        assert_eq!(s3.server.url, "http://backend-2");
        s3.record_response_time(Duration::from_millis(200)); // 200ms

        // All backends now have response data
        // Should select the fastest backend (backend-1 with 50ms)
        let selected = lb.select(None, None).unwrap();
        assert_eq!(selected.server.url, "http://backend-1");

        // Verify response time tracking
        assert_eq!(s2.avg_response_time_us(), 50000); // 50ms in microseconds
        assert!(s2.has_response_data());
        assert_eq!(s2.total_requests(), 1);
    }

    #[test]
    fn test_least_response_time_rolling_window() {
        let servers = create_test_servers(2);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastResponseTime, servers);

        // First select returns backend-0
        let s1 = lb.select(None, None).unwrap();
        assert_eq!(s1.server.url, "http://backend-0");
        s1.record_response_time(Duration::from_millis(100)); // Mark as probed

        // Second select returns backend-1 (only unprobed backend)
        let s2 = lb.select(None, None).unwrap();
        assert_eq!(s2.server.url, "http://backend-1");
        s2.record_response_time(Duration::from_millis(50)); // Mark as probed

        // Now add more response times
        s1.record_response_time(Duration::from_millis(200));
        s2.record_response_time(Duration::from_millis(100));

        // Verify averages
        assert_eq!(s1.avg_response_time_us(), 150000); // (100+200)/2 = 150ms average
        assert_eq!(s2.avg_response_time_us(), 75000);  // (50+100)/2 = 75ms average

        // s2 should be selected as it has lower average response time
        let selected = lb.select(None, None).unwrap();
        assert_eq!(selected.server.url, "http://backend-1");
    }

    #[test]
    fn test_least_response_time_with_tie_breaking() {
        let servers = create_test_servers(2);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastResponseTime, servers);

        // Probe both backends with same response time
        let s1 = lb.select(None, None).unwrap();
        s1.record_response_time(Duration::from_millis(100));

        let s2 = lb.select(None, None).unwrap();
        s2.record_response_time(Duration::from_millis(100));

        // Both have same response time (100ms)
        assert_eq!(s1.avg_response_time_us(), 100000);
        assert_eq!(s2.avg_response_time_us(), 100000);

        // With same response time, it should use connections as tie-breaker
        s1.acquire(); // s1 has 1 connection, s2 has 0

        // s2 should be selected (same response time but fewer connections)
        let selected = lb.select(None, None).unwrap();
        assert_eq!(selected.server.url, "http://backend-1");
    }

    #[test]
    fn test_response_time_percentile() {
        let servers = create_test_servers(1);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastResponseTime, servers);

        let server = lb.select(None, None).unwrap();

        // Record response times with known percentiles
        server.record_response_time(Duration::from_millis(10));
        server.record_response_time(Duration::from_millis(20));
        server.record_response_time(Duration::from_millis(30));
        server.record_response_time(Duration::from_millis(40));
        server.record_response_time(Duration::from_millis(100)); // outlier

        // Check percentiles (5 values: 10, 20, 30, 40, 100)
        let p50 = server.response_time_percentile(50.0).unwrap();
        assert_eq!(p50.as_millis(), 30); // median

        let p90 = server.response_time_percentile(90.0).unwrap();
        assert_eq!(p90.as_millis(), 100); // p90 catches the outlier

        assert_eq!(server.total_requests(), 5);
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

    // gRPC Load Balancing Tests

    #[test]
    fn test_grpc_select_round_robin() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let s1 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::RoundRobin, None, None).unwrap();
        let s2 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::RoundRobin, None, None).unwrap();
        let s3 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::RoundRobin, None, None).unwrap();
        let s4 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::RoundRobin, None, None).unwrap();

        assert_eq!(s1.server.url, "http://backend-0");
        assert_eq!(s2.server.url, "http://backend-1");
        assert_eq!(s3.server.url, "http://backend-2");
        assert_eq!(s4.server.url, "http://backend-0"); // Wraps around
    }

    #[test]
    fn test_grpc_select_least_request() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastConn, servers);

        let s1 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::LeastRequest, None, None).unwrap();
        s1.acquire(); // 1 connection

        let s2 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::LeastRequest, None, None).unwrap();
        // Should not be s1 since it has a connection
        assert_ne!(s1.server.url, s2.server.url);
    }

    #[test]
    fn test_grpc_select_consistent_hash_with_metadata() {
        let servers = create_test_servers(5);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::ConsistentHash, servers);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("x-grpc-affinity".to_string(), "user123".to_string());

        let s1 = lb.select_grpc(
            crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash,
            Some(&metadata),
            None,
        ).unwrap();

        let s2 = lb.select_grpc(
            crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash,
            Some(&metadata),
            None,
        ).unwrap();

        // Same metadata should route to same backend
        assert_eq!(s1.server.url, s2.server.url);
    }

    #[test]
    fn test_grpc_select_consistent_hash_with_custom_key() {
        let servers = create_test_servers(5);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::ConsistentHash, servers);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("custom-key".to_string(), "session456".to_string());

        let s1 = lb.select_grpc(
            crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash,
            Some(&metadata),
            Some("custom-key"),
        ).unwrap();

        let s2 = lb.select_grpc(
            crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash,
            Some(&metadata),
            Some("custom-key"),
        ).unwrap();

        // Same custom key should route to same backend
        assert_eq!(s1.server.url, s2.server.url);
    }

    #[test]
    fn test_grpc_select_consistent_hash_fallback() {
        let servers = create_test_servers(3);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        // No metadata provided, should fallback to round-robin
        let s1 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash, None, None).unwrap();
        let s2 = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::ConsistentHash, None, None).unwrap();

        // Should use round-robin behavior
        assert_ne!(s1.server.url, s2.server.url);
    }

    #[test]
    fn test_extract_affinity_value_with_common_headers() {
        let servers = create_test_servers(2);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        // Test x-grpc-affinity (highest priority)
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("x-grpc-affinity".to_string(), "affinity1".to_string());
        metadata.insert("x-session-id".to_string(), "session1".to_string());

        let value = lb.extract_affinity_value(Some(&metadata), None);
        assert_eq!(value, Some("affinity1".to_string()));

        // Test x-session-id (second priority)
        let mut metadata2 = std::collections::HashMap::new();
        metadata2.insert("x-session-id".to_string(), "session2".to_string());
        metadata2.insert("x-user-id".to_string(), "user2".to_string());

        let value2 = lb.extract_affinity_value(Some(&metadata2), None);
        assert_eq!(value2, Some("session2".to_string()));

        // Test x-user-id (third priority)
        let mut metadata3 = std::collections::HashMap::new();
        metadata3.insert("x-user-id".to_string(), "user3".to_string());

        let value3 = lb.extract_affinity_value(Some(&metadata3), None);
        assert_eq!(value3, Some("user3".to_string()));

        // Test authorization (fourth priority)
        let mut metadata4 = std::collections::HashMap::new();
        metadata4.insert("authorization".to_string(), "Bearer token123".to_string());

        let value4 = lb.extract_affinity_value(Some(&metadata4), None);
        assert_eq!(value4, Some("Bearer token123".to_string()));
    }

    #[test]
    fn test_extract_affinity_value_with_custom_key() {
        let servers = create_test_servers(2);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("my-custom-key".to_string(), "custom-value".to_string());
        metadata.insert("x-grpc-affinity".to_string(), "affinity-value".to_string());

        // Custom key should take precedence
        let value = lb.extract_affinity_value(Some(&metadata), Some("my-custom-key"));
        assert_eq!(value, Some("custom-value".to_string()));
    }

    #[test]
    fn test_extract_affinity_value_no_metadata() {
        let servers = create_test_servers(2);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let value = lb.extract_affinity_value(None, None);
        assert_eq!(value, None);
    }

    #[test]
    fn test_grpc_select_power_of_two() {
        let servers = create_test_servers(5);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::PowerOfTwo, servers);

        // Power of two should always return a valid backend
        for _ in 0..10 {
            let server = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::PowerOfTwo, None, None);
            assert!(server.is_some());
        }
    }

    #[test]
    fn test_grpc_select_random() {
        let servers = create_test_servers(5);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::Random, servers);

        let mut seen_servers = std::collections::HashSet::new();

        // With 5 servers and 20 selections, we should see multiple servers
        for _ in 0..20 {
            let server = lb.select_grpc(crate::grpc::GrpcLoadBalancingPolicy::Random, None, None).unwrap();
            seen_servers.insert(server.server.url.clone());
        }

        // Should have seen at least 3 different servers
        assert!(seen_servers.len() >= 3, "Random selection should distribute across servers");
    }

    // Slow Start Tests

    #[test]
    fn test_effective_weight_no_slow_start() {
        let server = ServerDef {
            url: "http://backend-0".to_string(),
            weight: 100,
            max_conns: 100,
            location: None,
            region: None,
        };
        let backend = BackendServer::new(server);

        // Without slow start config, should return full weight
        assert_eq!(backend.effective_weight(None), 100);
    }

    #[test]
    fn test_effective_weight_slow_start_disabled() {
        let server = ServerDef {
            url: "http://backend-0".to_string(),
            weight: 100,
            max_conns: 100,
            location: None,
            region: None,
        };
        let backend = BackendServer::new(server);

        let config = SlowStartConfig {
            enabled: false,
            duration_secs: 60,
            initial_weight_percent: 10,
        };

        // With disabled slow start, should return full weight
        assert_eq!(backend.effective_weight(Some(&config)), 100);
    }

    #[test]
    fn test_effective_weight_slow_start_initial() {
        let server = ServerDef {
            url: "http://backend-0".to_string(),
            weight: 100,
            max_conns: 100,
            location: None,
            region: None,
        };
        let backend = BackendServer::new(server);

        let config = SlowStartConfig {
            enabled: true,
            duration_secs: 60,
            initial_weight_percent: 10,
        };

        // At start (t=0), should be close to initial weight (10% of 100 = 10)
        let weight = backend.effective_weight(Some(&config));
        assert!(weight >= 10 && weight <= 15, "Initial weight should be ~10, got {}", weight);
    }

    #[test]
    fn test_effective_weight_minimum() {
        let server = ServerDef {
            url: "http://backend-0".to_string(),
            weight: 1, // Very low base weight
            max_conns: 100,
            location: None,
            region: None,
        };
        let backend = BackendServer::new(server);

        let config = SlowStartConfig {
            enabled: true,
            duration_secs: 60,
            initial_weight_percent: 10,
        };

        // Even with low base weight, minimum should be 1
        let weight = backend.effective_weight(Some(&config));
        assert!(weight >= 1, "Minimum weight should be 1, got {}", weight);
    }

    #[test]
    fn test_slow_start_weighted_selection() {
        let servers = create_test_servers(2);
        let mut lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let config = SlowStartConfig {
            enabled: true,
            duration_secs: 60,
            initial_weight_percent: 10,
        };
        lb.set_slow_start(Some(config));

        // With slow start, selection should use weighted selection
        // Both servers are new, so they should have similar weights
        let mut counts = std::collections::HashMap::new();
        for _ in 0..100 {
            let server = lb.select(None, None).unwrap();
            *counts.entry(server.server.url.clone()).or_insert(0) += 1;
        }

        // Both should get traffic (exact distribution depends on timing)
        assert!(counts.len() == 2, "Both servers should receive traffic");
    }

    #[test]
    fn test_reset_join_time() {
        let servers = create_test_servers(1);
        let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin, servers);

        let server = lb.select(None, None).unwrap();

        // Wait a bit and check time_since_join increases
        std::thread::sleep(Duration::from_millis(10));
        let elapsed1 = server.time_since_join();
        assert!(elapsed1.as_millis() >= 10);

        // Reset join time
        server.reset_join_time();
        let elapsed2 = server.time_since_join();
        assert!(elapsed2.as_millis() < 5, "After reset, elapsed time should be very small");
    }

    #[test]
    fn test_slow_start_config_defaults() {
        let config = SlowStartConfig::default();
        assert!(config.enabled);
        assert_eq!(config.duration_secs, 60);
        assert_eq!(config.initial_weight_percent, 10);
    }

    #[test]
    fn test_least_connections_with_slow_start() {
        let servers = create_test_servers(2);
        let mut lb = LoadBalancer::new(LoadBalancingAlgorithm::LeastConn, servers);

        let config = SlowStartConfig {
            enabled: true,
            duration_secs: 60,
            initial_weight_percent: 10,
        };
        lb.set_slow_start(Some(config));

        // Get first server and add a connection
        let s1 = lb.select(None, None).unwrap();
        s1.acquire();

        // Second selection should consider weight ratio
        // Both have similar effective weights, s1 has 1 connection, s2 has 0
        // So s2 should be selected (lower conn/weight ratio)
        let s2 = lb.select(None, None).unwrap();
        assert_ne!(s1.server.url, s2.server.url, "Should select server with lower conn/weight ratio");
    }
}
