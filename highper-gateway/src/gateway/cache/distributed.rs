//! Distributed caching with Redis
//!
//! Provides response caching across multiple proxy instances using Redis.

use super::CacheEntry;
use bytes::Bytes;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client, RedisError};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error};

/// Serializable cache entry for Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableCacheEntry {
    body: Vec<u8>,
    status: u16,
    headers: Vec<(String, String)>,
    created_at: u64,
    ttl_seconds: u64,
}

impl From<&CacheEntry> for SerializableCacheEntry {
    fn from(entry: &CacheEntry) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            body: entry.body.to_vec(),
            status: entry.status,
            headers: entry.headers.clone(),
            created_at,
            ttl_seconds: entry.ttl.as_secs(),
        }
    }
}

impl SerializableCacheEntry {
    fn to_cache_entry(&self) -> CacheEntry {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Calculate creation time based on current time and original creation
        let elapsed = now.saturating_sub(self.created_at);
        let created_at = Instant::now() - Duration::from_secs(elapsed);

        CacheEntry {
            body: Bytes::from(self.body.clone()),
            status: self.status,
            headers: self.headers.clone(),
            created_at,
            ttl: Duration::from_secs(self.ttl_seconds),
        }
    }
}

/// Distributed cache configuration
#[derive(Debug, Clone)]
pub struct DistributedCacheConfig {
    /// Redis connection string
    pub redis_url: String,
    /// Default TTL for cache entries
    pub default_ttl: Duration,
    /// Key prefix for Redis keys
    pub key_prefix: String,
    /// Enable compression for cached data
    pub compression: bool,
}

impl Default for DistributedCacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            default_ttl: Duration::from_secs(300),
            key_prefix: "cache".to_string(),
            compression: true,
        }
    }
}

/// Distributed cache using Redis
pub struct DistributedCache {
    config: DistributedCacheConfig,
    connection: ConnectionManager,
}

impl DistributedCache {
    /// Create a new distributed cache
    pub async fn new(config: DistributedCacheConfig) -> Result<Self, RedisError> {
        let client = Client::open(config.redis_url.clone())?;
        let connection = ConnectionManager::new(client).await?;

        Ok(Self { config, connection })
    }

    /// Get an entry from cache
    pub async fn get(&mut self, key: &str) -> Option<CacheEntry> {
        let redis_key = format!("{}:{}", self.config.key_prefix, key);

        match self.get_from_redis(&redis_key).await {
            Ok(Some(entry)) => {
                debug!("Distributed cache hit for key: {}", key);
                Some(entry)
            }
            Ok(None) => {
                debug!("Distributed cache miss for key: {}", key);
                None
            }
            Err(e) => {
                error!("Failed to get from Redis cache: {}", e);
                None
            }
        }
    }

    /// Get entry from Redis
    async fn get_from_redis(&mut self, key: &str) -> Result<Option<CacheEntry>, RedisError> {
        let data: Option<Vec<u8>> = self.connection.get(key).await?;

        match data {
            Some(bytes) => {
                let decompressed = if self.config.compression {
                    self.decompress(&bytes)
                } else {
                    bytes
                };

                match serde_json::from_slice::<SerializableCacheEntry>(&decompressed) {
                    Ok(serializable) => Ok(Some(serializable.to_cache_entry())),
                    Err(e) => {
                        error!("Failed to deserialize cache entry: {}", e);
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    /// Store an entry in cache
    pub async fn set(&mut self, key: String, entry: CacheEntry) -> Result<(), RedisError> {
        let redis_key = format!("{}:{}", self.config.key_prefix, key);

        debug!("Caching entry for key: {} (TTL: {:?})", key, entry.ttl);

        self.set_to_redis(&redis_key, &entry).await
    }

    /// Set entry to Redis
    async fn set_to_redis(&mut self, key: &str, entry: &CacheEntry) -> Result<(), RedisError> {
        let serializable = SerializableCacheEntry::from(entry);

        let json = serde_json::to_vec(&serializable).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Failed to serialize cache entry",
                e.to_string(),
            ))
        })?;

        let data = if self.config.compression {
            self.compress(&json)
        } else {
            json
        };

        // Set with TTL
        self.connection
            .set_ex(key, data, entry.ttl.as_secs())
            .await
    }

    /// Remove an entry from cache
    pub async fn remove(&mut self, key: &str) -> Result<(), RedisError> {
        let redis_key = format!("{}:{}", self.config.key_prefix, key);
        self.connection.del(&redis_key).await
    }

    /// Clear all cache entries (dangerous!)
    pub async fn clear(&mut self) -> Result<(), RedisError> {
        // Use SCAN to find all keys with our prefix
        let pattern = format!("{}:*", self.config.key_prefix);
        let mut cursor = 0u64;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut self.connection)
                .await?;

            if !keys.is_empty() {
                let _: () = self.connection.del(&keys).await?;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }

    /// Compress data using zstd
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        zstd::encode_all(data, 3).unwrap_or_else(|_| data.to_vec())
    }

    /// Decompress data using zstd
    fn decompress(&self, data: &[u8]) -> Vec<u8> {
        zstd::decode_all(data).unwrap_or_else(|_| data.to_vec())
    }

    /// Get cache statistics
    pub async fn stats(&mut self) -> Result<CacheStats, RedisError> {
        let pattern = format!("{}:*", self.config.key_prefix);

        // Count keys using SCAN
        let mut cursor = 0u64;
        let mut total_keys = 0usize;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut self.connection)
                .await?;

            total_keys += keys.len();
            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(CacheStats {
            total_entries: total_keys,
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_distributed_cache_basic() {
        let config = DistributedCacheConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            default_ttl: Duration::from_secs(300),
            key_prefix: "test-cache".to_string(),
            compression: false,
        };

        let mut cache = DistributedCache::new(config).await.unwrap();

        let entry = CacheEntry {
            body: Bytes::from("test response body"),
            status: 200,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            created_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        // Store entry
        cache.set("test-key".to_string(), entry.clone()).await.unwrap();

        // Retrieve entry
        let retrieved = cache.get("test-key").await;
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.status, 200);
        assert_eq!(retrieved.body, Bytes::from("test response body"));

        // Clean up
        cache.remove("test-key").await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_distributed_cache_miss() {
        let config = DistributedCacheConfig::default();
        let mut cache = DistributedCache::new(config).await.unwrap();

        let result = cache.get("nonexistent-key").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_distributed_cache_compression() {
        let config = DistributedCacheConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            default_ttl: Duration::from_secs(300),
            key_prefix: "test-cache-comp".to_string(),
            compression: true,
        };

        let mut cache = DistributedCache::new(config).await.unwrap();

        // Create a large response body
        let large_body = "x".repeat(10000);
        let entry = CacheEntry {
            body: Bytes::from(large_body.clone()),
            status: 200,
            headers: vec![],
            created_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };

        cache.set("compress-key".to_string(), entry).await.unwrap();

        let retrieved = cache.get("compress-key").await;
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.body, Bytes::from(large_body));

        cache.remove("compress-key").await.unwrap();
    }
}
