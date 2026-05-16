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
    /// Simple log level (for backward compatibility with "log debug" directive)
    pub log_level: Option<LogLevel>,
    /// Full logging configuration (when using "logging { ... }" directive)
    pub logging_config: Option<LoggingConfigDsl>,
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
            logging_config: None,
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
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Enhanced logging configuration from DSL
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingConfigDsl {
    pub format: Option<LogFormat>,
    pub level: Option<LogLevel>,
    pub output: Option<String>,
    pub protocols: Option<ProtocolLogLevelsDsl>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormat::Json => write!(f, "json"),
            LogFormat::Pretty => write!(f, "pretty"),
        }
    }
}

/// Protocol-specific log levels from DSL
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtocolLogLevelsDsl {
    pub tcp: Option<LogLevel>,
    pub tls: Option<LogLevel>,
    pub quic: Option<LogLevel>,
    pub grpc: Option<LogLevel>,
    pub graphql: Option<LogLevel>,
    pub http: Option<LogLevel>,
    pub websocket: Option<LogLevel>,
    pub cache: Option<LogLevel>,
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
    Tcp { port: u16, protocol: TcpProtocol },
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
    HeaderAdd { name: String, value: String },

    /// Remove header
    HeaderRemove { name: String },

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

    /// Security headers configuration
    SecurityHeaders(SecurityHeadersConfig),

    /// TLS client authentication (mTLS)
    TlsClientAuth(TlsClientAuthConfig),

    /// Authentication configuration
    Auth(AuthConfig),

    /// Retry policy configuration
    Retry(RetryConfig),

    /// Service discovery configuration
    Discovery(DiscoveryConfig),

    /// Geographic routing configuration
    GeoRouting(GeoRoutingConfig),

    /// Firewall/IP ACL configuration
    Firewall(FirewallConfig),
}

/// Resource limits configuration
#[derive(Debug, Clone, PartialEq)]
pub struct LimitsConfig {
    pub max_file_size: Option<u64>,
    pub max_request_body: Option<usize>,
    pub max_upload_size: Option<usize>,
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
    LeastResponseTime,
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
            LoadBalancingAlgorithm::LeastResponseTime => write!(f, "least_response_time"),
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
            methods: Some(vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
            ]),
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

/// Security Headers configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityHeadersConfig {
    pub enabled: bool,
    pub x_frame_options: Option<String>,
    pub x_content_type_options: Option<String>,
    pub x_xss_protection: Option<String>,
    pub strict_transport_security: Option<String>,
    pub content_security_policy: Option<String>,
    pub referrer_policy: Option<String>,
    pub permissions_policy: Option<String>,
    pub custom_headers: Vec<(String, String)>,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            x_frame_options: Some("DENY".to_string()),
            x_content_type_options: Some("nosniff".to_string()),
            x_xss_protection: Some("1; mode=block".to_string()),
            strict_transport_security: Some("max-age=31536000".to_string()),
            content_security_policy: Some("default-src 'self'".to_string()),
            referrer_policy: Some("strict-origin-when-cross-origin".to_string()),
            permissions_policy: None,
            custom_headers: Vec::new(),
        }
    }
}

/// TLS Client Authentication (mTLS) configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsClientAuthConfig {
    pub enabled: bool,
    pub required: bool,
    pub ca_cert_file: Option<String>,
    pub verify_depth: Option<u8>,
    pub crl_file: Option<String>,
}

impl Default for TlsClientAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            required: false,
            ca_cert_file: None,
            verify_depth: Some(3),
            crl_file: None,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, PartialEq)]
pub struct AuthConfig {
    pub auth_type: AuthType,
    pub enabled: bool,
    pub jwt: Option<JwtAuthConfig>,
    pub oauth2: Option<OAuth2AuthConfig>,
    pub basic: Option<BasicAuthConfig>,
    pub api_key: Option<ApiKeyAuthConfig>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            auth_type: AuthType::Jwt,
            enabled: false,
            jwt: None,
            oauth2: None,
            basic: None,
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    Jwt,
    OAuth2,
    Basic,
    ApiKey,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::Jwt => write!(f, "jwt"),
            AuthType::OAuth2 => write!(f, "oauth2"),
            AuthType::Basic => write!(f, "basic"),
            AuthType::ApiKey => write!(f, "api_key"),
        }
    }
}

/// JWT Authentication configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwtAuthConfig {
    pub secret: String,
    pub algorithm: JwtAlgorithm,
    pub header: String,
    pub prefix: String,
    pub claims_required: Vec<String>,
    pub validate_exp: bool,
    pub validate_nbf: bool,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

impl Default for JwtAuthConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
            algorithm: JwtAlgorithm::HS256,
            header: "Authorization".to_string(),
            prefix: "Bearer ".to_string(),
            claims_required: vec!["sub".to_string(), "exp".to_string()],
            validate_exp: true,
            validate_nbf: true,
            issuer: None,
            audience: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtAlgorithm {
    HS256,
    HS384,
    HS512,
    RS256,
    RS384,
    RS512,
    ES256,
    ES384,
    ES512,
}

impl std::fmt::Display for JwtAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtAlgorithm::HS256 => write!(f, "HS256"),
            JwtAlgorithm::HS384 => write!(f, "HS384"),
            JwtAlgorithm::HS512 => write!(f, "HS512"),
            JwtAlgorithm::RS256 => write!(f, "RS256"),
            JwtAlgorithm::RS384 => write!(f, "RS384"),
            JwtAlgorithm::RS512 => write!(f, "RS512"),
            JwtAlgorithm::ES256 => write!(f, "ES256"),
            JwtAlgorithm::ES384 => write!(f, "ES384"),
            JwtAlgorithm::ES512 => write!(f, "ES512"),
        }
    }
}

/// OAuth2 Authentication configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuth2AuthConfig {
    pub provider: OAuth2Provider,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub callback_url: String,
    pub authorize_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuth2Provider {
    Google,
    GitHub,
    Facebook,
    Microsoft,
    Okta,
    Auth0,
    Custom,
}

impl std::fmt::Display for OAuth2Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuth2Provider::Google => write!(f, "google"),
            OAuth2Provider::GitHub => write!(f, "github"),
            OAuth2Provider::Facebook => write!(f, "facebook"),
            OAuth2Provider::Microsoft => write!(f, "microsoft"),
            OAuth2Provider::Okta => write!(f, "okta"),
            OAuth2Provider::Auth0 => write!(f, "auth0"),
            OAuth2Provider::Custom => write!(f, "custom"),
        }
    }
}

/// Basic Authentication configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicAuthConfig {
    pub realm: String,
    pub htpasswd_file: Option<String>,
    pub users: Vec<(String, String)>, // (username, password_hash)
}

/// API Key Authentication configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyAuthConfig {
    pub header: Option<String>,
    pub query_param: Option<String>,
    pub keys: Vec<String>,
    pub keys_file: Option<String>,
}

/// Retry Policy configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RetryConfig {
    pub enabled: bool,
    pub attempts: u32,
    pub backoff: BackoffStrategy,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: bool,
    pub retry_on: Vec<RetryCondition>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            attempts: 3,
            backoff: BackoffStrategy::Exponential,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
            jitter: true,
            retry_on: vec![RetryCondition::Status5xx, RetryCondition::ConnectionError],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    Exponential,
    Linear,
    Constant,
    Fibonacci,
}

impl std::fmt::Display for BackoffStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackoffStrategy::Exponential => write!(f, "exponential"),
            BackoffStrategy::Linear => write!(f, "linear"),
            BackoffStrategy::Constant => write!(f, "constant"),
            BackoffStrategy::Fibonacci => write!(f, "fibonacci"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryCondition {
    Status5xx,
    ConnectionError,
    Timeout,
    StatusCode(u16),
}

/// Service Discovery configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    pub provider: DiscoveryProvider,
    pub address: String,
    pub service_name: String,
    pub refresh_interval: Duration,
    pub health_check_enabled: bool,
    pub tags: Vec<String>,
    pub consul: Option<ConsulDiscoveryConfig>,
    pub etcd: Option<EtcdDiscoveryConfig>,
    pub kubernetes: Option<KubernetesDiscoveryConfig>,
    pub dns: Option<DnsDiscoveryConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryProvider {
    Consul,
    Etcd,
    Kubernetes,
    Dns,
    Static,
}

impl std::fmt::Display for DiscoveryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryProvider::Consul => write!(f, "consul"),
            DiscoveryProvider::Etcd => write!(f, "etcd"),
            DiscoveryProvider::Kubernetes => write!(f, "kubernetes"),
            DiscoveryProvider::Dns => write!(f, "dns"),
            DiscoveryProvider::Static => write!(f, "static"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsulDiscoveryConfig {
    pub datacenter: Option<String>,
    pub token: Option<String>,
    pub namespace: Option<String>,
    pub passing_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtcdDiscoveryConfig {
    pub prefix: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesDiscoveryConfig {
    pub namespace: String,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
    pub port_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsDiscoveryConfig {
    pub resolver: Option<String>,
    pub record_type: DnsRecordType,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordType {
    A,
    AAAA,
    SRV,
}

impl std::fmt::Display for DnsRecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DnsRecordType::A => write!(f, "A"),
            DnsRecordType::AAAA => write!(f, "AAAA"),
            DnsRecordType::SRV => write!(f, "SRV"),
        }
    }
}

/// Geographic Routing configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoRoutingConfig {
    pub enabled: bool,
    pub database_path: Option<String>,
    pub database_type: GeoDatabaseType,
    pub fallback_strategy: GeoFallbackStrategy,
    pub regions: Vec<GeoRegion>,
    pub default_backends: Vec<Backend>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeoDatabaseType {
    MaxMind,
    Ip2Location,
    DbIp,
    GeoIp2,
}

impl std::fmt::Display for GeoDatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeoDatabaseType::MaxMind => write!(f, "maxmind"),
            GeoDatabaseType::Ip2Location => write!(f, "ip2location"),
            GeoDatabaseType::DbIp => write!(f, "dbip"),
            GeoDatabaseType::GeoIp2 => write!(f, "geoip2"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeoFallbackStrategy {
    Closest,
    Random,
    RoundRobin,
}

impl std::fmt::Display for GeoFallbackStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeoFallbackStrategy::Closest => write!(f, "closest"),
            GeoFallbackStrategy::Random => write!(f, "random"),
            GeoFallbackStrategy::RoundRobin => write!(f, "round_robin"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoRegion {
    pub name: String,
    pub countries: Vec<String>,
    pub continents: Vec<String>,
    pub cities: Vec<String>,
    pub ip_ranges: Vec<String>,
    pub backends: Vec<Backend>,
    pub weight: Option<u32>,
}

/// Firewall / IP ACL configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallConfig {
    pub enabled: bool,
    pub mode: FirewallMode,
    pub allowlist: Vec<String>,
    pub blocklist: Vec<String>,
    pub max_connections_per_ip: Option<u32>,
    pub rate_limit_per_second: Option<u32>,
    pub rate_limit_per_minute: Option<u32>,
    pub geo_block: Vec<String>,
    pub geo_allow: Vec<String>,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: FirewallMode::Allow,
            allowlist: Vec::new(),
            blocklist: Vec::new(),
            max_connections_per_ip: None,
            rate_limit_per_second: None,
            rate_limit_per_minute: None,
            geo_block: Vec::new(),
            geo_allow: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallMode {
    Allow,
    Deny,
}

impl std::fmt::Display for FirewallMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirewallMode::Allow => write!(f, "allow"),
            FirewallMode::Deny => write!(f, "deny"),
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
        assert_eq!(
            LoadBalancingAlgorithm::RoundRobin.to_string(),
            "round_robin"
        );
        assert_eq!(
            LoadBalancingAlgorithm::LeastConnections.to_string(),
            "least_conn"
        );
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
