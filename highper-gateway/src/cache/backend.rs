//! Cache backend trait and common types
//!
//! Defines the abstraction layer for different cache implementations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use thiserror::Error;

/// Cache entry with value and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    /// Cached value
    pub value: T,

    /// Time-to-live (optional)
    pub ttl: Option<Duration>,

    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry
    pub fn new(value: T, ttl: Option<Duration>) -> Self {
        Self {
            value,
            ttl,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            now > self.created_at + ttl.as_secs()
        } else {
            false
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of entries
    pub total_entries: usize,

    /// Number of active (non-expired) entries
    pub active_entries: usize,

    /// Number of expired entries
    pub expired_entries: usize,

    /// Hit rate (0.0 to 1.0)
    pub hit_rate: f64,

    /// Total hits
    pub hits: u64,

    /// Total misses
    pub misses: u64,

    /// Backend-specific info
    pub backend_info: String,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            total_entries: 0,
            active_entries: 0,
            expired_entries: 0,
            hit_rate: 0.0,
            hits: 0,
            misses: 0,
            backend_info: String::new(),
        }
    }
}

/// Cache errors
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid TTL: {0}")]
    InvalidTtl(String),

    #[error("Operation timeout")]
    Timeout,

    #[error("Other error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for CacheError {
    fn from(err: serde_json::Error) -> Self {
        CacheError::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for CacheError {
    fn from(err: bincode::Error) -> Self {
        CacheError::Serialization(err.to_string())
    }
}

/// Cache backend trait
///
/// All cache backends must implement this trait to provide a unified interface.
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// Get a value from the cache
    ///
    /// Returns None if the key doesn't exist or the entry is expired.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    /// Set a value in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    /// * `value` - Value to cache (as bytes)
    /// * `ttl` - Time-to-live (None = no expiration)
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError>;

    /// Delete a value from the cache
    async fn delete(&self, key: &str) -> Result<bool, CacheError>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;

    /// Clear all entries from the cache
    async fn clear(&self) -> Result<(), CacheError>;

    /// Get cache statistics
    async fn stats(&self) -> Result<CacheStats, CacheError>;

    /// Get all keys matching a pattern
    ///
    /// Pattern syntax depends on backend (e.g., Redis glob patterns)
    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError>;

    /// Delete all keys matching a pattern
    async fn delete_pattern(&self, pattern: &str) -> Result<usize, CacheError> {
        let keys = self.keys(pattern).await?;
        let mut deleted = 0;

        for key in keys {
            if self.delete(&key).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// Set multiple key-value pairs atomically
    async fn mset(&self, entries: Vec<(&str, Vec<u8>, Option<Duration>)>) -> Result<(), CacheError> {
        for (key, value, ttl) in entries {
            self.set(key, value, ttl).await?;
        }
        Ok(())
    }

    /// Get multiple values at once
    async fn mget(&self, keys: Vec<&str>) -> Result<Vec<Option<Vec<u8>>>, CacheError> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }

    /// Get backend name
    fn name(&self) -> &str;

    /// Health check
    async fn health_check(&self) -> Result<bool, CacheError> {
        // Default implementation: try to set and get a test key
        let test_key = "__health_check__";
        let test_value = b"ok".to_vec();

        // allow: Stage 3 — health-check TTL; could move to CacheRuntimeConfig::health_check_ttl
        self.set(test_key, test_value.clone(), Some(Duration::from_secs(5))).await?;
        let result = self.get(test_key).await?;
        self.delete(test_key).await?;

        Ok(result == Some(test_value))
    }
}

/// Cache backend with typed get/set methods
///
/// Provides convenient typed access on top of the raw byte-based CacheBackend trait.
pub struct TypedCache<B: CacheBackend> {
    backend: B,
}

impl<B: CacheBackend> TypedCache<B> {
    /// Create a new typed cache wrapper
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Get a typed value from cache
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>, CacheError> {
        if let Some(bytes) = self.backend.get(key).await? {
            let value: T = serde_json::from_slice(&bytes)
                .map_err(|e| CacheError::Deserialization(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Set a typed value in cache
    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError> {
        let bytes = serde_json::to_vec(value)?;
        self.backend.set(key, bytes, ttl).await
    }

    /// Access the underlying backend
    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: CacheBackend> fmt::Debug for TypedCache<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedCache")
            .field("backend", &self.backend.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_not_expired() {
        let entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(300)));
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_no_ttl() {
        let entry = CacheEntry::new("test_value".to_string(), None);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_expired() {
        let mut entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(1)));
        // Manually set created_at to the past
        entry.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 10; // 10 seconds ago

        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }
}
