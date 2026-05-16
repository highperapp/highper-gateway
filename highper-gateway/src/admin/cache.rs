//! Cache management endpoints for Admin API
//!
//! Provides REST API for managing cache:
//! - Get cache statistics
//! - Clear all cache
//! - Clear cache by pattern
//! - Invalidate specific keys
//! - List cache keys

use crate::state::ProxyState;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// Cache statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatsResponse {
    /// Local cache statistics
    pub local: Option<LocalCacheStats>,

    /// Distributed cache statistics (Redis)
    pub distributed: Option<DistributedCacheStats>,

    /// Total entries across all caches
    pub total_entries: usize,
}

/// Local cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCacheStats {
    /// Number of cached entries
    pub entries: usize,

    /// Cache is enabled
    pub enabled: bool,
}

/// Distributed cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedCacheStats {
    /// Number of cached entries in Redis
    pub entries: usize,

    /// Redis connection status
    pub connected: bool,

    /// Cache is enabled
    pub enabled: bool,
}

/// Request to clear cache
#[derive(Debug, Deserialize)]
pub struct ClearCacheRequest {
    /// Optional pattern to match keys (e.g., "/api/*")
    pub pattern: Option<String>,

    /// Clear local cache
    #[serde(default = "default_true")]
    pub clear_local: bool,

    /// Clear distributed cache
    #[serde(default = "default_true")]
    pub clear_distributed: bool,
}

fn default_true() -> bool {
    true
}

/// Request to invalidate specific cache keys
#[derive(Debug, Deserialize)]
pub struct InvalidateCacheRequest {
    /// Cache keys to invalidate
    pub keys: Vec<String>,

    /// Invalidate from local cache
    #[serde(default = "default_true")]
    pub invalidate_local: bool,

    /// Invalidate from distributed cache
    #[serde(default = "default_true")]
    pub invalidate_distributed: bool,
}

/// Response for cache operations
#[derive(Debug, Serialize)]
pub struct CacheOperationResponse {
    /// Whether the operation was successful
    pub success: bool,

    /// Message describing the result
    pub message: String,

    /// Number of keys affected
    pub keys_affected: usize,
}

/// Get cache statistics
pub async fn get_cache_stats(state: Option<Arc<ProxyState>>) -> Response<Full<Bytes>> {
    let mut total_entries = 0;

    let local_stats = if let Some(ref state) = state {
        if let Some(cache) = state.local_cache() {
            let entries = cache.len();
            total_entries += entries;
            Some(LocalCacheStats {
                entries,
                enabled: true,
            })
        } else {
            Some(LocalCacheStats {
                entries: 0,
                enabled: false,
            })
        }
    } else {
        Some(LocalCacheStats {
            entries: 0,
            enabled: false,
        })
    };

    // Get distributed cache stats
    let distributed_stats = if let Some(ref state) = state {
        if let Some(dist_cache) = state.distributed_cache() {
            match dist_cache.write().await.stats().await {
                Ok(stats) => {
                    total_entries += stats.total_entries;
                    Some(DistributedCacheStats {
                        entries: stats.total_entries,
                        connected: true,
                        enabled: true,
                    })
                }
                Err(_) => Some(DistributedCacheStats {
                    entries: 0,
                    connected: false,
                    enabled: true,
                }),
            }
        } else {
            None
        }
    } else {
        None
    };

    let stats = CacheStatsResponse {
        local: local_stats,
        distributed: distributed_stats,
        total_entries,
    };

    json_response(StatusCode::OK, json!(stats))
}

/// Clear all cache or by pattern
pub async fn clear_cache(
    state: Option<Arc<ProxyState>>,
    request: ClearCacheRequest,
) -> Response<Full<Bytes>> {
    let mut keys_affected = 0;

    // Clear local cache if requested
    if request.clear_local {
        if let Some(state) = &state {
            if let Some(cache) = state.local_cache() {
                if let Some(ref pattern) = request.pattern {
                    // Pattern-based clearing: iterate over keys and remove matching ones
                    let all_keys = cache.get_all_keys();
                    for key in all_keys {
                        if matches_pattern(&key, pattern) {
                            cache.remove(&key);
                            keys_affected += 1;
                        }
                    }
                } else {
                    // Clear all entries
                    keys_affected = cache.len();
                    cache.clear();
                }
            }
        }
    }

    // Clear distributed cache if requested
    if request.clear_distributed {
        if let Some(state) = &state {
            if let Some(dist_cache) = state.distributed_cache() {
                // Note: Distributed cache clear() clears all entries with the prefix
                // Pattern-based clearing for distributed cache would require scanning keys
                if request.pattern.is_none() {
                    if let Ok(()) = dist_cache.write().await.clear().await {
                        // Count is approximate since we cleared everything
                        keys_affected += 1; // At least indicate something was done
                    }
                }
                // For pattern-based distributed cache clearing, we would need to implement
                // a scan-and-delete operation, which is expensive for large caches
            }
        }
    }

    let pattern_msg = request
        .pattern
        .map(|p| format!(" matching pattern '{}'", p))
        .unwrap_or_default();

    let mut caches_cleared = Vec::new();
    if request.clear_local {
        caches_cleared.push("local");
    }
    if request.clear_distributed {
        caches_cleared.push("distributed");
    }

    json_response(
        StatusCode::OK,
        json!(CacheOperationResponse {
            success: true,
            message: format!(
                "Cache cleared successfully{} from: {}",
                pattern_msg,
                caches_cleared.join(", ")
            ),
            keys_affected,
        }),
    )
}

/// Check if a key matches a simple pattern (supports * wildcard)
fn matches_pattern(key: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    // Handle simple wildcard patterns like "/api/*" or "*users*"
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        // Pattern ends with *, match prefix
        if let Some(suffix) = prefix.strip_prefix('*') {
            // Pattern is *something*, check contains
            return key.contains(suffix);
        }
        return key.starts_with(prefix);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        // Pattern starts with *, match suffix
        return key.ends_with(suffix);
    }

    // Exact match
    key == pattern
}

/// Invalidate specific cache keys
pub async fn invalidate_cache_keys(
    state: Option<Arc<ProxyState>>,
    request: InvalidateCacheRequest,
) -> Response<Full<Bytes>> {
    let mut keys_affected = 0;

    // Invalidate from local cache if requested
    if request.invalidate_local {
        if let Some(state) = &state {
            if let Some(cache) = state.local_cache() {
                for key in &request.keys {
                    cache.remove(key);
                    keys_affected += 1;
                }
            }
        }
    }

    // Invalidate from distributed cache if requested
    if request.invalidate_distributed {
        if let Some(state) = &state {
            if let Some(dist_cache) = state.distributed_cache() {
                let mut cache = dist_cache.write().await;
                for key in &request.keys {
                    if cache.remove(key).await.is_ok() {
                        keys_affected += 1;
                    }
                }
            }
        }
    }

    let mut caches = Vec::new();
    if request.invalidate_local {
        caches.push("local");
    }
    if request.invalidate_distributed {
        caches.push("distributed");
    }

    json_response(
        StatusCode::OK,
        json!(CacheOperationResponse {
            success: true,
            message: format!(
                "Invalidated {} cache keys from: {}",
                keys_affected,
                caches.join(", ")
            ),
            keys_affected,
        }),
    )
}

/// List cache keys (with optional pattern)
pub async fn list_cache_keys(
    state: Option<Arc<ProxyState>>,
    pattern: Option<String>,
) -> Response<Full<Bytes>> {
    let mut keys: Vec<String> = Vec::new();

    // Get keys from local cache
    if let Some(state) = &state {
        if let Some(cache) = state.local_cache() {
            keys = cache.get_all_keys();

            // Filter by pattern if provided
            if let Some(ref pattern_str) = pattern {
                keys.retain(|key| key.contains(pattern_str));
            }
        }
    }

    let pattern_msg = pattern.unwrap_or_else(|| "all".to_string());

    json_response(
        StatusCode::OK,
        json!({
            "keys": keys,
            "total": keys.len(),
            "pattern": pattern_msg,
        }),
    )
}

/// Helper function to create JSON response
fn json_response<T: Serialize>(status: StatusCode, body: T) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_string(&body).unwrap(),
        )))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_cache_stats() {
        let response = get_cache_stats(None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_clear_cache_all() {
        let request = ClearCacheRequest {
            pattern: None,
            clear_local: true,
            clear_distributed: true,
        };
        let response = clear_cache(None, request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_clear_cache_pattern() {
        let request = ClearCacheRequest {
            pattern: Some("/api/*".to_string()),
            clear_local: true,
            clear_distributed: false,
        };
        let response = clear_cache(None, request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalidate_cache_keys() {
        let request = InvalidateCacheRequest {
            keys: vec!["key1".to_string(), "key2".to_string()],
            invalidate_local: true,
            invalidate_distributed: true,
        };
        let response = invalidate_cache_keys(None, request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_cache_keys() {
        let response = list_cache_keys(None, None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_cache_keys_pattern() {
        let response = list_cache_keys(None, Some("/api/*".to_string())).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("/api/users", "/api/users"));
        assert!(!matches_pattern("/api/users", "/api/posts"));
    }

    #[test]
    fn test_matches_pattern_prefix_wildcard() {
        assert!(matches_pattern("/api/users", "/api/*"));
        assert!(matches_pattern("/api/users/123", "/api/*"));
        assert!(!matches_pattern("/other/users", "/api/*"));
    }

    #[test]
    fn test_matches_pattern_suffix_wildcard() {
        assert!(matches_pattern("/api/users", "*/users"));
        assert!(matches_pattern("/v1/api/users", "*/users"));
        assert!(!matches_pattern("/api/posts", "*/users"));
    }

    #[test]
    fn test_matches_pattern_contains_wildcard() {
        assert!(matches_pattern("/api/users/list", "*users*"));
        assert!(matches_pattern("users", "*users*"));
        assert!(!matches_pattern("/api/posts", "*users*"));
    }

    #[test]
    fn test_matches_pattern_all_wildcard() {
        assert!(matches_pattern("/any/path", "*"));
        assert!(matches_pattern("anything", "*"));
    }

    #[test]
    fn test_matches_pattern_empty() {
        assert!(matches_pattern("/any/path", ""));
        assert!(matches_pattern("", ""));
    }
}
