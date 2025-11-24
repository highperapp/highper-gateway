//! GraphQL query cache
//!
//! Caches GraphQL query results with TTL support

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

use super::GraphQLResponse;

/// Cache entry for GraphQL responses
#[derive(Debug, Clone)]
struct CacheEntry {
    response: GraphQLResponse,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

/// Query cache for GraphQL responses
pub struct QueryCache {
    entries: Arc<DashMap<String, CacheEntry>>,
    default_ttl: Duration,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new(default_ttl: Duration) -> Self {
        let cache = Self {
            entries: Arc::new(DashMap::new()),
            default_ttl,
        };

        // Spawn background task to clean expired entries
        let entries = cache.entries.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                entries.retain(|_, entry| !entry.is_expired());
            }
        });

        cache
    }

    /// Get cached response
    pub async fn get(&self, key: &str) -> Option<GraphQLResponse> {
        if let Some(entry) = self.entries.get(key) {
            if !entry.is_expired() {
                debug!("Cache hit for key: {}", key);
                return Some(entry.response.clone());
            } else {
                debug!("Cache entry expired for key: {}", key);
                drop(entry);
                self.entries.remove(key);
            }
        }
        None
    }

    /// Set cached response
    pub async fn set(&self, key: String, response: GraphQLResponse) {
        self.set_with_ttl(key, response, self.default_ttl).await;
    }

    /// Set cached response with custom TTL
    pub async fn set_with_ttl(&self, key: String, response: GraphQLResponse, ttl: Duration) {
        let entry = CacheEntry {
            response,
            inserted_at: Instant::now(),
            ttl,
        };

        self.entries.insert(key, entry);
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &str) {
        self.entries.remove(key);
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        self.entries.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_entries = self.entries.len();
        let expired_entries = self.entries.iter().filter(|e| e.is_expired()).count();

        CacheStats {
            total_entries,
            active_entries: total_entries - expired_entries,
            expired_entries,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::graphql::GraphQLResponse;

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let cache = QueryCache::new(Duration::from_secs(300));

        let response = GraphQLResponse {
            data: Some(serde_json::json!({"hello": "world"})),
            errors: None,
        };

        cache.set("test_key".to_string(), response.clone()).await;

        let cached = cache.get("test_key").await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = QueryCache::new(Duration::from_millis(100));

        let response = GraphQLResponse {
            data: Some(serde_json::json!({"hello": "world"})),
            errors: None,
        };

        cache.set("test_key".to_string(), response).await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        let cached = cache.get("test_key").await;
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = QueryCache::new(Duration::from_secs(300));

        let response = GraphQLResponse {
            data: Some(serde_json::json!({"hello": "world"})),
            errors: None,
        };

        cache.set("test_key".to_string(), response).await;
        cache.invalidate("test_key").await;

        let cached = cache.get("test_key").await;
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = QueryCache::new(Duration::from_secs(300));

        let response = GraphQLResponse {
            data: Some(serde_json::json!({"hello": "world"})),
            errors: None,
        };

        cache.set("test_key1".to_string(), response.clone()).await;
        cache.set("test_key2".to_string(), response).await;

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.active_entries, 2);
    }
}
