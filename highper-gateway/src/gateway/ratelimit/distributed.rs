//! Distributed rate limiting with Redis
//!
//! Provides rate limiting across multiple proxy instances using Redis.
//!
//! Operator tunables (B4.2 — Workstream 0.C):
//! - `HIGHPER_RATELIMIT_KEY_SHARDS` — when `>1`, INCR lands on one of N
//!   shard sub-keys chosen randomly per request; the decision sums all N
//!   shards. Mitigates Valkey hot-key contention per `HA_ARCHITECTURE.md`
//!   §1.5.4 F1. Defaults to `1` (no sharding).
//! - `HIGHPER_RATELIMIT_REDIS_FAIL_MODE` — behaviour when Redis is
//!   unavailable: `local_fallback` (default; per-replica window counter),
//!   `fail_open` (allow), `fail_closed` (reject). Local fallback is
//!   best-effort — counters reset per replica and do not reconcile back
//!   into Redis when it returns.

use super::{RateLimitKey, RateLimitResult};
use dashmap::DashMap;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client, RedisError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, warn};

use crate::runtime_config::{self, RedisFailMode};

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

/// Per-replica window-counter fallback used when Redis is unreachable and
/// `redis_fail_mode == LocalFallback`. State does not reconcile back into
/// Redis when it returns; the budget window re-aligns at the next reset
/// boundary (acknowledged drift, mirrors UC16 §3.6.2 Valkey fallback).
#[derive(Debug)]
struct LocalBucket {
    count: u32,
    window_start: Instant,
}

/// Distributed rate limiter using Redis
pub struct DistributedRateLimiter {
    config: DistributedRateLimiterConfig,
    connection: ConnectionManager,
    local_fallback: Arc<DashMap<String, LocalBucket>>,
}

impl DistributedRateLimiter {
    /// Create a new distributed rate limiter
    pub async fn new(config: DistributedRateLimiterConfig) -> Result<Self, RedisError> {
        let client = Client::open(config.redis_url.clone())?;
        let connection = ConnectionManager::new(client).await?;

        Ok(Self {
            config,
            connection,
            local_fallback: Arc::new(DashMap::new()),
        })
    }

    /// Check if a request is allowed using Redis INCR and EXPIRE
    pub async fn check<K: RateLimitKey>(&mut self, key: K) -> RateLimitResult {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());

        match self.check_redis(&key_str).await {
            Ok(result) => result,
            Err(e) => self.on_redis_error(&key_str, e),
        }
    }

    /// Perform Redis rate limit check using sliding window counter.
    /// When `key_shards > 1`, write lands on one shard chosen randomly
    /// and the decision sums all shards.
    async fn check_redis(&mut self, base_key: &str) -> Result<RateLimitResult, RedisError> {
        let key_shards = current_key_shards();
        let window_secs = self.config.window.as_secs() as i64;

        if key_shards <= 1 {
            // Fast path — no sharding.
            let mut pipe = redis::pipe();
            pipe.incr(base_key, 1);
            pipe.expire(base_key, window_secs);
            pipe.get(base_key);

            let results: Vec<i64> = pipe.query_async(&mut self.connection).await?;
            let count = results.get(2).copied().unwrap_or(0);

            if count as u32 <= self.config.max_requests {
                debug!("Rate limit check passed: {}/{}", count, self.config.max_requests);
                Ok(RateLimitResult::Allowed)
            } else {
                let ttl: i64 = self.connection.ttl(base_key).await.unwrap_or(window_secs);
                debug!(
                    "Rate limit exceeded: {}/{}, retry after {}s",
                    count, self.config.max_requests, ttl
                );
                Ok(RateLimitResult::Limited { retry_after: ttl as u64 })
            }
        } else {
            // Sharded path — INCR one shard, sum all.
            let chosen = rand::random::<u32>() % key_shards;
            let chosen_key = format!("{}:{}", base_key, chosen);

            let mut pipe = redis::pipe();
            pipe.incr(&chosen_key, 1);
            pipe.expire(&chosen_key, window_secs);
            for s in 0..key_shards {
                pipe.get(format!("{}:{}", base_key, s));
            }
            let results: Vec<i64> = pipe.query_async(&mut self.connection).await?;
            // results[0] = INCR new value, results[1] = EXPIRE (1/0),
            // results[2..2+key_shards] = per-shard counts.
            let total: i64 = results
                .iter()
                .skip(2)
                .take(key_shards as usize)
                .sum();

            if total as u32 <= self.config.max_requests {
                debug!(
                    "Rate limit check passed (sharded x{}): {}/{}",
                    key_shards, total, self.config.max_requests
                );
                Ok(RateLimitResult::Allowed)
            } else {
                let ttl: i64 = self
                    .connection
                    .ttl(&chosen_key)
                    .await
                    .unwrap_or(window_secs);
                debug!(
                    "Rate limit exceeded (sharded x{}): {}/{}, retry after {}s",
                    key_shards, total, self.config.max_requests, ttl
                );
                Ok(RateLimitResult::Limited { retry_after: ttl as u64 })
            }
        }
    }

    /// Apply the operator-selected `HIGHPER_RATELIMIT_REDIS_FAIL_MODE`
    /// when a Redis call fails. Defaults to `local_fallback` when the
    /// runtime config is not yet installed (test/library paths).
    fn on_redis_error(&self, key: &str, err: RedisError) -> RateLimitResult {
        let mode = runtime_config::try_current()
            .map(|c| *c.ratelimit.redis_fail_mode.get())
            .unwrap_or(RedisFailMode::LocalFallback);

        match mode {
            RedisFailMode::FailOpen => {
                error!(error = %err, "Redis rate limit check failed; fail_open: allowing");
                RateLimitResult::Allowed
            }
            RedisFailMode::FailClosed => {
                error!(error = %err, "Redis rate limit check failed; fail_closed: rejecting");
                RateLimitResult::Limited {
                    retry_after: self.config.window.as_secs(),
                }
            }
            RedisFailMode::LocalFallback => {
                warn!(error = %err, "Redis rate limit check failed; local_fallback: best-effort per-replica counter");
                local_check(
                    &self.local_fallback,
                    key,
                    self.config.max_requests,
                    self.config.window,
                )
            }
        }
    }

    /// Reset rate limit for a specific key
    pub async fn reset<K: RateLimitKey>(&mut self, key: K) -> Result<(), RedisError> {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());
        let key_shards = current_key_shards();
        if key_shards <= 1 {
            self.connection.del(&key_str).await
        } else {
            let mut pipe = redis::pipe();
            for s in 0..key_shards {
                pipe.del(format!("{}:{}", key_str, s));
            }
            pipe.query_async(&mut self.connection).await
        }
    }

    /// Get current count for a key (sums all shards when sharding is on)
    pub async fn get_count<K: RateLimitKey>(&mut self, key: K) -> Result<u32, RedisError> {
        let key_str = format!("{}:{}", self.config.key_prefix, key.to_key());
        let key_shards = current_key_shards();
        if key_shards <= 1 {
            let count: i64 = self.connection.get(&key_str).await.unwrap_or(0);
            Ok(count as u32)
        } else {
            let mut pipe = redis::pipe();
            for s in 0..key_shards {
                pipe.get(format!("{}:{}", key_str, s));
            }
            let results: Vec<i64> = pipe.query_async(&mut self.connection).await?;
            Ok(results.iter().sum::<i64>() as u32)
        }
    }
}

/// Token bucket distributed rate limiter using Redis sorted sets
pub struct DistributedTokenBucketLimiter {
    config: DistributedRateLimiterConfig,
    connection: ConnectionManager,
    capacity: u32,
    refill_rate: f64,
    local_fallback: Arc<DashMap<String, LocalBucket>>,
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
            local_fallback: Arc::new(DashMap::new()),
        })
    }

    /// Check if request is allowed using Lua script for atomic token bucket
    pub async fn check<K: RateLimitKey>(&mut self, key: K, tokens: Option<f64>) -> RateLimitResult {
        let key_str = format!("{}:tb:{}", self.config.key_prefix, key.to_key());
        let tokens = tokens.unwrap_or(1.0);

        match self.check_redis(&key_str, tokens).await {
            Ok(result) => result,
            Err(e) => self.on_redis_error(&key_str, e),
        }
    }

    /// Perform Redis token bucket check using Lua script.
    ///
    /// Note: token-bucket sharding is not applied here — bucket state
    /// (`tokens`, `last_refill`) is order-sensitive and cannot be summed
    /// across shards without losing fairness. Hot-key mitigation for the
    /// token-bucket variant is a separate workstream (see B4
    /// follow-up: route-level pre-shard at the caller).
    async fn check_redis(&mut self, key: &str, tokens: f64) -> Result<RateLimitResult, RedisError> {
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

    /// Apply the operator-selected `HIGHPER_RATELIMIT_REDIS_FAIL_MODE`
    /// when a Redis call fails. Local fallback uses a window counter
    /// sized at `capacity` over `window` (degraded vs token bucket;
    /// acceptable as outage emergency).
    fn on_redis_error(&self, key: &str, err: RedisError) -> RateLimitResult {
        let mode = runtime_config::try_current()
            .map(|c| *c.ratelimit.redis_fail_mode.get())
            .unwrap_or(RedisFailMode::LocalFallback);

        match mode {
            RedisFailMode::FailOpen => {
                error!(error = %err, "Redis token bucket check failed; fail_open: allowing");
                RateLimitResult::Allowed
            }
            RedisFailMode::FailClosed => {
                error!(error = %err, "Redis token bucket check failed; fail_closed: rejecting");
                RateLimitResult::Limited {
                    retry_after: self.config.window.as_secs(),
                }
            }
            RedisFailMode::LocalFallback => {
                warn!(error = %err, "Redis token bucket check failed; local_fallback: best-effort per-replica window counter");
                local_check(
                    &self.local_fallback,
                    key,
                    self.capacity,
                    self.config.window,
                )
            }
        }
    }
}

/// Read `HIGHPER_RATELIMIT_KEY_SHARDS` via the runtime config singleton.
/// Defaults to 1 (no sharding) when not yet installed.
fn current_key_shards() -> u32 {
    runtime_config::try_current()
        .map(|c| c.ratelimit.key_shards)
        .unwrap_or(1)
        .max(1)
}

/// Per-replica window counter used for `local_fallback` when Redis is
/// unreachable. Pure (no I/O); shared by both limiter variants.
fn local_check(
    store: &DashMap<String, LocalBucket>,
    key: &str,
    max: u32,
    window: Duration,
) -> RateLimitResult {
    let now = Instant::now();
    let mut entry = store
        .entry(key.to_string())
        .or_insert(LocalBucket {
            count: 0,
            window_start: now,
        });

    if now.duration_since(entry.window_start) >= window {
        entry.count = 0;
        entry.window_start = now;
    }
    entry.count = entry.count.saturating_add(1);

    if entry.count <= max {
        RateLimitResult::Allowed
    } else {
        let elapsed = now.duration_since(entry.window_start);
        let retry_after = window.saturating_sub(elapsed).as_secs().max(1);
        RateLimitResult::Limited { retry_after }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_fallback_allows_under_max() {
        let store: DashMap<String, LocalBucket> = DashMap::new();
        for _ in 0..3 {
            let r = local_check(&store, "user:a", 5, Duration::from_secs(60));
            assert!(r.is_allowed(), "under-max requests should be allowed");
        }
    }

    #[test]
    fn local_fallback_limits_over_max() {
        let store: DashMap<String, LocalBucket> = DashMap::new();
        for _ in 0..2 {
            assert!(local_check(&store, "user:b", 2, Duration::from_secs(60)).is_allowed());
        }
        // Third request crosses max=2.
        let limited = local_check(&store, "user:b", 2, Duration::from_secs(60));
        assert!(!limited.is_allowed());
        assert!(limited.retry_after().unwrap() >= 1);
    }

    #[test]
    fn local_fallback_window_rolls_over() {
        let store: DashMap<String, LocalBucket> = DashMap::new();
        // Use a tiny window so the test rolls over within reasonable
        // sleep time without slowing CI.
        let win = Duration::from_millis(50);
        assert!(local_check(&store, "k", 1, win).is_allowed());
        assert!(!local_check(&store, "k", 1, win).is_allowed());
        std::thread::sleep(Duration::from_millis(70));
        // After the window elapses, the counter resets.
        assert!(local_check(&store, "k", 1, win).is_allowed());
    }

    #[test]
    fn current_key_shards_defaults_to_one_without_runtime_config() {
        // When runtime_config is not installed (or installed as default),
        // shards must be at least 1 and never zero.
        let shards = current_key_shards();
        assert!(shards >= 1);
    }

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
