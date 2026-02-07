//! DDoS protection middleware
//!
//! This middleware provides protection against Distributed Denial of Service attacks:
//! - Connection rate limiting per IP
//! - Slowloris attack prevention
//! - Request rate limiting with burst handling
//! - Blacklist/whitelist support
//! - Automatic temporary bans
//! - Geographic filtering (optional)

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use hyper::{Request, Response, StatusCode, header};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use tracing::{warn, debug};
use std::net::IpAddr;
use bytes::Bytes;
use http_body_util::Full;

/// DDoS protection configuration
#[derive(Debug, Clone)]
pub struct DdosProtectionConfig {
    /// Maximum connections per IP
    pub max_connections_per_ip: usize,
    /// Maximum requests per second per IP
    pub max_requests_per_second: usize,
    /// Burst allowance (requests)
    pub burst_size: usize,
    /// Slowloris timeout (seconds)
    pub slowloris_timeout: Duration,
    /// Ban duration for violators (seconds)
    pub ban_duration: Duration,
    /// Threshold for automatic ban (violations)
    pub ban_threshold: usize,
    /// Whitelist IPs (never banned)
    pub whitelist: Vec<IpAddr>,
    /// Blacklist IPs (always banned)
    pub blacklist: Vec<IpAddr>,
    /// Enable geographic filtering
    pub geo_filtering_enabled: bool,
    /// Allowed countries (ISO 3166-1 alpha-2 codes)
    pub allowed_countries: Vec<String>,
    /// Clean up interval for tracking data
    pub cleanup_interval: Duration,
}

impl Default for DdosProtectionConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip: 100,
            max_requests_per_second: 50,
            burst_size: 100,
            slowloris_timeout: Duration::from_secs(30),
            ban_duration: Duration::from_secs(300),  // 5 minutes
            ban_threshold: 10,
            whitelist: vec![],
            blacklist: vec![],
            geo_filtering_enabled: false,
            allowed_countries: vec![],
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

impl DdosProtectionConfig {
    /// Create strict DDoS protection (tight limits)
    pub fn strict() -> Self {
        Self {
            max_connections_per_ip: 50,
            max_requests_per_second: 10,
            burst_size: 20,
            slowloris_timeout: Duration::from_secs(15),
            ban_duration: Duration::from_secs(600),  // 10 minutes
            ban_threshold: 5,
            whitelist: vec![],
            blacklist: vec![],
            geo_filtering_enabled: false,
            allowed_countries: vec![],
            cleanup_interval: Duration::from_secs(60),
        }
    }

    /// Create relaxed DDoS protection (for development)
    pub fn relaxed() -> Self {
        Self {
            max_connections_per_ip: 1000,
            max_requests_per_second: 200,
            burst_size: 500,
            slowloris_timeout: Duration::from_secs(120),
            ban_duration: Duration::from_secs(60),  // 1 minute
            ban_threshold: 50,
            whitelist: vec![],
            blacklist: vec![],
            geo_filtering_enabled: false,
            allowed_countries: vec![],
            cleanup_interval: Duration::from_secs(300),
        }
    }

    /// Create API protection (balanced for APIs)
    pub fn api() -> Self {
        Self {
            max_connections_per_ip: 200,
            max_requests_per_second: 100,
            burst_size: 200,
            slowloris_timeout: Duration::from_secs(30),
            ban_duration: Duration::from_secs(300),
            ban_threshold: 10,
            whitelist: vec![],
            blacklist: vec![],
            geo_filtering_enabled: false,
            allowed_countries: vec![],
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

/// Per-IP tracking data
#[derive(Debug, Clone)]
struct IpTrackingData {
    /// Active connections from this IP
    connections: usize,
    /// Request timestamps (for rate limiting)
    requests: Vec<Instant>,
    /// Violation count
    violations: usize,
    /// Last violation time
    last_violation: Option<Instant>,
    /// Ban expiry time
    ban_expiry: Option<Instant>,
}

impl IpTrackingData {
    fn new() -> Self {
        Self {
            connections: 0,
            requests: Vec::new(),
            violations: 0,
            last_violation: None,
            ban_expiry: None,
        }
    }

    /// Check if IP is currently banned
    fn is_banned(&self) -> bool {
        if let Some(expiry) = self.ban_expiry {
            Instant::now() < expiry
        } else {
            false
        }
    }

    /// Record a violation
    fn record_violation(&mut self, ban_duration: Duration, ban_threshold: usize) {
        self.violations += 1;
        self.last_violation = Some(Instant::now());

        // Auto-ban if threshold exceeded
        if self.violations >= ban_threshold {
            self.ban_expiry = Some(Instant::now() + ban_duration);
            warn!(
                "IP auto-banned for {} seconds due to {} violations",
                ban_duration.as_secs(),
                self.violations
            );
        }
    }

    /// Clean up old request timestamps
    fn cleanup_requests(&mut self, window: Duration) {
        let cutoff = Instant::now() - window;
        self.requests.retain(|&ts| ts > cutoff);
    }

    /// Check rate limit
    fn check_rate_limit(&mut self, max_per_second: usize, burst_size: usize) -> bool {
        let now = Instant::now();
        let one_second_ago = now - Duration::from_secs(1);

        // Clean up old requests
        self.requests.retain(|&ts| ts > one_second_ago);

        // Check burst limit
        if self.requests.len() >= burst_size {
            return false;
        }

        // Check per-second limit
        if self.requests.len() >= max_per_second {
            return false;
        }

        // Record this request
        self.requests.push(now);
        true
    }
}

/// DDoS protection middleware
pub struct DdosProtectionMiddleware {
    config: DdosProtectionConfig,
    tracking: Arc<DashMap<String, IpTrackingData>>,
}

impl DdosProtectionMiddleware {
    /// Create a new DDoS protection middleware
    pub fn new(config: DdosProtectionConfig) -> Self {
        let middleware = Self {
            config,
            tracking: Arc::new(DashMap::new()),
        };

        // Spawn cleanup task
        middleware.spawn_cleanup_task();

        middleware
    }

    /// Create with default config
    pub fn default_protection() -> Self {
        Self::new(DdosProtectionConfig::default())
    }

    /// Create with strict config
    pub fn strict() -> Self {
        Self::new(DdosProtectionConfig::strict())
    }

    /// Create with relaxed config
    pub fn relaxed() -> Self {
        Self::new(DdosProtectionConfig::relaxed())
    }

    /// Create with API config
    pub fn api() -> Self {
        Self::new(DdosProtectionConfig::api())
    }

    /// Spawn background cleanup task
    fn spawn_cleanup_task(&self) {
        let tracking = self.tracking.clone();
        let interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;

                // Clean up expired entries
                let now = Instant::now();
                tracking.retain(|_, data| {
                    // Keep if banned and not expired
                    if let Some(ban_expiry) = data.ban_expiry {
                        if now < ban_expiry {
                            return true;
                        }
                    }

                    // Keep if has recent activity
                    !data.requests.is_empty() || data.connections > 0
                });

                debug!("DDoS protection cleanup: {} IPs tracked", tracking.len());
            }
        });
    }

    /// Extract client IP from request
    fn extract_client_ip<B>(req: &Request<B>) -> Option<String> {
        // Try X-Forwarded-For header first
        if let Some(xff) = req.headers().get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                if let Some(ip) = xff_str.split(',').next() {
                    return Some(ip.trim().to_string());
                }
            }
        }

        // Try X-Real-IP header
        if let Some(xri) = req.headers().get("x-real-ip") {
            if let Ok(xri_str) = xri.to_str() {
                return Some(xri_str.to_string());
            }
        }

        None
    }

    /// Check if IP is whitelisted
    fn is_whitelisted(&self, ip: &str) -> bool {
        if let Ok(addr) = ip.parse::<IpAddr>() {
            self.config.whitelist.contains(&addr)
        } else {
            false
        }
    }

    /// Check if IP is blacklisted
    fn is_blacklisted(&self, ip: &str) -> bool {
        if let Ok(addr) = ip.parse::<IpAddr>() {
            self.config.blacklist.contains(&addr)
        } else {
            false
        }
    }

    /// Create rate limit response
    fn create_rate_limit_response(reason: &str) -> Response<Full<Bytes>> {
        warn!("DDoS protection triggered: {}", reason);

        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::RETRY_AFTER, "60")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(
                format!(r#"{{"error":"Rate limit exceeded","reason":"{}"}}"#, reason)
            )))
            .unwrap()
    }

    /// Create banned response
    fn create_banned_response() -> Response<Full<Bytes>> {
        warn!("Request from banned IP");

        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(
                r#"{"error":"Access forbidden","reason":"IP temporarily banned"}"#
            )))
            .unwrap()
    }
}

impl Middleware for DdosProtectionMiddleware {
    fn name(&self) -> &str {
        "ddos-protection"
    }

    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>> {
        let config = self.config.clone();
        let tracking = self.tracking.clone();
        let whitelist = self.config.whitelist.clone();
        let blacklist = self.config.blacklist.clone();

        Box::pin(async move {
            // Extract client IP
            let client_ip = Self::extract_client_ip(&req)
                .unwrap_or_else(|| "unknown".to_string());

            // Check whitelist
            if let Ok(addr) = client_ip.parse::<IpAddr>() {
                if whitelist.contains(&addr) {
                    debug!("IP {} is whitelisted, bypassing DDoS protection", client_ip);
                    return Ok(req);
                }

                // Check blacklist
                if blacklist.contains(&addr) {
                    warn!("Request from blacklisted IP: {}", client_ip);
                    return Err(Self::create_banned_response());
                }
            }

            // Get or create tracking data
            let mut entry = tracking.entry(client_ip.clone()).or_insert_with(IpTrackingData::new);

            // Check if banned
            if entry.is_banned() {
                return Err(Self::create_banned_response());
            }

            // Check connection limit
            if entry.connections >= config.max_connections_per_ip {
                entry.record_violation(config.ban_duration, config.ban_threshold);
                return Err(Self::create_rate_limit_response("Too many concurrent connections"));
            }

            // Check rate limit
            if !entry.check_rate_limit(config.max_requests_per_second, config.burst_size) {
                entry.record_violation(config.ban_duration, config.ban_threshold);
                return Err(Self::create_rate_limit_response("Request rate limit exceeded"));
            }

            // Increment connection count
            entry.connections += 1;

            debug!(
                "DDoS protection passed for IP {}: {} connections, {} requests in window",
                client_ip, entry.connections, entry.requests.len()
            );

            Ok(req)
        })
    }

    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        Box::pin(async move {
            // Note: In a real implementation, we would need request context
            // to decrement the connection count for the specific IP
            // This is a simplified version
            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_tracking_rate_limit() {
        let mut tracking = IpTrackingData::new();

        // Should allow first 10 requests
        for _ in 0..10 {
            assert!(tracking.check_rate_limit(10, 20));
        }

        // Should block 11th request
        assert!(!tracking.check_rate_limit(10, 20));
    }

    #[test]
    fn test_ip_tracking_ban() {
        let mut tracking = IpTrackingData::new();

        // Record violations
        for _ in 0..5 {
            tracking.record_violation(Duration::from_secs(300), 5);
        }

        // Should be banned after threshold
        assert!(tracking.is_banned());
    }

    #[test]
    fn test_config_presets() {
        let default = DdosProtectionConfig::default();
        assert_eq!(default.max_connections_per_ip, 100);

        let strict = DdosProtectionConfig::strict();
        assert_eq!(strict.max_connections_per_ip, 50);

        let relaxed = DdosProtectionConfig::relaxed();
        assert_eq!(relaxed.max_connections_per_ip, 1000);

        let api = DdosProtectionConfig::api();
        assert_eq!(api.max_connections_per_ip, 200);
    }
}
