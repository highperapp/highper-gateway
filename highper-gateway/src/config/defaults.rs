//! Smart defaults by protocol
//!
//! Provides intelligent default configurations optimized for different protocols and use cases:
//! - Database protocols (MySQL, PostgreSQL, Redis)
//! - HTTP/HTTPS APIs
//! - gRPC services
//! - WebSocket connections
//! - GraphQL gateways
//! - Static file serving
//! - PHP-FPM applications
//!
//! Each configuration is tuned for the specific protocol's characteristics and common patterns.

use super::schema::*;
use std::time::Duration;

/// Protocol-specific configuration presets
pub struct ProtocolDefaults;

impl ProtocolDefaults {
    /// Helper function to create a minimal base Config with all required fields
    fn create_base_config() -> Config {
        Config {
            server: ServerConfig::default(),
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: ObservabilityConfig::default(),
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for MySQL load balancing
    ///
    /// Optimized for:
    /// - Long-lived TCP connections
    /// - Connection pooling with pre-warming
    /// - Read/write splitting support
    /// - Health checks for database availability
    pub fn mysql() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:3306".to_string()],
                tls_bind: vec![],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1], // Placeholder, MySQL is TCP-based
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 10000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(60),
                    idle_timeout: Duration::from_secs(300), // 5 minutes
                    connection_pool: ConnectionPoolConfig {
                        max_idle_per_host: 200,
                        min_idle_per_host: 10, // Pre-warm 10 connections
                        max_connection_lifetime: Some(Duration::from_secs(3600)), // 1 hour
                        idle_timeout: Duration::from_secs(300),
                        prewarm: true,
                        metrics_enabled: true,
                    },
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: ObservabilityConfig {
                metrics: MetricsConfig {
                    enabled: true,
                    bind: "0.0.0.0:9090".to_string(),
                    endpoint: "/metrics".to_string(),
                    port: 9090,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        tcp: Some("debug".to_string()),
                        tls: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                tracing: TracingConfig::default(),
            },
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: Some(AdminConfig {
                enabled: true,
                bind: "127.0.0.1:9000".to_string(),
                auth_enabled: true,
                api_keys: vec![],
                jwt_secret: None,
                jwt_expiration: "24h".to_string(),
                cors_enabled: false,
                cors_origins: vec![],
                read_only: false,
            }),
            cache: None, // Caching not useful for database connections
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for PostgreSQL load balancing
    ///
    /// Similar to MySQL but with PostgreSQL-specific tuning
    pub fn postgresql() -> Config {
        let mut config = Self::mysql();
        config.server.bind = vec!["0.0.0.0:5432".to_string()];
        config.server.performance.idle_timeout = Duration::from_secs(600); // PostgreSQL prefers longer idle
        config
    }

    /// Get optimized configuration for Redis load balancing
    ///
    /// Optimized for:
    /// - Very fast response times
    /// - Many short-lived connections
    /// - Pipeline support
    /// - Pub/Sub patterns
    pub fn redis() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:6379".to_string()],
                tls_bind: vec![],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 50000, // Redis can handle many connections
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(2),
                    request_timeout: Duration::from_secs(5), // Redis is fast
                    idle_timeout: Duration::from_secs(60),
                    connection_pool: ConnectionPoolConfig {
                        max_idle_per_host: 500,
                        min_idle_per_host: 50,
                        max_connection_lifetime: Some(Duration::from_secs(1800)),
                        idle_timeout: Duration::from_secs(60),
                        prewarm: true,
                        metrics_enabled: true,
                    },
                },
                shutdown_timeout: Duration::from_secs(10),
                http3: Http3Config::default(),
            },
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        tcp: Some("info".to_string()),
                        cache: Some("debug".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for HTTP/HTTPS API gateway
    ///
    /// Optimized for:
    /// - RESTful APIs
    /// - JSON payloads
    /// - Rate limiting
    /// - Security headers
    /// - Compression
    pub fn http_api() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:8080".to_string()],
                tls_bind: vec!["0.0.0.0:8443".to_string()],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1, Protocol::Http2],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 100000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(30),
                    idle_timeout: Duration::from_secs(90),
                    connection_pool: ConnectionPoolConfig {
                        max_idle_per_host: 100,
                        min_idle_per_host: 0,
                        max_connection_lifetime: None,
                        idle_timeout: Duration::from_secs(90),
                        prewarm: false,
                        metrics_enabled: true,
                    },
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            tls: Some(TlsConfig {
                auto: false,
                acme: None,
                certificates: vec![],
                passthrough: None,
                min_version: "1.2".to_string(),
                session_cache: SessionCacheConfig::default(),
                mtls: None,
                ocsp_stapling: OcspStaplingConfig::default(),
            }),
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        http: Some("info".to_string()),
                        tls: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            cache: Some(CacheConfig {
                enabled: true,
                default_ttl: Duration::from_secs(300),
                max_size: 10000,
                cleanup_interval: Duration::from_secs(60),
                cache_only_success: true,
                methods: vec!["GET".to_string(), "HEAD".to_string()],
                key_headers: vec![],
            }),
            rate_limit: Some(RateLimitConfig {
                enabled: true,
                algorithm: RateLimitAlgorithm::TokenBucket,
                capacity: 1000,
                refill_rate: 100.0,
                window: Duration::from_secs(60),
                key_type: RateLimitKeyType::Ip,
                key_header: None,
                cleanup_interval: Duration::from_secs(60),
            }),
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for gRPC services
    ///
    /// Optimized for:
    /// - HTTP/2 multiplexing
    /// - Streaming support
    /// - Load balancing
    /// - Health checking
    pub fn grpc() -> Config {
        Config {
            server: ServerConfig {
                bind: vec![],
                tls_bind: vec!["0.0.0.0:50051".to_string()], // gRPC typically uses TLS
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http2], // gRPC uses HTTP/2
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 10000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(60), // gRPC can be long-running
                    idle_timeout: Duration::from_secs(300),
                    connection_pool: ConnectionPoolConfig {
                        max_idle_per_host: 50,
                        min_idle_per_host: 5,
                        max_connection_lifetime: Some(Duration::from_secs(1800)),
                        idle_timeout: Duration::from_secs(300),
                        prewarm: true,
                        metrics_enabled: true,
                    },
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            tls: Some(TlsConfig {
                auto: false,
                acme: None,
                certificates: vec![],
                passthrough: None,
                min_version: "1.2".to_string(),
                session_cache: SessionCacheConfig::default(),
                mtls: None, // mTLS often used with gRPC
                ocsp_stapling: OcspStaplingConfig::default(),
            }),
            grpc: crate::grpc::GrpcConfig {
                enabled: true,
                max_message_size: 4 * 1024 * 1024, // 4 MB
                timeout_seconds: 60,
                health_check_enabled: true,
                health_check_interval: 10, // 10 seconds
                health_check_service: None,
                reflection_enabled: false,
                load_balancing: crate::grpc::GrpcLoadBalancing {
                    policy: crate::grpc::GrpcLoadBalancingPolicy::RoundRobin,
                    enable_affinity: false,
                    affinity_key: None,
                },
            },
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        grpc: Some("debug".to_string()),
                        http: Some("info".to_string()),
                        tls: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for WebSocket connections
    ///
    /// Optimized for:
    /// - Long-lived connections
    /// - Bidirectional communication
    /// - Low latency
    pub fn websocket() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:8080".to_string()],
                tls_bind: vec!["0.0.0.0:8443".to_string()],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1, Protocol::Http2],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 100000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(0), // No timeout for WebSocket
                    idle_timeout: Duration::from_secs(0), // No idle timeout
                    connection_pool: ConnectionPoolConfig::default(),
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            websocket: crate::websocket::WebSocketConfig {
                enabled: true,
                max_message_size: 67108864, // 64 MB
                ping_interval: 30, // 30 seconds
                timeout: 300, // 5 minutes
                sticky_sessions: true,
                session_cookie_name: "ws_session".to_string(),
                session_timeout: 3600, // 1 hour
                track_connections: true,
                idle_timeout: 0, // No idle timeout for WebSocket
            },
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        websocket: Some("debug".to_string()),
                        http: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for GraphQL gateway
    ///
    /// Optimized for:
    /// - Query parsing and validation
    /// - Schema stitching
    /// - Batching
    /// - Caching
    pub fn graphql() -> Config {
        let mut config = Self::http_api();
        config.graphql = Some(crate::gateway::graphql::GraphQLConfig {
            enable_stitching: false,
            enable_cache: true,
            cache_ttl: Duration::from_secs(60),
            enable_batching: true,
            max_batch_size: 10,
            introspection_enabled: false, // Disable in production
            backends: vec![],
        });
        config.observability.logging.protocols = Some(ProtocolLogLevels {
            graphql: Some("debug".to_string()),
            http: Some("info".to_string()),
            ..Default::default()
        });
        config.cache.as_mut().unwrap().default_ttl = Duration::from_secs(60); // Shorter TTL for GraphQL
        config
    }

    /// Get optimized configuration for static file serving
    ///
    /// Optimized for:
    /// - High throughput
    /// - Aggressive caching
    /// - Compression
    /// - Security headers
    pub fn static_files() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:8080".to_string()],
                tls_bind: vec!["0.0.0.0:8443".to_string()],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1, Protocol::Http2, Protocol::Http3],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 100000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(30),
                    idle_timeout: Duration::from_secs(60),
                    connection_pool: ConnectionPoolConfig::default(),
                },
                shutdown_timeout: Duration::from_secs(10),
                http3: Http3Config {
                    enabled: true,
                    port: 8443,
                    bind: "0.0.0.0".to_string(),
                    enable_0rtt: false,
                    max_concurrent_streams: 100,
                    initial_window_size: 10485760,
                    max_datagram_size: 1350,
                    idle_timeout_secs: 30,
                },
            },
            cache: Some(CacheConfig {
                enabled: true,
                default_ttl: Duration::from_secs(86400), // 1 day
                max_size: 100000,
                cleanup_interval: Duration::from_secs(300),
                cache_only_success: true,
                methods: vec!["GET".to_string(), "HEAD".to_string()],
                key_headers: vec!["Accept-Encoding".to_string()],
            }),
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "warn".to_string(), // Less verbose for static files
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        http: Some("warn".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for PHP-FPM applications
    ///
    /// Optimized for:
    /// - FastCGI protocol
    /// - File upload handling
    /// - Script execution timeouts
    /// - Security
    pub fn php_fpm() -> Config {
        Config {
            server: ServerConfig {
                bind: vec!["0.0.0.0:8080".to_string()],
                tls_bind: vec!["0.0.0.0:8443".to_string()],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1, Protocol::Http2],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 10000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(90), // PHP scripts can be slow
                    idle_timeout: Duration::from_secs(60),
                    connection_pool: ConnectionPoolConfig::default(),
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config::default(),
            },
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        http: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            tls: None,
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    /// Get optimized configuration for HTTP/3 edge server
    ///
    /// Optimized for:
    /// - QUIC protocol
    /// - 0-RTT
    /// - Connection migration
    /// - Modern web standards
    pub fn http3_edge() -> Config {
        Config {
            server: ServerConfig {
                bind: vec![],
                tls_bind: vec!["0.0.0.0:443".to_string()],
                workers: "auto".to_string(),
                protocols: vec![Protocol::Http1, Protocol::Http2, Protocol::Http3],
                performance: PerformanceConfig {
                    read_buffer_size: 8192,
                    write_buffer_size: 8192,
                    max_connections: 100000,
                    max_requests_per_connection: 1000,
                    connect_timeout: Duration::from_secs(5),
                    request_timeout: Duration::from_secs(30),
                    idle_timeout: Duration::from_secs(90),
                    connection_pool: ConnectionPoolConfig::default(),
                },
                shutdown_timeout: Duration::from_secs(30),
                http3: Http3Config {
                    enabled: true,
                    port: 443,
                    bind: "0.0.0.0".to_string(),
                    enable_0rtt: true, // Enable 0-RTT for best performance
                    max_concurrent_streams: 100,
                    initial_window_size: 10485760, // 10 MB
                    max_datagram_size: 1350,
                    idle_timeout_secs: 30,
                },
            },
            tls: Some(TlsConfig {
                auto: true,
                acme: Some(AcmeConfig {
                    provider: "letsencrypt".to_string(),
                    email: "admin@example.com".to_string(),
                    directory_url: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
                    challenge_type: "http-01".to_string(),
                    storage: StorageConfig::default(),
                    renewal_days: 30,
                    renew_check_interval: Duration::from_secs(3600),
                }),
                certificates: vec![],
                passthrough: None,
                min_version: "1.3".to_string(), // TLS 1.3 required for HTTP/3
                session_cache: SessionCacheConfig::default(),
                mtls: None,
                ocsp_stapling: OcspStaplingConfig {
                    enabled: true,
                    ..Default::default()
                },
            }),
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    output: "stdout".to_string(),
                    protocols: Some(ProtocolLogLevels {
                        http: Some("info".to_string()),
                        quic: Some("debug".to_string()),
                        tls: Some("info".to_string()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
            cache: Some(CacheConfig {
                enabled: true,
                default_ttl: Duration::from_secs(300),
                max_size: 10000,
                cleanup_interval: Duration::from_secs(60),
                cache_only_success: true,
                methods: vec!["GET".to_string(), "HEAD".to_string()],
                key_headers: vec![],
            }),
            upstreams: vec![],
            routes: vec![],
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mysql_defaults() {
        let config = ProtocolDefaults::mysql();
        assert_eq!(config.server.bind, vec!["0.0.0.0:3306"]);
        assert!(config.server.performance.connection_pool.prewarm);
        assert_eq!(config.server.performance.connection_pool.min_idle_per_host, 10);
    }

    #[test]
    fn test_redis_defaults() {
        let config = ProtocolDefaults::redis();
        assert_eq!(config.server.bind, vec!["0.0.0.0:6379"]);
        assert_eq!(config.server.performance.max_connections, 50000);
    }

    #[test]
    fn test_http_api_defaults() {
        let config = ProtocolDefaults::http_api();
        assert!(config.cache.is_some());
        assert!(config.rate_limit.is_some());
        assert_eq!(config.server.protocols, vec![Protocol::Http1, Protocol::Http2]);
    }

    #[test]
    fn test_grpc_defaults() {
        let config = ProtocolDefaults::grpc();
        assert!(config.grpc.enabled);
        assert!(config.tls.is_some());
        assert!(config.server.protocols.contains(&Protocol::Http2));
    }

    #[test]
    fn test_http3_edge_defaults() {
        let config = ProtocolDefaults::http3_edge();
        assert!(config.server.http3.enabled);
        assert!(config.server.http3.enable_0rtt);
        assert!(config.tls.is_some());
        assert!(config.tls.unwrap().auto);
    }

    #[test]
    fn test_websocket_defaults() {
        let config = ProtocolDefaults::websocket();
        assert!(config.websocket.enabled);
        assert_eq!(config.server.performance.request_timeout, Duration::from_secs(0));
    }

    #[test]
    fn test_graphql_defaults() {
        let config = ProtocolDefaults::graphql();
        assert!(config.graphql.is_some());
        let graphql = config.graphql.unwrap();
        assert!(!graphql.introspection_enabled); // Should be disabled in production
    }

    #[test]
    fn test_static_files_defaults() {
        let config = ProtocolDefaults::static_files();
        assert!(config.cache.is_some());
        let cache = config.cache.unwrap();
        assert_eq!(cache.default_ttl, Duration::from_secs(86400)); // 1 day
        assert!(config.server.http3.enabled);
    }

    #[test]
    fn test_php_fpm_defaults() {
        let config = ProtocolDefaults::php_fpm();
        assert_eq!(config.server.performance.request_timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_postgresql_defaults() {
        let config = ProtocolDefaults::postgresql();
        assert_eq!(config.server.bind, vec!["0.0.0.0:5432"]);
        assert_eq!(config.server.performance.idle_timeout, Duration::from_secs(600));
    }
}
