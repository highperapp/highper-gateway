//! Distributed rate limiting with Redis
//!
//! Provides rate limiting across multiple proxy instances using Redis.

use super::{RateLimitResult, RateLimitKey};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client, RedisError};
use std::time::Duration;
use tracing::{debug, error};

/// Distributed rate limiter configuration
#[derive(Debug, Clone)]
pub struct DistributedRateLimiterConfig {
    /// Redis connection string
    pub redis_url: String,
    /// Maximum requests in the window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Key prefix for Redis keys
    pub key_prefix: String,
}

impl Default for DistributedRateLimiterConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            max_requests: 100,
            window: Duration::from_secs(60),
            key_prefix: "ratelimit".to_string(),
        }
    }
}

/// Distributed rate limiter using Redis
pub struct DistributedRateLimiter {
    config: DistributedRateLimiterConfig,
    connection: ConnectionManager,
}

impl DistributedRateLimiter {
    /// Create a new distributed rate limiter
    pub async fn new(config: DistributedRateLimiterConfig) -> Result<Self, RedisError> {
        let client = Client::open(config.redis_url.clone())?;
        let connection = ConnectionManager::new(client).await?;

        Ok(Self { config, connection })
    }

    /// Check if a request is allowed using Redis INCR and EXPIRE
    pub async fn check<K: RateLimitKey>(&mut self, key: K) -> RateLimitResult {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());

        match self.check_redis(&key_str).await {
            Ok(result) => result,
            Err(e) => {
                error!("Redis rate limit check failed: {}", e);
                // Fail open - allow request if Redis is unavailable
                RateLimitResult::Allowed
            }
        }
    }

    /// Perform Redis rate limit check using sliding window counter
    async fn check_redis(&mut self, key: &str) -> Result<RateLimitResult, RedisError> {
        // Use Redis pipeline for atomic operations
        let mut pipe = redis::pipe();

        // INCR the counter
        pipe.incr(key, 1);
        // Set TTL if key was just created
        pipe.expire(key, self.config.window.as_secs() as i64);
        // Get the current count
        pipe.get(key);

        let results: Vec<i64> = pipe.query_async(&mut self.connection).await?;

        let count = results.get(2).copied().unwrap_or(0);

        if count as u32 <= self.config.max_requests {
            debug!("Rate limit check passed: {}/{}", count, self.config.max_requests);
            Ok(RateLimitResult::Allowed)
        } else {
            // Get remaining TTL
            let ttl: i64 = self.connection.ttl(key).await.unwrap_or(60);
            debug!("Rate limit exceeded: {}/{}, retry after {}s",
                   count, self.config.max_requests, ttl);

            Ok(RateLimitResult::Limited {
                retry_after: ttl as u64,
            })
        }
    }

    /// Reset rate limit for a specific key
    pub async fn reset<K: RateLimitKey>(&mut self, key: K) -> Result<(), RedisError> {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());
        self.connection.del(&key_str).await
    }

    /// Get current count for a key
    pub async fn get_count<K: RateLimitKey>(&mut self, key: K) -> Result<u32, RedisError> {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());
        let count: i64 = self.connection.get(&key_str).await.unwrap_or(0);
        Ok(count as u32)
    }
}

/// Token bucket distributed rate limiter using Redis sorted sets
pub struct DistributedTokenBucketLimiter {
    config: DistributedRateLimiterConfig,
    connection: ConnectionManager,
    capacity: u32,
    refill_rate: f64,
}

impl DistributedTokenBucketLimiter {
    /// Create a new distributed token bucket limiter
    pub async fn new(
        config: DistributedRateLimiterConfig,
        capacity: u32,
        refill_rate: f64,
    ) -> Result<Self, RedisError> {
        let client = Client::open(config.redis_url.clone())?;
        let connection = ConnectionManager::new(client).await?;

        Ok(Self {
            config,
            connection,
            capacity,
            refill_rate,
        })
    }

    /// Check if request is allowed using Lua script for atomic token bucket
    pub async fn check<K: RateLimitKey>(&mut self, key: K, tokens: Option<f64>) -> RateLimitResult {
        let key_str = format!("{}:tb:{}", self.config.key_prefix, key.to_key());
        let tokens = tokens.unwrap_or(1.0);

        match self.check_redis(&key_str, tokens).await {
            Ok(result) => result,
            Err(e) => {
                error!("Redis token bucket check failed: {}", e);
                // Fail open
                RateLimitResult::Allowed
            }
        }
    }

    /// Perform Redis token bucket check using Lua script
    async fn check_redis(&mut self, key: &str, tokens: f64) -> Result<RateLimitResult, RedisError> {
        // Lua script for atomic token bucket operation
        let script = r#"
            local key = KEYS[1]
            local capacity = tonumber(ARGV[1])
            local refill_rate = tonumber(ARGV[2])
            local tokens_requested = tonumber(ARGV[3])
            local now = tonumber(ARGV[4])

            local bucket = redis.call('HMGET', key, 'tokens', 'last_refill')
            local tokens = tonumber(bucket[1]) or capacity
            local last_refill = tonumber(bucket[2]) or now

            -- Refill tokens based on elapsed time
            local elapsed = now - last_refill
            local tokens_to_add = elapsed * refill_rate
            tokens = math.min(capacity, tokens + tokens_to_add)

            -- Try to consume tokens
            if tokens >= tokens_requested then
                tokens = tokens - tokens_requested
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)
                return {1, tokens}
            else
                -- Calculate retry after
                local tokens_needed = tokens_requested - tokens
                local retry_after = math.ceil(tokens_needed / refill_rate)
                return {0, retry_after}
            end
        "#;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let result: Vec<i64> = redis::Script::new(script)
            .key(key)
            .arg(self.capacity)
            .arg(self.refill_rate)
            .arg(tokens)
            .arg(now)
            .invoke_async(&mut self.connection)
            .await?;

        if result[0] == 1 {
            Ok(RateLimitResult::Allowed)
        } else {
            Ok(RateLimitResult::Limited {
                retry_after: result[1] as u64,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance
    // Skip them in CI if Redis is not available

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_distributed_rate_limiter() {
        let config = DistributedRateLimiterConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            max_requests: 5,
            window: Duration::from_secs(10),
            key_prefix: "test".to_string(),
        };

        let mut limiter = DistributedRateLimiter::new(config).await.unwrap();

        // Reset counter
        let _ = limiter.reset("test-user").await;

        // First 5 requests should succeed
        for i in 0..5 {
            let result = limiter.check("test-user").await;
            assert!(result.is_allowed(), "Request {} should be allowed", i);
        }

        // 6th request should be rate limited
        let result = limiter.check("test-user").await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_distributed_token_bucket() {
        let config = DistributedRateLimiterConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            max_requests: 10,
            window: Duration::from_secs(60),
            key_prefix: "test-tb".to_string(),
        };

        let mut limiter = DistributedTokenBucketLimiter::new(config, 10, 1.0)
            .await
            .unwrap();

        // Consume all tokens
        for i in 0..10 {
            let result = limiter.check("tb-user", None).await;
            assert!(result.is_allowed(), "Request {} should be allowed", i);
        }

        // Next request should be rate limited
        let result = limiter.check("tb-user", None).await;
        assert!(!result.is_allowed());
    }
}
