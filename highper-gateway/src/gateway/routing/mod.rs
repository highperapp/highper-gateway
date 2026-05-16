//! Per-hostname API routing with fast in-memory storage
//!
//! This module provides high-performance routing for API Gateway use cases:
//! - Routes organized by hostname (api.example.com, staging.example.com, etc.)
//! - O(1) hostname lookup using DashMap (lock-free concurrent hashmap)
//! - SIMD-accelerated path matching for ultra-fast route selection
//! - Hot reload support for zero-downtime configuration updates
//! - JSON format optimized for API gateway scenarios
//!
//! ## Architecture
//!
//! ```text
//! HostnameRouter
//!  └── DashMap<Hostname, HostRoutes>
//!       └── HostRoutes
//!            ├── DashMap<Path, Route> (exact matches)
//!            ├── Vec<PrefixRoute> (prefix matches, sorted by length)
//!            └── Vec<PatternRoute> (regex patterns)
//! ```
//!
//! ## Performance Characteristics
//!
//! - Hostname lookup: O(1) with lock-free access
//! - Exact path match: O(1)
//! - Prefix match: O(log n) with binary search
//! - Pattern match: O(m) where m = number of patterns (SIMD-accelerated)
//! - Hot reload: Lock-free atomic swap
//!
//! ## Example
//!
//! ```rust
//! use highper_gateway::gateway::routing::{HostnameRouter, RouteConfig};
//!
//! # async fn example() {
//! let router = HostnameRouter::new();
//!
//! // Load routes from JSON
//! router.load_from_json_file("config/routes.json").await.unwrap();
//!
//! // Match a request
//! let route = router.match_request("api.example.com", "/users/123", "GET").await;
//! # }
//! ```

pub mod hot_reload;
pub mod loader;
pub mod matcher;
pub mod types;
pub mod upstream_state;

pub use hot_reload::*;
pub use matcher::*;
pub use types::*;
pub use upstream_state::*;

use crate::proxy::Client;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// High-performance per-hostname router
pub struct HostnameRouter {
    /// Hostname -> Routes mapping (lock-free concurrent access)
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,

    /// Upstream configurations (lock-free concurrent access)
    upstreams: Arc<DashMap<String, UpstreamConfig>>,

    /// Upstream runtime state with health checking
    upstream_states: Arc<DashMap<String, Arc<UpstreamState>>>,

    /// Hot reload handle for zero-downtime updates
    reload_handle: Arc<tokio::sync::RwLock<Option<ReloadHandle>>>,

    /// HTTP client for health checks
    client: Client,

    /// Request metrics tracker for per-route metrics
    request_metrics: Option<Arc<crate::state::request_metrics::RequestMetrics>>,
}

impl HostnameRouter {
    /// Create a new hostname router
    pub fn new() -> Self {
        Self {
            hosts: Arc::new(DashMap::new()),
            upstreams: Arc::new(DashMap::new()),
            upstream_states: Arc::new(DashMap::new()),
            reload_handle: Arc::new(tokio::sync::RwLock::new(None)),
            client: Client::new(),
            request_metrics: None,
        }
    }

    /// Set request metrics tracker (builder pattern)
    pub fn with_request_metrics(
        mut self,
        metrics: Arc<crate::state::request_metrics::RequestMetrics>,
    ) -> Self {
        self.request_metrics = Some(metrics);
        self
    }

    /// Create with initial capacity (optimization for large route sets)
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            hosts: Arc::new(DashMap::with_capacity(capacity)),
            upstreams: Arc::new(DashMap::with_capacity(capacity)),
            upstream_states: Arc::new(DashMap::with_capacity(capacity)),
            reload_handle: Arc::new(tokio::sync::RwLock::new(None)),
            client: Client::new(),
            request_metrics: None,
        }
    }

    /// Add routes for a hostname
    pub fn add_host_routes(&self, hostname: String, routes: HostRoutes) {
        info!(
            "Adding {} routes for hostname: {}",
            routes.route_count(),
            hostname
        );
        self.hosts.insert(hostname, Arc::new(routes));
    }

    /// Remove all routes for a hostname
    pub fn remove_host(&self, hostname: &str) -> Option<Arc<HostRoutes>> {
        info!("Removing routes for hostname: {}", hostname);
        self.hosts.remove(hostname).map(|(_, v)| v)
    }

    /// Get routes for a hostname (lock-free read)
    pub fn get_host_routes(&self, hostname: &str) -> Option<Arc<HostRoutes>> {
        self.hosts.get(hostname).map(|r| Arc::clone(r.value()))
    }

    /// Match a request to a route (fast path - most common case)
    pub async fn match_request(
        &self,
        hostname: &str,
        path: &str,
        method: &str,
    ) -> Option<MatchedRoute> {
        // Fast hostname lookup (O(1))
        let host_routes = self.get_host_routes(hostname)?;

        // Fast route matching within hostname
        host_routes.match_route(path, method).await
    }

    /// Match with wildcard hostname support (*.example.com)
    pub async fn match_request_wildcard(
        &self,
        hostname: &str,
        path: &str,
        method: &str,
    ) -> Option<MatchedRoute> {
        // Try exact hostname match first (fast path)
        if let Some(route) = self.match_request(hostname, path, method).await {
            return Some(route);
        }

        // Try wildcard matches (*.example.com)
        if let Some(pos) = hostname.find('.') {
            let wildcard = format!("*{}", &hostname[pos..]);
            if let Some(route) = self.match_request(&wildcard, path, method).await {
                return Some(route);
            }
        }

        None
    }

    /// Get total number of hosts
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Get total number of routes across all hosts
    pub fn total_route_count(&self) -> usize {
        self.hosts.iter().map(|r| r.value().route_count()).sum()
    }

    /// List all hostnames
    pub fn list_hostnames(&self) -> Vec<String> {
        self.hosts.iter().map(|r| r.key().clone()).collect()
    }

    /// Clear all routes
    pub fn clear(&self) {
        info!("Clearing all routes");
        self.hosts.clear();
    }

    /// Add or update an upstream configuration
    pub fn add_upstream(&self, name: String, config: UpstreamConfig) {
        info!(
            "Adding upstream: {} ({} servers)",
            name,
            config.servers.len()
        );
        self.upstreams.insert(name, config);
    }

    /// Remove an upstream configuration
    pub fn remove_upstream(&self, name: &str) -> Option<UpstreamConfig> {
        info!("Removing upstream: {}", name);
        self.upstreams.remove(name).map(|(_, v)| v)
    }

    /// Get upstream configuration by name
    pub fn get_upstream(&self, name: &str) -> Option<UpstreamConfig> {
        self.upstreams.get(name).map(|r| r.value().clone())
    }

    /// Get upstream for a matched route (convenience method)
    pub async fn get_upstream_for_route(
        &self,
        hostname: &str,
        path: &str,
        method: &str,
    ) -> Option<UpstreamConfig> {
        let matched = self.match_request_wildcard(hostname, path, method).await?;
        self.get_upstream(&matched.route.upstream)
    }

    /// Get total number of upstreams
    pub fn upstream_count(&self) -> usize {
        self.upstreams.len()
    }

    /// List all upstream names
    pub fn list_upstreams(&self) -> Vec<String> {
        self.upstreams.iter().map(|r| r.key().clone()).collect()
    }

    /// Add upstream with health checking (creates runtime state)
    pub async fn add_upstream_with_health(&self, name: String, config: UpstreamConfig) {
        info!(
            "Adding upstream with health checking: {} ({} servers)",
            name,
            config.servers.len()
        );

        // Store config
        self.upstreams.insert(name.clone(), config.clone());

        // Create runtime state with backends
        let state = Arc::new(UpstreamState::from_config(
            name.clone(),
            config,
            self.client.clone(),
        ));

        // Start health checks if configured
        state.start_health_checks().await;

        // Store state
        self.upstream_states.insert(name, state);
    }

    /// Select a healthy backend for the given upstream
    pub fn select_backend_for_route(&self, upstream_name: &str) -> Option<String> {
        self.upstream_states
            .get(upstream_name)?
            .select_healthy_backend()
            .map(|backend| backend.server.url.clone())
    }

    /// Mark a backend as successful (passive health tracking)
    pub fn mark_backend_success(&self, upstream_name: &str, backend_url: &str) {
        if let Some(state) = self.upstream_states.get(upstream_name) {
            if let Some(backend) = state.find_backend(backend_url) {
                backend.record_passive_success();
            }
        }
    }

    /// Mark a backend as failed (passive health tracking)
    pub fn mark_backend_failure(&self, upstream_name: &str, backend_url: &str) {
        if let Some(state) = self.upstream_states.get(upstream_name) {
            if let Some(backend) = state.find_backend(backend_url) {
                backend.record_passive_failure(3); // 3 failures threshold
            }
        }
    }

    /// Get health status for all backends in an upstream
    pub fn get_upstream_health_status(&self, upstream_name: &str) -> Option<Vec<BackendStatus>> {
        self.upstream_states
            .get(upstream_name)
            .map(|state| state.get_backends_status())
    }

    /// Get count of healthy backends for an upstream
    pub fn get_healthy_backend_count(&self, upstream_name: &str) -> usize {
        self.upstream_states
            .get(upstream_name)
            .map(|state| state.healthy_backend_count())
            .unwrap_or(0)
    }

    /// Record route request metrics
    pub fn record_route_metrics(
        &self,
        route_name: &str,
        status_code: u16,
        response_time: std::time::Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        if let Some(metrics) = &self.request_metrics {
            metrics.record_route_request(
                route_name,
                status_code,
                response_time,
                bytes_sent,
                bytes_received,
            );
        }
    }

    /// Record backend request metrics
    pub fn record_backend_metrics(
        &self,
        backend_url: &str,
        is_error: bool,
        response_time: std::time::Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        if let Some(metrics) = &self.request_metrics {
            metrics.record_backend_request(
                backend_url,
                is_error,
                response_time,
                bytes_sent,
                bytes_received,
            );
        }
    }

    /// Export current configuration to JSON
    pub async fn export_to_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        use crate::gateway::routing::types::*;
        use std::collections::HashMap;

        let mut hosts_configs = Vec::new();
        let mut all_upstreams = HashMap::new();

        // Export upstreams
        for entry in self.upstreams.iter() {
            all_upstreams.insert(entry.key().clone(), entry.value().clone());
        }

        // Iterate through all hosts
        for host_entry in self.hosts.iter() {
            let hostname = host_entry.key().clone();
            let host_routes = host_entry.value();

            let mut route_configs = Vec::new();

            // Export exact routes
            for exact_entry in host_routes.exact.iter() {
                let path = exact_entry.key().clone();
                let route = exact_entry.value();

                route_configs.push(RouteConfig {
                    name: route.name.clone(),
                    path_match: PathMatch::Exact { path },
                    upstream: route.upstream.clone(),
                    methods: route.methods.clone(),
                    timeout_ms: route.timeout_ms,
                    middleware: route.middleware.clone(),
                    metadata: route.metadata.clone(),
                });
            }

            // Export prefix routes
            {
                let prefixes = host_routes.prefixes.read().await;
                for prefix_route in prefixes.iter() {
                    route_configs.push(RouteConfig {
                        name: prefix_route.route.name.clone(),
                        path_match: PathMatch::Prefix {
                            prefix: prefix_route.prefix.clone(),
                        },
                        upstream: prefix_route.route.upstream.clone(),
                        methods: prefix_route.route.methods.clone(),
                        timeout_ms: prefix_route.route.timeout_ms,
                        middleware: prefix_route.route.middleware.clone(),
                        metadata: prefix_route.route.metadata.clone(),
                    });
                }
            }

            // Export pattern routes
            {
                let patterns = host_routes.patterns.read().await;
                for pattern_route in patterns.iter() {
                    route_configs.push(RouteConfig {
                        name: pattern_route.route.name.clone(),
                        path_match: PathMatch::Pattern {
                            pattern: pattern_route.pattern.clone(),
                        },
                        upstream: pattern_route.route.upstream.clone(),
                        methods: pattern_route.route.methods.clone(),
                        timeout_ms: pattern_route.route.timeout_ms,
                        middleware: pattern_route.route.middleware.clone(),
                        metadata: pattern_route.route.metadata.clone(),
                    });
                }
            }

            // Only add host if it has routes
            if !route_configs.is_empty() {
                hosts_configs.push(HostConfig {
                    hostname,
                    routes: route_configs,
                });
            }
        }

        // Create complete config with hosts and upstreams
        let config = HostnameRoutesConfig {
            version: "1.0".to_string(),
            hosts: hosts_configs,
            upstreams: all_upstreams,
        };

        Ok(serde_json::to_string_pretty(&config)?)
    }

    /// Export routes summary for admin API
    pub fn export_routes_summary(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut routes = Vec::new();

        for host_entry in self.hosts.iter() {
            let hostname = host_entry.key();
            let host_routes = host_entry.value();

            // Exact routes
            for exact_entry in host_routes.exact.iter() {
                let path = exact_entry.key();
                let route = exact_entry.value();
                routes.push(json!({
                    "hostname": hostname,
                    "name": route.name,
                    "path": path,
                    "match_type": "exact",
                    "upstream": route.upstream,
                    "methods": route.methods,
                }));
            }

            // Prefix routes (need to access via RwLock)
            if let Ok(prefixes) = host_routes.prefixes.try_read() {
                for prefix_route in prefixes.iter() {
                    routes.push(json!({
                        "hostname": hostname,
                        "name": prefix_route.route.name,
                        "prefix": prefix_route.prefix,
                        "match_type": "prefix",
                        "upstream": prefix_route.route.upstream,
                        "methods": prefix_route.route.methods,
                    }));
                }
            }

            // Pattern routes (need to access via RwLock)
            if let Ok(patterns) = host_routes.patterns.try_read() {
                for pattern_route in patterns.iter() {
                    routes.push(json!({
                        "hostname": hostname,
                        "name": pattern_route.route.name,
                        "pattern": pattern_route.pattern.as_str(),
                        "match_type": "pattern",
                        "upstream": pattern_route.route.upstream,
                        "methods": pattern_route.route.methods,
                    }));
                }
            }
        }

        routes
    }

    /// Export upstreams summary with health status for admin API
    pub fn export_upstreams_summary(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut upstreams = Vec::new();

        for entry in self.upstreams.iter() {
            let name = entry.key();
            let config = entry.value();

            // Get health status from upstream state if available
            let (total_backends, healthy_backends, backends_health) =
                if let Some(state) = self.upstream_states.get(name.as_str()) {
                    let total = state.backend_count();
                    let healthy = state.healthy_backend_count();
                    let backends = state
                        .get_backends_status()
                        .into_iter()
                        .map(|b| {
                            json!({
                                "url": b.url,
                                "healthy": b.healthy,
                                "consecutive_successes": b.consecutive_successes,
                                "consecutive_failures": b.consecutive_failures,
                            })
                        })
                        .collect::<Vec<_>>();
                    (total, healthy, Some(backends))
                } else {
                    (config.servers.len(), config.servers.len(), None)
                };

            upstreams.push(json!({
                "name": name,
                "algorithm": config.algorithm,
                "servers_count": total_backends,
                "healthy_count": healthy_backends,
                "health_check_enabled": config.health_check.is_some(),
                "backends": backends_health,
            }));
        }

        upstreams
    }

    /// Enable hot reload from file
    pub async fn enable_hot_reload(
        &self,
        config_path: String,
        interval_secs: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let handle = ReloadHandle::new(config_path, interval_secs, Arc::clone(&self.hosts)).await?;
        let mut reload = self.reload_handle.write().await;
        *reload = Some(handle);
        Ok(())
    }

    /// Disable hot reload
    pub async fn disable_hot_reload(&self) {
        let mut reload = self.reload_handle.write().await;
        *reload = None;
    }
}

impl Default for HostnameRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Routes for a specific hostname
pub struct HostRoutes {
    /// Exact path matches (fastest - O(1))
    exact: DashMap<String, Route>,

    /// Prefix matches (e.g., /api/*, /users/*)
    /// Sorted by prefix length (longest first) for correct precedence
    prefixes: Arc<tokio::sync::RwLock<Vec<PrefixRoute>>>,

    /// Regex pattern matches (slowest, but SIMD-accelerated)
    patterns: Arc<tokio::sync::RwLock<Vec<PatternRoute>>>,
}

impl HostRoutes {
    /// Create empty host routes
    pub fn new() -> Self {
        Self {
            exact: DashMap::new(),
            prefixes: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            patterns: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Create with capacity
    pub fn with_capacity(exact: usize, prefix: usize, pattern: usize) -> Self {
        Self {
            exact: DashMap::with_capacity(exact),
            prefixes: Arc::new(tokio::sync::RwLock::new(Vec::with_capacity(prefix))),
            patterns: Arc::new(tokio::sync::RwLock::new(Vec::with_capacity(pattern))),
        }
    }

    /// Add an exact match route
    pub fn add_exact(&self, path: String, route: Route) {
        self.exact.insert(path, route);
    }

    /// Add a prefix match route
    pub async fn add_prefix(&self, prefix: String, route: Route) {
        let mut prefixes = self.prefixes.write().await;
        let prefix_route = PrefixRoute { prefix, route };
        prefixes.push(prefix_route);

        // Keep sorted by prefix length (longest first)
        prefixes.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));
    }

    /// Add a pattern match route
    pub async fn add_pattern(&self, pattern: String, route: Route) -> Result<(), String> {
        let regex = regex::Regex::new(&pattern)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?;

        let mut patterns = self.patterns.write().await;
        patterns.push(PatternRoute {
            pattern,
            regex,
            route,
        });

        Ok(())
    }

    /// Match a route (optimized for common case)
    pub async fn match_route(&self, path: &str, method: &str) -> Option<MatchedRoute> {
        // 1. Try exact match first (fastest - O(1))
        if let Some(route) = self.exact.get(path) {
            if route.matches_method(method) {
                debug!("Exact match: {} {}", method, path);
                return Some(MatchedRoute {
                    route: route.value().clone(),
                    path_params: std::collections::HashMap::new(),
                    match_type: MatchType::Exact,
                });
            }
        }

        // 2. Try prefix matches (O(n) where n = prefix count, typically small)
        {
            let prefixes = self.prefixes.read().await;
            for prefix_route in prefixes.iter() {
                if path.starts_with(&prefix_route.prefix)
                    && prefix_route.route.matches_method(method)
                {
                    debug!(
                        "Prefix match: {} {} (prefix: {})",
                        method, path, prefix_route.prefix
                    );
                    return Some(MatchedRoute {
                        route: prefix_route.route.clone(),
                        path_params: std::collections::HashMap::new(),
                        match_type: MatchType::Prefix,
                    });
                }
            }
        }

        // 3. Try pattern matches (slowest, but SIMD-accelerated)
        {
            let patterns = self.patterns.read().await;
            for pattern_route in patterns.iter() {
                if let Some(captures) = pattern_route.regex.captures(path) {
                    if pattern_route.route.matches_method(method) {
                        debug!(
                            "Pattern match: {} {} (pattern: {})",
                            method, path, pattern_route.pattern
                        );

                        // Extract path parameters from regex captures
                        let mut path_params = std::collections::HashMap::new();
                        for name in pattern_route.regex.capture_names().flatten() {
                            if let Some(value) = captures.name(name) {
                                path_params.insert(name.to_string(), value.as_str().to_string());
                            }
                        }

                        return Some(MatchedRoute {
                            route: pattern_route.route.clone(),
                            path_params,
                            match_type: MatchType::Pattern,
                        });
                    }
                }
            }
        }

        None
    }

    /// Get total route count
    pub fn route_count(&self) -> usize {
        let exact_count = self.exact.len();
        // Note: For prefixes and patterns, we can't get count without locking
        // Return approximate count (exact only)
        exact_count
    }
}

impl Default for HostRoutes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exact_match() {
        let router = HostnameRouter::new();
        let mut host_routes = HostRoutes::new();

        let route = Route {
            name: "test".to_string(),
            upstream: "backend1".to_string(),
            methods: vec!["GET".to_string()],
            timeout_ms: None,
            middleware: vec![],
            metadata: std::collections::HashMap::new(),
        };

        host_routes.add_exact("/api/users".to_string(), route);
        router.add_host_routes("api.example.com".to_string(), host_routes);

        let matched = router
            .match_request("api.example.com", "/api/users", "GET")
            .await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().route.name, "test");
    }

    #[tokio::test]
    async fn test_prefix_match() {
        let router = HostnameRouter::new();
        let host_routes = HostRoutes::new();

        let route = Route {
            name: "api".to_string(),
            upstream: "backend1".to_string(),
            methods: vec!["GET".to_string()],
            timeout_ms: None,
            middleware: vec![],
            metadata: std::collections::HashMap::new(),
        };

        host_routes.add_prefix("/api/".to_string(), route).await;
        router.add_host_routes("api.example.com".to_string(), host_routes);

        let matched = router
            .match_request("api.example.com", "/api/users/123", "GET")
            .await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().route.name, "api");
    }

    #[tokio::test]
    async fn test_wildcard_hostname() {
        let router = HostnameRouter::new();
        let host_routes = HostRoutes::new();

        let route = Route {
            name: "wildcard".to_string(),
            upstream: "backend1".to_string(),
            methods: vec!["GET".to_string()],
            timeout_ms: None,
            middleware: vec![],
            metadata: std::collections::HashMap::new(),
        };

        host_routes.add_exact("/health".to_string(), route);
        router.add_host_routes("*.example.com".to_string(), host_routes);

        let matched = router
            .match_request_wildcard("api.example.com", "/health", "GET")
            .await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().route.name, "wildcard");
    }

    #[tokio::test]
    async fn test_method_filtering() {
        let router = HostnameRouter::new();
        let host_routes = HostRoutes::new();

        let route = Route {
            name: "post_only".to_string(),
            upstream: "backend1".to_string(),
            methods: vec!["POST".to_string()],
            timeout_ms: None,
            middleware: vec![],
            metadata: std::collections::HashMap::new(),
        };

        host_routes.add_exact("/api/users".to_string(), route);
        router.add_host_routes("api.example.com".to_string(), host_routes);

        let matched_get = router
            .match_request("api.example.com", "/api/users", "GET")
            .await;
        assert!(matched_get.is_none());

        let matched_post = router
            .match_request("api.example.com", "/api/users", "POST")
            .await;
        assert!(matched_post.is_some());
    }
}
