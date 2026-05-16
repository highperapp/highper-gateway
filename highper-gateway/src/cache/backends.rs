//! Cache backend implementations
//!
//! Provides concrete implementations of the CacheBackend trait:
//! - InMemoryBackend: Fast local cache using DashMap
//! - RedisBackend: Distributed cache using Redis
//! - MemcachedBackend: Distributed cache using Memcached
//! - MultiTierBackend: Combines local + distributed for optimal performance

use super::backend::{CacheBackend, CacheError, CacheStats};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// In-memory cache backend using DashMap
pub struct InMemoryBackend {
    /// Cache entries
    entries: Arc<DashMap<String, CacheValue>>,

    /// Hit counter
    hits: Arc<AtomicU64>,

    /// Miss counter
    misses: Arc<AtomicU64>,
}

/// Cache value with expiration
#[derive(Debug, Clone)]
struct CacheValue {
    data: Vec<u8>,
    expires_at: Option<u128>, // Unix timestamp in milliseconds
}

impl CacheValue {
    fn new(data: Vec<u8>, ttl: Option<Duration>) -> Self {
        let expires_at = ttl.map(|duration| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                + duration.as_millis()
        });

        Self { data, expires_at }
    }

    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            now >= expires_at
        } else {
            false
        }
    }
}

impl InMemoryBackend {
    /// Create a new in-memory cache backend
    pub fn new() -> Self {
        let backend = Self {
            entries: Arc::new(DashMap::new()),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        };

        // Spawn background cleanup task
        let entries = backend.entries.clone();
        tokio::spawn(async move {
            let cleanup_secs = *crate::runtime_config::current()
                .cache
                .cleanup_interval_secs
                .get();
            let mut interval = tokio::time::interval(Duration::from_secs(cleanup_secs));
            loop {
                interval.tick().await;
                entries.retain(|_, value| !value.is_expired());
            }
        });

        backend
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CacheBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                debug!("Cache entry expired: {}", key);
                drop(entry);
                self.entries.remove(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            } else {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(entry.data.clone()))
            }
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        }
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let cache_value = CacheValue::new(value, ttl);
        self.entries.insert(key.to_string(), cache_value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        Ok(self.entries.remove(key).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        if let Some(entry) = self.entries.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.entries.clear();
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        let total = self.entries.len();
        let expired = self.entries.iter().filter(|e| e.is_expired()).count();
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(CacheStats {
            total_entries: total,
            active_entries: total - expired,
            expired_entries: expired,
            hit_rate,
            hits,
            misses,
            backend_info: format!("InMemory (DashMap)"),
        })
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        let keys: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| {
                if pattern == "*" {
                    true
                } else {
                    // Simple pattern matching (contains)
                    entry.key().contains(pattern.trim_matches('*'))
                }
            })
            .map(|entry| entry.key().clone())
            .collect();

        Ok(keys)
    }

    fn name(&self) -> &str {
        "InMemory"
    }
}

/// Redis cache backend
pub struct RedisBackend {
    /// Redis connection pool
    pool: Arc<redis::aio::ConnectionManager>,

    /// Hit counter
    hits: Arc<AtomicU64>,

    /// Miss counter
    misses: Arc<AtomicU64>,
}

impl RedisBackend {
    /// Create a new Redis cache backend
    pub async fn new(redis_url: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| CacheError::Connection(format!("Failed to create Redis client: {}", e)))?;

        let conn = client
            .get_connection_manager()
            .await
            .map_err(|e| CacheError::Connection(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            pool: Arc::new(conn),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        })
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        use redis::AsyncCommands;

        let mut conn = (*self.pool).clone();
        let result: Option<Vec<u8>> = conn
            .get(key)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis GET error: {}", e)))?;

        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        Ok(result)
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        use redis::AsyncCommands;

        let mut conn = (*self.pool).clone();

        if let Some(ttl) = ttl {
            conn.set_ex::<_, _, ()>(key, value, ttl.as_secs())
                .await
                .map_err(|e| CacheError::Backend(format!("Redis SETEX error: {}", e)))?;
        } else {
            conn.set::<_, _, ()>(key, value)
                .await
                .map_err(|e| CacheError::Backend(format!("Redis SET error: {}", e)))?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        use redis::AsyncCommands;

        let mut conn = (*self.pool).clone();
        let count: u32 = conn
            .del(key)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis DEL error: {}", e)))?;

        Ok(count > 0)
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        use redis::AsyncCommands;

        let mut conn = (*self.pool).clone();
        let exists: bool = conn
            .exists(key)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis EXISTS error: {}", e)))?;

        Ok(exists)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut conn = (*self.pool).clone();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis FLUSHDB error: {}", e)))?;

        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        let mut conn = (*self.pool).clone();
        let db_size: usize = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis DBSIZE error: {}", e)))?;

        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(CacheStats {
            total_entries: db_size,
            active_entries: db_size, // Redis handles expiration automatically
            expired_entries: 0,
            hit_rate,
            hits,
            misses,
            backend_info: format!("Redis"),
        })
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        use redis::AsyncCommands;

        let mut conn = (*self.pool).clone();
        let keys: Vec<String> = conn
            .keys(pattern)
            .await
            .map_err(|e| CacheError::Backend(format!("Redis KEYS error: {}", e)))?;

        Ok(keys)
    }

    fn name(&self) -> &str {
        "Redis"
    }
}

/// Multi-tier cache backend (local + distributed)
///
/// Provides L1 (local) and L2 (distributed) caching for optimal performance.
pub struct MultiTierBackend {
    /// L1 cache (local, fast)
    local: Arc<InMemoryBackend>,

    /// L2 cache (distributed, shared)
    distributed: Arc<dyn CacheBackend>,
}

impl MultiTierBackend {
    /// Create a new multi-tier cache
    pub fn new(distributed: Arc<dyn CacheBackend>) -> Self {
        Self {
            local: Arc::new(InMemoryBackend::new()),
            distributed,
        }
    }
}

#[async_trait]
impl CacheBackend for MultiTierBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        // Try L1 first
        if let Some(value) = self.local.get(key).await? {
            debug!("L1 cache hit: {}", key);
            return Ok(Some(value));
        }

        // Try L2
        if let Some(value) = self.distributed.get(key).await? {
            debug!("L2 cache hit: {}", key);
            // Backfill L1
            let l1_ttl = *crate::runtime_config::current()
                .cache
                .multi_tier_l1_ttl_secs
                .get();
            self.local
                .set(key, value.clone(), Some(Duration::from_secs(l1_ttl)))
                .await?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        // Set in both L1 and L2
        let l1_max_secs = *crate::runtime_config::current()
            .cache
            .multi_tier_l1_max_ttl_secs
            .get();
        let l1_ttl = ttl.map(|t| t.min(Duration::from_secs(l1_max_secs))); // L1 cap configurable
        self.local.set(key, value.clone(), l1_ttl).await?;
        self.distributed.set(key, value, ttl).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        // Delete from both tiers
        let l1_deleted = self.local.delete(key).await?;
        let l2_deleted = self.distributed.delete(key).await?;
        Ok(l1_deleted || l2_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        // Check L1 first
        if self.local.exists(key).await? {
            return Ok(true);
        }

        // Check L2
        self.distributed.exists(key).await
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.local.clear().await?;
        self.distributed.clear().await?;
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        let l1_stats = self.local.stats().await?;
        let l2_stats = self.distributed.stats().await?;

        Ok(CacheStats {
            total_entries: l1_stats.total_entries + l2_stats.total_entries,
            active_entries: l1_stats.active_entries + l2_stats.active_entries,
            expired_entries: l1_stats.expired_entries + l2_stats.expired_entries,
            hit_rate: (l1_stats.hit_rate + l2_stats.hit_rate) / 2.0,
            hits: l1_stats.hits + l2_stats.hits,
            misses: l1_stats.misses + l2_stats.misses,
            backend_info: format!(
                "MultiTier (L1: {}, L2: {})",
                self.local.name(),
                self.distributed.name()
            ),
        })
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        // Get keys from distributed cache (source of truth)
        self.distributed.keys(pattern).await
    }

    fn name(&self) -> &str {
        "MultiTier"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_backend() {
        let cache = InMemoryBackend::new();

        // Test set and get
        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(cache.exists("key1").await.unwrap());
        assert!(!cache.exists("nonexistent").await.unwrap());

        // Test delete
        assert!(cache.delete("key1").await.unwrap());
        assert!(!cache.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_expiration() {
        let cache = InMemoryBackend::new();

        cache
            .set("key1", b"value1".to_vec(), Some(Duration::from_millis(100)))
            .await
            .unwrap();

        // Should exist initially
        assert!(cache.exists("key1").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        let value = cache.get("key1").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_stats() {
        let cache = InMemoryBackend::new();

        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        cache.set("key2", b"value2".to_vec(), None).await.unwrap();

        let _ = cache.get("key1").await; // Hit
        let _ = cache.get("key3").await; // Miss

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_in_memory_keys() {
        let cache = InMemoryBackend::new();

        cache.set("user:1", b"alice".to_vec(), None).await.unwrap();
        cache.set("user:2", b"bob".to_vec(), None).await.unwrap();
        cache.set("post:1", b"hello".to_vec(), None).await.unwrap();

        let keys = cache.keys("user:*").await.unwrap();
        assert_eq!(keys.len(), 2);

        let all_keys = cache.keys("*").await.unwrap();
        assert_eq!(all_keys.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_clear() {
        let cache = InMemoryBackend::new();

        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        cache.set("key2", b"value2".to_vec(), None).await.unwrap();

        cache.clear().await.unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
    }
}
