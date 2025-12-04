//! Configuration validation

use super::Config;
use anyhow::{bail, Result};

/// Validate configuration
pub fn validate_config(config: &Config) -> Result<()> {
    // Validate server configuration
    if config.server.bind.is_empty() && config.server.tls_bind.is_empty() {
        bail!("Server must have at least one bind address (HTTP or HTTPS)");
    }

    for bind in &config.server.bind {
        if bind.parse::<std::net::SocketAddr>().is_err() {
            bail!("Invalid bind address: {}", bind);
        }
    }

    for bind in &config.server.tls_bind {
        if bind.parse::<std::net::SocketAddr>().is_err() {
            bail!("Invalid TLS bind address: {}", bind);
        }
    }

    // Validate workers
    if config.server.workers != "auto" {
        if let Ok(workers) = config.server.workers.parse::<usize>() {
            if workers == 0 {
                bail!("Number of workers must be greater than 0");
            }
        } else {
            bail!("Invalid workers value: must be 'auto' or a positive number");
        }
    }

    // Validate upstreams
    for upstream in &config.upstreams {
        if upstream.name.is_empty() {
            bail!("Upstream name cannot be empty");
        }

        if upstream.servers.is_empty() {
            bail!("Upstream '{}' must have at least one server", upstream.name);
        }

        for server in &upstream.servers {
            if server.url.is_empty() {
                bail!("Server URL cannot be empty in upstream '{}'", upstream.name);
            }
        }
    }

    // Validate routes
    for route in &config.routes {
        if route.name.is_empty() {
            bail!("Route name cannot be empty");
        }

        if route.upstream.is_empty() {
            bail!("Route '{}' must specify an upstream", route.name);
        }

        // Check that the upstream exists
        if !config.upstreams.iter().any(|u| u.name == route.upstream) {
            bail!(
                "Route '{}' references non-existent upstream '{}'",
                route.name,
                route.upstream
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    #[test]
    fn test_validate_empty_bind() {
        let config = Config {
            server: ServerConfig {
                bind: vec![],
                tls_bind: vec![],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1],
                performance: PerformanceConfig::default(),
                shutdown_timeout: std::time::Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: ObservabilityConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
        };

        assert!(validate_config(&config).is_err());
    }
}
