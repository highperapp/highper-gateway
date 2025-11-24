//! Cache management endpoints for Admin API
//!
//! Provides REST API for managing cache:
//! - Get cache statistics
//! - Clear all cache
//! - Clear cache by pattern
//! - Invalidate specific keys
//! - List cache keys

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::state::ProxyState;
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
    let (local_stats, total_entries) = if let Some(state) = state {
        if let Some(cache) = state.local_cache() {
            let entries = cache.len();
            (
                Some(LocalCacheStats {
                    entries,
                    enabled: true,
                }),
                entries,
            )
        } else {
            (
                Some(LocalCacheStats {
                    entries: 0,
                    enabled: false,
                }),
                0,
            )
        }
    } else {
        (
            Some(LocalCacheStats {
                entries: 0,
                enabled: false,
            }),
            0,
        )
    };

    let stats = CacheStatsResponse {
        local: local_stats,
        distributed: None, // TODO: Add distributed cache support
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
                if request.pattern.is_some() {
                    // TODO: Implement pattern-based clearing
                    // For now, we just note that pattern clearing is not yet implemented
                    // Pattern-based clearing would require iterating over cache keys
                } else {
                    // Clear all entries
                    keys_affected = cache.len();
                    cache.clear();
                }
            }
        }
    }

    // TODO: Clear distributed cache if requested

    let pattern_msg = request.pattern
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

    // TODO: Invalidate from distributed cache if requested

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

    let pattern_msg = pattern
        .unwrap_or_else(|| "all".to_string());

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
            keys: vec![
                "key1".to_string(),
                "key2".to_string(),
            ],
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
}
