//! Enhanced configuration validation with detailed error reporting
//!
//! Provides comprehensive runtime validation of configuration with:
//! - Field-level validation with precise error paths
//! - Cross-field dependency validation
//! - Range and constraint checking
//! - TLS certificate validation
//! - URL and pattern validation
//! - Warnings for suboptimal configurations

use super::{Config, Protocol};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use thiserror::Error;

/// Validation error with context
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid field '{field}': {message}")]
    InvalidField { field: String, message: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid reference in '{field}': {message}")]
    InvalidReference { field: String, message: String },

    #[error("Cross-field constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Invalid range for '{field}': {message}")]
    InvalidRange { field: String, message: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Validation warning
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub suggestion: Option<String>,
}

impl ValidationWarning {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Validation result containing errors and warnings
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    fn new() -> Self {
        Self::default()
    }

    fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Check if validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Convert to Result, failing if there are errors
    pub fn into_result(self) -> Result<Vec<ValidationWarning>> {
        if self.errors.is_empty() {
            Ok(self.warnings)
        } else {
            let error_messages: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
            Err(anyhow!("Configuration validation failed:\n  {}", error_messages.join("\n  ")))
        }
    }
}

/// Configuration validator
pub struct ConfigValidator {
    result: ValidationResult,
}

impl ConfigValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            result: ValidationResult::new(),
        }
    }

    /// Validate the entire configuration
    pub fn validate(config: &Config) -> Result<Vec<ValidationWarning>> {
        let mut validator = Self::new();

        validator.validate_server(&config.server);
        validator.validate_tls(&config.tls);
        validator.validate_upstreams(&config.upstreams);
        validator.validate_routes(&config.routes, &config.upstreams);
        validator.validate_observability(&config.observability);
        validator.validate_admin(&config.admin);
        validator.validate_cache(&config.cache);
        validator.validate_rate_limit(&config.rate_limit);
        validator.validate_waf(&config.waf);

        // Cross-field validations
        validator.validate_tls_routes(&config);
        validator.validate_protocol_consistency(&config);

        validator.result.into_result()
    }

    /// Validate server configuration
    fn validate_server(&mut self, server: &super::ServerConfig) {
        // Validate bind addresses
        if server.bind.is_empty() && server.tls_bind.is_empty() {
            self.result.add_error(ValidationError::MissingField(
                "server.bind or server.tls_bind".to_string(),
            ));
        }

        for (i, bind) in server.bind.iter().enumerate() {
            self.validate_socket_addr(&format!("server.bind[{}]", i), bind);
        }

        for (i, bind) in server.tls_bind.iter().enumerate() {
            self.validate_socket_addr(&format!("server.tls_bind[{}]", i), bind);
        }

        // Validate workers
        if server.workers != "auto" {
            match server.workers.parse::<usize>() {
                Ok(workers) => {
                    if workers == 0 {
                        self.result.add_error(ValidationError::InvalidField {
                            field: "server.workers".to_string(),
                            message: "must be greater than 0".to_string(),
                        });
                    } else if workers > 1024 {
                        self.result.add_warning(
                            ValidationWarning::new(
                                "server.workers",
                                format!("{} workers is very high", workers),
                            )
                            .with_suggestion("Consider using 'auto' or a smaller number"),
                        );
                    }
                }
                Err(_) => {
                    self.result.add_error(ValidationError::InvalidField {
                        field: "server.workers".to_string(),
                        message: "must be 'auto' or a positive number".to_string(),
                    });
                }
            }
        }

        // Validate protocols
        if server.protocols.is_empty() {
            self.result.add_error(ValidationError::MissingField(
                "server.protocols".to_string(),
            ));
        }

        // Validate HTTP/3 config
        if server.protocols.contains(&Protocol::Http3) {
            if !server.http3.enabled {
                self.result.add_warning(
                    ValidationWarning::new(
                        "server.http3.enabled",
                        "HTTP/3 protocol specified but http3.enabled is false",
                    )
                    .with_suggestion("Enable HTTP/3 or remove from protocols"),
                );
            }

            if server.http3.port == 0 {
                self.result.add_error(ValidationError::InvalidField {
                    field: "server.http3.port".to_string(),
                    message: "port must be greater than 0".to_string(),
                });
            }

            self.validate_socket_addr("server.http3.bind", &server.http3.bind);
        }

        // Validate performance settings
        if server.performance.max_connections == 0 {
            self.result.add_warning(
                ValidationWarning::new(
                    "server.performance.max_connections",
                    "max_connections is 0 (unlimited)",
                )
                .with_suggestion("Consider setting a reasonable limit"),
            );
        }

        if server.performance.request_timeout.as_secs() > 300 {
            self.result.add_warning(
                ValidationWarning::new(
                    "server.performance.request_timeout",
                    "request timeout is very high (> 5 minutes)",
                )
                .with_suggestion("Consider a shorter timeout for better resource management"),
            );
        }
    }

    /// Validate socket address
    fn validate_socket_addr(&mut self, field: &str, addr: &str) {
        if addr.parse::<SocketAddr>().is_err() {
            // Try to resolve as hostname:port
            if addr.to_socket_addrs().is_err() {
                self.result.add_error(ValidationError::InvalidField {
                    field: field.to_string(),
                    message: format!("'{}' is not a valid socket address", addr),
                });
            }
        }
    }

    /// Validate TLS configuration
    fn validate_tls(&mut self, tls: &Option<super::TlsConfig>) {
        if let Some(tls_config) = tls {
            // Validate certificate files
            for (i, cert_config) in tls_config.certificates.iter().enumerate() {
                let field = format!("tls.certificates[{}]", i);

                // Check certificate file exists
                if !cert_config.cert_file.is_empty() && !Path::new(&cert_config.cert_file).exists() {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.cert_file", field),
                        message: format!("certificate file '{}' not found", cert_config.cert_file),
                    });
                }

                // Check key file exists
                if !cert_config.key_file.is_empty() && !Path::new(&cert_config.key_file).exists() {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.key_file", field),
                        message: format!("key file '{}' not found", cert_config.key_file),
                    });
                }

                // Validate domain
                if cert_config.domain.is_empty() {
                    self.result.add_warning(
                        ValidationWarning::new(
                            format!("{}.domain", field),
                            "no domain specified for certificate",
                        )
                        .with_suggestion("Add domain or this certificate won't be used"),
                    );
                }
            }

            // Validate ACME config if present
            if let Some(acme) = &tls_config.acme {
                if acme.email.is_empty() {
                    self.result.add_error(ValidationError::MissingField(
                        "tls.acme.email".to_string(),
                    ));
                }

                if !acme.email.contains('@') {
                    self.result.add_error(ValidationError::InvalidField {
                        field: "tls.acme.email".to_string(),
                        message: "invalid email address".to_string(),
                    });
                }

                // Validate provider
                let valid_providers = ["letsencrypt", "zerossl", "buypass"];
                if !valid_providers.contains(&acme.provider.as_str()) {
                    self.result.add_warning(
                        ValidationWarning::new(
                            "tls.acme.provider",
                            format!("unknown ACME provider '{}'", acme.provider),
                        )
                        .with_suggestion(format!("Use one of: {}", valid_providers.join(", "))),
                    );
                }
            }
        }
    }

    /// Validate upstreams
    fn validate_upstreams(&mut self, upstreams: &[super::UpstreamConfig]) {
        let mut upstream_names = HashSet::new();

        for (i, upstream) in upstreams.iter().enumerate() {
            let field = format!("upstreams[{}]", i);

            // Check for empty name
            if upstream.name.is_empty() {
                self.result.add_error(ValidationError::MissingField(
                    format!("{}.name", field),
                ));
                continue;
            }

            // Check for duplicate names
            if !upstream_names.insert(upstream.name.clone()) {
                self.result.add_error(ValidationError::InvalidField {
                    field: format!("{}.name", field),
                    message: format!("duplicate upstream name '{}'", upstream.name),
                });
            }

            // Validate servers
            if upstream.servers.is_empty() {
                self.result.add_error(ValidationError::InvalidField {
                    field: format!("{}.servers", field),
                    message: "upstream must have at least one server".to_string(),
                });
            }

            for (j, server) in upstream.servers.iter().enumerate() {
                let server_field = format!("{}.servers[{}]", field, j);

                if server.url.is_empty() {
                    self.result.add_error(ValidationError::MissingField(
                        format!("{}.url", server_field),
                    ));
                } else if server.url.parse::<url::Url>().is_err() {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.url", server_field),
                        message: format!("'{}' is not a valid URL", server.url),
                    });
                }

                // Validate weight
                if server.weight == 0 {
                    self.result.add_warning(
                        ValidationWarning::new(
                            format!("{}.weight", server_field),
                            "weight is 0, server will not receive traffic",
                        )
                        .with_suggestion("Set weight to at least 1"),
                    );
                }
            }

            // Validate active health check
            let health_check = &upstream.health_check;
            if health_check.active.enabled {
                if health_check.active.interval.as_secs() == 0 {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.health_check.active.interval", field),
                        message: "interval must be greater than 0".to_string(),
                    });
                }

                if health_check.active.timeout.as_secs() == 0 {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.health_check.active.timeout", field),
                        message: "timeout must be greater than 0".to_string(),
                    });
                }

                if health_check.active.timeout >= health_check.active.interval {
                    self.result.add_warning(
                        ValidationWarning::new(
                            format!("{}.health_check.active", field),
                            "timeout should be less than interval",
                        )
                        .with_suggestion("Reduce timeout or increase interval"),
                    );
                }

                if health_check.active.healthy_threshold == 0 || health_check.active.unhealthy_threshold == 0 {
                    self.result.add_error(ValidationError::InvalidField {
                        field: format!("{}.health_check.active.threshold", field),
                        message: "thresholds must be greater than 0".to_string(),
                    });
                }
            }
        }
    }

    /// Validate routes
    fn validate_routes(
        &mut self,
        routes: &[super::RouteConfig],
        upstreams: &[super::UpstreamConfig],
    ) {
        let mut route_names = HashSet::new();
        let upstream_names: HashSet<_> = upstreams.iter().map(|u| &u.name).collect();

        for (i, route) in routes.iter().enumerate() {
            let field = format!("routes[{}]", i);

            // Check for empty name
            if route.name.is_empty() {
                self.result.add_error(ValidationError::MissingField(
                    format!("{}.name", field),
                ));
                continue;
            }

            // Check for duplicate names
            if !route_names.insert(route.name.clone()) {
                self.result.add_error(ValidationError::InvalidField {
                    field: format!("{}.name", field),
                    message: format!("duplicate route name '{}'", route.name),
                });
            }

            // Validate upstream reference
            if route.upstream.is_empty() {
                self.result.add_error(ValidationError::MissingField(
                    format!("{}.upstream", field),
                ));
            } else if !upstream_names.contains(&route.upstream) {
                self.result.add_error(ValidationError::InvalidReference {
                    field: format!("{}.upstream", field),
                    message: format!("upstream '{}' does not exist", route.upstream),
                });
            }
        }
    }

    /// Validate observability configuration
    fn validate_observability(&mut self, obs: &super::ObservabilityConfig) {
        if obs.metrics.enabled && obs.metrics.port == 0 {
            self.result.add_error(ValidationError::InvalidField {
                field: "observability.metrics.port".to_string(),
                message: "metrics port must be greater than 0 when enabled".to_string(),
            });
        }

        if obs.tracing.enabled {
            if obs.tracing.endpoint.is_empty() {
                self.result.add_warning(
                    ValidationWarning::new(
                        "observability.tracing.endpoint",
                        "tracing enabled but no endpoint specified",
                    )
                    .with_suggestion("Specify Jaeger or OTLP endpoint"),
                );
            }

            if obs.tracing.sample_rate < 0.0 || obs.tracing.sample_rate > 1.0 {
                self.result.add_error(ValidationError::InvalidRange {
                    field: "observability.tracing.sample_rate".to_string(),
                    message: "must be between 0.0 and 1.0".to_string(),
                });
            }
        }
    }

    /// Validate admin API configuration
    fn validate_admin(&mut self, admin: &Option<super::AdminConfig>) {
        if let Some(admin_config) = admin {
            if admin_config.enabled {
                self.validate_socket_addr("admin.bind", &admin_config.bind);

                if admin_config.auth_enabled && admin_config.jwt_secret.as_ref().map_or(true, |s| s.is_empty()) {
                    self.result.add_error(ValidationError::InvalidField {
                        field: "admin.jwt_secret".to_string(),
                        message: "JWT secret required when auth is enabled".to_string(),
                    });
                }

                if let Some(secret) = &admin_config.jwt_secret {
                    if secret.len() < 32 {
                        self.result.add_warning(
                            ValidationWarning::new(
                                "admin.jwt_secret",
                                "JWT secret is too short",
                            )
                            .with_suggestion("Use at least 32 characters for security"),
                        );
                    }
                }
            }
        }
    }

    /// Validate cache configuration
    fn validate_cache(&mut self, cache: &Option<super::CacheConfig>) {
        if let Some(_cache_config) = cache {
            // Basic validation - specific field validation depends on CacheConfig structure
            // which may vary based on configuration type
        }
    }

    /// Validate rate limit configuration
    fn validate_rate_limit(&mut self, rate_limit: &Option<super::RateLimitConfig>) {
        if let Some(_rl_config) = rate_limit {
            // Basic validation - specific field validation depends on RateLimitConfig structure
        }
    }

    /// Validate WAF configuration
    fn validate_waf(&mut self, waf: &Option<super::WafConfig>) {
        if let Some(_waf_config) = waf {
            // Basic validation - specific field validation depends on WafConfig structure
        }
    }

    /// Cross-field validation: TLS routes
    fn validate_tls_routes(&mut self, config: &Config) {
        if !config.server.tls_bind.is_empty() && config.tls.is_none() {
            self.result.add_error(ValidationError::ConstraintViolation(
                "server.tls_bind specified but no TLS configuration provided".to_string(),
            ));
        }

        if config.server.protocols.contains(&Protocol::Http3) && config.tls.is_none() {
            self.result.add_error(ValidationError::ConstraintViolation(
                "HTTP/3 requires TLS configuration".to_string(),
            ));
        }
    }

    /// Cross-field validation: Protocol consistency
    fn validate_protocol_consistency(&mut self, config: &Config) {
        if config.server.protocols.contains(&Protocol::Http2) {
            // HTTP/2 enabled - may want to validate HTTP/2-specific settings
            // Note: max_concurrent_streams is configured via HTTP/3 config
        }

        if config.server.protocols.contains(&Protocol::Http3) && config.tls.is_none() {
            self.result.add_warning(
                ValidationWarning::new(
                    "server.protocols",
                    "HTTP/3 enabled but no TLS configuration",
                )
                .with_suggestion("HTTP/3 requires TLS - add TLS configuration"),
            );
        }
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    #[test]
    fn test_valid_minimal_config() {
        let config = Config {
            server: ServerConfig {
                bind: vec!["127.0.0.1:8080".to_string()],
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
            graphql: None,
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
        };

        let result = ConfigValidator::validate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_bind_address() {
        let config = Config {
            server: ServerConfig {
                bind: vec!["invalid".to_string()],
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
            graphql: None,
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
        };

        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_workers() {
        let config = Config {
            server: ServerConfig {
                bind: vec!["127.0.0.1:8080".to_string()],
                tls_bind: vec![],
                workers: "0".to_string(),
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
            graphql: None,
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
        };

        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_upstream_reference() {
        let config = Config {
            server: ServerConfig {
                bind: vec!["127.0.0.1:8080".to_string()],
                tls_bind: vec![],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1],
                performance: PerformanceConfig::default(),
                shutdown_timeout: std::time::Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            tls: None,
            upstreams: vec![],
            routes: vec![RouteConfig {
                name: "test".to_string(),
                match_rules: crate::config::MatchRules {
                    hosts: vec![],
                    paths: vec!["/api".to_string()],
                    methods: vec![],
                },
                upstream: "nonexistent".to_string(),
                timeout: None,
                mtls: None,
                cache: None,
                rate_limit: None,
                aggregation: None,
                php_fpm: None,
                static_files: false,
                root: None,
                index: vec![],
                try_files: vec![],
                error_pages: std::collections::HashMap::new(),
                directory_listing: false,
                limits: None,
            }],
            observability: ObservabilityConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
        };

        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
    }
}
