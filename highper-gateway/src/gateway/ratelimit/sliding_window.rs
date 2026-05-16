//! Sliding window rate limiting algorithm
//!
//! More accurate than fixed windows, prevents burst at window boundaries.

use super::{RateLimitKey, RateLimitResult};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

/// Sliding window configuration
#[derive(Debug, Clone)]
pub struct SlidingWindowConfig {
    /// Maximum number of requests in the window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
}

impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
        }
    }
}

/// Sliding window state
#[derive(Debug)]
struct WindowState {
    /// Timestamps of requests in the current window
    requests: VecDeque<Instant>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            requests: VecDeque::new(),
        }
    }

    /// Remove expired requests outside the window
    fn cleanup(&mut self, window: Duration) {
        let cutoff = Instant::now() - window;
        while let Some(&first) = self.requests.front() {
            if first < cutoff {
                self.requests.pop_front();
            } else {
                break;
            }
        }
    }

    /// Add a new request timestamp
    fn add_request(&mut self) {
        self.requests.push_back(Instant::now());
    }

    /// Get current request count
    fn count(&self) -> usize {
        self.requests.len()
    }

    /// Get time until oldest request expires
    fn time_until_reset(&self, window: Duration) -> Duration {
        if let Some(&oldest) = self.requests.front() {
            let elapsed = oldest.elapsed();
            if elapsed < window {
                window - elapsed
            } else {
                Duration::from_secs(0)
            }
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Sliding window rate limiter
pub struct SlidingWindowLimiter {
    config: SlidingWindowConfig,
    windows: Arc<DashMap<String, WindowState>>,
}

impl SlidingWindowLimiter {
    /// Create a new sliding window rate limiter
    pub fn new(config: SlidingWindowConfig) -> Self {
        Self {
            config,
            windows: Arc::new(DashMap::new()),
        }
    }

    /// Create with default configuration
    pub fn default_limiter() -> Self {
        Self::new(SlidingWindowConfig::default())
    }

    /// Check if a request is allowed
    pub fn check<K: RateLimitKey>(&self, key: K) -> RateLimitResult {
        let key_str = key.to_key();

        // Get or create window for this key
        let mut window_ref = self
            .windows
            .entry(key_str.clone())
            .or_insert_with(WindowState::new);

        // Clean up expired requests
        window_ref.cleanup(self.config.window);

        // Check if we're under the limit
        if window_ref.count() < self.config.max_requests as usize {
            window_ref.add_request();
            debug!(
                "Rate limit check passed for key: {} ({}/{})",
                key_str,
                window_ref.count(),
                self.config.max_requests
            );
            RateLimitResult::Allowed
        } else {
            let retry_after = window_ref.time_until_reset(self.config.window);
            debug!(
                "Rate limit exceeded for key: {}, retry after: {:?}",
                key_str, retry_after
            );
            RateLimitResult::Limited {
                retry_after: retry_after.as_secs().max(1),
            }
        }
    }

    /// Get current request count for a key
    pub fn get_count<K: RateLimitKey>(&self, key: K) -> usize {
        let key_str = key.to_key();
        self.windows.get(&key_str).map(|w| w.count()).unwrap_or(0)
    }

    /// Reset rate limit for a specific key
    pub fn reset<K: RateLimitKey>(&self, key: K) {
        let key_str = key.to_key();
        self.windows.remove(&key_str);
    }

    /// Clear all rate limit data
    pub fn clear_all(&self) {
        self.windows.clear();
    }

    /// Get number of tracked keys
    pub fn key_count(&self) -> usize {
        self.windows.len()
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

    /// Remove entries with no requests in the window
    fn cleanup_old_entries(&self) {
        self.windows.retain(|_, window| window.count() > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_sliding_window_basic() {
        let config = SlidingWindowConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
        };

        let limiter = SlidingWindowLimiter::new(config);

        // First 5 requests should succeed
        for i in 0..5 {
            let result = limiter.check("user1");
            assert!(result.is_allowed(), "Request {} should be allowed", i);
        }

        assert_eq!(limiter.get_count("user1"), 5);

        // 6th request should be rate limited
        let result = limiter.check("user1");
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_sliding_window_cleanup() {
        let config = SlidingWindowConfig {
            max_requests: 5,
            window: Duration::from_millis(100),
        };

        let limiter = SlidingWindowLimiter::new(config);

        // Make 5 requests
        for _ in 0..5 {
            limiter.check("user1");
        }

        // Should be rate limited
        let result = limiter.check("user1");
        assert!(!result.is_allowed());

        // Wait for window to expire
        sleep(Duration::from_millis(150));

        // Should be allowed again
        let result = limiter.check("user1");
        assert!(result.is_allowed());
    }

    #[test]
    fn test_sliding_window_multiple_keys() {
        let limiter = SlidingWindowLimiter::default_limiter();

        // Different keys should have independent limits
        let result1 = limiter.check("user1");
        let result2 = limiter.check("user2");

        assert!(result1.is_allowed());
        assert!(result2.is_allowed());
        assert_eq!(limiter.key_count(), 2);
    }

    #[test]
    fn test_sliding_window_reset() {
        let config = SlidingWindowConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
        };

        let limiter = SlidingWindowLimiter::new(config);

        // Make 3 requests
        for _ in 0..3 {
            limiter.check("user1");
        }

        // Should be rate limited
        let result = limiter.check("user1");
        assert!(!result.is_allowed());

        // Reset
        limiter.reset("user1");

        // Should be allowed again
        let result = limiter.check("user1");
        assert!(result.is_allowed());
    }

    #[test]
    fn test_sliding_window_partial_expiry() {
        let config = SlidingWindowConfig {
            max_requests: 3,
            window: Duration::from_millis(100),
        };

        let limiter = SlidingWindowLimiter::new(config);

        // Make 2 requests
        limiter.check("user1");
        limiter.check("user1");

        // Wait a bit
        sleep(Duration::from_millis(60));

        // Make 1 more request (total 3)
        limiter.check("user1");

        // Should be rate limited
        let result = limiter.check("user1");
        assert!(!result.is_allowed());

        // Wait for first 2 requests to expire
        sleep(Duration::from_millis(50));

        // Should have space for more requests now
        let result = limiter.check("user1");
        assert!(result.is_allowed());
    }

    #[test]
    fn test_sliding_window_get_count() {
        let config = SlidingWindowConfig {
            max_requests: 10,
            window: Duration::from_secs(60),
        };

        let limiter = SlidingWindowLimiter::new(config);

        assert_eq!(limiter.get_count("user1"), 0);

        limiter.check("user1");
        assert_eq!(limiter.get_count("user1"), 1);

        limiter.check("user1");
        limiter.check("user1");
        assert_eq!(limiter.get_count("user1"), 3);
    }
}
