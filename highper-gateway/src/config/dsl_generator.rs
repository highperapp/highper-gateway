//! DSL Generator - Convert Config to DSL format
//!
//! This module provides functionality to generate DSL configuration text
//! from a runtime Config structure. This enables migration from YAML/JSON
//! to the simpler DSL format.

use crate::config::schema::*;
use anyhow::{Context, Result};

/// Generate DSL text from a Config structure
pub fn generate_dsl(config: &Config) -> Result<String> {
    let mut output = String::new();

    // Generate global directives FIRST
    let global = generate_global_directives(config)?;
    if !global.is_empty() {
        output.push_str(&global);
    }

    // Then generate sites (combination of routes + upstreams)
    let sites = group_routes_into_sites(config)?;

    for site in sites.iter() {
        output.push_str(&generate_site(site)?);
    }

    // Trim trailing whitespace
    Ok(output.trim_end().to_string())
}

/// Represents a logical site (domain + routes + upstream)
#[derive(Debug)]
struct Site {
    address: String,
    routes: Vec<RouteConfig>,
    upstream: Option<UpstreamConfig>,
    tls_enabled: bool,
}

/// Group routes and upstreams into logical sites
fn group_routes_into_sites(config: &Config) -> Result<Vec<Site>> {
    let mut sites = Vec::new();

    // Determine primary bind address
    let bind_addr = if !config.server.tls_bind.is_empty() {
        config.server.tls_bind.first()
            .context("No TLS bind address")?
            .clone()
    } else {
        config.server.bind.first()
            .context("No bind address")?
            .clone()
    };

    let tls_enabled = !config.server.tls_bind.is_empty();

    // Group routes by upstream
    for route in &config.routes {
        // Find matching upstream
        let upstream = config.upstreams.iter()
            .find(|u| u.name == route.upstream)
            .cloned();

        sites.push(Site {
            address: bind_addr.clone(),
            routes: vec![route.clone()],
            upstream,
            tls_enabled,
        });
    }

    Ok(sites)
}

/// Generate DSL text for a site
fn generate_site(site: &Site) -> Result<String> {
    let mut output = String::new();

    // Get first path from match_rules
    let is_simple_catchall = site.routes.len() == 1
        && !site.routes[0].match_rules.paths.is_empty()
        && site.routes[0].match_rules.paths[0] == "/*";

    // Simple single-route site
    if is_simple_catchall {
        // Simple format: address proxy backends
        if let Some(upstream) = &site.upstream {
            if !upstream.servers.is_empty() {
                let backends: Vec<String> = upstream.servers.iter()
                    .map(|s| extract_backend_address(&s.url))
                    .collect();

                output.push_str(&format!("{} proxy {}",
                    site.address,
                    backends.join(" ")
                ));

                // Add load balancing if not default
                let lb = &upstream.load_balancing;
                if format!("{:?}", lb.algorithm) != "RoundRobin" {
                    output.push('\n');
                    output.push_str(&format!("    lb {:?}", lb.algorithm).to_lowercase());
                }
            }
        }
    } else {
        // Complex multi-route site
        let protocol = if site.tls_enabled { "https" } else { "http" };
        output.push_str(&format!("{}://{} {{", protocol, site.address));

        for route in &site.routes {
            if let Some(upstream) = &site.upstream {
                if !upstream.servers.is_empty() {
                    output.push('\n'); // Newline before each route block

                    let backends: Vec<String> = upstream.servers.iter()
                        .map(|s| extract_backend_address(&s.url))
                        .collect();

                    // Use first path from match_rules
                    let mut path = route.match_rules.paths.first()
                        .map(|s| s.as_str())
                        .unwrap_or("/*")
                        .to_string();

                    // Normalize path - convert "/" to "/*"
                    if path == "/" {
                        path = "/*".to_string();
                    }

                    // Use route block format
                    output.push_str(&format!("    {} {{\n", path));
                    output.push_str(&format!("        proxy {}\n", backends.join(" ")));
                    output.push_str("    }");
                }
            }
        }

        output.push_str("\n}");
    }

    Ok(output)
}

/// Extract backend address from URL
fn extract_backend_address(url: &str) -> String {
    // Remove scheme if present
    url.replace("http://", "")
        .replace("https://", "")
}

/// Generate global directives
fn generate_global_directives(config: &Config) -> Result<String> {
    let mut output = String::new();

    // Logging
    if let Some(level) = get_log_level(&config.observability) {
        output.push_str(&format!("log {}\n", level));
    }

    // Admin API
    if let Some(admin) = &config.admin {
        // Extract port from bind address (e.g., "127.0.0.1:9000" -> ":9000")
        let admin_addr = if admin.bind.contains(':') {
            let port = admin.bind.split(':').last().unwrap_or(&admin.bind);
            format!(":{}", port)
        } else {
            admin.bind.clone()
        };
        output.push_str(&format!("admin {}\n", admin_addr));
    }

    // Metrics
    if config.observability.metrics.enabled {
        output.push_str("metrics on\n");
    }

    // Rate limiting
    if let Some(rate_limit) = &config.rate_limit {
        if rate_limit.enabled {
            output.push_str(&format!("rate_limit {} per {}s\n",
                rate_limit.capacity,
                rate_limit.window.as_secs()
            ));
        }
    }

    // Compression
    if let Some(compression) = get_compression_algorithms(config) {
        if !compression.is_empty() {
            output.push_str(&format!("compress {}\n", compression.join(" ")));
        }
    }

    // CORS
    if has_cors_enabled(config) {
        output.push_str("cors\n");
    }

    Ok(output)
}

/// Extract log level from observability config
fn get_log_level(obs: &ObservabilityConfig) -> Option<String> {
    // Return log level if configured
    Some(obs.logging.level.to_lowercase())
}

/// Format duration for DSL
fn format_duration(duration: &std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

/// Get compression algorithms
fn get_compression_algorithms(_config: &Config) -> Option<Vec<String>> {
    // TODO: Extract from middleware config when available
    None
}

/// Check if CORS is enabled
fn has_cors_enabled(_config: &Config) -> bool {
    // TODO: Extract from middleware config when available
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_backend_address() {
        assert_eq!(extract_backend_address("http://backend:3000"), "backend:3000");
        assert_eq!(extract_backend_address("https://backend:3000"), "backend:3000");
        assert_eq!(extract_backend_address("backend:3000"), "backend:3000");
    }

    #[test]
    fn test_format_duration() {
        use std::time::Duration;
        assert_eq!(format_duration(&Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(&Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(&Duration::from_secs(120)), "2m");
    }

    // Note: Full integration tests require constructing complete Config structures
    // which need all required fields. These tests verify individual functions.
}
