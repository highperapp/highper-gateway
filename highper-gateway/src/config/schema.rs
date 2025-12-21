//! Configuration schema definitions

use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::middleware::waf::{WafMode, CustomWafConfig, ModSecurityConfig};

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// TLS configuration
    #[serde(default)]
    pub tls: Option<TlsConfig>,

    /// Upstream backend configurations
    #[serde(default)]
    pub upstreams: Vec<UpstreamConfig>,

    /// Route configurations
    #[serde(default)]
    pub routes: Vec<RouteConfig>,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// WebSocket configuration
    #[serde(default)]
    pub websocket: crate::websocket::WebSocketConfig,

    /// gRPC configuration
    #[serde(default)]
    pub grpc: crate::grpc::GrpcConfig,

    /// Admin API configuration
    #[serde(default)]
    pub admin: Option<AdminConfig>,

    /// Cache configuration
    #[serde(default)]
    pub cache: Option<CacheConfig>,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// WAF (Web Application Firewall) configuration
    #[serde(default)]
    pub waf: Option<WafConfig>,

    /// GraphQL gateway configuration
    #[serde(default)]
    pub graphql: Option<crate::gateway::graphql::GraphQLConfig>,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// HTTP bind addresses
    pub bind: Vec<String>,

    /// HTTPS bind addresses (requires TLS configuration)
    #[serde(default)]
    pub tls_bind: Vec<String>,

    /// Number of worker threads ("auto" or number)
    #[serde(default = "default_workers")]
    pub workers: String,

    /// Supported protocols
    #[serde(default = "default_protocols")]
    pub protocols: Vec<Protocol>,

    /// Performance settings
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Graceful shutdown timeout
    #[serde(default = "default_shutdown_timeout", with = "humantime_serde")]
    pub shutdown_timeout: Duration,

    /// HTTP/3 configuration
    #[serde(default)]
    pub http3: Http3Config,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: vec!["0.0.0.0:8080".to_string()],
            tls_bind: vec![],
            workers: default_workers(),
            protocols: default_protocols(),
            performance: PerformanceConfig::default(),
            shutdown_timeout: default_shutdown_timeout(),
            http3: Http3Config::default(),
        }
    }
}

fn default_workers() -> String {
    "auto".to_string()
}

fn default_protocols() -> Vec<Protocol> {
    vec![Protocol::Http1, Protocol::Http2]
}

fn default_shutdown_timeout() -> Duration {
    Duration::from_secs(30)
}

/// Protocol support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Http1,
    Http2,
    Http3,
}

/// HTTP/3 configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Http3Config {
    /// Enable HTTP/3
    #[serde(default)]
    pub enabled: bool,

    /// HTTP/3 port (usually same as HTTPS port 443)
    #[serde(default = "default_http3_port")]
    pub port: u16,

    /// Bind address for HTTP/3 (UDP)
    #[serde(default = "default_http3_bind")]
    pub bind: String,

    /// Enable 0-RTT (early data)
    #[serde(default)]
    pub enable_0rtt: bool,

    /// Maximum number of concurrent streams per connection
    #[serde(default = "default_max_streams")]
    pub max_concurrent_streams: u64,

    /// Initial flow control window size (bytes)
    #[serde(default = "default_initial_window")]
    pub initial_window_size: u64,

    /// Maximum datagram size (bytes)
    #[serde(default = "default_max_datagram_size")]
    pub max_datagram_size: u64,

    /// Connection idle timeout (seconds)
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

impl Default for Http3Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_http3_port(),
            bind: default_http3_bind(),
            enable_0rtt: false,
            max_concurrent_streams: default_max_streams(),
            initial_window_size: default_initial_window(),
            max_datagram_size: default_max_datagram_size(),
            idle_timeout_secs: default_idle_timeout_secs(),
        }
    }
}

fn default_http3_port() -> u16 {
    443
}

fn default_http3_bind() -> String {
    "0.0.0.0".to_string()
}

fn default_max_streams() -> u64 {
    100
}

fn default_initial_window() -> u64 {
    10_485_760 // 10 MB
}

fn default_max_datagram_size() -> u64 {
    1350
}

fn default_idle_timeout_secs() -> u64 {
    30
}

/// Performance tuning configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// Read buffer size in bytes
    #[serde(default = "default_buffer_size")]
    pub read_buffer_size: usize,

    /// Write buffer size in bytes
    #[serde(default = "default_buffer_size")]
    pub write_buffer_size: usize,

    /// Maximum concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Maximum requests per connection
    #[serde(default = "default_max_requests_per_conn")]
    pub max_requests_per_connection: usize,

    /// Connect timeout
    #[serde(default = "default_connect_timeout", with = "humantime_serde")]
    pub connect_timeout: Duration,

    /// Request timeout
    #[serde(default = "default_request_timeout", with = "humantime_serde")]
    pub request_timeout: Duration,

    /// Idle timeout
    #[serde(default = "default_idle_timeout", with = "humantime_serde")]
    pub idle_timeout: Duration,

    /// Connection pool configuration
    #[serde(default)]
    pub connection_pool: ConnectionPoolConfig,
}

/// Connection pool configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionPoolConfig {
    /// Maximum idle connections per host
    #[serde(default = "default_pool_max_idle_per_host")]
    pub max_idle_per_host: usize,

    /// Minimum idle connections to maintain per host (pre-warming)
    #[serde(default = "default_pool_min_idle_per_host")]
    pub min_idle_per_host: usize,

    /// Maximum connection lifetime before forced recycling
    #[serde(default, with = "humantime_serde")]
    pub max_connection_lifetime: Option<Duration>,

    /// Idle timeout for pooled connections
    #[serde(default = "default_pool_idle_timeout", with = "humantime_serde")]
    pub idle_timeout: Duration,

    /// Enable connection pre-warming (maintain min_idle connections)
    #[serde(default = "default_pool_prewarm")]
    pub prewarm: bool,

    /// Enable connection pool metrics tracking
    #[serde(default = "default_pool_metrics_enabled")]
    pub metrics_enabled: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: default_pool_max_idle_per_host(),
            min_idle_per_host: default_pool_min_idle_per_host(),
            max_connection_lifetime: None,
            idle_timeout: default_pool_idle_timeout(),
            prewarm: default_pool_prewarm(),
            metrics_enabled: default_pool_metrics_enabled(),
        }
    }
}

fn default_pool_max_idle_per_host() -> usize {
    100
}

fn default_pool_min_idle_per_host() -> usize {
    0 // Disabled by default
}

fn default_pool_idle_timeout() -> Duration {
    Duration::from_secs(90)
}

fn default_pool_prewarm() -> bool {
    false
}

fn default_pool_metrics_enabled() -> bool {
    true
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: default_buffer_size(),
            write_buffer_size: default_buffer_size(),
            max_connections: default_max_connections(),
            max_requests_per_connection: default_max_requests_per_conn(),
            connect_timeout: default_connect_timeout(),
            request_timeout: default_request_timeout(),
            idle_timeout: default_idle_timeout(),
            connection_pool: ConnectionPoolConfig::default(),
        }
    }
}

fn default_buffer_size() -> usize {
    16384
}

fn default_max_connections() -> usize {
    100000
}

fn default_max_requests_per_conn() -> usize {
    1000
}

fn default_connect_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_request_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_idle_timeout() -> Duration {
    Duration::from_secs(90)
}

/// Upstream backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpstreamConfig {
    /// Upstream name
    pub name: String,

    /// Backend servers
    pub servers: Vec<ServerDef>,

    /// Load balancing configuration
    #[serde(default)]
    pub load_balancing: LoadBalancingConfig,

    /// Connection settings
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// Health check configuration
    #[serde(default)]
    pub health_check: HealthCheckConfig,
}

/// Backend server definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerDef {
    /// Server URL
    pub url: String,

    /// Server weight for weighted load balancing
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Maximum connections to this server
    #[serde(default = "default_max_conns")]
    pub max_conns: usize,

    /// Geographic location (for geographic load balancing)
    #[serde(default)]
    pub location: Option<GeoLocation>,

    /// Region identifier (e.g., "us-east-1", "eu-west-1")
    #[serde(default)]
    pub region: Option<String>,
}

/// Geographic location
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoLocation {
    /// Latitude
    pub lat: f64,

    /// Longitude
    pub lon: f64,
}

fn default_weight() -> u32 {
    1
}

fn default_max_conns() -> usize {
    100
}

/// Load balancing algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConn,
    Random,
    IpHash,
    ConsistentHash,
    PowerOfTwo,
    Geographic,
    Maglev,
}

impl Default for LoadBalancingAlgorithm {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// Load balancing configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoadBalancingConfig {
    /// Load balancing algorithm
    #[serde(default)]
    pub algorithm: LoadBalancingAlgorithm,

    /// GeoIP database provider
    #[serde(default)]
    pub geoip_provider: GeoIpProvider,

    /// GeoIP database path (required for geographic load balancing)
    #[serde(default)]
    pub geoip_db_path: Option<String>,
}

/// GeoIP database provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GeoIpProvider {
    /// MaxMind GeoLite2/GeoIP2 (MMDB format)
    MaxMind,

    /// IP2Location (BIN format)
    Ip2Location,
}

impl Default for GeoIpProvider {
    fn default() -> Self {
        Self::MaxMind
    }
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            algorithm: LoadBalancingAlgorithm::default(),
            geoip_provider: GeoIpProvider::default(),
            geoip_db_path: None,
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionConfig {
    /// Connection timeout
    #[serde(default = "default_connect_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Keep-alive duration
    #[serde(default = "default_keepalive", with = "humantime_serde")]
    pub keepalive: Duration,

    /// Connection pool size per backend
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,

    /// TCP nodelay
    #[serde(default = "default_tcp_nodelay")]
    pub tcp_nodelay: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            timeout: default_connect_timeout(),
            keepalive: default_keepalive(),
            pool_size: default_pool_size(),
            tcp_nodelay: default_tcp_nodelay(),
        }
    }
}

fn default_keepalive() -> Duration {
    Duration::from_secs(60)
}

fn default_pool_size() -> usize {
    50
}

fn default_tcp_nodelay() -> bool {
    true
}

/// Health check configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthCheckConfig {
    /// Active health check configuration
    #[serde(default)]
    pub active: ActiveHealthCheckConfig,

    /// Passive health monitoring
    #[serde(default)]
    pub passive: PassiveHealthCheckConfig,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            active: ActiveHealthCheckConfig::default(),
            passive: PassiveHealthCheckConfig::default(),
        }
    }
}

/// Active health check configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActiveHealthCheckConfig {
    /// Enable active health checks
    #[serde(default)]
    pub enabled: bool,

    /// Health check path
    #[serde(default = "default_health_path")]
    pub path: String,

    /// Check interval
    #[serde(default = "default_health_interval", with = "humantime_serde")]
    pub interval: Duration,

    /// Check timeout
    #[serde(default = "default_health_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Healthy threshold (consecutive successes)
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,

    /// Unhealthy threshold (consecutive failures)
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
}

impl Default for ActiveHealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: default_health_path(),
            interval: default_health_interval(),
            timeout: default_health_timeout(),
            healthy_threshold: default_healthy_threshold(),
            unhealthy_threshold: default_unhealthy_threshold(),
        }
    }
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_health_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_health_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_healthy_threshold() -> u32 {
    2
}

fn default_unhealthy_threshold() -> u32 {
    3
}

/// Passive health check configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PassiveHealthCheckConfig {
    /// Enable passive health monitoring
    #[serde(default)]
    pub enabled: bool,

    /// Monitoring period
    #[serde(default = "default_monitor_period", with = "humantime_serde")]
    pub monitor_period: Duration,

    /// Maximum failures before marking unhealthy
    #[serde(default = "default_max_failures")]
    pub max_failures: u32,
}

impl Default for PassiveHealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            monitor_period: default_monitor_period(),
            max_failures: default_max_failures(),
        }
    }
}

fn default_monitor_period() -> Duration {
    Duration::from_secs(10)
}

fn default_max_failures() -> u32 {
    5
}

/// Route configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteConfig {
    /// Route name
    pub name: String,

    /// Matching rules
    #[serde(rename = "match")]
    pub match_rules: MatchRules,

    /// Upstream name to route to
    pub upstream: String,

    /// Timeouts (override global)
    #[serde(default)]
    pub timeout: Option<TimeoutConfig>,

    /// Per-route mTLS requirement (overrides global)
    #[serde(default)]
    pub mtls: Option<RouteMtlsPolicy>,

    /// Per-route cache settings (overrides global)
    #[serde(default)]
    pub cache: Option<RouteCacheConfig>,

    /// Per-route rate limiting (overrides global)
    #[serde(default)]
    pub rate_limit: Option<RouteRateLimitConfig>,

    /// API aggregation configuration
    #[serde(default)]
    pub aggregation: Option<crate::gateway::aggregation::AggregationConfig>,

    /// PHP-FPM configuration
    #[serde(default)]
    pub php_fpm: Option<PhpFpmConfig>,

    /// Enable static file serving
    #[serde(default)]
    pub static_files: bool,

    /// Document root for static files and PHP scripts
    #[serde(default)]
    pub root: Option<String>,

    /// Index files to try (e.g., index.php, index.html)
    #[serde(default)]
    pub index: Vec<String>,

    /// Try files pattern (Nginx-style: $uri, $uri/, /index.php, =404)
    #[serde(default)]
    pub try_files: Vec<String>,
}

/// Route matching rules
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchRules {
    /// Match hosts (with wildcard support)
    #[serde(default)]
    pub hosts: Vec<String>,

    /// Match paths (with wildcard support)
    #[serde(default)]
    pub paths: Vec<String>,

    /// Match HTTP methods
    #[serde(default)]
    pub methods: Vec<String>,
}

/// Timeout configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeoutConfig {
    /// Connect timeout
    #[serde(default, with = "humantime_serde")]
    pub connect: Option<Duration>,

    /// Request timeout
    #[serde(default, with = "humantime_serde")]
    pub request: Option<Duration>,

    /// Idle timeout
    #[serde(default, with = "humantime_serde")]
    pub idle: Option<Duration>,
}

/// Observability configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObservabilityConfig {
    /// Metrics configuration
    #[serde(default)]
    pub metrics: MetricsConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Tracing configuration
    #[serde(default)]
    pub tracing: TracingConfig,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            logging: LoggingConfig::default(),
            tracing: TracingConfig::default(),
        }
    }
}

/// Metrics configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    /// Enable metrics
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics bind address
    #[serde(default = "default_metrics_bind")]
    pub bind: String,

    /// Metrics endpoint
    #[serde(default = "default_metrics_endpoint")]
    pub endpoint: String,

    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: default_metrics_bind(),
            endpoint: default_metrics_endpoint(),
            port: default_metrics_port(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_metrics_bind() -> String {
    "0.0.0.0:9090".to_string()
}

fn default_metrics_endpoint() -> String {
    "/metrics".to_string()
}

fn default_metrics_port() -> u16 {
    9090
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format
    #[serde(default = "default_log_format")]
    pub format: LogFormat,

    /// Output destination
    #[serde(default = "default_log_output")]
    pub output: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            output: default_log_output(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> LogFormat {
    LogFormat::Json
}

fn default_log_output() -> String {
    "stdout".to_string()
}

/// Log format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
}

/// TLS configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable automatic HTTPS
    #[serde(default)]
    pub auto: bool,

    /// ACME configuration for automatic certificates
    #[serde(default)]
    pub acme: Option<AcmeConfig>,

    /// Manual certificate configurations
    #[serde(default)]
    pub certificates: Vec<CertificateConfig>,

    /// TLS passthrough configuration (SNI-based routing without termination)
    #[serde(default)]
    pub passthrough: Option<TlsPassthroughConfig>,

    /// Minimum TLS version
    #[serde(default = "default_min_tls_version")]
    pub min_version: String,

    /// Session cache configuration
    #[serde(default)]
    pub session_cache: SessionCacheConfig,

    /// Mutual TLS (mTLS) configuration
    #[serde(default)]
    pub mtls: Option<MtlsConfig>,

    /// OCSP stapling configuration
    #[serde(default)]
    pub ocsp_stapling: OcspStaplingConfig,
}

fn default_min_tls_version() -> String {
    "1.2".to_string()
}

/// ACME configuration for automatic certificate management
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcmeConfig {
    /// ACME provider (letsencrypt, zerossl, buypass)
    #[serde(default = "default_acme_provider")]
    pub provider: String,

    /// Contact email for ACME account
    pub email: String,

    /// ACME directory URL
    #[serde(default = "default_acme_directory")]
    pub directory_url: String,

    /// Challenge type
    #[serde(default = "default_challenge_type")]
    pub challenge_type: String,

    /// Certificate storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Days before expiry to renew
    #[serde(default = "default_renewal_days")]
    pub renewal_days: u32,

    /// Check interval for renewal
    #[serde(default = "default_renew_check_interval", with = "humantime_serde")]
    pub renew_check_interval: Duration,
}

fn default_acme_provider() -> String {
    "letsencrypt".to_string()
}

fn default_acme_directory() -> String {
    "https://acme-v02.api.letsencrypt.org/directory".to_string()
}

fn default_challenge_type() -> String {
    "http-01".to_string()
}

fn default_renewal_days() -> u32 {
    30
}

fn default_renew_check_interval() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

/// Certificate storage configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Storage type (file, redis)
    #[serde(default = "default_storage_type")]
    pub storage_type: String,

    /// File storage path
    #[serde(default = "default_storage_path")]
    pub path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: default_storage_type(),
            path: default_storage_path(),
        }
    }
}

fn default_storage_type() -> String {
    "file".to_string()
}

fn default_storage_path() -> String {
    "/var/lib/highper-gateway/certs".to_string()
}

/// Manual certificate configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CertificateConfig {
    /// Domain name
    pub domain: String,

    /// Certificate file path
    pub cert_file: String,

    /// Private key file path
    pub key_file: String,
}

/// TLS session cache configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionCacheConfig {
    /// Enable session cache
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cache size
    #[serde(default = "default_session_cache_size")]
    pub size: usize,

    /// Session TTL
    #[serde(default = "default_session_ttl", with = "humantime_serde")]
    pub ttl: Duration,
}

impl Default for SessionCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size: default_session_cache_size(),
            ttl: default_session_ttl(),
        }
    }
}

fn default_session_cache_size() -> usize {
    10000
}

fn default_session_ttl() -> Duration {
    Duration::from_secs(3600)
}

/// TLS passthrough configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsPassthroughConfig {
    /// Enable TLS passthrough
    #[serde(default)]
    pub enabled: bool,

    /// Bind address for TLS passthrough (separate from TLS termination)
    #[serde(default = "default_passthrough_bind")]
    pub bind: Vec<String>,

    /// SNI-based routing rules
    #[serde(default)]
    pub routes: Vec<PassthroughRoute>,

    /// Default backend when no SNI match (optional)
    #[serde(default)]
    pub default_backend: Option<String>,

    /// Connection timeout
    #[serde(default = "default_passthrough_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

fn default_passthrough_bind() -> Vec<String> {
    vec!["0.0.0.0:8443".to_string()]
}

fn default_passthrough_timeout() -> Duration {
    Duration::from_secs(60)
}

/// TLS passthrough route
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PassthroughRoute {
    /// Server name (SNI hostname)
    pub server_name: String,

    /// Backend upstream name or URL
    pub upstream: String,

    /// Enable this route
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Mutual TLS (mTLS) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MtlsConfig {
    /// Enable mTLS
    #[serde(default)]
    pub enabled: bool,

    /// Path to CA certificate(s) for verifying client certificates
    /// Can be a file or directory
    pub ca_cert_path: String,

    /// Client certificate verification mode (global default)
    #[serde(default = "default_verification_mode")]
    pub verification_mode: CertVerificationMode,

    /// Certificate Revocation List (CRL) path (optional)
    #[serde(default)]
    pub crl_path: Option<String>,

    /// OCSP configuration
    #[serde(default)]
    pub ocsp: OcspConfig,

    /// Additional trusted certificate authorities
    #[serde(default)]
    pub additional_cas: Vec<String>,
}

/// Certificate verification mode
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CertVerificationMode {
    /// Require valid client certificate (connection fails without it)
    Required,

    /// Request client certificate but allow connection without it
    Optional,

    /// Request client certificate, verify if provided, allow if not
    OptionalNoCA,
}

fn default_verification_mode() -> CertVerificationMode {
    CertVerificationMode::Optional
}

/// OCSP configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcspConfig {
    /// Enable OCSP revocation checking
    #[serde(default)]
    pub enabled: bool,

    /// OCSP responder URL (optional, uses cert's AIA if not set)
    #[serde(default)]
    pub responder_url: Option<String>,

    /// Timeout for OCSP requests (seconds)
    #[serde(default = "default_ocsp_timeout")]
    pub timeout: u64,

    /// Fail open if OCSP check fails (security vs availability)
    #[serde(default)]
    pub fail_open: bool,
}

impl Default for OcspConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            responder_url: None,
            timeout: default_ocsp_timeout(),
            fail_open: false,
        }
    }
}

fn default_ocsp_timeout() -> u64 {
    5
}

/// OCSP stapling configuration (for server certificates)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcspStaplingConfig {
    /// Enable OCSP stapling
    #[serde(default)]
    pub enabled: bool,

    /// OCSP responder URL (optional, uses cert's AIA extension if not set)
    #[serde(default)]
    pub responder_url: Option<String>,

    /// How often to refresh OCSP response (seconds)
    #[serde(default = "default_ocsp_refresh_interval")]
    pub refresh_interval: u64,

    /// Timeout for OCSP requests (seconds)
    #[serde(default = "default_ocsp_stapling_timeout")]
    pub timeout: u64,
}

impl Default for OcspStaplingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            responder_url: None,
            refresh_interval: default_ocsp_refresh_interval(),
            timeout: default_ocsp_stapling_timeout(),
        }
    }
}

fn default_ocsp_refresh_interval() -> u64 {
    21600 // 6 hours
}

fn default_ocsp_stapling_timeout() -> u64 {
    10 // 10 seconds
}

/// Per-route mTLS policy
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteMtlsPolicy {
    /// Verification mode for this route (optional, overrides global)
    #[serde(default)]
    pub verification_mode: Option<CertVerificationMode>,

    /// Allowed client certificate subjects (DN patterns)
    #[serde(default)]
    pub allowed_subjects: Vec<String>,

    /// Allowed client certificate issuers (DN patterns)
    #[serde(default)]
    pub allowed_issuers: Vec<String>,

    /// Allowed certificate serial numbers
    #[serde(default)]
    pub allowed_serials: Vec<String>,

    /// Allowed certificate fingerprints (SHA-256, hex format)
    #[serde(default)]
    pub allowed_fingerprints: Vec<String>,
}

/// Admin API configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminConfig {
    /// Enable admin API
    #[serde(default)]
    pub enabled: bool,

    /// Admin API bind address
    #[serde(default = "default_admin_bind")]
    pub bind: String,

    /// Enable authentication
    #[serde(default = "default_true")]
    pub auth_enabled: bool,

    /// API keys for authentication (key name -> key value)
    #[serde(default)]
    pub api_keys: Vec<String>,

    /// JWT secret for token-based authentication
    pub jwt_secret: Option<String>,

    /// JWT token expiration time
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration: String,

    /// Enable CORS
    #[serde(default)]
    pub cors_enabled: bool,

    /// Allowed CORS origins
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Read-only mode (only GET endpoints)
    #[serde(default)]
    pub read_only: bool,
}

fn default_admin_bind() -> String {
    "127.0.0.1:9000".to_string()
}

fn default_jwt_expiration() -> String {
    "24h".to_string()
}

/// Cache configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    /// Enable caching
    #[serde(default)]
    pub enabled: bool,

    /// Default TTL for cache entries
    #[serde(default = "default_cache_ttl", with = "humantime_serde")]
    pub default_ttl: Duration,

    /// Maximum cache size (number of entries)
    #[serde(default = "default_cache_max_size")]
    pub max_size: usize,

    /// Cleanup interval for expired entries
    #[serde(default = "default_cache_cleanup_interval", with = "humantime_serde")]
    pub cleanup_interval: Duration,

    /// Cache only successful responses (2xx status codes)
    #[serde(default = "default_true")]
    pub cache_only_success: bool,

    /// HTTP methods to cache (default: GET, HEAD)
    #[serde(default = "default_cache_methods")]
    pub methods: Vec<String>,

    /// Headers to include in cache key
    #[serde(default)]
    pub key_headers: Vec<String>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_ttl: default_cache_ttl(),
            max_size: default_cache_max_size(),
            cleanup_interval: default_cache_cleanup_interval(),
            cache_only_success: true,
            methods: default_cache_methods(),
            key_headers: vec![],
        }
    }
}

fn default_cache_ttl() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_cache_max_size() -> usize {
    10000
}

fn default_cache_cleanup_interval() -> Duration {
    Duration::from_secs(60) // 1 minute
}

fn default_cache_methods() -> Vec<String> {
    vec!["GET".to_string(), "HEAD".to_string()]
}

/// Per-route cache configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteCacheConfig {
    /// Enable caching for this route
    #[serde(default)]
    pub enabled: bool,

    /// TTL for this route (overrides global)
    #[serde(default, with = "humantime_serde")]
    pub ttl: Option<Duration>,

    /// Cache key suffix for this route
    pub key_suffix: Option<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    #[serde(default)]
    pub enabled: bool,

    /// Rate limiting algorithm
    #[serde(default = "default_rate_limit_algorithm")]
    pub algorithm: RateLimitAlgorithm,

    /// Token bucket: maximum requests per window
    #[serde(default = "default_rate_limit_capacity")]
    pub capacity: u32,

    /// Token bucket: refill rate (tokens per second)
    #[serde(default = "default_rate_limit_refill_rate")]
    pub refill_rate: f64,

    /// Time window for rate limiting
    #[serde(default = "default_rate_limit_window", with = "humantime_serde")]
    pub window: Duration,

    /// Key to use for rate limiting
    #[serde(default = "default_rate_limit_key")]
    pub key_type: RateLimitKeyType,

    /// Custom header name for rate limit key (when key_type = "header")
    pub key_header: Option<String>,

    /// Cleanup interval for old entries
    #[serde(default = "default_rate_limit_cleanup_interval", with = "humantime_serde")]
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            algorithm: default_rate_limit_algorithm(),
            capacity: default_rate_limit_capacity(),
            refill_rate: default_rate_limit_refill_rate(),
            window: default_rate_limit_window(),
            key_type: default_rate_limit_key(),
            key_header: None,
            cleanup_interval: default_rate_limit_cleanup_interval(),
        }
    }
}

fn default_rate_limit_algorithm() -> RateLimitAlgorithm {
    RateLimitAlgorithm::TokenBucket
}

fn default_rate_limit_capacity() -> u32 {
    100
}

fn default_rate_limit_refill_rate() -> f64 {
    10.0 // 10 requests per second
}

fn default_rate_limit_window() -> Duration {
    Duration::from_secs(60)
}

fn default_rate_limit_key() -> RateLimitKeyType {
    RateLimitKeyType::Ip
}

fn default_rate_limit_cleanup_interval() -> Duration {
    Duration::from_secs(60)
}

/// Rate limiting algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitAlgorithm {
    /// Token bucket algorithm (smooth rate limiting)
    TokenBucket,
    /// Sliding window algorithm (more precise)
    SlidingWindow,
}

/// Rate limit key type
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitKeyType {
    /// Client IP address
    Ip,
    /// HTTP header value
    Header,
    /// Combination of IP and path
    IpPath,
}

/// Per-route rate limiting configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteRateLimitConfig {
    /// Enable rate limiting for this route
    #[serde(default)]
    pub enabled: bool,

    /// Capacity for this route (overrides global)
    pub capacity: Option<u32>,

    /// Refill rate for this route (overrides global)
    pub refill_rate: Option<f64>,

    /// Window for this route (overrides global)
    #[serde(default, with = "humantime_serde")]
    pub window: Option<Duration>,

    /// Key type for this route (overrides global)
    pub key_type: Option<RateLimitKeyType>,
}

/// PHP-FPM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PhpFpmConfig {
    /// Enable PHP-FPM processing
    #[serde(default)]
    pub enabled: bool,

    /// PHP-FPM socket path (Unix socket or TCP address like 127.0.0.1:9000)
    pub socket: String,

    /// Connection pool size
    #[serde(default = "default_php_fpm_pool_size")]
    pub pool_size: usize,

    /// Connect timeout in seconds
    #[serde(default = "default_php_fpm_connect_timeout")]
    pub connect_timeout_secs: u64,

    /// Read timeout in seconds
    #[serde(default = "default_php_fpm_read_timeout")]
    pub read_timeout_secs: u64,

    /// Write timeout in seconds
    #[serde(default = "default_php_fpm_write_timeout")]
    pub write_timeout_secs: u64,

    /// Keepalive timeout in seconds
    #[serde(default = "default_php_fpm_keepalive_timeout")]
    pub keepalive_timeout_secs: u64,

    /// Script file extensions to process (.php, .phtml, etc.)
    #[serde(default = "default_php_fpm_script_extensions")]
    pub script_extensions: Vec<String>,
}

fn default_php_fpm_pool_size() -> usize {
    50
}

fn default_php_fpm_connect_timeout() -> u64 {
    5
}

fn default_php_fpm_read_timeout() -> u64 {
    60
}

fn default_php_fpm_write_timeout() -> u64 {
    60
}

fn default_php_fpm_keepalive_timeout() -> u64 {
    90
}

fn default_php_fpm_script_extensions() -> Vec<String> {
    vec![".php".to_string()]
}


/// Tracing configuration for OpenTelemetry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    #[serde(default)]
    pub enabled: bool,

    /// Exporter type: jaeger, zipkin, otlp
    #[serde(default = "default_tracing_exporter")]
    pub exporter: String,

    /// Exporter endpoint
    #[serde(default = "default_tracing_endpoint")]
    pub endpoint: String,

    /// Sample rate (0.0 to 1.0)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,

    /// Service name in traces
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Additional resource attributes
    #[serde(default)]
    pub resource_attributes: std::collections::HashMap<String, String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exporter: default_tracing_exporter(),
            endpoint: default_tracing_endpoint(),
            sample_rate: default_sample_rate(),
            service_name: default_service_name(),
            resource_attributes: std::collections::HashMap::new(),
        }
    }
}

fn default_tracing_exporter() -> String {
    "jaeger".to_string()
}

fn default_tracing_endpoint() -> String {
    "http://localhost:14268/api/traces".to_string()
}

fn default_sample_rate() -> f64 {
    1.0
}

fn default_service_name() -> String {
    "highper-gateway".to_string()
}

/// WAF (Web Application Firewall) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WafConfig {
    /// Enable WAF
    #[serde(default = "default_waf_enabled")]
    pub enabled: bool,

    /// WAF engine mode: custom, coraza, modsecurity, aws
    #[serde(default = "default_waf_mode")]
    pub mode: WafMode,

    /// Block mode (true) or log-only mode (false)
    #[serde(default = "default_waf_block_mode")]
    pub block_mode: bool,

    /// Custom engine configuration
    #[serde(default)]
    pub custom: Option<CustomWafConfig>,

    /// Maximum request body size to inspect (bytes)
    #[serde(default = "default_waf_max_body_size")]
    pub max_body_size: usize,

    /// Coraza configuration (if using Coraza mode)
    #[serde(default)]
    pub coraza: Option<CorazaConfig>,

    /// ModSecurity configuration (if using ModSecurity mode)
    #[serde(default)]
    pub modsecurity: Option<ModSecurityConfig>,

    /// AWS WAF configuration (if using AWS mode)
    #[serde(default)]
    pub aws: Option<AwsWafConfig>,
}

impl Default for WafConfig {
    fn default() -> Self {
        Self {
            enabled: default_waf_enabled(),
            mode: default_waf_mode(),
            block_mode: default_waf_block_mode(),
            custom: Some(CustomWafConfig::default()),
            max_body_size: default_waf_max_body_size(),
            coraza: None,
            modsecurity: None,
            aws: None,
        }
    }
}

fn default_waf_enabled() -> bool {
    false
}

fn default_waf_mode() -> WafMode {
    WafMode::Custom
}

fn default_waf_block_mode() -> bool {
    true
}

fn default_waf_max_body_size() -> usize {
    1024 * 1024 // 1 MB
}

/// Coraza WAF configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorazaConfig {
    /// Path to OWASP CRS configuration directory
    pub crs_path: String,

    /// Enable OWASP Core Rule Set
    #[serde(default = "default_config_coraza_true")]
    pub enable_crs: bool,

    /// Custom rule files
    #[serde(default)]
    pub custom_rules: Vec<String>,

    /// Paranoia level (1-4)
    #[serde(default = "default_config_paranoia_level")]
    pub paranoia_level: u8,
}

impl Default for CorazaConfig {
    fn default() -> Self {
        Self {
            crs_path: "/etc/coraza/crs".to_string(),
            enable_crs: true,
            custom_rules: Vec::new(),
            paranoia_level: 2,
        }
    }
}

fn default_config_coraza_true() -> bool {
    true
}

fn default_config_paranoia_level() -> u8 {
    2
}

/// AWS WAF configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AwsWafConfig {
    /// AWS region
    pub region: String,

    /// Web ACL ARN
    pub web_acl_arn: String,

    /// AWS access key ID (or use IAM role)
    #[serde(default)]
    pub access_key_id: Option<String>,

    /// AWS secret access key (or use IAM role)
    #[serde(default)]
    pub secret_access_key: Option<String>,

    /// Cache TTL for WAF decisions (seconds)
    #[serde(default = "default_aws_waf_cache_ttl")]
    pub cache_ttl: u64,
}

fn default_aws_waf_cache_ttl() -> u64 {
    300 // 5 minutes
}

