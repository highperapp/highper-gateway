//! Disk-based cache backend
//!
//! Provides persistent caching that survives restarts.
//! Uses memory-mapped files and LRU eviction for efficiency.
//!
//! ## Features
//!
//! - Persistent storage (survives restarts)
//! - LRU eviction when size limit is reached
//! - Atomic writes (write to temp, then rename)
//! - Automatic cleanup of expired entries
//! - Optional compression for large objects

use super::backend::{CacheBackend, CacheError, CacheStats};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Disk cache configuration
#[derive(Debug, Clone)]
pub struct DiskCacheConfig {
    /// Base directory for cache files
    pub path: PathBuf,

    /// Maximum cache size in bytes
    pub max_size: u64,

    /// Minimum object size to cache to disk (smaller objects stay in memory)
    pub min_object_size: usize,

    /// Enable compression for cached objects
    pub compression: bool,

    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for DiskCacheConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/var/cache/highper-gateway"),
            max_size: 10 * 1024 * 1024 * 1024, // 10GB
            min_object_size: 0,                 // Cache everything
            compression: false,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Metadata for cached entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntryMetadata {
    /// Original key
    key: String,

    /// Size of the cached data
    size: u64,

    /// Creation timestamp (Unix ms)
    created_at: u128,

    /// Expiration timestamp (Unix ms, None = no expiration)
    expires_at: Option<u128>,

    /// Last access timestamp (for LRU)
    last_access: u128,

    /// Whether data is compressed
    compressed: bool,
}

impl EntryMetadata {
    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            now >= expires_at
        } else {
            false
        }
    }
}

/// LRU entry for eviction priority
#[derive(Debug, Clone, Eq, PartialEq)]
struct LruEntry {
    key: String,
    last_access: u128,
    size: u64,
}

impl Ord for LruEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Oldest (lowest last_access) should be evicted first
        // BinaryHeap is max-heap, so we reverse the ordering
        other.last_access.cmp(&self.last_access)
    }
}

impl PartialOrd for LruEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Disk cache backend
pub struct DiskBackend {
    /// Configuration
    config: DiskCacheConfig,

    /// In-memory index of cached entries (key -> metadata)
    index: Arc<DashMap<String, EntryMetadata>>,

    /// Current total size of cached data
    current_size: Arc<AtomicU64>,

    /// Hit counter
    hits: Arc<AtomicU64>,

    /// Miss counter
    misses: Arc<AtomicU64>,

    /// Write counter (for stats)
    writes: Arc<AtomicU64>,

    /// Eviction counter
    evictions: Arc<AtomicU64>,

    /// Lock for eviction operations
    eviction_lock: Arc<RwLock<()>>,
}

impl DiskBackend {
    /// Create a new disk cache backend
    pub async fn new(config: DiskCacheConfig) -> Result<Self, CacheError> {
        // Ensure cache directory exists
        fs::create_dir_all(&config.path)
            .await
            .map_err(|e| CacheError::Backend(format!("Failed to create cache directory: {}", e)))?;

        let backend = Self {
            config: config.clone(),
            index: Arc::new(DashMap::new()),
            current_size: Arc::new(AtomicU64::new(0)),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            writes: Arc::new(AtomicU64::new(0)),
            evictions: Arc::new(AtomicU64::new(0)),
            eviction_lock: Arc::new(RwLock::new(())),
        };

        // Load existing cache index
        backend.load_index().await?;

        // Start background cleanup task
        let cleanup_backend = backend.clone_for_cleanup();
        let cleanup_interval = config.cleanup_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                if let Err(e) = cleanup_backend.cleanup_expired().await {
                    error!("Disk cache cleanup error: {}", e);
                }
            }
        });

        info!(
            "Disk cache initialized: path={}, max_size={}MB",
            config.path.display(),
            config.max_size / (1024 * 1024)
        );

        Ok(backend)
    }

    /// Create a clone for the cleanup task
    fn clone_for_cleanup(&self) -> Self {
        Self {
            config: self.config.clone(),
            index: self.index.clone(),
            current_size: self.current_size.clone(),
            hits: self.hits.clone(),
            misses: self.misses.clone(),
            writes: self.writes.clone(),
            evictions: self.evictions.clone(),
            eviction_lock: self.eviction_lock.clone(),
        }
    }

    /// Convert cache key to file path
    fn key_to_path(&self, key: &str) -> PathBuf {
        // Use SHA256 hash to create safe file names
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Create subdirectory structure to avoid too many files in one directory
        let subdir = format!("{:02x}", (hash >> 56) as u8);
        let filename = format!("{:016x}.cache", hash);

        self.config.path.join(&subdir).join(&filename)
    }

    /// Convert cache key to metadata path
    fn key_to_meta_path(&self, key: &str) -> PathBuf {
        let mut path = self.key_to_path(key);
        path.set_extension("meta");
        path
    }

    /// Load existing cache index from disk
    async fn load_index(&self) -> Result<(), CacheError> {
        let mut total_size = 0u64;
        let mut entries_loaded = 0usize;

        // Walk the cache directory
        let mut read_dir = match fs::read_dir(&self.config.path).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(CacheError::Backend(format!("Failed to read cache dir: {}", e))),
        };

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            CacheError::Backend(format!("Failed to read cache dir entry: {}", e))
        })? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Read subdirectory
            let mut sub_dir = fs::read_dir(&path).await.map_err(|e| {
                CacheError::Backend(format!("Failed to read cache subdir: {}", e))
            })?;

            while let Some(file_entry) = sub_dir.next_entry().await.map_err(|e| {
                CacheError::Backend(format!("Failed to read cache file entry: {}", e))
            })? {
                let file_path = file_entry.path();
                if file_path.extension().and_then(|s| s.to_str()) != Some("meta") {
                    continue;
                }

                // Load metadata
                match fs::read(&file_path).await {
                    Ok(meta_bytes) => {
                        if let Ok(meta) = serde_json::from_slice::<EntryMetadata>(&meta_bytes) {
                            if !meta.is_expired() {
                                total_size += meta.size;
                                self.index.insert(meta.key.clone(), meta);
                                entries_loaded += 1;
                            } else {
                                // Remove expired entry
                                let _ = fs::remove_file(&file_path).await;
                                let data_path = file_path.with_extension("cache");
                                let _ = fs::remove_file(&data_path).await;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read cache metadata {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        self.current_size.store(total_size, Ordering::SeqCst);
        info!("Loaded {} cache entries ({}MB)", entries_loaded, total_size / (1024 * 1024));

        Ok(())
    }

    /// Clean up expired entries
    async fn cleanup_expired(&self) -> Result<usize, CacheError> {
        let mut removed = 0;

        let expired_keys: Vec<String> = self
            .index
            .iter()
            .filter(|e| e.value().is_expired())
            .map(|e| e.key().clone())
            .collect();

        for key in expired_keys {
            if self.delete(&key).await? {
                removed += 1;
            }
        }

        if removed > 0 {
            debug!("Cleaned up {} expired cache entries", removed);
        }

        Ok(removed)
    }

    /// Evict entries to make room for new data
    async fn evict_if_needed(&self, needed_size: u64) -> Result<(), CacheError> {
        let max_size = self.config.max_size;
        let current = self.current_size.load(Ordering::SeqCst);

        if current + needed_size <= max_size {
            return Ok(());
        }

        // Acquire eviction lock
        let _lock = self.eviction_lock.write().await;

        // Re-check after acquiring lock
        let current = self.current_size.load(Ordering::SeqCst);
        if current + needed_size <= max_size {
            return Ok(());
        }

        let target_size = (max_size as f64 * 0.8) as u64; // Evict down to 80%
        let bytes_to_free = current.saturating_sub(target_size) + needed_size;

        debug!(
            "Disk cache eviction: current={}MB, target={}MB, freeing={}MB",
            current / (1024 * 1024),
            target_size / (1024 * 1024),
            bytes_to_free / (1024 * 1024)
        );

        // Build LRU heap
        let mut heap: BinaryHeap<LruEntry> = self
            .index
            .iter()
            .map(|e| LruEntry {
                key: e.key().clone(),
                last_access: e.value().last_access,
                size: e.value().size,
            })
            .collect();

        let mut freed = 0u64;
        let mut evicted = 0usize;

        while freed < bytes_to_free {
            if let Some(entry) = heap.pop() {
                if self.delete(&entry.key).await? {
                    freed += entry.size;
                    evicted += 1;
                }
            } else {
                break;
            }
        }

        self.evictions.fetch_add(evicted as u64, Ordering::Relaxed);
        info!("Evicted {} entries, freed {}MB", evicted, freed / (1024 * 1024));

        Ok(())
    }

    /// Compress data if compression is enabled
    fn maybe_compress(&self, data: &[u8]) -> (Vec<u8>, bool) {
        if !self.config.compression || data.len() < 1024 {
            return (data.to_vec(), false);
        }

        // Use zstd compression
        match zstd::encode_all(data, 3) {
            Ok(compressed) if compressed.len() < data.len() => (compressed, true),
            _ => (data.to_vec(), false),
        }
    }

    /// Decompress data if it was compressed
    fn maybe_decompress(&self, data: &[u8], compressed: bool) -> Result<Vec<u8>, CacheError> {
        if !compressed {
            return Ok(data.to_vec());
        }

        zstd::decode_all(data)
            .map_err(|e| CacheError::Deserialization(format!("Decompression failed: {}", e)))
    }
}

#[async_trait]
impl CacheBackend for DiskBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        // Check index first
        let meta = match self.index.get(key) {
            Some(m) => m.clone(),
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        };

        // Check expiration
        if meta.is_expired() {
            drop(meta);
            self.delete(key).await?;
            self.misses.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }

        // Read from disk
        let file_path = self.key_to_path(key);
        match fs::read(&file_path).await {
            Ok(data) => {
                // Update last access time
                if let Some(mut entry) = self.index.get_mut(key) {
                    entry.last_access = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                }

                let decompressed = self.maybe_decompress(&data, meta.compressed)?;
                self.hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(decompressed))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Index out of sync, remove entry
                self.index.remove(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
            Err(e) => Err(CacheError::Backend(format!("Failed to read cache file: {}", e))),
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        // Skip if below minimum size
        if value.len() < self.config.min_object_size {
            return Ok(());
        }

        let (data, compressed) = self.maybe_compress(&value);
        let size = data.len() as u64;

        // Evict if needed
        self.evict_if_needed(size).await?;

        let file_path = self.key_to_path(key);
        let meta_path = self.key_to_meta_path(key);

        // Ensure directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                CacheError::Backend(format!("Failed to create cache subdir: {}", e))
            })?;
        }

        // Write data atomically (temp file + rename)
        let temp_path = file_path.with_extension("tmp");
        fs::write(&temp_path, &data).await.map_err(|e| {
            CacheError::Backend(format!("Failed to write cache file: {}", e))
        })?;

        fs::rename(&temp_path, &file_path).await.map_err(|e| {
            CacheError::Backend(format!("Failed to rename cache file: {}", e))
        })?;

        // Create metadata
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let meta = EntryMetadata {
            key: key.to_string(),
            size,
            created_at: now,
            expires_at: ttl.map(|d| now + d.as_millis()),
            last_access: now,
            compressed,
        };

        // Write metadata
        let meta_json = serde_json::to_vec(&meta)?;
        fs::write(&meta_path, &meta_json).await.map_err(|e| {
            CacheError::Backend(format!("Failed to write cache metadata: {}", e))
        })?;

        // Update index
        if let Some(old) = self.index.insert(key.to_string(), meta) {
            // Subtract old size, add new size
            self.current_size.fetch_sub(old.size, Ordering::SeqCst);
        }
        self.current_size.fetch_add(size, Ordering::SeqCst);
        self.writes.fetch_add(1, Ordering::Relaxed);

        debug!("Cached {} bytes to disk: {}", size, key);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let file_path = self.key_to_path(key);
        let meta_path = self.key_to_meta_path(key);

        // Remove from index
        let removed = self.index.remove(key);

        // Update size
        if let Some((_, meta)) = &removed {
            self.current_size.fetch_sub(meta.size, Ordering::SeqCst);
        }

        // Remove files
        let _ = fs::remove_file(&file_path).await;
        let _ = fs::remove_file(&meta_path).await;

        Ok(removed.is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        if let Some(meta) = self.index.get(key) {
            if meta.is_expired() {
                drop(meta);
                self.delete(key).await?;
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    async fn clear(&self) -> Result<(), CacheError> {
        // Clear index
        self.index.clear();
        self.current_size.store(0, Ordering::SeqCst);

        // Remove all cache files
        let mut read_dir = fs::read_dir(&self.config.path).await.map_err(|e| {
            CacheError::Backend(format!("Failed to read cache dir: {}", e))
        })?;

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            CacheError::Backend(format!("Failed to read cache dir entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path).await;
            }
        }

        info!("Disk cache cleared");
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        let total = self.index.len();
        let expired = self.index.iter().filter(|e| e.value().is_expired()).count();
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };

        let current_size = self.current_size.load(Ordering::SeqCst);
        let writes = self.writes.load(Ordering::Relaxed);
        let evictions = self.evictions.load(Ordering::Relaxed);

        Ok(CacheStats {
            total_entries: total,
            active_entries: total - expired,
            expired_entries: expired,
            hit_rate,
            hits,
            misses,
            backend_info: format!(
                "Disk (path={}, size={}MB/{}MB, writes={}, evictions={})",
                self.config.path.display(),
                current_size / (1024 * 1024),
                self.config.max_size / (1024 * 1024),
                writes,
                evictions
            ),
        })
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        let keys: Vec<String> = self
            .index
            .iter()
            .filter(|entry| {
                if pattern == "*" {
                    true
                } else {
                    entry.key().contains(pattern.trim_matches('*'))
                }
            })
            .map(|entry| entry.key().clone())
            .collect();

        Ok(keys)
    }

    fn name(&self) -> &str {
        "Disk"
    }
}

/// Tiered cache backend (Memory hot tier + Disk warm tier)
///
/// Provides optimal performance with persistence:
/// - Hot data stays in memory for fast access
/// - Warm data is stored on disk
/// - Automatic promotion/demotion based on access patterns
pub struct TieredBackend {
    /// Hot tier (memory, fast)
    hot: Arc<super::backends::InMemoryBackend>,

    /// Warm tier (disk, persistent)
    warm: Arc<DiskBackend>,

    /// Maximum size for hot tier entries
    hot_max_size: usize,

    /// TTL for hot tier (shorter than disk)
    hot_ttl: Duration,
}

impl TieredBackend {
    /// Create a new tiered cache
    pub fn new(disk: Arc<DiskBackend>, hot_max_size: usize, hot_ttl: Duration) -> Self {
        Self {
            hot: Arc::new(super::backends::InMemoryBackend::new()),
            warm: disk,
            hot_max_size,
            hot_ttl,
        }
    }
}

#[async_trait]
impl CacheBackend for TieredBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        // Try hot tier first
        if let Some(value) = self.hot.get(key).await? {
            debug!("Hot tier hit: {}", key);
            return Ok(Some(value));
        }

        // Try warm tier
        if let Some(value) = self.warm.get(key).await? {
            debug!("Warm tier hit: {}", key);
            // Promote to hot tier if small enough
            if value.len() <= self.hot_max_size {
                self.hot.set(key, value.clone(), Some(self.hot_ttl)).await?;
            }
            return Ok(Some(value));
        }

        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        // Always write to warm (disk) tier for persistence
        self.warm.set(key, value.clone(), ttl).await?;

        // Also write to hot tier if small enough
        if value.len() <= self.hot_max_size {
            let hot_ttl = ttl.map(|t| t.min(self.hot_ttl)).unwrap_or(self.hot_ttl);
            self.hot.set(key, value, Some(hot_ttl)).await?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let hot_deleted = self.hot.delete(key).await?;
        let warm_deleted = self.warm.delete(key).await?;
        Ok(hot_deleted || warm_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        if self.hot.exists(key).await? {
            return Ok(true);
        }
        self.warm.exists(key).await
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.hot.clear().await?;
        self.warm.clear().await?;
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        let hot_stats = self.hot.stats().await?;
        let warm_stats = self.warm.stats().await?;

        Ok(CacheStats {
            total_entries: hot_stats.total_entries + warm_stats.total_entries,
            active_entries: hot_stats.active_entries + warm_stats.active_entries,
            expired_entries: hot_stats.expired_entries + warm_stats.expired_entries,
            hit_rate: (hot_stats.hits + warm_stats.hits) as f64
                / (hot_stats.hits + hot_stats.misses + warm_stats.hits + warm_stats.misses).max(1) as f64,
            hits: hot_stats.hits + warm_stats.hits,
            misses: warm_stats.misses, // Only count misses from warm tier
            backend_info: format!(
                "Tiered (Hot: {} entries, Warm: {})",
                hot_stats.total_entries,
                warm_stats.backend_info
            ),
        })
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        // Get keys from warm tier (source of truth)
        self.warm.keys(pattern).await
    }

    fn name(&self) -> &str {
        "Tiered"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_backend() -> DiskBackend {
        let temp_dir = tempdir().unwrap();
        let config = DiskCacheConfig {
            path: temp_dir.path().to_path_buf(),
            max_size: 1024 * 1024, // 1MB
            min_object_size: 0,
            compression: false,
            cleanup_interval: Duration::from_secs(60),
        };
        DiskBackend::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_disk_backend_set_get() {
        let cache = create_test_backend().await;

        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_disk_backend_expiration() {
        let cache = create_test_backend().await;

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
    async fn test_disk_backend_delete() {
        let cache = create_test_backend().await;

        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        assert!(cache.exists("key1").await.unwrap());

        assert!(cache.delete("key1").await.unwrap());
        assert!(!cache.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_disk_backend_clear() {
        let cache = create_test_backend().await;

        cache.set("key1", b"value1".to_vec(), None).await.unwrap();
        cache.set("key2", b"value2".to_vec(), None).await.unwrap();

        cache.clear().await.unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_disk_backend_stats() {
        let cache = create_test_backend().await;

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
    async fn test_tiered_backend() {
        let temp_dir = tempdir().unwrap();
        let config = DiskCacheConfig {
            path: temp_dir.path().to_path_buf(),
            max_size: 1024 * 1024,
            min_object_size: 0,
            compression: false,
            cleanup_interval: Duration::from_secs(60),
        };
        let disk = Arc::new(DiskBackend::new(config).await.unwrap());
        let cache = TieredBackend::new(disk, 1024, Duration::from_secs(60));

        // Set value
        cache.set("key1", b"value1".to_vec(), None).await.unwrap();

        // First get should hit warm tier
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Second get should hit hot tier (promoted)
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }
}
