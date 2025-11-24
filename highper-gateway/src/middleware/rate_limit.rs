//! Rate limiting middleware
//!
//! Implements token bucket algorithm for rate limiting requests per IP address.
//! Prevents abuse and ensures fair resource allocation.

use dashmap::DashMap;
use hyper::{Request, Response, StatusCode, body::Incoming};
use http_body_util::Full;
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub requests_per_window: u32,

    /// Time window duration
    pub window_duration: Duration,

    /// Whether to enable rate limiting
    pub enabled: bool,

    /// Custom response message for rate limited requests
    pub rate_limit_message: Option<String>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 100,
            window_duration: Duration::from_secs(60),
            enabled: true,
            rate_limit_message: None,
        }
    }
}

/// Token bucket for a single client
#[derive(Debug)]
struct TokenBucket {
    /// Number of tokens available
    tokens: f64,

    /// Maximum token capacity
    capacity: f64,

    /// Token refill rate (tokens per second)
    refill_rate: f64,

    /// Last refill timestamp
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, window: Duration) -> Self {
        let refill_rate = capacity as f64 / window.as_secs_f64();
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self) -> bool {
        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        let new_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;

        // Try to consume one token
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.tokens = self.capacity;
        self.last_refill = Instant::now();
    }
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<DashMap<String, TokenBucket>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// Check if request should be allowed
    pub fn check_rate_limit(&self, client_ip: &str) -> bool {
        if !self.config.enabled {
            return true;
        }

        // Get or create bucket for this client
        let mut entry = self.buckets.entry(client_ip.to_string()).or_insert_with(|| {
            TokenBucket::new(
                self.config.requests_per_window,
                self.config.window_duration,
            )
        });

        entry.try_consume()
    }

    /// Reset rate limit for a specific client (admin function)
    pub fn reset_client(&self, client_ip: &str) {
        if let Some(mut bucket) = self.buckets.get_mut(client_ip) {
            bucket.reset();
            debug!("Reset rate limit for client: {}", client_ip);
        }
    }

    /// Get current bucket stats for a client
    pub fn get_client_stats(&self, client_ip: &str) -> Option<(f64, f64)> {
        self.buckets.get(client_ip).map(|bucket| {
            (bucket.tokens, bucket.capacity)
        })
    }

    /// Clean up old buckets (call periodically)
    pub fn cleanup_old_buckets(&self, max_age: Duration) {
        let now = Instant::now();
        self.buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < max_age
        });
    }

    /// Create rate limit exceeded response
    pub fn rate_limit_response(&self) -> Response<Full<Bytes>> {
        let message = self.config.rate_limit_message.clone()
            .unwrap_or_else(|| "Rate limit exceeded. Please try again later.".to_string());

        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "text/plain")
            .header("Retry-After", self.config.window_duration.as_secs().to_string())
            .body(Full::new(Bytes::from(message)))
            .unwrap()
    }
}

/// Extract client IP from request
pub fn extract_client_ip(req: &Request<Incoming>) -> String {
    // Check X-Forwarded-For header first
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            if let Some(first_ip) = xff_str.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }

    // Check X-Real-IP header
    if let Some(xri) = req.headers().get("x-real-ip") {
        if let Ok(xri_str) = xri.to_str() {
            return xri_str.to_string();
        }
    }

    // Fallback to connection remote addr (not available in this context)
    // In production, this would come from the connection metadata
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(10, Duration::from_secs(10));

        // Should be able to consume 10 tokens
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        // 11th should fail
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10, Duration::from_secs(1));

        // Consume all tokens
        for _ in 0..10 {
            bucket.try_consume();
        }

        // Wait for refill
        sleep(Duration::from_millis(200));

        // Should have refilled ~2 tokens (10 tokens/sec * 0.2sec)
        assert!(bucket.try_consume());
        assert!(bucket.try_consume());
    }

    #[test]
    fn test_rate_limiter() {
        let config = RateLimitConfig {
            requests_per_window: 5,
            window_duration: Duration::from_secs(1),
            enabled: true,
            rate_limit_message: None,
        };

        let limiter = RateLimiter::new(config);

        // First 5 requests should pass
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("192.168.1.1"));
        }

        // 6th should fail
        assert!(!limiter.check_rate_limit("192.168.1.1"));

        // Different IP should pass
        assert!(limiter.check_rate_limit("192.168.1.2"));
    }

    #[test]
    fn test_rate_limiter_reset() {
        let config = RateLimitConfig {
            requests_per_window: 5,
            window_duration: Duration::from_secs(1),
            enabled: true,
            rate_limit_message: None,
        };

        let limiter = RateLimiter::new(config);

        // Exhaust limit
        for _ in 0..5 {
            limiter.check_rate_limit("192.168.1.1");
        }

        assert!(!limiter.check_rate_limit("192.168.1.1"));

        // Reset
        limiter.reset_client("192.168.1.1");

        // Should work again
        assert!(limiter.check_rate_limit("192.168.1.1"));
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let config = RateLimitConfig {
            requests_per_window: 1,
            window_duration: Duration::from_secs(1),
            enabled: false,
            rate_limit_message: None,
        };

        let limiter = RateLimiter::new(config);

        // Should allow unlimited requests when disabled
        for _ in 0..100 {
            assert!(limiter.check_rate_limit("192.168.1.1"));
        }
    }
}
