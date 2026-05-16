//! Configuration loader for per-hostname routes

use super::{HostRoutes, HostnameRouter, HostnameRoutesConfig, PathMatch, Route};
use std::path::Path;
use tokio::fs;
use tracing::{info, warn};

impl HostnameRouter {
    /// Load routes from JSON file
    pub async fn load_from_json_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        info!("Loading routes from: {}", path.display());

        let content = fs::read_to_string(path).await?;
        self.load_from_json(&content).await
    }

    /// Load routes from JSON string
    pub async fn load_from_json(&self, json: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config: HostnameRoutesConfig = serde_json::from_str(json)?;

        info!("Loading configuration version: {}", config.version);
        let host_count = config.hosts.len();
        let upstream_count = config.upstreams.len();
        info!("Total hosts: {}", host_count);
        info!("Total upstreams: {}", upstream_count);

        // Load upstreams first (with health checking)
        for (name, upstream_config) in config.upstreams {
            self.add_upstream_with_health(name, upstream_config).await;
        }

        let mut total_routes = 0;

        for host_config in config.hosts {
            let hostname = host_config.hostname.clone();
            let route_count = host_config.routes.len();

            info!("Loading {} routes for hostname: {}", route_count, hostname);

            // Create host routes
            let host_routes = HostRoutes::new();

            for route_config in host_config.routes {
                let route = Route {
                    name: route_config.name.clone(),
                    upstream: route_config.upstream.clone(),
                    methods: route_config.methods.clone(),
                    timeout_ms: route_config.timeout_ms,
                    middleware: route_config.middleware.clone(),
                    metadata: route_config.metadata.clone(),
                };

                match route_config.path_match {
                    PathMatch::Exact { path } => {
                        host_routes.add_exact(path.clone(), route);
                    }
                    PathMatch::Prefix { prefix } => {
                        host_routes.add_prefix(prefix.clone(), route).await;
                    }
                    PathMatch::Pattern { pattern } => {
                        if let Err(e) = host_routes.add_pattern(pattern.clone(), route).await {
                            warn!("Failed to add pattern route '{}': {}", route_config.name, e);
                        }
                    }
                }
            }

            self.add_host_routes(hostname, host_routes);
            total_routes += route_count;
        }

        info!(
            "Successfully loaded {} routes across {} hosts",
            total_routes, host_count
        );
        Ok(())
    }

    /// Reload routes from file (atomic swap)
    pub async fn reload_from_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Reloading routes from: {}", path.as_ref().display());

        // Create new router
        let new_router = HostnameRouter::new();

        // Load into new router
        new_router.load_from_json_file(path).await?;

        // Atomic swap (replace all hosts and upstreams)
        self.clear();
        self.upstreams.clear();

        for entry in new_router.hosts.iter() {
            let hostname = entry.key().clone();
            let routes = entry.value().clone();
            self.hosts.insert(hostname, routes);
        }

        for entry in new_router.upstreams.iter() {
            let name = entry.key().clone();
            let config = entry.value().clone();
            self.upstreams.insert(name, config);
        }

        info!("Routes and upstreams reloaded successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_from_json() {
        let json = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "api.example.com",
                    "routes": [
                        {
                            "name": "users",
                            "match_type": "exact",
                            "path": "/api/users",
                            "upstream": "user-service",
                            "methods": ["GET", "POST"]
                        },
                        {
                            "name": "products",
                            "match_type": "prefix",
                            "prefix": "/api/products/",
                            "upstream": "product-service",
                            "methods": ["GET"]
                        }
                    ]
                }
            ]
        }
        "#;

        let router = HostnameRouter::new();
        router.load_from_json(json).await.unwrap();

        assert_eq!(router.host_count(), 1);

        let matched = router
            .match_request("api.example.com", "/api/users", "GET")
            .await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().route.name, "users");

        let matched = router
            .match_request("api.example.com", "/api/products/123", "GET")
            .await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().route.name, "products");
    }

    #[tokio::test]
    async fn test_export_to_json() {
        let json_input = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "api.example.com",
                    "routes": [
                        {
                            "name": "test",
                            "match_type": "exact",
                            "path": "/test",
                            "upstream": "backend",
                            "methods": ["GET"]
                        }
                    ]
                }
            ]
        }
        "#;

        let router = HostnameRouter::new();
        router.load_from_json(json_input).await.unwrap();

        let json_output = router.export_to_json().await.unwrap();
        assert!(json_output.contains("api.example.com"));
        assert!(json_output.contains("test"));
    }

    #[tokio::test]
    async fn test_upstream_loading() {
        let json = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "api.example.com",
                    "routes": [
                        {
                            "name": "users",
                            "match_type": "exact",
                            "path": "/api/users",
                            "upstream": "user-service",
                            "methods": ["GET"]
                        }
                    ]
                }
            ],
            "upstreams": {
                "user-service": {
                    "servers": ["http://localhost:3001", "http://localhost:3002"],
                    "algorithm": "round_robin"
                }
            }
        }
        "#;

        let router = HostnameRouter::new();
        router.load_from_json(json).await.unwrap();

        // Verify upstream was loaded
        assert_eq!(router.upstream_count(), 1);

        let upstream = router.get_upstream("user-service").unwrap();
        assert_eq!(upstream.servers.len(), 2);
        assert_eq!(upstream.algorithm, "round_robin");

        // Verify export includes upstreams
        let exported = router.export_to_json().await.unwrap();
        assert!(exported.contains("user-service"));
        assert!(exported.contains("http://localhost:3001"));
    }

    #[tokio::test]
    async fn test_get_upstream_for_route() {
        let json = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "api.example.com",
                    "routes": [
                        {
                            "name": "products",
                            "match_type": "prefix",
                            "prefix": "/api/products/",
                            "upstream": "product-service",
                            "methods": ["GET"]
                        }
                    ]
                }
            ],
            "upstreams": {
                "product-service": {
                    "servers": ["http://localhost:4001"],
                    "algorithm": "round_robin"
                }
            }
        }
        "#;

        let router = HostnameRouter::new();
        router.load_from_json(json).await.unwrap();

        // Get upstream for a specific route match
        let upstream = router
            .get_upstream_for_route("api.example.com", "/api/products/123", "GET")
            .await
            .unwrap();

        assert_eq!(upstream.servers.len(), 1);
        assert_eq!(upstream.servers[0], "http://localhost:4001");
    }
}
