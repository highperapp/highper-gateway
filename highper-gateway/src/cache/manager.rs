//! Cache manager with unified interface
//!
//! Provides a high-level cache management interface that supports
//! multiple backend types through the adapter pattern.

use super::backend::{CacheBackend, CacheError, CacheStats};
use super::backends::{InMemoryBackend, MultiTierBackend, RedisBackend};
use super::disk::{DiskBackend, DiskCacheConfig, TieredBackend};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Cache backend type configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CacheBackendType {
    /// In-memory cache (local, fast, non-persistent)
    InMemory,

    /// Disk cache (persistent, survives restarts)
    Disk {
        /// Path to cache directory
        path: PathBuf,
        /// Maximum cache size in bytes
        #[serde(default = "default_disk_size")]
        max_size: u64,
        /// Minimum object size to cache (smaller objects skipped)
        #[serde(default)]
        min_object_size: usize,
        /// Enable compression
        #[serde(default)]
        compression: bool,
    },

    /// Tiered cache (memory hot tier + disk warm tier)
    Tiered {
        /// Path to disk cache directory
        disk_path: PathBuf,
        /// Maximum disk cache size in bytes
        #[serde(default = "default_disk_size")]
        disk_size: u64,
        /// Maximum size for entries in hot tier (memory)
        #[serde(default = "default_hot_max_size")]
        hot_max_size: usize,
        /// Enable compression for disk tier
        #[serde(default)]
        compression: bool,
    },

    /// Redis cache (distributed, persistent)
    Redis {
        /// Redis connection URL (e.g., "redis://localhost:6379")
        url: String,
    },

    /// Multi-tier cache (local + distributed)
    MultiTier {
        /// Distributed backend configuration
        distributed: Box<CacheBackendType>,
    },
}

fn default_disk_size() -> u64 {
    10 * 1024 * 1024 * 1024 // 10GB
}

fn default_hot_max_size() -> usize {
    1024 * 1024 // 1MB
}

impl Default for CacheBackendType {
    fn default() -> Self {
        CacheBackendType::InMemory
    }
}

/// Cache manager
///
/// Provides a unified interface for cache operations across different backends.
pub struct CacheManager {
    backend: Arc<dyn CacheBackend>,
}

impl CacheManager {
    /// Create a new cache manager with the specified backend
    pub async fn new(backend_type: CacheBackendType) -> Result<Self, CacheError> {
        let backend: Arc<dyn CacheBackend> = match backend_type {
            CacheBackendType::InMemory => Arc::new(InMemoryBackend::new()),

            CacheBackendType::Disk {
                path,
                max_size,
                min_object_size,
                compression,
            } => {
                let config = DiskCacheConfig {
                    path,
                    max_size,
                    min_object_size,
                    compression,
                    // allow: Stage 3 — disk-cache cleanup default; CacheRuntimeConfig::disk_cleanup_interval
                    cleanup_interval: Duration::from_secs(300),
                };
                Arc::new(DiskBackend::new(config).await?)
            }

            CacheBackendType::Tiered {
                disk_path,
                disk_size,
                hot_max_size,
                compression,
            } => {
                let disk_config = DiskCacheConfig {
                    path: disk_path,
                    max_size: disk_size,
                    min_object_size: 0,
                    compression,
                    // allow: Stage 3 — disk-cache cleanup default; CacheRuntimeConfig::disk_cleanup_interval
                    cleanup_interval: Duration::from_secs(300),
                };
                let disk = Arc::new(DiskBackend::new(disk_config).await?);
                // allow: Stage 3 — tiered hot-tier TTL; CacheRuntimeConfig::tiered_hot_ttl
                Arc::new(TieredBackend::new(disk, hot_max_size, Duration::from_secs(300)))
            }

            CacheBackendType::Redis { url } => {
                Arc::new(RedisBackend::new(&url).await?)
            }

            CacheBackendType::MultiTier { distributed } => {
                let dist_backend = Self::create_backend(*distributed).await?;
                Arc::new(MultiTierBackend::new(dist_backend))
            }
        };

        Ok(Self { backend })
    }

    /// Create a backend from configuration (internal helper)
    fn create_backend(backend_type: CacheBackendType) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Arc<dyn CacheBackend>, CacheError>> + Send>> {
        Box::pin(async move {
            match backend_type {
                CacheBackendType::InMemory => {
                    Ok(Arc::new(InMemoryBackend::new()) as Arc<dyn CacheBackend>)
                }

                CacheBackendType::Disk {
                    path,
                    max_size,
                    min_object_size,
                    compression,
                } => {
                    let config = DiskCacheConfig {
                        path,
                        max_size,
                        min_object_size,
                        compression,
                        // allow: Stage 3 — disk-cache cleanup default; CacheRuntimeConfig::disk_cleanup_interval
                        cleanup_interval: Duration::from_secs(300),
                    };
                    Ok(Arc::new(DiskBackend::new(config).await?) as Arc<dyn CacheBackend>)
                }

                CacheBackendType::Tiered {
                    disk_path,
                    disk_size,
                    hot_max_size,
                    compression,
                } => {
                    let disk_config = DiskCacheConfig {
                        path: disk_path,
                        max_size: disk_size,
                        min_object_size: 0,
                        compression,
                        // allow: Stage 3 — disk-cache cleanup default; CacheRuntimeConfig::disk_cleanup_interval
                        cleanup_interval: Duration::from_secs(300),
                    };
                    let disk = Arc::new(DiskBackend::new(disk_config).await?);
                    // allow: Stage 3 — tiered hot-tier TTL; CacheRuntimeConfig::tiered_hot_ttl
                    Ok(Arc::new(TieredBackend::new(disk, hot_max_size, Duration::from_secs(300))) as Arc<dyn CacheBackend>)
                }

                CacheBackendType::Redis { url } => {
                    Ok(Arc::new(RedisBackend::new(&url).await?) as Arc<dyn CacheBackend>)
                }

                CacheBackendType::MultiTier { distributed } => {
                    let dist_backend = Self::create_backend(*distributed).await?;
                    Ok(Arc::new(MultiTierBackend::new(dist_backend)) as Arc<dyn CacheBackend>)
                }
            }
        })
    }

    /// Get a value from cache
    ///
    /// Returns None if key doesn't exist or is expired.
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(bytes) = self.backend.get(key).await? {
            let value: T = serde_json::from_slice(&bytes)
                .map_err(|e| CacheError::Deserialization(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Get raw bytes from cache
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        self.backend.get(key).await
    }

    /// Set a value in cache
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| CacheError::Serialization(e.to_string()))?;
        self.backend.set(key, bytes, ttl).await
    }

    /// Set raw bytes in cache
    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        self.backend.set(key, value, ttl).await
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        self.backend.delete(key).await
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        self.backend.exists(key).await
    }

    /// Clear all entries
    pub async fn clear(&self) -> Result<(), CacheError> {
        self.backend.clear().await
    }

    /// Get cache statistics
    pub async fn stats(&self) -> Result<CacheStats, CacheError> {
        self.backend.stats().await
    }

    /// Get all keys matching a pattern
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        self.backend.keys(pattern).await
    }

    /// Delete all keys matching a pattern
    pub async fn delete_pattern(&self, pattern: &str) -> Result<usize, CacheError> {
        self.backend.delete_pattern(pattern).await
    }

    /// Set multiple key-value pairs
    pub async fn mset<T>(&self, entries: Vec<(&str, &T, Option<Duration>)>) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let mut byte_entries = Vec::with_capacity(entries.len());

        for (key, value, ttl) in entries {
            let bytes = serde_json::to_vec(value)
                .map_err(|e| CacheError::Serialization(e.to_string()))?;
            byte_entries.push((key, bytes, ttl));
        }

        self.backend.mset(byte_entries).await
    }

    /// Get multiple values
    pub async fn mget<T>(&self, keys: Vec<&str>) -> Result<Vec<Option<T>>, CacheError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let byte_results = self.backend.mget(keys).await?;
        let mut results = Vec::with_capacity(byte_results.len());

        for byte_opt in byte_results {
            if let Some(bytes) = byte_opt {
                let value: T = serde_json::from_slice(&bytes)
                    .map_err(|e| CacheError::Deserialization(e.to_string()))?;
                results.push(Some(value));
            } else {
                results.push(None);
            }
        }

        Ok(results)
    }

    /// Get backend name
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, CacheError> {
        self.backend.health_check().await
    }

    /// Get or set (lazy cache pattern)
    ///
    /// Gets value from cache, or executes the provided function and caches the result.
    pub async fn get_or_set<T, F, Fut>(&self, key: &str, ttl: Option<Duration>, f: F) -> Result<T, CacheError>
    where
        T: Serialize + for<'de> Deserialize<'de>,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, CacheError>>,
    {
        // Try to get from cache
        if let Some(value) = self.get::<T>(key).await? {
            return Ok(value);
        }

        // Cache miss - execute function
        let value = f().await?;

        // Store in cache
        self.set(key, &value, ttl).await?;

        Ok(value)
    }
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn test_in_memory_manager() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        // Test set and get
        manager.set("key1", &data, None).await.unwrap();
        let retrieved: Option<TestData> = manager.get("key1").await.unwrap();
        assert_eq!(retrieved, Some(data.clone()));

        // Test exists
        assert!(manager.exists("key1").await.unwrap());

        // Test delete
        assert!(manager.delete("key1").await.unwrap());
        assert!(!manager.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_stats() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        manager.set("key1", &data, None).await.unwrap();
        manager.set("key2", &data, None).await.unwrap();

        let stats = manager.stats().await.unwrap();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.backend_info, "InMemory (DashMap)");
    }

    #[tokio::test]
    async fn test_manager_keys() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        manager.set("user:1", &data, None).await.unwrap();
        manager.set("user:2", &data, None).await.unwrap();
        manager.set("post:1", &data, None).await.unwrap();

        let keys = manager.keys("user:*").await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_manager_mget_mset() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data1 = TestData { id: 1, name: "alice".to_string() };
        let data2 = TestData { id: 2, name: "bob".to_string() };

        // Test mset
        manager.mset(vec![
            ("key1", &data1, None),
            ("key2", &data2, None),
        ]).await.unwrap();

        // Test mget
        let results: Vec<Option<TestData>> = manager.mget(vec!["key1", "key2", "key3"]).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(data1));
        assert_eq!(results[1], Some(data2));
        assert_eq!(results[2], None);
    }

    #[tokio::test]
    async fn test_get_or_set() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        // First call should execute function
        let result = manager.get_or_set("key1", None, || async {
            Ok::<_, CacheError>(data.clone())
        }).await.unwrap();
        assert_eq!(result, data);

        // Second call should get from cache (function not executed)
        let result2 = manager.get_or_set("key1", None, || async {
            Ok::<_, CacheError>(TestData { id: 999, name: "should not be used".to_string() })
        }).await.unwrap();
        assert_eq!(result2, data); // Should still be original data
    }

    #[tokio::test]
    async fn test_health_check() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();
        assert!(manager.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_clear() {
        let manager = CacheManager::new(CacheBackendType::InMemory).await.unwrap();

        let data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        manager.set("key1", &data, None).await.unwrap();
        manager.set("key2", &data, None).await.unwrap();

        manager.clear().await.unwrap();

        let stats = manager.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
    }
}
