//! Response caching for API gateway
//!
//! Provides local and distributed caching capabilities.

pub mod distributed;

use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

/// Cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Cached response body
    pub body: Bytes,
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: Vec<(String, String)>,
    /// When this entry was created
    pub created_at: Instant,
    /// Time-to-live
    pub ttl: Duration,
}

impl CacheEntry {
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Duration {
        let elapsed = self.created_at.elapsed();
        if elapsed < self.ttl {
            self.ttl - elapsed
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Local in-memory cache
pub struct LocalCache {
    entries: Arc<DashMap<String, CacheEntry>>,
    default_ttl: Duration,
}

impl LocalCache {
    /// Create a new local cache
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            default_ttl,
        }
    }

    /// Create with default TTL of 5 minutes
    pub fn default_cache() -> Self {
        Self::new(Duration::from_secs(300))
    }

    /// Get an entry from cache
    pub fn get(&self, key: &str) -> Option<CacheEntry> {
        match self.entries.get(key) {
            Some(entry) => {
                if entry.is_expired() {
                    drop(entry);
                    self.entries.remove(key);
                    debug!("Cache expired for key: {}", key);
                    None
                } else {
                    debug!("Cache hit for key: {}", key);
                    Some(entry.clone())
                }
            }
            None => {
                debug!("Cache miss for key: {}", key);
                None
            }
        }
    }

    /// Store an entry in cache
    pub fn set(&self, key: String, entry: CacheEntry) {
        debug!("Caching entry for key: {} (TTL: {:?})", key, entry.ttl);
        self.entries.insert(key, entry);
    }

    /// Remove an entry from cache
    pub fn remove(&self, key: &str) {
        self.entries.remove(key);
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Get number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all cache keys
    pub fn get_all_keys(&self) -> Vec<String> {
        self.entries.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Start cleanup task to remove expired entries
    pub fn start_cleanup_task(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);
            loop {
                cleanup_interval.tick().await;
                self.cleanup_expired();
            }
        });
    }

    /// Remove expired entries
    fn cleanup_expired(&self) {
        let mut removed = 0;
        self.entries.retain(|_, entry| {
            let keep = !entry.is_expired();
            if !keep {
                removed += 1;
            }
            keep
        });
        if removed > 0 {
            debug!("Cleaned up {} expired cache entries", removed);
        }
    }

    /// Generate cache key from request components
    pub fn generate_key(method: &str, uri: &str, headers: &[(String, String)]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        method.hash(&mut hasher);
        uri.hash(&mut hasher);
        for (name, value) in headers {
            name.hash(&mut hasher);
            value.hash(&mut hasher);
        }

        format!("{}:{}:{:x}", method, uri, hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_cache_basic() {
        let cache = LocalCache::default_cache();

        let entry = CacheEntry {
            body: Bytes::from("test response"),
            status: 200,
            headers: vec![],
            created_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        cache.set("key1".to_string(), entry);
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get("key1");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_cache_expiry() {
        let cache = LocalCache::default_cache();

        let entry = CacheEntry {
            body: Bytes::from("test response"),
            status: 200,
            headers: vec![],
            created_at: Instant::now(),
            ttl: Duration::from_millis(50),
        };

        cache.set("key1".to_string(), entry);

        // Should be cached
        assert!(cache.get("key1").is_some());

        // Wait for expiry
        sleep(Duration::from_millis(100));

        // Should be expired
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_cache_miss() {
        let cache = LocalCache::default_cache();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_cache_remove() {
        let cache = LocalCache::default_cache();

        let entry = CacheEntry {
            body: Bytes::from("test"),
            status: 200,
            headers: vec![],
            created_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        cache.set("key1".to_string(), entry);
        assert!(cache.get("key1").is_some());

        cache.remove("key1");
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_generate_key() {
        let key1 = LocalCache::generate_key("GET", "/api/users", &[]);
        let key2 = LocalCache::generate_key("GET", "/api/users", &[]);
        let key3 = LocalCache::generate_key("POST", "/api/users", &[]);

        // Same request should generate same key
        assert_eq!(key1, key2);
        // Different method should generate different key
        assert_ne!(key1, key3);
    }
}
