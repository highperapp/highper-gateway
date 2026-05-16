//! Token bucket rate limiting algorithm
//!
//! Classic token bucket implementation for smooth rate limiting.

use super::{RateLimitKey, RateLimitResult};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

/// Token bucket configuration
#[derive(Debug, Clone)]
pub struct TokenBucketConfig {
    /// Maximum number of tokens in the bucket
    pub capacity: u32,
    /// Rate at which tokens are refilled (tokens per second)
    pub refill_rate: f64,
    /// Time window for rate limiting
    pub window: Duration,
}

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            refill_rate: 10.0, // 10 tokens per second
            window: Duration::from_secs(60),
        }
    }
}

/// Token bucket state
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(capacity: u32) -> Self {
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self, refill_rate: f64, capacity: u32) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * refill_rate;

        self.tokens = (self.tokens + tokens_to_add).min(capacity as f64);
        self.last_refill = now;
    }

    /// Try to consume a token
    fn consume(&mut self, tokens: f64) -> bool {
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Get time until next token is available
    fn time_until_token(&self, refill_rate: f64) -> Duration {
        if refill_rate <= 0.0 {
            return Duration::from_secs(0);
        }

        let tokens_needed = 1.0 - self.tokens;
        if tokens_needed <= 0.0 {
            return Duration::from_secs(0);
        }

        let seconds = tokens_needed / refill_rate;
        Duration::from_secs_f64(seconds.max(0.0))
    }
}

/// Token bucket rate limiter
pub struct TokenBucketLimiter {
    config: TokenBucketConfig,
    buckets: Arc<DashMap<String, Bucket>>,
}

impl TokenBucketLimiter {
    /// Create a new token bucket rate limiter
    pub fn new(config: TokenBucketConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// Create with default configuration
    pub fn default_limiter() -> Self {
        Self::new(TokenBucketConfig::default())
    }

    /// Check if a request is allowed
    pub fn check<K: RateLimitKey>(&self, key: K, tokens: Option<f64>) -> RateLimitResult {
        let key_str = key.to_key();
        let tokens = tokens.unwrap_or(1.0);

        // Get or create bucket for this key
        let mut bucket_ref = self
            .buckets
            .entry(key_str.clone())
            .or_insert_with(|| Bucket::new(self.config.capacity));

        // Refill tokens based on elapsed time
        bucket_ref.refill(self.config.refill_rate, self.config.capacity);

        // Try to consume tokens
        if bucket_ref.consume(tokens) {
            debug!("Rate limit check passed for key: {}", key_str);
            RateLimitResult::Allowed
        } else {
            let retry_after = bucket_ref.time_until_token(self.config.refill_rate);
            debug!(
                "Rate limit exceeded for key: {}, retry after: {:?}",
                key_str, retry_after
            );
            RateLimitResult::Limited {
                retry_after: retry_after.as_secs(),
            }
        }
    }

    /// Reset rate limit for a specific key
    pub fn reset<K: RateLimitKey>(&self, key: K) {
        let key_str = key.to_key();
        self.buckets.remove(&key_str);
    }

    /// Clear all rate limit data
    pub fn clear_all(&self) {
        self.buckets.clear();
    }

    /// Get current token count for a key
    pub fn get_tokens<K: RateLimitKey>(&self, key: K) -> Option<f64> {
        let key_str = key.to_key();
        self.buckets.get(&key_str).map(|b| b.tokens)
    }

    /// Get number of tracked keys
    pub fn key_count(&self) -> usize {
        self.buckets.len()
    }

    /// Start cleanup task to remove old entries
    pub fn start_cleanup_task(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);
            loop {
                cleanup_interval.tick().await;
                self.cleanup_old_entries();
            }
        });
    }

    /// Remove entries that haven't been accessed recently
    fn cleanup_old_entries(&self) {
        let cutoff = Instant::now() - self.config.window;
        self.buckets.retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_token_bucket_basic() {
        let config = TokenBucketConfig {
            capacity: 10,
            refill_rate: 10.0,
            window: Duration::from_secs(60),
        };

        let limiter = TokenBucketLimiter::new(config);

        // First request should succeed
        let result = limiter.check("user1", None);
        assert!(result.is_allowed());

        // Consume all tokens
        for _ in 0..9 {
            let result = limiter.check("user1", None);
            assert!(result.is_allowed());
        }

        // Next request should be rate limited
        let result = limiter.check("user1", None);
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_token_bucket_refill() {
        let config = TokenBucketConfig {
            capacity: 10,
            refill_rate: 100.0, // Fast refill for testing
            window: Duration::from_secs(60),
        };

        let limiter = TokenBucketLimiter::new(config);

        // Consume all tokens
        for _ in 0..10 {
            limiter.check("user1", None);
        }

        // Should be rate limited
        let result = limiter.check("user1", None);
        assert!(!result.is_allowed());

        // Wait for refill (100 tokens/sec = 1 token per 10ms)
        sleep(Duration::from_millis(50)); // Wait for 5 tokens

        // Should be allowed again
        let result = limiter.check("user1", None);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_token_bucket_multiple_keys() {
        let limiter = TokenBucketLimiter::default_limiter();

        // Different keys should have independent limits
        let result1 = limiter.check("user1", None);
        let result2 = limiter.check("user2", None);

        assert!(result1.is_allowed());
        assert!(result2.is_allowed());
        assert_eq!(limiter.key_count(), 2);
    }

    #[test]
    fn test_token_bucket_reset() {
        let config = TokenBucketConfig {
            capacity: 5,
            refill_rate: 1.0,
            window: Duration::from_secs(60),
        };

        let limiter = TokenBucketLimiter::new(config);

        // Consume all tokens
        for _ in 0..5 {
            limiter.check("user1", None);
        }

        // Should be rate limited
        let result = limiter.check("user1", None);
        assert!(!result.is_allowed());

        // Reset
        limiter.reset("user1");

        // Should be allowed again
        let result = limiter.check("user1", None);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_token_bucket_custom_tokens() {
        let config = TokenBucketConfig {
            capacity: 100,
            refill_rate: 10.0,
            window: Duration::from_secs(60),
        };

        let limiter = TokenBucketLimiter::new(config);

        // Consume 50 tokens at once
        let result = limiter.check("user1", Some(50.0));
        assert!(result.is_allowed());

        // Should have 50 tokens left
        let tokens = limiter.get_tokens("user1").unwrap();
        assert!((tokens - 50.0).abs() < 0.01);

        // Try to consume 60 tokens (should fail)
        let result = limiter.check("user1", Some(60.0));
        assert!(!result.is_allowed());
    }
}
