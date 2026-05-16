//! Rate limiting for API gateway
//!
//! Implements token bucket and sliding window algorithms for rate limiting.

pub mod distributed;
pub mod sliding_window;
pub mod token_bucket;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Rate limit result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed,
    /// Request is rate limited
    Limited {
        /// Time until reset in seconds
        retry_after: u64,
    },
}

impl RateLimitResult {
    /// Check if request is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed)
    }

    /// Get retry_after value if rate limited
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            RateLimitResult::Allowed => None,
            RateLimitResult::Limited { retry_after } => Some(*retry_after),
        }
    }
}

/// Rate limit key generator
pub trait RateLimitKey {
    /// Generate a key for rate limiting
    fn to_key(&self) -> String;
}

impl RateLimitKey for &str {
    fn to_key(&self) -> String {
        self.to_string()
    }
}

impl RateLimitKey for String {
    fn to_key(&self) -> String {
        self.clone()
    }
}

/// Generate hash from key
pub fn hash_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_result() {
        let allowed = RateLimitResult::Allowed;
        assert!(allowed.is_allowed());
        assert_eq!(allowed.retry_after(), None);

        let limited = RateLimitResult::Limited { retry_after: 60 };
        assert!(!limited.is_allowed());
        assert_eq!(limited.retry_after(), Some(60));
    }

    #[test]
    fn test_hash_key() {
        let key1 = "user123";
        let key2 = "user456";
        let key3 = "user123"; // Same as key1

        let hash1 = hash_key(key1);
        let hash2 = hash_key(key2);
        let hash3 = hash_key(key3);

        // Same keys should produce same hash
        assert_eq!(hash1, hash3);
        // Different keys should (usually) produce different hashes
        assert_ne!(hash1, hash2);
    }
}
