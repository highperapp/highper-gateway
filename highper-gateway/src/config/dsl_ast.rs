//! DSL Abstract Syntax Tree
//!
//! Represents the parsed structure of a DSL configuration file.

use std::time::Duration;

/// Top-level configuration
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub global: GlobalConfig,
    pub sites: Vec<Site>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            sites: Vec::new(),
        }
    }
}

/// Global configuration (applies to all sites)
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalConfig {
    pub log_level: Option<LogLevel>,
    pub admin_address: Option<String>,
    pub metrics_enabled: bool,
    pub metrics_prometheus: bool,
    pub metrics_port: Option<u16>,
    pub buffer_pool: Option<BufferPoolConfig>,
    pub backpressure: Option<BackpressureConfig>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: Some(LogLevel::Info),
            admin_address: None,
            metrics_enabled: true,
            metrics_prometheus: true,
            metrics_port: Some(9090),
            buffer_pool: None,
            backpressure: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Site definition (domain/address + routes/directives)
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    pub address: SiteAddress,
    pub routes: Vec<Route>,
    pub directives: Vec<Directive>,
}

/// Site address (HTTP/HTTPS domain or TCP port)
#[derive(Debug, Clone, PartialEq)]
pub enum SiteAddress {
    /// HTTP/HTTPS site: scheme, domain, port, path
    Http {
        scheme: Scheme,
        domain: String,
        port: Option<u16>,
        base_path: Option<String>,
    },
    /// TCP-only site: port, protocol
    Tcp {
        port: u16,
        protocol: TcpProtocol,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    Grpc,
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scheme::Http => write!(f, "http"),
            Scheme::Https => write!(f, "https"),
            Scheme::Grpc => write!(f, "grpc"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpProtocol {
    Mysql,
    Postgres,
    Redis,
    Generic,
}

impl std::fmt::Display for TcpProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpProtocol::Mysql => write!(f, "mysql"),
            TcpProtocol::Postgres => write!(f, "postgres"),
            TcpProtocol::Redis => write!(f, "redis"),
            TcpProtocol::Generic => write!(f, "tcp"),
        }
    }
}

/// Path-based route
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub path: String,
    pub directives: Vec<Directive>,
}

/// Configuration directive
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// Proxy to backends
    Proxy(Vec<Backend>),

    /// Load balancing algorithm
    LoadBalancing(LoadBalancingAlgorithm),

    /// Connection pool settings
    Pool(PoolConfig),

    /// Health check settings
    HealthCheck(HealthCheckConfig),

    /// TLS configuration
    Tls(TlsConfig),

    /// TLS protocols (versions)
    TlsProtocols(Vec<TlsVersion>),

    /// CORS settings
    Cors(CorsConfig),

    /// Enable WebSocket support
    WebSocket,

    /// WebSocket configuration
    WebSocketConfig(WebSocketConfig),

    /// Enable gRPC support
    Grpc,

    /// gRPC configuration
    GrpcConfig(GrpcConfig),

    /// HTTP/2 configuration
    Http2Config(Http2Config),

    /// HTTP/3 configuration
    Http3Config(Http3Config),

    /// QUIC configuration
    QuicConfig(QuicConfig),

    /// Compression settings
    Compress(Vec<CompressionAlgorithm>),

    /// Compression configuration with level
    CompressConfig(CompressionConfig),

    /// Rate limiting
    RateLimit {
        rate: u64,
        burst: Option<u64>,
        per: Option<Duration>,
        per_ip: bool,
    },

    /// Request/response timeout
    Timeout(Duration),

    /// Request timeout
    RequestTimeout(Duration),

    /// Header manipulation
    Header {
        direction: HeaderDirection,
        name: String,
        value: String,
    },

    /// Add header
    HeaderAdd {
        name: String,
        value: String,
    },

    /// Remove header
    HeaderRemove {
        name: String,
    },

    /// Header passthrough
    HeaderPassthrough(Vec<String>),

    /// Circuit breaker configuration
    CircuitBreaker(CircuitBreakerConfig),

    /// TLS passthrough
    TlsPassthrough {
        server_name: String,
        backend: Backend,
    },

    /// HTTP keepalive duration
    Keepalive(Duration),

    /// Maximum concurrent connections
    MaxConnections(u64),

    /// Connection timeout
    ConnectTimeout(Duration),

    /// Idle connection timeout
    IdleTimeout(Duration),

    /// WebSocket timeout
    WebSocketTimeout(Duration),

    /// gRPC timeout
    GrpcTimeout(Duration),

    /// Buffer pool configuration
    BufferPool(BufferPoolConfig),

    /// Backpressure configuration
    Backpressure(BackpressureConfig),

    /// Cache configuration
    Cache(CacheConfig),

    /// WAF (Web Application Firewall) configuration
    Waf(WafConfig),

    /// GraphQL configuration
    GraphQL(GraphQLConfig),

    /// PHP-FPM configuration
    PhpFpm(PhpFpmConfig),

    /// Static file serving
    StaticFiles,

    /// Document root for static files
    Root(String),

    /// Index files
    Index(Vec<String>),

    /// Try files pattern
    TryFiles(Vec<String>),

    /// Custom error page (status code, file path)
    ErrorPage(u16, String),

    /// Enable directory listing
    DirectoryListing(bool),

    /// Resource limits configuration
    Limits(LimitsConfig),
}

/// Resource limits configuration
#[derive(Debug, Clone, PartialEq)]
pub struct LimitsConfig {
    pub max_file_size: Option<u64>,
    pub max_request_body: Option<usize>,
    pub max_path_depth: Option<usize>,
    pub max_connections_per_ip: Option<usize>,
    pub max_requests_per_second: Option<u32>,
}

/// Backend server
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backend {
    pub address: String,
    pub port: Option<u16>,
    pub weight: Option<u32>,
}

impl Backend {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            port: None,
            weight: None,
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Get full address (address:port)
    pub fn full_address(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{}", self.address, port),
            None => self.address.clone(),
        }
    }
}

/// Load balancing algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConnections,
    IpHash,
    Random,
    Weighted,
    ConsistentHash,
}

impl std::fmt::Display for LoadBalancingAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadBalancingAlgorithm::RoundRobin => write!(f, "round_robin"),
            LoadBalancingAlgorithm::LeastConnections => write!(f, "least_conn"),
            LoadBalancingAlgorithm::IpHash => write!(f, "ip_hash"),
            LoadBalancingAlgorithm::Random => write!(f, "random"),
            LoadBalancingAlgorithm::Weighted => write!(f, "weighted"),
            LoadBalancingAlgorithm::ConsistentHash => write!(f, "consistent_hash"),
        }
    }
}

/// Connection pool configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolConfig {
    pub max_size: Option<usize>,
    pub min_idle: Option<usize>,
    pub max_idle: Option<usize>,
    pub max_lifetime: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub http2_multiplexing: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: Some(100),
            min_idle: Some(10),
            max_idle: Some(100),
            max_lifetime: Some(Duration::from_secs(3600)), // 1 hour
            idle_timeout: Some(Duration::from_secs(300)),  // 5 minutes
            http2_multiplexing: false,
        }
    }
}

/// Health check configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheckConfig {
    pub interval: Option<Duration>,
    pub timeout: Option<Duration>,
    pub path: Option<String>,
    pub healthy_threshold: Option<u32>,
    pub unhealthy_threshold: Option<u32>,
    pub grpc: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Some(Duration::from_secs(10)),
            timeout: Some(Duration::from_secs(5)),
            path: Some("/health".to_string()),
            healthy_threshold: Some(2),
            unhealthy_threshold: Some(3),
            grpc: false,
        }
    }
}

/// TLS configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsConfig {
    /// Auto TLS with ACME/Let's Encrypt
    Auto { email: Option<String> },

    /// Self-signed certificate (for development)
    Internal,

    /// Manual certificate files
    Manual { cert_file: String, key_file: String },
}

/// CORS configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsConfig {
    pub origins: Option<Vec<String>>,
    pub methods: Option<Vec<String>>,
    pub headers: Option<Vec<String>>,
    pub credentials: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: Some(vec!["*".to_string()]),
            methods: Some(vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()]),
            headers: Some(vec!["*".to_string()]),
            credentials: false,
        }
    }
}

/// Compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    Gzip,
    Brotli,
    Deflate,
    Zstd,
}

impl std::fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionAlgorithm::Gzip => write!(f, "gzip"),
            CompressionAlgorithm::Brotli => write!(f, "br"),
            CompressionAlgorithm::Deflate => write!(f, "deflate"),
            CompressionAlgorithm::Zstd => write!(f, "zstd"),
        }
    }
}

/// Header manipulation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderDirection {
    /// Request headers (to upstream)
    Up,
    /// Response headers (to client)
    Down,
}

/// Buffer pool configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferPoolConfig {
    pub enabled: bool,
    pub size: Option<usize>,
    pub pool_size: Option<usize>,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size: Some(16384),
            pool_size: Some(16777216),
        }
    }
}

/// Backpressure configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackpressureConfig {
    pub enabled: bool,
    pub max_connections: Option<u64>,
    pub memory_limit: Option<usize>, // in bytes
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_connections: Some(3000000),
            memory_limit: Some(49152 * 1024 * 1024), // 49152 MB
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: Option<Duration>,
    pub max_size: Option<usize>,
    pub cleanup_interval: Option<Duration>,
    pub only_success: bool,
    pub methods: Vec<String>,
    pub key_headers: Vec<String>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl: Some(Duration::from_secs(300)), // 5 minutes
            max_size: Some(10000),
            cleanup_interval: Some(Duration::from_secs(60)),
            only_success: true,
            methods: vec!["GET".to_string(), "HEAD".to_string()],
            key_headers: vec![],
        }
    }
}

/// WAF (Web Application Firewall) configuration
#[derive(Debug, Clone, PartialEq)]
pub struct WafConfig {
    pub enabled: bool,
    pub mode: WafMode,
    pub block_mode: bool, // true = block, false = log only
    pub max_body_size: Option<usize>,
    pub modsecurity: Option<ModSecurityConfig>,
    pub aws_waf: Option<AwsWafConfig>,
    pub coraza: Option<CorazaConfig>,
    pub custom_rules: Vec<WafRule>,
}

impl Default for WafConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: WafMode::Custom,
            block_mode: true,
            max_body_size: Some(10485760), // 10MB
            modsecurity: None,
            aws_waf: None,
            coraza: None,
            custom_rules: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafMode {
    Custom,
    ModSecurity,
    Coraza,
    Aws,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSecurityConfig {
    pub rules_file: Option<String>,
    pub paranoia_level: Option<u8>,
    pub audit_log: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsWafConfig {
    pub web_acl_id: String,
    pub region: String,
    pub api_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorazaConfig {
    pub rules_dir: Option<String>,
    pub audit_log: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WafRule {
    pub rule_type: WafRuleType,
    pub action: WafRuleAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafRuleType {
    SqlInjection,
    Xss,
    PathTraversal,
    RateLimit,
    UserAgent,
    Method,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafRuleAction {
    Enabled,
    Disabled,
    Block,
    Log,
}

/// GraphQL configuration
#[derive(Debug, Clone, PartialEq)]
pub struct GraphQLConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub introspection_enabled: bool,
    pub enable_cache: bool,
    pub cache_ttl: Option<Duration>,
    pub enable_batching: bool,
    pub max_batch_size: Option<usize>,
    pub backends: Vec<GraphQLBackend>,
}

impl Default for GraphQLConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: Some("/graphql".to_string()),
            introspection_enabled: true,
            enable_cache: false,
            cache_ttl: None,
            enable_batching: false,
            max_batch_size: None,
            backends: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQLBackend {
    pub name: String,
    pub url: String,
    pub namespace: Option<String>,
}

/// PHP-FPM configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhpFpmConfig {
    pub enabled: bool,
    pub socket: Option<String>,
    pub pool_size: Option<usize>,
    pub connect_timeout: Option<Duration>,
    pub read_timeout: Option<Duration>,
    pub write_timeout: Option<Duration>,
    pub keepalive_timeout: Option<Duration>,
    pub script_extensions: Vec<String>,
}

impl Default for PhpFpmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            socket: None,
            pool_size: Some(50),
            connect_timeout: Some(Duration::from_secs(5)),
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(60)),
            keepalive_timeout: Some(Duration::from_secs(90)),
            script_extensions: vec![".php".to_string()],
        }
    }
}

/// TLS protocol versions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    TLSv1_2,
    TLSv1_3,
}

impl std::fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsVersion::TLSv1_2 => write!(f, "TLSv1.2"),
            TlsVersion::TLSv1_3 => write!(f, "TLSv1.3"),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    pub threshold: Option<u32>,
    pub timeout: Option<Duration>,
    pub window: Option<Duration>,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            threshold: Some(10),
            timeout: Some(Duration::from_secs(30)),
            window: Some(Duration::from_secs(60)),
        }
    }
}

/// Compression configuration with level
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionConfig {
    pub algorithms: Vec<CompressionAlgorithm>,
    pub level: Option<u8>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithms: vec![CompressionAlgorithm::Gzip],
            level: Some(6),
        }
    }
}

/// WebSocket configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub max_message_size: Option<usize>,
    pub buffer_size: Option<usize>,
    pub compression: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_message_size: Some(65536),
            buffer_size: Some(8192),
            compression: false,
        }
    }
}

/// gRPC configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcConfig {
    pub enabled: bool,
    pub timeout: Option<Duration>,
    pub max_message_size: Option<usize>,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: Some(Duration::from_secs(60)),
            max_message_size: Some(4 * 1024 * 1024), // 4MB
        }
    }
}

/// HTTP/2 configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Config {
    pub enabled: bool,
    pub max_concurrent_streams: Option<u32>,
    pub initial_window_size: Option<u32>,
}

impl Default for Http2Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_streams: Some(128),
            initial_window_size: Some(65535),
        }
    }
}

/// HTTP/3 configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http3Config {
    pub enabled: bool,
    pub port: Option<u16>,
    pub max_streams: Option<u32>,
    pub initial_max_data: Option<u64>,
}

impl Default for Http3Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: Some(443),
            max_streams: Some(100),
            initial_max_data: Some(10485760), // 10MB
        }
    }
}

/// QUIC configuration
#[derive(Debug, Clone, PartialEq)]
pub struct QuicConfig {
    pub ack_delay: Option<Duration>,
    pub max_idle_timeout: Option<Duration>,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            ack_delay: Some(Duration::from_millis(25)),
            max_idle_timeout: Some(Duration::from_secs(30)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_full_address() {
        let backend = Backend::new("example.com").with_port(8080);
        assert_eq!(backend.full_address(), "example.com:8080");

        let backend_no_port = Backend::new("example.com");
        assert_eq!(backend_no_port.full_address(), "example.com");
    }

    #[test]
    fn test_default_pool_config() {
        let pool = PoolConfig::default();
        assert_eq!(pool.max_size, Some(100));
        assert_eq!(pool.min_idle, Some(10));
        assert_eq!(pool.max_lifetime, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_default_health_check_config() {
        let health = HealthCheckConfig::default();
        assert_eq!(health.interval, Some(Duration::from_secs(10)));
        assert_eq!(health.timeout, Some(Duration::from_secs(5)));
        assert_eq!(health.path, Some("/health".to_string()));
    }

    #[test]
    fn test_default_cors_config() {
        let cors = CorsConfig::default();
        assert_eq!(cors.origins, Some(vec!["*".to_string()]));
        assert!(!cors.credentials);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_scheme_display() {
        assert_eq!(Scheme::Http.to_string(), "http");
        assert_eq!(Scheme::Https.to_string(), "https");
        assert_eq!(Scheme::Grpc.to_string(), "grpc");
    }

    #[test]
    fn test_tcp_protocol_display() {
        assert_eq!(TcpProtocol::Mysql.to_string(), "mysql");
        assert_eq!(TcpProtocol::Postgres.to_string(), "postgres");
        assert_eq!(TcpProtocol::Redis.to_string(), "redis");
        assert_eq!(TcpProtocol::Generic.to_string(), "tcp");
    }

    #[test]
    fn test_lb_algorithm_display() {
        assert_eq!(LoadBalancingAlgorithm::RoundRobin.to_string(), "round_robin");
        assert_eq!(LoadBalancingAlgorithm::LeastConnections.to_string(), "least_conn");
        assert_eq!(LoadBalancingAlgorithm::IpHash.to_string(), "ip_hash");
    }

    #[test]
    fn test_compression_algo_display() {
        assert_eq!(CompressionAlgorithm::Gzip.to_string(), "gzip");
        assert_eq!(CompressionAlgorithm::Brotli.to_string(), "br");
        assert_eq!(CompressionAlgorithm::Deflate.to_string(), "deflate");
        assert_eq!(CompressionAlgorithm::Zstd.to_string(), "zstd");
    }
}
