//! DSL Parser
//!
//! Parses the Caddy-like DSL into an AST using pest.

use pest::Parser;
use pest_derive::Parser;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;

use super::dsl_ast::*;

#[derive(Parser)]
#[grammar = "config/dsl.pest"]
pub struct DslParser;

/// Parse DSL configuration from string
pub fn parse_dsl(input: &str) -> Result<Config> {
    let mut pairs = DslParser::parse(Rule::config, input)
        .context("Failed to parse DSL configuration")?;

    let mut config = Config::default();

    // Get the config rule pair
    if let Some(config_pair) = pairs.next() {
        // Iterate through inner rules (global_directive, site, etc.)
        for pair in config_pair.into_inner() {
            match pair.as_rule() {
                Rule::global_directive => {
                    parse_global_directive(&mut config.global, pair)?;
                }
                Rule::site => {
                    let site = parse_site(pair)?;
                    config.sites.push(site);
                }
                Rule::EOI => {} // End of input
                _ => {}
            }
        }
    }

    Ok(config)
}

fn parse_global_directive(global: &mut GlobalConfig, pair: pest::iterators::Pair<Rule>) -> Result<()> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::log_directive => {
                for log_pair in inner.into_inner() {
                    if let Rule::log_level = log_pair.as_rule() {
                        global.log_level = Some(parse_log_level(log_pair.as_str())?);
                    }
                }
            }
            Rule::admin_directive => {
                for admin_pair in inner.into_inner() {
                    if let Rule::admin_address = admin_pair.as_rule() {
                        global.admin_address = Some(admin_pair.as_str().to_string());
                    }
                }
            }
            Rule::metrics_directive => {
                parse_metrics_directive(global, inner)?;
            }
            Rule::buffer_pool_directive => {
                if let Some(Directive::BufferPool(config)) = parse_directive(inner)? {
                    global.buffer_pool = Some(config);
                }
            }
            Rule::backpressure_directive => {
                if let Some(Directive::Backpressure(config)) = parse_directive(inner)? {
                    global.backpressure = Some(config);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_log_level(s: &str) -> Result<LogLevel> {
    match s {
        "debug" => Ok(LogLevel::Debug),
        "info" => Ok(LogLevel::Info),
        "warn" => Ok(LogLevel::Warn),
        "error" => Ok(LogLevel::Error),
        _ => Err(anyhow!("Invalid log level: {}", s)),
    }
}

fn parse_metrics_directive(global: &mut GlobalConfig, pair: pest::iterators::Pair<Rule>) -> Result<()> {
    for inner in pair.into_inner() {
        if let Rule::metrics_option = inner.as_rule() {
            let text = inner.as_str();
            if text == "off" {
                global.metrics_enabled = false;
            } else if text == "on" {
                global.metrics_enabled = true;
            } else if text == "prometheus" {
                global.metrics_prometheus = true;
            } else if let Some(port_str) = text.strip_prefix("port=") {
                global.metrics_port = Some(port_str.parse()?);
            }
        }
    }
    Ok(())
}

fn parse_site(pair: pest::iterators::Pair<Rule>) -> Result<Site> {
    let mut address = None;
    let mut routes = Vec::new();
    let mut directives = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::site_address => {
                address = Some(parse_site_address(inner)?);
            }
            Rule::simple_proxy => {
                let backends = parse_simple_proxy(inner)?;
                directives.push(Directive::Proxy(backends));
            }
            Rule::site_block => {
                let (block_routes, block_directives) = parse_site_block(inner)?;
                routes.extend(block_routes);
                directives.extend(block_directives);
            }
            _ => {}
        }
    }

    Ok(Site {
        address: address.ok_or_else(|| anyhow!("Site missing address"))?,
        routes,
        directives,
    })
}

fn parse_site_address(pair: pest::iterators::Pair<Rule>) -> Result<SiteAddress> {
    let mut scheme = None;
    let mut domain = None;
    let mut port = None;
    let mut path = None;
    let mut tcp_protocol = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::scheme => {
                scheme = Some(parse_scheme(inner.as_str())?);
            }
            Rule::domain => {
                domain = Some(inner.as_str().to_string());
            }
            Rule::port => {
                port = Some(inner.as_str().parse()?);
            }
            Rule::path => {
                path = Some(inner.as_str().to_string());
            }
            Rule::tcp_protocol => {
                tcp_protocol = Some(parse_tcp_protocol(inner.as_str())?);
            }
            _ => {}
        }
    }

    // TCP-only (port without domain)
    if domain.is_none() && port.is_some() {
        return Ok(SiteAddress::Tcp {
            port: port.unwrap(),
            protocol: tcp_protocol.unwrap_or(TcpProtocol::Generic),
        });
    }

    // HTTP/HTTPS site
    Ok(SiteAddress::Http {
        scheme: scheme.unwrap_or(Scheme::Http),
        domain: domain.ok_or_else(|| anyhow!("Missing domain"))?,
        port,
        base_path: path,
    })
}

fn parse_scheme(s: &str) -> Result<Scheme> {
    match s {
        "http://" => Ok(Scheme::Http),
        "https://" => Ok(Scheme::Https),
        "grpc://" => Ok(Scheme::Grpc),
        _ => Err(anyhow!("Invalid scheme: {}", s)),
    }
}

fn parse_tcp_protocol(s: &str) -> Result<TcpProtocol> {
    match s {
        "mysql" => Ok(TcpProtocol::Mysql),
        "postgres" => Ok(TcpProtocol::Postgres),
        "redis" => Ok(TcpProtocol::Redis),
        "tcp" => Ok(TcpProtocol::Generic),
        _ => Err(anyhow!("Invalid TCP protocol: {}", s)),
    }
}

fn parse_simple_proxy(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Backend>> {
    let mut backends = Vec::new();

    for inner in pair.into_inner() {
        if let Rule::backend = inner.as_rule() {
            backends.push(parse_backend(inner)?);
        }
    }

    Ok(backends)
}

fn parse_backend(pair: pest::iterators::Pair<Rule>) -> Result<Backend> {
    let mut address = None;
    let mut port = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::domain | Rule::ip_address => {
                address = Some(inner.as_str().to_string());
            }
            Rule::port => {
                port = Some(inner.as_str().parse()?);
            }
            Rule::scheme => {
                // Ignore scheme in backend for now
            }
            _ => {}
        }
    }

    let mut backend = Backend::new(address.ok_or_else(|| anyhow!("Backend missing address"))?);
    if let Some(p) = port {
        backend = backend.with_port(p);
    }

    Ok(backend)
}

fn parse_site_block(pair: pest::iterators::Pair<Rule>) -> Result<(Vec<Route>, Vec<Directive>)> {
    let mut routes = Vec::new();
    let mut directives = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::route => {
                routes.push(parse_route(inner)?);
            }
            Rule::directive => {
                if let Some(dir) = parse_directive(inner)? {
                    directives.push(dir);
                }
            }
            _ => {}
        }
    }

    Ok((routes, directives))
}

fn parse_route(pair: pest::iterators::Pair<Rule>) -> Result<Route> {
    let mut path = None;
    let mut route_directives = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::path => {
                path = Some(inner.as_str().to_string());
            }
            Rule::simple_proxy => {
                let backends = parse_simple_proxy(inner)?;
                route_directives.push(Directive::Proxy(backends));
            }
            Rule::route_block => {
                for block_inner in inner.into_inner() {
                    if let Rule::directive = block_inner.as_rule() {
                        if let Some(dir) = parse_directive(block_inner)? {
                            route_directives.push(dir);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Route {
        path: path.ok_or_else(|| anyhow!("Route missing path"))?,
        directives: route_directives,
    })
}

fn parse_directive(pair: pest::iterators::Pair<Rule>) -> Result<Option<Directive>> {
    for inner in pair.into_inner() {
        return match inner.as_rule() {
            Rule::proxy_directive => {
                let backends = parse_proxy_directive(inner)?;
                Ok(Some(Directive::Proxy(backends)))
            }
            Rule::lb_directive => {
                Ok(Some(parse_lb_directive(inner)?))
            }
            Rule::pool_directive => {
                Ok(Some(Directive::Pool(parse_pool_directive(inner)?)))
            }
            Rule::health_directive => {
                Ok(Some(Directive::HealthCheck(parse_health_directive(inner)?)))
            }
            Rule::tls_directive => {
                Ok(Some(Directive::Tls(parse_tls_directive(inner)?)))
            }
            Rule::tls_protocols_directive => {
                Ok(Some(parse_tls_protocols_directive(inner)?))
            }
            Rule::cors_directive => {
                Ok(Some(Directive::Cors(parse_cors_directive(inner)?)))
            }
            Rule::websocket_directive => {
                Ok(Some(Directive::WebSocket))
            }
            Rule::websocket_config_directive => {
                Ok(Some(parse_websocket_config_directive(inner)?))
            }
            Rule::grpc_directive => {
                Ok(Some(Directive::Grpc))
            }
            Rule::grpc_config_directive => {
                Ok(Some(parse_grpc_config_directive(inner)?))
            }
            Rule::http2_directive => {
                Ok(Some(parse_http2_directive(inner)?))
            }
            Rule::http3_directive => {
                Ok(Some(parse_http3_directive(inner)?))
            }
            Rule::quic_directive => {
                Ok(Some(parse_quic_directive(inner)?))
            }
            Rule::compress_directive => {
                Ok(Some(parse_compress_directive(inner)?))
            }
            Rule::compress_config_directive => {
                Ok(Some(parse_compress_config_directive(inner)?))
            }
            Rule::rate_limit_directive => {
                Ok(Some(parse_rate_limit_directive(inner)?))
            }
            Rule::timeout_directive => {
                Ok(Some(parse_timeout_directive(inner)?))
            }
            Rule::request_timeout_directive => {
                Ok(Some(parse_request_timeout_directive(inner)?))
            }
            Rule::headers_directive => {
                Ok(Some(parse_headers_directive(inner)?))
            }
            Rule::header_add_directive => {
                Ok(Some(parse_header_add_directive(inner)?))
            }
            Rule::header_remove_directive => {
                Ok(Some(parse_header_remove_directive(inner)?))
            }
            Rule::header_passthrough_directive => {
                Ok(Some(parse_header_passthrough_directive(inner)?))
            }
            Rule::circuit_breaker_directive => {
                Ok(Some(parse_circuit_breaker_directive(inner)?))
            }
            Rule::tls_passthrough_directive => {
                Ok(Some(parse_tls_passthrough_directive(inner)?))
            }
            Rule::keepalive_directive => {
                Ok(Some(parse_keepalive_directive(inner)?))
            }
            Rule::max_conns_directive => {
                Ok(Some(parse_max_conns_directive(inner)?))
            }
            Rule::connect_timeout_directive => {
                Ok(Some(parse_connect_timeout_directive(inner)?))
            }
            Rule::idle_timeout_directive => {
                Ok(Some(parse_idle_timeout_directive(inner)?))
            }
            Rule::websocket_timeout_directive => {
                Ok(Some(parse_websocket_timeout_directive(inner)?))
            }
            Rule::grpc_timeout_directive => {
                Ok(Some(parse_grpc_timeout_directive(inner)?))
            }
            Rule::buffer_pool_directive => {
                Ok(Some(parse_buffer_pool_directive(inner)?))
            }
            Rule::backpressure_directive => {
                Ok(Some(parse_backpressure_directive(inner)?))
            }
            Rule::cache_directive => {
                Ok(Some(parse_cache_directive(inner)?))
            }
            Rule::waf_directive => {
                Ok(Some(parse_waf_directive(inner)?))
            }
            Rule::graphql_directive => {
                Ok(Some(parse_graphql_directive(inner)?))
            }
            _ => Ok(None),
        };
    }
    Ok(None)
}

fn parse_proxy_directive(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Backend>> {
    let mut backends = Vec::new();

    for inner in pair.into_inner() {
        if let Rule::backend = inner.as_rule() {
            backends.push(parse_backend(inner)?);
        }
    }

    Ok(backends)
}

fn parse_lb_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::lb_algorithm = inner.as_rule() {
            let algo = match inner.as_str() {
                "round_robin" => LoadBalancingAlgorithm::RoundRobin,
                "least_conn" => LoadBalancingAlgorithm::LeastConnections,
                "ip_hash" => LoadBalancingAlgorithm::IpHash,
                "random" => LoadBalancingAlgorithm::Random,
                "weighted" => LoadBalancingAlgorithm::Weighted,
                "consistent_hash" => LoadBalancingAlgorithm::ConsistentHash,
                _ => return Err(anyhow!("Invalid load balancing algorithm")),
            };
            return Ok(Directive::LoadBalancing(algo));
        }
    }
    Err(anyhow!("Missing load balancing algorithm"))
}

fn parse_pool_directive(pair: pest::iterators::Pair<Rule>) -> Result<PoolConfig> {
    let mut config = PoolConfig::default();

    for inner in pair.into_inner() {
        if let Rule::pool_option = inner.as_rule() {
            let opt_str = inner.as_str();
            if let Some(value) = opt_str.strip_prefix("max=") {
                config.max_size = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("min=") {
                config.min_idle = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("min_idle=") {
                config.min_idle = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("max_idle=") {
                config.max_idle = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("max_open=") {
                config.max_size = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("lifetime=") {
                config.max_lifetime = Some(parse_duration(value)?);
            } else if let Some(value) = opt_str.strip_prefix("idle=") {
                config.idle_timeout = Some(parse_duration(value)?);
            } else if opt_str == "http2_multiplexing" {
                config.http2_multiplexing = true;
            }
        }
    }

    Ok(config)
}

fn parse_health_directive(pair: pest::iterators::Pair<Rule>) -> Result<HealthCheckConfig> {
    let mut config = HealthCheckConfig::default();

    for inner in pair.into_inner() {
        if let Rule::health_option = inner.as_rule() {
            let opt_str = inner.as_str();
            if let Some(value) = opt_str.strip_prefix("interval=") {
                config.interval = Some(parse_duration(value)?);
            } else if let Some(value) = opt_str.strip_prefix("timeout=") {
                config.timeout = Some(parse_duration(value)?);
            } else if let Some(value) = opt_str.strip_prefix("path=") {
                config.path = Some(value.trim_matches('"').to_string());
            } else if let Some(value) = opt_str.strip_prefix("healthy=") {
                config.healthy_threshold = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("unhealthy=") {
                config.unhealthy_threshold = Some(value.parse()?);
            } else if opt_str == "grpc" {
                config.grpc = true;
            }
        }
    }

    Ok(config)
}

fn parse_tls_directive(pair: pest::iterators::Pair<Rule>) -> Result<TlsConfig> {
    let mut cert_file = None;
    let mut key_file = None;

    for inner in pair.into_inner() {
        let text = inner.as_str();
        match text {
            "internal" => return Ok(TlsConfig::Internal),
            s if s.contains('@') => {
                return Ok(TlsConfig::Auto {
                    email: Some(s.to_string()),
                });
            }
            s if s.starts_with("cert=") => {
                cert_file = Some(s.strip_prefix("cert=").unwrap().trim_matches('"').to_string());
            }
            s if s.starts_with("key=") => {
                key_file = Some(s.strip_prefix("key=").unwrap().trim_matches('"').to_string());
            }
            _ => {
                // Grammar produces two quoted_string tokens for cert and key
                if cert_file.is_none() {
                    cert_file = Some(text.trim_matches('"').to_string());
                } else if key_file.is_none() {
                    key_file = Some(text.trim_matches('"').to_string());
                }
            }
        }
    }

    // If we got both cert and key files, return Manual config
    if let (Some(cert), Some(key)) = (cert_file, key_file) {
        return Ok(TlsConfig::Manual {
            cert_file: cert,
            key_file: key,
        });
    }

    // Default: Auto TLS without email
    Ok(TlsConfig::Auto { email: None })
}

fn parse_cors_directive(pair: pest::iterators::Pair<Rule>) -> Result<CorsConfig> {
    let mut config = CorsConfig::default();

    for inner in pair.into_inner() {
        if let Rule::cors_option = inner.as_rule() {
            let opt_str = inner.as_str();
            if let Some(value) = opt_str.strip_prefix("origins=") {
                config.origins = Some(vec![value.trim_matches('"').to_string()]);
            } else if let Some(value) = opt_str.strip_prefix("methods=") {
                config.methods = Some(value.trim_matches('"').split(',').map(|s| s.trim().to_string()).collect());
            } else if let Some(value) = opt_str.strip_prefix("headers=") {
                config.headers = Some(value.trim_matches('"').split(',').map(|s| s.trim().to_string()).collect());
            } else if opt_str == "credentials" {
                config.credentials = true;
            }
        }
    }

    Ok(config)
}

fn parse_compress_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut algorithms = Vec::new();

    for inner in pair.into_inner() {
        if let Rule::compression_algo = inner.as_rule() {
            let algo = match inner.as_str() {
                "gzip" => CompressionAlgorithm::Gzip,
                "br" => CompressionAlgorithm::Brotli,
                "deflate" => CompressionAlgorithm::Deflate,
                "zstd" => CompressionAlgorithm::Zstd,
                _ => continue,
            };
            algorithms.push(algo);
        }
    }

    // Default to gzip if no algorithms specified
    if algorithms.is_empty() {
        algorithms.push(CompressionAlgorithm::Gzip);
    }

    Ok(Directive::Compress(algorithms))
}

fn parse_rate_limit_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut rate = None;
    let mut burst = None;
    let mut per = None;
    let mut per_ip = false;
    let mut seen_rate = false;

    let text = pair.as_str();
    if text.contains("per_ip") {
        per_ip = true;
    }

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::number => {
                if !seen_rate {
                    rate = Some(inner.as_str().parse()?);
                    seen_rate = true;
                } else {
                    burst = Some(inner.as_str().parse()?);
                }
            }
            Rule::duration => {
                per = Some(parse_duration(inner.as_str())?);
            }
            _ => {}
        }
    }

    Ok(Directive::RateLimit {
        rate: rate.ok_or_else(|| anyhow!("Rate limit missing rate"))?,
        burst,
        per,
        per_ip,
    })
}

fn parse_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::Timeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("Timeout directive missing duration"))
}

fn parse_headers_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut direction = None;
    let mut name = None;
    let mut value = None;

    let text = pair.as_str();
    direction = if text.starts_with("header_up") {
        Some(HeaderDirection::Up)
    } else {
        Some(HeaderDirection::Down)
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::header_name => {
                name = Some(inner.as_str().to_string());
            }
            Rule::header_value => {
                value = Some(inner.as_str().trim_matches('"').to_string());
            }
            _ => {}
        }
    }

    Ok(Directive::Header {
        direction: direction.unwrap(),
        name: name.ok_or_else(|| anyhow!("Header missing name"))?,
        value: value.ok_or_else(|| anyhow!("Header missing value"))?,
    })
}

fn parse_tls_passthrough_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut server_name = None;
    let mut backend = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::domain => {
                server_name = Some(inner.as_str().to_string());
            }
            Rule::backend => {
                backend = Some(parse_backend(inner)?);
            }
            _ => {}
        }
    }

    Ok(Directive::TlsPassthrough {
        server_name: server_name.ok_or_else(|| anyhow!("TLS passthrough missing server name"))?,
        backend: backend.ok_or_else(|| anyhow!("TLS passthrough missing backend"))?,
    })
}

fn parse_keepalive_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::Keepalive(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("Keepalive directive missing duration"))
}

fn parse_max_conns_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::number = inner.as_rule() {
            return Ok(Directive::MaxConnections(inner.as_str().parse()?));
        }
    }
    Err(anyhow!("max_conns directive missing number"))
}

fn parse_connect_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::ConnectTimeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("connect_timeout directive missing duration"))
}

fn parse_idle_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::IdleTimeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("idle_timeout directive missing duration"))
}

fn parse_buffer_pool_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = BufferPoolConfig {
        enabled: false,
        size: None,
        pool_size: None,
    };

    for inner in pair.into_inner() {
        if let Rule::buffer_pool_option = inner.as_rule() {
            let text = inner.as_str();
            if text == "enabled" {
                config.enabled = true;
            } else if let Some(size_str) = text.strip_prefix("size=") {
                config.size = Some(size_str.parse()?);
            } else if let Some(pool_size_str) = text.strip_prefix("pool_size=") {
                config.pool_size = Some(pool_size_str.parse()?);
            }
        }
    }

    Ok(Directive::BufferPool(config))
}

fn parse_backpressure_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = BackpressureConfig {
        enabled: false,
        max_connections: None,
        memory_limit: None,
    };

    for inner in pair.into_inner() {
        if let Rule::backpressure_option = inner.as_rule() {
            let text = inner.as_str();
            if text == "enabled" {
                config.enabled = true;
            } else if let Some(max_str) = text.strip_prefix("max_conns=") {
                config.max_connections = Some(max_str.parse()?);
            } else if let Some(mem_str) = text.strip_prefix("memory_limit=") {
                config.memory_limit = Some(parse_memory_size(mem_str)?);
            }
        }
    }

    Ok(Directive::Backpressure(config))
}

fn parse_memory_size(s: &str) -> Result<usize> {
    let s = s.trim();
    let (num_str, unit) = if s.ends_with("kb") {
        (&s[..s.len()-2], 1024)
    } else if s.ends_with("mb") {
        (&s[..s.len()-2], 1024 * 1024)
    } else if s.ends_with("gb") {
        (&s[..s.len()-2], 1024 * 1024 * 1024)
    } else if s.ends_with("tb") {
        (&s[..s.len()-2], 1024 * 1024 * 1024 * 1024)
    } else {
        (s, 1)
    };

    let num: usize = num_str.parse()?;
    Ok(num * unit)
}

fn parse_cache_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = CacheConfig {
        enabled: false,
        ttl: None,
        max_size: None,
        cleanup_interval: None,
        only_success: false,
        methods: Vec::new(),
        key_headers: Vec::new(),
    };

    for inner in pair.into_inner() {
        if let Rule::cache_option = inner.as_rule() {
            let text = inner.as_str();
            if text == "enabled" {
                config.enabled = true;
            } else if text == "only_success" {
                config.only_success = true;
            } else if let Some(ttl_str) = text.strip_prefix("ttl=") {
                config.ttl = Some(parse_duration(ttl_str)?);
            } else if let Some(size_str) = text.strip_prefix("max_size=") {
                config.max_size = Some(size_str.parse()?);
            } else if let Some(interval_str) = text.strip_prefix("cleanup_interval=") {
                config.cleanup_interval = Some(parse_duration(interval_str)?);
            } else if text.starts_with("methods") {
                // Parse HTTP methods
                for method_inner in inner.into_inner() {
                    if let Rule::http_method = method_inner.as_rule() {
                        config.methods.push(method_inner.as_str().to_string());
                    }
                }
            } else if text.starts_with("key_headers") {
                // Parse header names
                for header_inner in inner.into_inner() {
                    if let Rule::header_name = header_inner.as_rule() {
                        config.key_headers.push(header_inner.as_str().to_string());
                    }
                }
            }
        }
    }

    Ok(Directive::Cache(config))
}

fn parse_waf_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    use crate::config::dsl_ast::{WafMode, WafConfig, ModSecurityConfig, AwsWafConfig, CorazaConfig, WafRule, WafRuleType, WafRuleAction};

    let mut config = WafConfig {
        enabled: false,
        mode: WafMode::Custom,
        block_mode: true,
        max_body_size: None,
        modsecurity: None,
        aws_waf: None,
        coraza: None,
        custom_rules: Vec::new(),
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::waf_option => {
                let text = inner.as_str();
                if text == "enabled" {
                    config.enabled = true;
                } else if text == "block" {
                    config.block_mode = true;
                } else if text == "log_only" {
                    config.block_mode = false;
                } else if let Some(mode_str) = text.strip_prefix("mode=") {
                    config.mode = match mode_str {
                        "custom" => WafMode::Custom,
                        "modsecurity" => WafMode::ModSecurity,
                        "coraza" => WafMode::Coraza,
                        "aws" => WafMode::Aws,
                        _ => return Err(anyhow!("Invalid WAF mode: {}", mode_str)),
                    };
                } else if let Some(size_str) = text.strip_prefix("max_body_size=") {
                    config.max_body_size = Some(size_str.parse()?);
                }
            }
            Rule::waf_block => {
                // Parse nested WAF engine configurations
                for engine_inner in inner.into_inner() {
                    parse_waf_engine_directive(engine_inner, &mut config)?;
                }
            }
            _ => {}
        }
    }

    Ok(Directive::Waf(config))
}

fn parse_waf_engine_directive(pair: pest::iterators::Pair<Rule>, config: &mut crate::config::dsl_ast::WafConfig) -> Result<()> {
    use crate::config::dsl_ast::{ModSecurityConfig, AwsWafConfig, CorazaConfig, WafRule, WafRuleType, WafRuleAction};

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::modsecurity_directive => {
                let mut modsec_config = ModSecurityConfig {
                    rules_file: None,
                    paranoia_level: None,
                    audit_log: None,
                };

                for block_inner in inner.into_inner() {
                    if let Rule::modsecurity_block = block_inner.as_rule() {
                        for opt_inner in block_inner.into_inner() {
                            if let Rule::modsecurity_option = opt_inner.as_rule() {
                                let text = opt_inner.as_str();
                                if let Some(rules) = text.strip_prefix("rules_file ") {
                                    modsec_config.rules_file = Some(rules.trim_matches('"').to_string());
                                } else if let Some(level) = text.strip_prefix("paranoia_level ") {
                                    modsec_config.paranoia_level = Some(level.parse()?);
                                } else if let Some(log) = text.strip_prefix("audit_log ") {
                                    modsec_config.audit_log = Some(log.trim_matches('"').to_string());
                                }
                            }
                        }
                    }
                }
                config.modsecurity = Some(modsec_config);
            }
            Rule::aws_waf_directive => {
                let mut aws_config = AwsWafConfig {
                    web_acl_id: String::new(),
                    region: String::new(),
                    api_mode: None,
                };

                for block_inner in inner.into_inner() {
                    if let Rule::aws_waf_block = block_inner.as_rule() {
                        for opt_inner in block_inner.into_inner() {
                            if let Rule::aws_waf_option = opt_inner.as_rule() {
                                let text = opt_inner.as_str();
                                if let Some(acl) = text.strip_prefix("web_acl_id ") {
                                    aws_config.web_acl_id = acl.trim_matches('"').to_string();
                                } else if let Some(region) = text.strip_prefix("region ") {
                                    aws_config.region = region.to_string();
                                } else if let Some(mode) = text.strip_prefix("api_mode ") {
                                    aws_config.api_mode = Some(mode.to_string());
                                }
                            }
                        }
                    }
                }
                config.aws_waf = Some(aws_config);
            }
            Rule::coraza_directive => {
                let mut coraza_config = CorazaConfig {
                    rules_dir: None,
                    audit_log: None,
                };

                for block_inner in inner.into_inner() {
                    if let Rule::coraza_block = block_inner.as_rule() {
                        for opt_inner in block_inner.into_inner() {
                            if let Rule::coraza_option = opt_inner.as_rule() {
                                let text = opt_inner.as_str();
                                if let Some(dir) = text.strip_prefix("rules_dir ") {
                                    coraza_config.rules_dir = Some(dir.trim_matches('"').to_string());
                                } else if let Some(log) = text.strip_prefix("audit_log ") {
                                    coraza_config.audit_log = Some(log.trim_matches('"').to_string());
                                }
                            }
                        }
                    }
                }
                config.coraza = Some(coraza_config);
            }
            Rule::waf_rule_directive => {
                let mut rule_type = None;
                let mut rule_action = WafRuleAction::Enabled;

                for rule_inner in inner.into_inner() {
                    match rule_inner.as_rule() {
                        Rule::waf_rule_type => {
                            rule_type = Some(match rule_inner.as_str() {
                                "sql_injection" => WafRuleType::SqlInjection,
                                "xss" => WafRuleType::Xss,
                                "path_traversal" => WafRuleType::PathTraversal,
                                "rate_limit" => WafRuleType::RateLimit,
                                "user_agent" => WafRuleType::UserAgent,
                                "method" => WafRuleType::Method,
                                _ => return Err(anyhow!("Unknown WAF rule type")),
                            });
                        }
                        Rule::waf_rule_action => {
                            rule_action = match rule_inner.as_str() {
                                "enabled" => WafRuleAction::Enabled,
                                "disabled" => WafRuleAction::Disabled,
                                "block" => WafRuleAction::Block,
                                "log" => WafRuleAction::Log,
                                _ => return Err(anyhow!("Unknown WAF rule action")),
                            };
                        }
                        _ => {}
                    }
                }

                if let Some(rt) = rule_type {
                    config.custom_rules.push(WafRule {
                        rule_type: rt,
                        action: rule_action,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn parse_graphql_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    use crate::config::dsl_ast::{GraphQLConfig, GraphQLBackend};

    let mut config = GraphQLConfig::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::graphql_option => {
                let text = inner.as_str();
                if text == "enabled" {
                    config.enabled = true;
                } else if text == "introspection" {
                    config.introspection_enabled = true;
                } else if let Some(endpoint_str) = text.strip_prefix("endpoint=") {
                    let endpoint = endpoint_str.trim_matches('"');
                    config.endpoint = Some(endpoint.to_string());
                } else if text.starts_with("cache") {
                    config.enable_cache = true;
                    // Look for ttl option in the nested parts
                    for cache_inner in inner.into_inner() {
                        let cache_text = cache_inner.as_str();
                        if let Some(ttl_str) = cache_text.strip_prefix("ttl=") {
                            config.cache_ttl = Some(parse_duration(ttl_str)?);
                        }
                    }
                } else if text.starts_with("batching") {
                    config.enable_batching = true;
                    // Look for max_size option in the nested parts
                    for batch_inner in inner.into_inner() {
                        let batch_text = batch_inner.as_str();
                        if let Some(size_str) = batch_text.strip_prefix("max_size=") {
                            config.max_batch_size = Some(size_str.parse()?);
                        }
                    }
                }
            }
            Rule::graphql_block => {
                for block_inner in inner.into_inner() {
                    if let Rule::graphql_stitching_directive = block_inner.as_rule() {
                        parse_graphql_stitching(block_inner, &mut config)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Directive::GraphQL(config))
}

fn parse_graphql_stitching(pair: pest::iterators::Pair<Rule>, config: &mut crate::config::dsl_ast::GraphQLConfig) -> Result<()> {
    use crate::config::dsl_ast::GraphQLBackend;

    for inner in pair.into_inner() {
        if let Rule::graphql_backend_block = inner.as_rule() {
            let mut backend_name = String::new();
            let mut backend_url = String::new();
            let mut backend_namespace: Option<String> = None;

            for backend_inner in inner.into_inner() {
                match backend_inner.as_rule() {
                    Rule::identifier => {
                        // First identifier is the backend name
                        if backend_name.is_empty() {
                            backend_name = backend_inner.as_str().to_string();
                        }
                    }
                    Rule::graphql_backend_option => {
                        let text = backend_inner.as_str();
                        if text.starts_with("url") {
                            for url_inner in backend_inner.into_inner() {
                                if let Rule::quoted_string = url_inner.as_rule() {
                                    backend_url = url_inner.as_str().trim_matches('"').to_string();
                                }
                            }
                        } else if text.starts_with("namespace") {
                            for ns_inner in backend_inner.into_inner() {
                                if let Rule::identifier = ns_inner.as_rule() {
                                    backend_namespace = Some(ns_inner.as_str().to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !backend_name.is_empty() && !backend_url.is_empty() {
                config.backends.push(GraphQLBackend {
                    name: backend_name,
                    url: backend_url,
                    namespace: backend_namespace,
                });
            }
        }
    }

    Ok(())
}

fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();

    // Parse number and unit
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len()-2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len()-1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len()-1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len()-1], "h")
    } else if s.ends_with('d') {
        (&s[..s.len()-1], "d")
    } else {
        return Err(anyhow!("Invalid duration format: {}", s));
    };

    let num: u64 = num_str.parse()
        .with_context(|| format!("Invalid duration number: {}", num_str))?;

    let duration = match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        _ => return Err(anyhow!("Invalid duration unit: {}", unit)),
    };

    Ok(duration)
}

// New directive parsers

fn parse_tls_protocols_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut versions = Vec::new();

    for inner in pair.into_inner() {
        if let Rule::tls_version = inner.as_rule() {
            let version = match inner.as_str() {
                "TLSv1.2" => TlsVersion::TLSv1_2,
                "TLSv1.3" => TlsVersion::TLSv1_3,
                _ => continue,
            };
            versions.push(version);
        }
    }

    Ok(Directive::TlsProtocols(versions))
}

fn parse_request_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::RequestTimeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("Request timeout directive missing duration"))
}

fn parse_header_add_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut name = None;
    let mut value = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::header_name => {
                name = Some(inner.as_str().to_string());
            }
            Rule::header_value => {
                value = Some(inner.as_str().trim_matches('"').to_string());
            }
            _ => {}
        }
    }

    Ok(Directive::HeaderAdd {
        name: name.ok_or_else(|| anyhow!("Header add missing name"))?,
        value: value.ok_or_else(|| anyhow!("Header add missing value"))?,
    })
}

fn parse_header_remove_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::header_name = inner.as_rule() {
            return Ok(Directive::HeaderRemove {
                name: inner.as_str().to_string(),
            });
        }
    }
    Err(anyhow!("Header remove directive missing name"))
}

fn parse_header_passthrough_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut headers = Vec::new();

    for inner in pair.into_inner() {
        if let Rule::header_name = inner.as_rule() {
            headers.push(inner.as_str().to_string());
        }
    }

    Ok(Directive::HeaderPassthrough(headers))
}

fn parse_circuit_breaker_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = CircuitBreakerConfig::default();

    for inner in pair.into_inner() {
        if let Rule::circuit_breaker_option = inner.as_rule() {
            let opt_str = inner.as_str();
            if let Some(value) = opt_str.strip_prefix("threshold=") {
                config.threshold = Some(value.parse()?);
            } else if let Some(value) = opt_str.strip_prefix("timeout=") {
                config.timeout = Some(parse_duration(value)?);
            } else if let Some(value) = opt_str.strip_prefix("window=") {
                config.window = Some(parse_duration(value)?);
            }
        }
    }

    Ok(Directive::CircuitBreaker(config))
}

fn parse_compress_config_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut algorithms = Vec::new();
    let mut level = None;

    for inner in pair.into_inner() {
        if let Rule::compression_algo = inner.as_rule() {
            let algo = match inner.as_str() {
                "gzip" => CompressionAlgorithm::Gzip,
                "br" => CompressionAlgorithm::Brotli,
                "deflate" => CompressionAlgorithm::Deflate,
                "zstd" => CompressionAlgorithm::Zstd,
                _ => continue,
            };
            algorithms.push(algo);
        } else if let Rule::number = inner.as_rule() {
            level = Some(inner.as_str().parse()?);
        }
    }

    if algorithms.is_empty() {
        algorithms.push(CompressionAlgorithm::Gzip);
    }

    Ok(Directive::CompressConfig(CompressionConfig {
        algorithms,
        level,
    }))
}

fn parse_websocket_config_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = WebSocketConfig::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::websocket_enabled_directive => {
                config.enabled = true;
            }
            Rule::websocket_max_message_size_directive => {
                for size_inner in inner.into_inner() {
                    if let Rule::number = size_inner.as_rule() {
                        config.max_message_size = Some(size_inner.as_str().parse()?);
                    }
                }
            }
            Rule::websocket_buffer_size_directive => {
                for size_inner in inner.into_inner() {
                    if let Rule::number = size_inner.as_rule() {
                        config.buffer_size = Some(size_inner.as_str().parse()?);
                    }
                }
            }
            Rule::websocket_compression_directive => {
                let text = inner.as_str();
                config.compression = text.contains("enabled");
            }
            _ => {}
        }
    }

    Ok(Directive::WebSocketConfig(config))
}

fn parse_websocket_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::WebSocketTimeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("WebSocket timeout directive missing duration"))
}

fn parse_grpc_config_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = GrpcConfig::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::grpc_enabled_directive => {
                config.enabled = true;
            }
            Rule::grpc_timeout_directive_inline => {
                for timeout_inner in inner.into_inner() {
                    if let Rule::duration = timeout_inner.as_rule() {
                        config.timeout = Some(parse_duration(timeout_inner.as_str())?);
                    }
                }
            }
            Rule::grpc_max_message_size_directive => {
                for size_inner in inner.into_inner() {
                    if let Rule::number = size_inner.as_rule() {
                        config.max_message_size = Some(size_inner.as_str().parse()?);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Directive::GrpcConfig(config))
}

fn parse_grpc_timeout_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    for inner in pair.into_inner() {
        if let Rule::duration = inner.as_rule() {
            return Ok(Directive::GrpcTimeout(parse_duration(inner.as_str())?));
        }
    }
    Err(anyhow!("gRPC timeout directive missing duration"))
}

fn parse_http2_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = Http2Config::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::http2_enabled_directive => {
                config.enabled = true;
            }
            Rule::http2_max_concurrent_streams_directive => {
                for streams_inner in inner.into_inner() {
                    if let Rule::number = streams_inner.as_rule() {
                        config.max_concurrent_streams = Some(streams_inner.as_str().parse()?);
                    }
                }
            }
            Rule::http2_initial_window_size_directive => {
                for window_inner in inner.into_inner() {
                    if let Rule::number = window_inner.as_rule() {
                        config.initial_window_size = Some(window_inner.as_str().parse()?);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Directive::Http2Config(config))
}

fn parse_http3_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = Http3Config::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::http3_enabled_directive => {
                config.enabled = true;
                let text = inner.as_str();
                if let Some(port_str) = text.strip_prefix("http3 enabled port=") {
                    if let Ok(port) = port_str.trim().parse::<u16>() {
                        config.port = Some(port);
                    }
                }
            }
            Rule::http3_max_streams_directive => {
                for streams_inner in inner.into_inner() {
                    if let Rule::number = streams_inner.as_rule() {
                        config.max_streams = Some(streams_inner.as_str().parse()?);
                    }
                }
            }
            Rule::http3_initial_max_data_directive => {
                for data_inner in inner.into_inner() {
                    if let Rule::number = data_inner.as_rule() {
                        config.initial_max_data = Some(data_inner.as_str().parse()?);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Directive::Http3Config(config))
}

fn parse_quic_directive(pair: pest::iterators::Pair<Rule>) -> Result<Directive> {
    let mut config = QuicConfig::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::quic_ack_delay_directive => {
                for delay_inner in inner.into_inner() {
                    if let Rule::duration = delay_inner.as_rule() {
                        config.ack_delay = Some(parse_duration(delay_inner.as_str())?);
                    }
                }
            }
            Rule::quic_max_idle_timeout_directive => {
                for timeout_inner in inner.into_inner() {
                    if let Rule::duration = timeout_inner.as_rule() {
                        config.max_idle_timeout = Some(parse_duration(timeout_inner.as_str())?);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Directive::QuicConfig(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172800));
    }

    #[test]
    fn test_parse_simple_proxy() {
        let input = "localhost:8080 proxy backend:3000\n";
        let config = parse_dsl(input).unwrap();

        assert_eq!(config.sites.len(), 1);
        assert_eq!(config.sites[0].directives.len(), 1);

        if let Directive::Proxy(backends) = &config.sites[0].directives[0] {
            assert_eq!(backends.len(), 1);
            assert_eq!(backends[0].address, "backend");
            assert_eq!(backends[0].port, Some(3000));
        } else {
            panic!("Expected Proxy directive");
        }
    }

    #[test]
    fn test_parse_https_site() {
        let input = "https://example.com proxy localhost:3000\n";
        let config = parse_dsl(input).unwrap();

        assert_eq!(config.sites.len(), 1);

        if let SiteAddress::Http { scheme, domain, .. } = &config.sites[0].address {
            assert_eq!(*scheme, Scheme::Https);
            assert_eq!(domain, "example.com");
        } else {
            panic!("Expected HTTP site address");
        }
    }

    #[test]
    fn test_parse_tcp_proxy() {
        let input = ":3306 mysql proxy db1:3306 db2:3306\n";
        let config = parse_dsl(input).unwrap();

        assert_eq!(config.sites.len(), 1);

        if let SiteAddress::Tcp { port, protocol } = &config.sites[0].address {
            assert_eq!(*port, 3306);
            assert_eq!(*protocol, TcpProtocol::Mysql);
        } else {
            panic!("Expected TCP site address");
        }

        if let Directive::Proxy(backends) = &config.sites[0].directives[0] {
            assert_eq!(backends.len(), 2);
        } else {
            panic!("Expected Proxy directive");
        }
    }

    #[test]
    fn test_parse_load_balancing() {
        let input = r#"
example.com {
    proxy server1:8080 server2:8080
    lb least_conn
}
"#;
        let config = parse_dsl(input).unwrap();

        assert_eq!(config.sites.len(), 1);
        assert_eq!(config.sites[0].directives.len(), 2);

        if let Directive::LoadBalancing(algo) = &config.sites[0].directives[1] {
            assert_eq!(*algo, LoadBalancingAlgorithm::LeastConnections);
        } else {
            panic!("Expected LoadBalancing directive");
        }
    }

    #[test]
    fn test_parse_global_log_directive() {
        let input = "log debug\nlocalhost:8080 proxy backend:3000\n";
        let config = parse_dsl(input).unwrap();

        assert_eq!(config.global.log_level, Some(LogLevel::Debug));
    }

    #[test]
    fn test_parse_pool_config() {
        let input = r#"
:5432 postgres {
    proxy pg1:5432
    pool max=500 min=20 lifetime=1h
}
"#;
        let config = parse_dsl(input).unwrap();

        if let Directive::Pool(pool) = &config.sites[0].directives[1] {
            assert_eq!(pool.max_size, Some(500));
            assert_eq!(pool.min_idle, Some(20));
            assert_eq!(pool.max_lifetime, Some(Duration::from_secs(3600)));
        } else {
            panic!("Expected Pool directive");
        }
    }

    #[test]
    fn test_parse_cors() {
        let input = r#"
example.com {
    proxy backend:8080
    cors
}
"#;
        let config = parse_dsl(input).unwrap();

        if let Directive::Cors(_) = &config.sites[0].directives[1] {
            // Success
        } else {
            panic!("Expected CORS directive");
        }
    }

    #[test]
    fn test_parse_websocket() {
        let input = r#"
ws.example.com {
    websocket
    proxy ws:8081
}
"#;
        let config = parse_dsl(input).unwrap();

        if let Directive::WebSocket = &config.sites[0].directives[0] {
            // Success
        } else {
            panic!("Expected WebSocket directive");
        }
    }

    #[test]
    fn test_parse_rate_limit() {
        let input = r#"
api.example.com {
    proxy api:8080
    rate_limit 100 per 1s
}
"#;
        let config = parse_dsl(input).unwrap();

        if let Directive::RateLimit { rate, per, .. } = &config.sites[0].directives[1] {
            assert_eq!(*rate, 100);
            assert_eq!(*per, Some(Duration::from_secs(1)));
        } else {
            panic!("Expected RateLimit directive");
        }
    }
}
