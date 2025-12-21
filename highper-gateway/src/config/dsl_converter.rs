/// DSL to Config Converter
///
/// Converts parsed DSL AST (`dsl_ast::Config`) into the runtime Config (`schema::Config`)

use anyhow::{Context, Result};

use crate::config::dsl_ast::{self, Directive, LoadBalancingAlgorithm, Scheme, SiteAddress};
use crate::config::loader::load_config;

/// Convert DSL Config to runtime Config
///
/// Strategy: Generate YAML from DSL AST, then use existing YAML loader
pub fn convert_dsl_to_config(dsl_config: dsl_ast::Config) -> Result<crate::config::schema::Config> {
    // Generate YAML from DSL
    let yaml_str = generate_yaml_from_dsl(&dsl_config)?;

    // Use existing YAML loader with validation
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("dsl_temp_{}.yaml", std::process::id()));

    std::fs::write(&temp_file, yaml_str.as_bytes())
        .context("Failed to write temporary YAML file")?;

    let config = load_config(temp_file.to_str().unwrap())
        .context("Failed to load generated YAML config")?;

    // Don't delete temp file - keep it for hot reload monitoring
    // let _ = std::fs::remove_file(&temp_file);

    Ok(config)
}

/// Generate YAML configuration from DSL AST
fn generate_yaml_from_dsl(dsl_config: &dsl_ast::Config) -> Result<String> {
    let mut yaml = String::new();

    // Collect bind addresses and TLS bind addresses
    let mut http_binds: Vec<String> = Vec::new();
    let mut https_binds: Vec<String> = Vec::new();
    let mut upstreams: Vec<UpstreamYaml> = Vec::new();
    let mut routes: Vec<RouteYaml> = Vec::new();
    let mut tls_certificates: Vec<CertYaml> = Vec::new();
    let mut global_rate_limit: Option<RateLimitYaml> = None;
    let mut acme_email: Option<String> = None;

    // Process each site
    for (idx, site) in dsl_config.sites.iter().enumerate() {
        let upstream_name = format!("upstream_{}", idx);

        // Extract bind addresses from site
        match &site.address {
            SiteAddress::Http { scheme, domain, port, .. } => {
                let bind_port = port.unwrap_or(match scheme {
                    Scheme::Http => 80,
                    Scheme::Https | Scheme::Grpc => 443,
                });

                let bind_addr = if domain == "*" || domain.is_empty() {
                    format!("0.0.0.0:{}", bind_port)
                } else {
                    format!("0.0.0.0:{}", bind_port)
                };

                match scheme {
                    Scheme::Http => {
                        if !http_binds.contains(&bind_addr) {
                            http_binds.push(bind_addr);
                        }
                    }
                    Scheme::Https | Scheme::Grpc => {
                        if !https_binds.contains(&bind_addr) {
                            https_binds.push(bind_addr);
                        }
                    }
                }

                // Create upstream from site directives
                let mut upstream = UpstreamYaml {
                    name: upstream_name.clone(),
                    servers: Vec::new(),
                    algorithm: "round_robin".to_string(),
                    health_check: None,
                };

                // Create route for this site
                let mut route = RouteYaml {
                    name: format!("route_{}", idx),
                    hosts: if domain != "*" && !domain.is_empty() {
                        vec![domain.clone()]
                    } else {
                        vec![]
                    },
                    paths: vec!["/*".to_string()], // Use wildcard to match all paths
                    upstream: upstream_name.clone(),
                    timeout: None,
                    rate_limit: None,
                    cache: None,
                    waf: None,
                    graphql: None,
                    php_fpm: None,
                    static_files: false,
                    root: None,
                    index: Vec::new(),
                    try_files: Vec::new(),
                    error_pages: std::collections::HashMap::new(),
                    directory_listing: false,
                };

                // Process site-level directives
                for directive in &site.directives {
                    process_directive(
                        directive,
                        &mut upstream,
                        &mut route,
                        &mut tls_certificates,
                        &mut global_rate_limit,
                        &mut acme_email,
                        domain,
                    );
                }

                // Process routes within the site
                for site_route in &site.routes {
                    let mut sub_upstream = UpstreamYaml {
                        name: format!("{}_{}", upstream_name, site_route.path.replace('/', "_")),
                        servers: upstream.servers.clone(), // Inherit from parent
                        algorithm: upstream.algorithm.clone(),
                        health_check: upstream.health_check.clone(),
                    };

                    let mut sub_route = RouteYaml {
                        name: format!("route_{}_{}", idx, site_route.path.replace('/', "_")),
                        hosts: route.hosts.clone(),
                        paths: vec![site_route.path.clone()],
                        upstream: sub_upstream.name.clone(),
                        timeout: None,
                        rate_limit: None,
                        cache: None,
                        waf: None,
                        graphql: None,
                        php_fpm: None,
                        static_files: false,
                        root: None,
                        index: Vec::new(),
                        try_files: Vec::new(),
                        error_pages: std::collections::HashMap::new(),
                        directory_listing: false,
                    };

                    for directive in &site_route.directives {
                        process_directive(
                            directive,
                            &mut sub_upstream,
                            &mut sub_route,
                            &mut tls_certificates,
                            &mut global_rate_limit,
                            &mut acme_email,
                            domain,
                        );
                    }

                    // Add sub-route if it has servers or static file configuration
                    let sub_has_static_config = sub_route.static_files
                        || sub_route.root.is_some()
                        || !sub_route.index.is_empty()
                        || !sub_route.try_files.is_empty()
                        || sub_route.php_fpm.is_some();

                    if !sub_upstream.servers.is_empty() || sub_has_static_config {
                        if !sub_upstream.servers.is_empty() {
                            upstreams.push(sub_upstream);
                        }
                        routes.push(sub_route);
                    }
                }

                // Add site-level upstream and route if it has servers or static file configuration
                let has_static_config = route.static_files
                    || route.root.is_some()
                    || !route.index.is_empty()
                    || !route.try_files.is_empty()
                    || route.php_fpm.is_some();

                if !upstream.servers.is_empty() || has_static_config {
                    if !upstream.servers.is_empty() {
                        upstreams.push(upstream);
                    }
                    routes.push(route);
                }
            }
            SiteAddress::Tcp { port, protocol } => {
                // TCP proxy: treat as HTTP for now (works for HTTP backends)
                // Future: add dedicated TCP proxy configuration section
                let bind_addr = format!("0.0.0.0:{}", port);
                if !http_binds.contains(&bind_addr) {
                    http_binds.push(bind_addr);
                }

                // Create upstream and route for TCP proxy
                let mut upstream = UpstreamYaml {
                    name: upstream_name.clone(),
                    servers: Vec::new(),
                    algorithm: "round_robin".to_string(),
                    health_check: None,
                };

                let mut route = RouteYaml {
                    name: format!("route_{}", idx),
                    hosts: vec![], // TCP doesn't use host matching
                    paths: vec!["/*".to_string()],
                    upstream: upstream_name.clone(),
                    timeout: None,
                    rate_limit: None,
                    cache: None,
                    waf: None,
                    graphql: None,
                    php_fpm: None,
                    static_files: false,
                    root: None,
                    index: Vec::new(),
                    try_files: Vec::new(),
                    error_pages: std::collections::HashMap::new(),
                    directory_listing: false,
                };

                // Process TCP site directives
                for directive in &site.directives {
                    process_directive(
                        directive,
                        &mut upstream,
                        &mut route,
                        &mut tls_certificates,
                        &mut global_rate_limit,
                        &mut acme_email,
                        "", // No domain for TCP
                    );
                }

                // Add upstream and route if backends were specified
                if !upstream.servers.is_empty() {
                    upstreams.push(upstream);
                    routes.push(route);
                }
            }
        }
    }

    // Default bind if none specified
    if http_binds.is_empty() && https_binds.is_empty() {
        http_binds.push("0.0.0.0:8080".to_string());
    }

    // Generate server section
    yaml.push_str("server:\n");
    // Always output bind field (required by schema)
    yaml.push_str("  bind:\n");
    for bind in &http_binds {
        yaml.push_str(&format!("    - \"{}\"\n", bind));
    }
    // If no HTTP binds, output empty array
    if http_binds.is_empty() {
        yaml.push_str("    []\n");
    }
    if !https_binds.is_empty() {
        yaml.push_str("  tls_bind:\n");
        for bind in &https_binds {
            yaml.push_str(&format!("    - \"{}\"\n", bind));
        }
    }

    // Log level from global config
    if let Some(ref level) = dsl_config.global.log_level {
        yaml.push_str(&format!("  # log_level: {}\n", level));
    }

    // Generate upstreams section
    yaml.push_str("\nupstreams:\n");
    for upstream in &upstreams {
        yaml.push_str(&format!("  - name: \"{}\"\n", upstream.name));
        yaml.push_str("    servers:\n");
        for server in &upstream.servers {
            yaml.push_str(&format!("      - url: \"{}\"\n", server.url));
            yaml.push_str(&format!("        weight: {}\n", server.weight));
        }
        yaml.push_str("    load_balancing:\n");
        yaml.push_str(&format!("      algorithm: {}\n", upstream.algorithm));

        if let Some(ref hc) = upstream.health_check {
            yaml.push_str("    health_check:\n");
            yaml.push_str("      active:\n");
            yaml.push_str("        enabled: true\n");
            yaml.push_str(&format!("        path: \"{}\"\n", hc.path));
            yaml.push_str(&format!("        interval: {}s\n", hc.interval_secs));
            yaml.push_str(&format!("        timeout: {}s\n", hc.timeout_secs));
            yaml.push_str(&format!("        healthy_threshold: {}\n", hc.healthy_threshold));
            yaml.push_str(&format!("        unhealthy_threshold: {}\n", hc.unhealthy_threshold));
        }
    }

    // Generate routes section
    yaml.push_str("\nroutes:\n");
    for route in &routes {
        yaml.push_str(&format!("  - name: \"{}\"\n", route.name));
        yaml.push_str("    match:\n");
        if !route.hosts.is_empty() {
            yaml.push_str("      hosts:\n");
            for host in &route.hosts {
                yaml.push_str(&format!("        - \"{}\"\n", host));
            }
        }
        yaml.push_str("      paths:\n");
        for path in &route.paths {
            yaml.push_str(&format!("        - \"{}\"\n", path));
        }
        yaml.push_str(&format!("    upstream: \"{}\"\n", route.upstream));

        if let Some(ref timeout) = route.timeout {
            yaml.push_str("    timeout:\n");
            yaml.push_str(&format!("      request: {}s\n", timeout));
        }

        if let Some(ref rl) = route.rate_limit {
            yaml.push_str("    rate_limit:\n");
            yaml.push_str("      enabled: true\n");
            yaml.push_str(&format!("      capacity: {}\n", rl.rate));
            yaml.push_str(&format!("      window: {}s\n", rl.window_secs));
        }

        if let Some(ref cache) = route.cache {
            yaml.push_str("    cache:\n");
            yaml.push_str(&format!("      enabled: {}\n", cache.enabled));
            yaml.push_str(&format!("      default_ttl: {}s\n", cache.ttl_secs));
            yaml.push_str(&format!("      max_size: {}\n", cache.max_size));
            yaml.push_str(&format!("      cleanup_interval: {}s\n", cache.cleanup_interval_secs));
            yaml.push_str(&format!("      cache_only_success: {}\n", cache.only_success));
            if !cache.methods.is_empty() {
                yaml.push_str("      methods:\n");
                for method in &cache.methods {
                    yaml.push_str(&format!("        - \"{}\"\n", method));
                }
            }
            if !cache.key_headers.is_empty() {
                yaml.push_str("      key_headers:\n");
                for header in &cache.key_headers {
                    yaml.push_str(&format!("        - \"{}\"\n", header));
                }
            }
        }

        if let Some(ref waf) = route.waf {
            yaml.push_str("    waf:\n");
            yaml.push_str(&format!("      enabled: {}\n", waf.enabled));
            yaml.push_str(&format!("      mode: \"{}\"\n", waf.mode));
            yaml.push_str(&format!("      block_mode: {}\n", waf.block_mode));
            yaml.push_str(&format!("      max_body_size: {}\n", waf.max_body_size));

            if let Some(ref modsec) = waf.modsecurity {
                yaml.push_str("      modsecurity:\n");
                if let Some(ref rules) = modsec.rules_file {
                    yaml.push_str(&format!("        rules_file: \"{}\"\n", rules));
                }
                if let Some(level) = modsec.paranoia_level {
                    yaml.push_str(&format!("        paranoia_level: {}\n", level));
                }
                if let Some(ref log) = modsec.audit_log {
                    yaml.push_str(&format!("        audit_log: \"{}\"\n", log));
                }
            }

            if let Some(ref aws) = waf.aws_waf {
                yaml.push_str("      aws_waf:\n");
                yaml.push_str(&format!("        web_acl_id: \"{}\"\n", aws.web_acl_id));
                yaml.push_str(&format!("        region: \"{}\"\n", aws.region));
                if let Some(ref mode) = aws.api_mode {
                    yaml.push_str(&format!("        api_mode: \"{}\"\n", mode));
                }
            }

            if let Some(ref coraza) = waf.coraza {
                yaml.push_str("      coraza:\n");
                if let Some(ref dir) = coraza.rules_dir {
                    yaml.push_str(&format!("        rules_dir: \"{}\"\n", dir));
                }
                if let Some(ref log) = coraza.audit_log {
                    yaml.push_str(&format!("        audit_log: \"{}\"\n", log));
                }
            }

            if !waf.custom_rules.is_empty() {
                yaml.push_str("      custom_rules:\n");
                for rule in &waf.custom_rules {
                    yaml.push_str(&format!("        - rule_type: \"{}\"\n", rule.rule_type));
                    yaml.push_str(&format!("          action: \"{}\"\n", rule.action));
                }
            }
        }

        if let Some(ref graphql) = route.graphql {
            yaml.push_str("    graphql:\n");
            yaml.push_str(&format!("      enabled: {}\n", graphql.enabled));
            yaml.push_str(&format!("      endpoint: \"{}\"\n", graphql.endpoint));
            yaml.push_str(&format!("      introspection_enabled: {}\n", graphql.introspection_enabled));
            yaml.push_str(&format!("      enable_cache: {}\n", graphql.enable_cache));
            yaml.push_str(&format!("      cache_ttl_secs: {}\n", graphql.cache_ttl_secs));
            yaml.push_str(&format!("      enable_batching: {}\n", graphql.enable_batching));
            yaml.push_str(&format!("      max_batch_size: {}\n", graphql.max_batch_size));

            if !graphql.backends.is_empty() {
                yaml.push_str("      backends:\n");
                for backend in &graphql.backends {
                    yaml.push_str(&format!("        - name: \"{}\"\n", backend.name));
                    yaml.push_str(&format!("          url: \"{}\"\n", backend.url));
                    if let Some(ref ns) = backend.namespace {
                        yaml.push_str(&format!("          namespace: \"{}\"\n", ns));
                    }
                }
            }
        }

        if let Some(ref php_fpm) = route.php_fpm {
            yaml.push_str("    php_fpm:\n");
            yaml.push_str(&format!("      enabled: {}\n", php_fpm.enabled));
            yaml.push_str(&format!("      socket: \"{}\"\n", php_fpm.socket));
            yaml.push_str(&format!("      pool_size: {}\n", php_fpm.pool_size));
            yaml.push_str(&format!("      connect_timeout_secs: {}\n", php_fpm.connect_timeout_secs));
            yaml.push_str(&format!("      read_timeout_secs: {}\n", php_fpm.read_timeout_secs));
            yaml.push_str(&format!("      write_timeout_secs: {}\n", php_fpm.write_timeout_secs));
            yaml.push_str(&format!("      keepalive_timeout_secs: {}\n", php_fpm.keepalive_timeout_secs));
            if !php_fpm.script_extensions.is_empty() {
                yaml.push_str("      script_extensions:\n");
                for ext in &php_fpm.script_extensions {
                    yaml.push_str(&format!("        - \"{}\"\n", ext));
                }
            }
        }

        if route.static_files {
            yaml.push_str("    static_files: true\n");
        }

        if let Some(ref root) = route.root {
            yaml.push_str(&format!("    root: \"{}\"\n", root));
        }

        if !route.index.is_empty() {
            yaml.push_str("    index:\n");
            for idx in &route.index {
                yaml.push_str(&format!("      - \"{}\"\n", idx));
            }
        }

        if !route.try_files.is_empty() {
            yaml.push_str("    try_files:\n");
            for pattern in &route.try_files {
                yaml.push_str(&format!("      - \"{}\"\n", pattern));
            }
        }

        if !route.error_pages.is_empty() {
            yaml.push_str("    error_pages:\n");
            for (status_code, file_path) in &route.error_pages {
                yaml.push_str(&format!("      {}: \"{}\"\n", status_code, file_path));
            }
        }

        if route.directory_listing {
            yaml.push_str("    directory_listing: true\n");
        }
    }

    // Generate TLS section
    yaml.push_str("\ntls:\n");
    if !tls_certificates.is_empty() || acme_email.is_some() {
        if let Some(email) = acme_email {
            yaml.push_str("  auto: true\n");
            yaml.push_str("  acme:\n");
            yaml.push_str(&format!("    email: \"{}\"\n", email));
            yaml.push_str("    provider: letsencrypt\n");
        }

        if !tls_certificates.is_empty() {
            yaml.push_str("  certificates:\n");
            for cert in &tls_certificates {
                yaml.push_str(&format!("    - domain: \"{}\"\n", cert.domain));
                yaml.push_str(&format!("      cert_file: \"{}\"\n", cert.cert_file));
                yaml.push_str(&format!("      key_file: \"{}\"\n", cert.key_file));
            }
        }
    }

    // Generate rate limiting section (global)
    if let Some(ref rl) = global_rate_limit {
        yaml.push_str("\nrate_limit:\n");
        yaml.push_str("  enabled: true\n");
        yaml.push_str(&format!("  capacity: {}\n", rl.rate));
        yaml.push_str(&format!("  window: {}s\n", rl.window_secs));
    }

    // Generate observability section
    yaml.push_str("\nobservability:\n");
    yaml.push_str("  logging:\n");
    if let Some(ref level) = dsl_config.global.log_level {
        yaml.push_str(&format!("    level: \"{}\"\n", level));
    } else {
        yaml.push_str("    level: \"info\"\n");
    }
    yaml.push_str("  metrics:\n");
    yaml.push_str(&format!("    enabled: {}\n", dsl_config.global.metrics_enabled));

    // Admin section
    if let Some(ref admin_addr) = dsl_config.global.admin_address {
        yaml.push_str("\nadmin:\n");
        yaml.push_str("  enabled: true\n");
        yaml.push_str(&format!("  bind: \"{}\"\n", admin_addr));
    }

    Ok(yaml)
}

/// Process a single directive and update the relevant YAML structures
fn process_directive(
    directive: &Directive,
    upstream: &mut UpstreamYaml,
    route: &mut RouteYaml,
    tls_certificates: &mut Vec<CertYaml>,
    global_rate_limit: &mut Option<RateLimitYaml>,
    acme_email: &mut Option<String>,
    domain: &str,
) {
    match directive {
        Directive::Proxy(backends) => {
            for backend in backends {
                let url = if backend.address.starts_with("http://") || backend.address.starts_with("https://") {
                    backend.full_address()
                } else {
                    format!("http://{}", backend.full_address())
                };

                upstream.servers.push(ServerYaml {
                    url,
                    weight: backend.weight.unwrap_or(1),
                });
            }
        }

        Directive::LoadBalancing(algo) => {
            upstream.algorithm = match algo {
                LoadBalancingAlgorithm::RoundRobin => "round_robin",
                LoadBalancingAlgorithm::LeastConnections => "least_conn",
                LoadBalancingAlgorithm::IpHash => "ip_hash",
                LoadBalancingAlgorithm::Random => "random",
                LoadBalancingAlgorithm::Weighted => "round_robin", // Use round_robin with weights
                LoadBalancingAlgorithm::ConsistentHash => "consistent_hash",
            }.to_string();
        }

        Directive::HealthCheck(hc) => {
            upstream.health_check = Some(HealthCheckYaml {
                path: hc.path.clone().unwrap_or_else(|| "/health".to_string()),
                interval_secs: hc.interval.map(|d| d.as_secs()).unwrap_or(10),
                timeout_secs: hc.timeout.map(|d| d.as_secs()).unwrap_or(5),
                healthy_threshold: hc.healthy_threshold.unwrap_or(2),
                unhealthy_threshold: hc.unhealthy_threshold.unwrap_or(3),
            });
        }

        Directive::Tls(tls_config) => {
            match tls_config {
                dsl_ast::TlsConfig::Auto { email } => {
                    *acme_email = email.clone().or_else(|| Some("admin@example.com".to_string()));
                }
                dsl_ast::TlsConfig::Manual { cert_file, key_file } => {
                    tls_certificates.push(CertYaml {
                        domain: domain.to_string(),
                        cert_file: cert_file.clone(),
                        key_file: key_file.clone(),
                    });
                }
                dsl_ast::TlsConfig::Internal => {
                    // Internal/self-signed - no additional config needed
                }
            }
        }

        Directive::RateLimit { rate, burst, per, per_ip } => {
            let window_secs = per.map(|d| d.as_secs()).unwrap_or(60);
            let rl = RateLimitYaml {
                rate: *rate as u32,
                window_secs,
            };

            // Note: burst and per_ip not yet supported in YAML schema
            let _ = (burst, per_ip);

            // Set on route if we have one, otherwise global
            if route.rate_limit.is_none() {
                route.rate_limit = Some(rl.clone());
            }
            if global_rate_limit.is_none() {
                *global_rate_limit = Some(rl);
            }
        }

        Directive::Timeout(duration) => {
            route.timeout = Some(duration.as_secs());
        }

        Directive::Pool(pool_config) => {
            // Pool config is handled at server level, not per-upstream currently
            // This would need schema changes to support per-upstream pools
            let _ = pool_config; // Acknowledge but not yet implemented
        }

        Directive::Cors(_cors) => {
            // CORS is handled by middleware, not directly in config
            // Would need additional schema support
        }

        Directive::WebSocket | Directive::Grpc => {
            // These are enabled at server level
        }

        Directive::Compress(_algos) => {
            // Compression is handled by middleware
        }

        Directive::Header { .. } => {
            // Header manipulation needs middleware support
        }

        Directive::TlsPassthrough { .. } => {
            // TLS passthrough needs special handling
        }

        Directive::Keepalive(_duration) => {
            // TODO: Add keepalive to server-level or upstream config
            // Requires YAML schema changes for per-upstream keepalive
        }

        Directive::MaxConnections(_max) => {
            // TODO: Add to server performance config
            // Requires passing global config through
        }

        Directive::ConnectTimeout(_duration) => {
            // TODO: Add to upstream timeout config
            // May need to extend UpstreamYaml struct
        }

        Directive::IdleTimeout(_duration) => {
            // TODO: Add to connection pool config
            // May need to extend pool configuration
        }

        Directive::BufferPool(_config) => {
            // TODO: Add to server performance config
            // Requires global config section in YAML generation
        }

        Directive::Backpressure(_config) => {
            // TODO: Add to server performance config
            // Requires global config section in YAML generation
        }

        Directive::Cache(cache_config) => {
            // Convert DSL CacheConfig to CacheYaml
            route.cache = Some(CacheYaml {
                enabled: cache_config.enabled,
                ttl_secs: cache_config.ttl.map(|d| d.as_secs()).unwrap_or(300),
                max_size: cache_config.max_size.unwrap_or(10000),
                cleanup_interval_secs: cache_config.cleanup_interval.map(|d| d.as_secs()).unwrap_or(60),
                only_success: cache_config.only_success,
                methods: if cache_config.methods.is_empty() {
                    vec!["GET".to_string(), "HEAD".to_string()]
                } else {
                    cache_config.methods.clone()
                },
                key_headers: cache_config.key_headers.clone(),
            });
        }

        Directive::Waf(waf_config) => {
            // Convert DSL WafConfig to WafYaml
            route.waf = Some(WafYaml {
                enabled: waf_config.enabled,
                mode: match waf_config.mode {
                    dsl_ast::WafMode::Custom => "custom".to_string(),
                    dsl_ast::WafMode::ModSecurity => "modsecurity".to_string(),
                    dsl_ast::WafMode::Coraza => "coraza".to_string(),
                    dsl_ast::WafMode::Aws => "aws".to_string(),
                },
                block_mode: waf_config.block_mode,
                max_body_size: waf_config.max_body_size.unwrap_or(10485760),
                modsecurity: waf_config.modsecurity.as_ref().map(|ms| ModSecurityYaml {
                    rules_file: ms.rules_file.clone(),
                    paranoia_level: ms.paranoia_level,
                    audit_log: ms.audit_log.clone(),
                }),
                aws_waf: waf_config.aws_waf.as_ref().map(|aws| AwsWafYaml {
                    web_acl_id: aws.web_acl_id.clone(),
                    region: aws.region.clone(),
                    api_mode: aws.api_mode.clone(),
                }),
                coraza: waf_config.coraza.as_ref().map(|cor| CorazaYaml {
                    rules_dir: cor.rules_dir.clone(),
                    audit_log: cor.audit_log.clone(),
                }),
                custom_rules: waf_config.custom_rules.iter().map(|rule| WafRuleYaml {
                    rule_type: format!("{:?}", rule.rule_type).to_lowercase(),
                    action: format!("{:?}", rule.action).to_lowercase(),
                }).collect(),
            });
        }

        Directive::GraphQL(graphql_config) => {
            // Convert DSL GraphQLConfig to GraphQLYaml
            route.graphql = Some(GraphQLYaml {
                enabled: graphql_config.enabled,
                endpoint: graphql_config.endpoint.clone().unwrap_or_else(|| "/graphql".to_string()),
                introspection_enabled: graphql_config.introspection_enabled,
                enable_cache: graphql_config.enable_cache,
                cache_ttl_secs: graphql_config.cache_ttl.map(|d| d.as_secs()).unwrap_or(300),
                enable_batching: graphql_config.enable_batching,
                max_batch_size: graphql_config.max_batch_size.unwrap_or(10),
                backends: graphql_config.backends.iter().map(|backend| GraphQLBackendYaml {
                    name: backend.name.clone(),
                    url: backend.url.clone(),
                    namespace: backend.namespace.clone(),
                }).collect(),
            });
        }

        Directive::PhpFpm(php_config) => {
            // Convert DSL PhpFpmConfig to PhpFpmYaml
            route.php_fpm = Some(PhpFpmYaml {
                enabled: php_config.enabled,
                socket: php_config.socket.clone().unwrap_or_else(|| "/var/run/php/php-fpm.sock".to_string()),
                pool_size: php_config.pool_size.unwrap_or(50),
                connect_timeout_secs: php_config.connect_timeout.map(|d| d.as_secs()).unwrap_or(5),
                read_timeout_secs: php_config.read_timeout.map(|d| d.as_secs()).unwrap_or(60),
                write_timeout_secs: php_config.write_timeout.map(|d| d.as_secs()).unwrap_or(60),
                keepalive_timeout_secs: php_config.keepalive_timeout.map(|d| d.as_secs()).unwrap_or(90),
                script_extensions: if php_config.script_extensions.is_empty() {
                    vec![".php".to_string()]
                } else {
                    php_config.script_extensions.clone()
                },
            });
        }

        Directive::StaticFiles => {
            route.static_files = true;
        }

        Directive::Root(path) => {
            route.root = Some(path.clone());
        }

        Directive::Index(files) => {
            route.index = files.clone();
        }

        Directive::TryFiles(patterns) => {
            route.try_files = patterns.clone();
        }

        Directive::ErrorPage(status_code, file_path) => {
            route.error_pages.insert(*status_code, file_path.clone());
        }

        Directive::DirectoryListing(enabled) => {
            route.directory_listing = *enabled;
        }

        // New advanced directives - stub implementations for validation
        Directive::TlsProtocols(_versions) => {
            // TLS protocol versions - would be applied at server TLS config level
        }

        Directive::RequestTimeout(_duration) => {
            // Request timeout - similar to Timeout directive
        }

        Directive::HeaderAdd { .. } | Directive::HeaderRemove { .. } => {
            // Header manipulation needs middleware support
        }

        Directive::HeaderPassthrough(_headers) => {
            // Header passthrough for WebSocket/gRPC protocols
        }

        Directive::CircuitBreaker(_config) => {
            // Circuit breaker pattern - requires middleware implementation
        }

        Directive::CompressConfig(_config) => {
            // Compression with level - extends basic compress directive
        }

        Directive::WebSocketConfig(_config) => {
            // WebSocket-specific configuration
        }

        Directive::WebSocketTimeout(_duration) => {
            // WebSocket connection timeout
        }

        Directive::GrpcConfig(_config) => {
            // gRPC-specific configuration
        }

        Directive::GrpcTimeout(_duration) => {
            // gRPC request timeout
        }

        Directive::Http2Config(_config) => {
            // HTTP/2 protocol configuration
        }

        Directive::Http3Config(_config) => {
            // HTTP/3 protocol configuration
        }

        Directive::QuicConfig(_config) => {
            // QUIC transport configuration
        }
    }
}

// Helper structs for YAML generation
#[derive(Clone)]
struct UpstreamYaml {
    name: String,
    servers: Vec<ServerYaml>,
    algorithm: String,
    health_check: Option<HealthCheckYaml>,
}

#[derive(Clone)]
struct ServerYaml {
    url: String,
    weight: u32,
}

#[derive(Clone)]
struct HealthCheckYaml {
    path: String,
    interval_secs: u64,
    timeout_secs: u64,
    healthy_threshold: u32,
    unhealthy_threshold: u32,
}

struct RouteYaml {
    name: String,
    hosts: Vec<String>,
    paths: Vec<String>,
    upstream: String,
    timeout: Option<u64>,
    rate_limit: Option<RateLimitYaml>,
    cache: Option<CacheYaml>,
    waf: Option<WafYaml>,
    graphql: Option<GraphQLYaml>,
    php_fpm: Option<PhpFpmYaml>,
    static_files: bool,
    root: Option<String>,
    index: Vec<String>,
    try_files: Vec<String>,
    error_pages: std::collections::HashMap<u16, String>,
    directory_listing: bool,
}

struct CertYaml {
    domain: String,
    cert_file: String,
    key_file: String,
}

#[derive(Clone)]
struct RateLimitYaml {
    rate: u32,
    window_secs: u64,
}

#[derive(Clone)]
struct CacheYaml {
    enabled: bool,
    ttl_secs: u64,
    max_size: usize,
    cleanup_interval_secs: u64,
    only_success: bool,
    methods: Vec<String>,
    key_headers: Vec<String>,
}

#[derive(Clone)]
struct WafYaml {
    enabled: bool,
    mode: String,
    block_mode: bool,
    max_body_size: usize,
    modsecurity: Option<ModSecurityYaml>,
    aws_waf: Option<AwsWafYaml>,
    coraza: Option<CorazaYaml>,
    custom_rules: Vec<WafRuleYaml>,
}

#[derive(Clone)]
struct ModSecurityYaml {
    rules_file: Option<String>,
    paranoia_level: Option<u8>,
    audit_log: Option<String>,
}

#[derive(Clone)]
struct AwsWafYaml {
    web_acl_id: String,
    region: String,
    api_mode: Option<String>,
}

#[derive(Clone)]
struct CorazaYaml {
    rules_dir: Option<String>,
    audit_log: Option<String>,
}

#[derive(Clone)]
struct WafRuleYaml {
    rule_type: String,
    action: String,
}

#[derive(Clone)]
struct GraphQLYaml {
    enabled: bool,
    endpoint: String,
    introspection_enabled: bool,
    enable_cache: bool,
    cache_ttl_secs: u64,
    enable_batching: bool,
    max_batch_size: usize,
    backends: Vec<GraphQLBackendYaml>,
}

#[derive(Clone)]
struct GraphQLBackendYaml {
    name: String,
    url: String,
    namespace: Option<String>,
}

#[derive(Clone)]
struct PhpFpmYaml {
    enabled: bool,
    socket: String,
    pool_size: usize,
    connect_timeout_secs: u64,
    read_timeout_secs: u64,
    write_timeout_secs: u64,
    keepalive_timeout_secs: u64,
    script_extensions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::dsl_ast::{Backend, GlobalConfig, Site};

    #[test]
    fn test_generate_yaml_empty() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("server:"));
        assert!(yaml.contains("bind:"));
        assert!(yaml.contains("upstreams:"));
        assert!(yaml.contains("routes:"));
    }

    #[test]
    fn test_generate_yaml_with_site() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "example.com".to_string(),
                    port: Some(8080),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![
                        Backend::new("localhost").with_port(3000),
                        Backend::new("localhost").with_port(3001).with_weight(2),
                    ]),
                    Directive::LoadBalancing(LoadBalancingAlgorithm::RoundRobin),
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("0.0.0.0:8080"));
        assert!(yaml.contains("upstream_0"));
        assert!(yaml.contains("http://localhost:3000"));
        assert!(yaml.contains("http://localhost:3001"));
        assert!(yaml.contains("weight: 2"));
        assert!(yaml.contains("round_robin"));
        assert!(yaml.contains("example.com"));
    }

    #[test]
    fn test_generate_yaml_with_health_check() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "api.example.com".to_string(),
                    port: Some(80),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![Backend::new("backend1").with_port(8080)]),
                    Directive::HealthCheck(dsl_ast::HealthCheckConfig {
                        interval: Some(std::time::Duration::from_secs(5)),
                        timeout: Some(std::time::Duration::from_secs(2)),
                        path: Some("/healthz".to_string()),
                        healthy_threshold: Some(3),
                        unhealthy_threshold: Some(2),
                        grpc: false,
                    }),
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("health_check:"));
        assert!(yaml.contains("path: \"/healthz\""));
        assert!(yaml.contains("interval: 5s"));
        assert!(yaml.contains("timeout: 2s"));
        assert!(yaml.contains("healthy_threshold: 3"));
        assert!(yaml.contains("unhealthy_threshold: 2"));
    }

    #[test]
    fn test_generate_yaml_with_https() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Https,
                    domain: "secure.example.com".to_string(),
                    port: Some(443),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![Backend::new("backend").with_port(8080)]),
                    Directive::Tls(dsl_ast::TlsConfig::Auto {
                        email: Some("admin@example.com".to_string()),
                    }),
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("tls_bind:"));
        assert!(yaml.contains("0.0.0.0:443"));
        assert!(yaml.contains("auto: true"));
        assert!(yaml.contains("email: \"admin@example.com\""));
    }

    #[test]
    fn test_generate_yaml_with_rate_limit() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "*".to_string(),
                    port: Some(8080),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![Backend::new("backend").with_port(3000)]),
                    Directive::RateLimit {
                        rate: 100,
                        per: Some(std::time::Duration::from_secs(60)),
                        burst: None,
                        per_ip: false,
                    },
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("rate_limit:"));
        assert!(yaml.contains("capacity: 100"));
        assert!(yaml.contains("window: 60s"));
    }

    #[test]
    fn test_generate_yaml_with_timeout() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "api.test".to_string(),
                    port: Some(80),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![Backend::new("slow-backend").with_port(8080)]),
                    Directive::Timeout(std::time::Duration::from_secs(120)),
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("timeout:"));
        assert!(yaml.contains("request: 120s"));
    }

    #[test]
    fn test_generate_yaml_with_manual_tls() {
        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Https,
                    domain: "mysite.com".to_string(),
                    port: Some(443),
                    base_path: None,
                },
                routes: vec![],
                directives: vec![
                    Directive::Proxy(vec![Backend::new("backend").with_port(8080)]),
                    Directive::Tls(dsl_ast::TlsConfig::Manual {
                        cert_file: "/etc/certs/mysite.crt".to_string(),
                        key_file: "/etc/certs/mysite.key".to_string(),
                    }),
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
        assert!(yaml.contains("certificates:"));
        assert!(yaml.contains("domain: \"mysite.com\""));
        assert!(yaml.contains("cert_file: \"/etc/certs/mysite.crt\""));
        assert!(yaml.contains("key_file: \"/etc/certs/mysite.key\""));
    }

    #[test]
    fn test_generate_yaml_multiple_lb_algorithms() {
        for (algo, expected) in [
            (LoadBalancingAlgorithm::RoundRobin, "round_robin"),
            (LoadBalancingAlgorithm::LeastConnections, "least_conn"),
            (LoadBalancingAlgorithm::IpHash, "ip_hash"),
            (LoadBalancingAlgorithm::Random, "random"),
            (LoadBalancingAlgorithm::ConsistentHash, "consistent_hash"),
        ] {
            let dsl_config = dsl_ast::Config {
                global: GlobalConfig::default(),
                sites: vec![Site {
                    address: SiteAddress::Http {
                        scheme: Scheme::Http,
                        domain: "test.com".to_string(),
                        port: Some(80),
                        base_path: None,
                    },
                    routes: vec![],
                    directives: vec![
                        Directive::Proxy(vec![Backend::new("backend").with_port(8080)]),
                        Directive::LoadBalancing(algo),
                    ],
                }],
            };

            let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();
            assert!(yaml.contains(&format!("algorithm: {}", expected)),
                "Expected {} for {:?}", expected, algo);
        }
    }

    #[test]
    fn test_generate_yaml_with_php_fpm() {
        use crate::config::dsl_ast::{PhpFpmConfig, Route};
        use std::time::Duration;

        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "php.example.com".to_string(),
                    port: Some(8080),
                    base_path: None,
                },
                routes: vec![
                    Route {
                        path: "/*.php".to_string(),
                        directives: vec![
                            Directive::PhpFpm(PhpFpmConfig {
                                enabled: true,
                                socket: Some("/var/run/php/php8.2-fpm.sock".to_string()),
                                pool_size: Some(50),
                                connect_timeout: Some(Duration::from_secs(5)),
                                read_timeout: Some(Duration::from_secs(60)),
                                write_timeout: Some(Duration::from_secs(60)),
                                keepalive_timeout: Some(Duration::from_secs(90)),
                                script_extensions: vec![".php".to_string(), ".phtml".to_string()],
                            }),
                            Directive::Proxy(vec![Backend::new("localhost").with_port(9000)]),
                        ],
                    },
                ],
                directives: vec![
                    Directive::Root("/var/www/html".to_string()),
                    Directive::Index(vec!["index.php".to_string(), "index.html".to_string()]),
                    Directive::StaticFiles,
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();

        // Verify PHP-FPM configuration
        assert!(yaml.contains("php_fpm:"), "Missing php_fpm section");
        assert!(yaml.contains("enabled: true"), "Missing enabled flag");
        assert!(yaml.contains("socket: \"/var/run/php/php8.2-fpm.sock\""), "Missing socket path");
        assert!(yaml.contains("pool_size: 50"), "Missing pool_size");
        assert!(yaml.contains("connect_timeout_secs: 5"), "Missing connect_timeout");
        assert!(yaml.contains("read_timeout_secs: 60"), "Missing read_timeout");
        assert!(yaml.contains("write_timeout_secs: 60"), "Missing write_timeout");
        assert!(yaml.contains("keepalive_timeout_secs: 90"), "Missing keepalive_timeout");
        assert!(yaml.contains("\".php\""), "Missing .php extension");
        assert!(yaml.contains("\".phtml\""), "Missing .phtml extension");

        // Verify static file configuration
        assert!(yaml.contains("root: \"/var/www/html\""), "Missing root directive");
        assert!(yaml.contains("index:"), "Missing index section");
        assert!(yaml.contains("\"index.php\""), "Missing index.php");
        assert!(yaml.contains("\"index.html\""), "Missing index.html");
        assert!(yaml.contains("static_files: true"), "Missing static_files flag");

        // Verify route path
        assert!(yaml.contains("/*.php"), "Missing PHP route path");
    }

    #[test]
    fn test_generate_yaml_with_try_files() {
        use crate::config::dsl_ast::Route;

        let dsl_config = dsl_ast::Config {
            global: GlobalConfig::default(),
            sites: vec![Site {
                address: SiteAddress::Http {
                    scheme: Scheme::Http,
                    domain: "static.example.com".to_string(),
                    port: Some(80),
                    base_path: None,
                },
                routes: vec![
                    Route {
                        path: "/*".to_string(),
                        directives: vec![
                            Directive::TryFiles(vec![
                                "$uri".to_string(),
                                "$uri/".to_string(),
                                "/index.html".to_string(),
                            ]),
                        ],
                    },
                ],
                directives: vec![
                    Directive::Root("/var/www/static".to_string()),
                    Directive::StaticFiles,
                ],
            }],
        };

        let yaml = generate_yaml_from_dsl(&dsl_config).unwrap();

        assert!(yaml.contains("try_files:"), "Missing try_files section");
        assert!(yaml.contains("\"$uri\""), "Missing $uri pattern");
        assert!(yaml.contains("\"$uri/\""), "Missing $uri/ pattern");
        assert!(yaml.contains("\"/index.html\""), "Missing fallback file");
        assert!(yaml.contains("root: \"/var/www/static\""), "Missing root directive");
        assert!(yaml.contains("static_files: true"), "Missing static_files flag");
    }
}
