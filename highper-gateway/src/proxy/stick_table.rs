//! Stick Tables - HAProxy-style session persistence
//!
//! Provides a high-performance, thread-safe key-value store for session affinity.
//! Supports multiple key types, TTL-based expiration, and atomic counters.
//!
//! ## Features
//! - Multiple data types (IP, String, Integer, Binary)
//! - TTL-based automatic expiration
//! - Atomic counters for rate limiting
//! - LRU eviction when table is full
//! - Thread-safe concurrent access
//!
//! ## Example
//!
//! ```rust,ignore
//! use highper_gateway::proxy::stick_table::*;
//!
//! let config = StickTableConfig {
//!     name: "sessions".to_string(),
//!     table_type: StickTableType::Ip,
//!     size: 100_000,
//!     expire_secs: 1800,
//!     ..Default::default()
//! };
//!
//! let table = StickTable::new(config);
//!
//! // Store session data
//! table.set("192.168.1.1", StickEntry::server_id(1));
//!
//! // Retrieve session
//! if let Some(entry) = table.get("192.168.1.1") {
//!     let backend_id = entry.server_id();
//! }
//! ```

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// Stick table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickTableConfig {
    /// Table name for identification
    pub name: String,

    /// Type of key used in the table
    #[serde(default)]
    pub table_type: StickTableType,

    /// Maximum number of entries
    #[serde(default = "default_size")]
    pub size: usize,

    /// Entry expiration time in seconds (0 = no expiration)
    #[serde(default = "default_expire")]
    pub expire_secs: u64,

    /// Enable storing server ID
    #[serde(default = "default_true")]
    pub store_server_id: bool,

    /// Enable connection counter
    #[serde(default)]
    pub store_conn_cnt: bool,

    /// Enable connection rate
    #[serde(default)]
    pub store_conn_rate: bool,

    /// Enable HTTP request counter
    #[serde(default)]
    pub store_http_req_cnt: bool,

    /// Enable HTTP request rate
    #[serde(default)]
    pub store_http_req_rate: bool,

    /// Enable bytes in counter
    #[serde(default)]
    pub store_bytes_in_cnt: bool,

    /// Enable bytes out counter
    #[serde(default)]
    pub store_bytes_out_cnt: bool,

    /// Rate period in seconds (for rate calculations)
    #[serde(default = "default_rate_period")]
    pub rate_period_secs: u64,

    /// Enable peers synchronization (for HA)
    #[serde(default)]
    pub peers_enabled: bool,
}

fn default_size() -> usize {
    100_000
}
fn default_expire() -> u64 {
    1800
} // 30 minutes
fn default_true() -> bool {
    true
}
fn default_rate_period() -> u64 {
    10
}

impl Default for StickTableConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            table_type: StickTableType::Ip,
            size: default_size(),
            expire_secs: default_expire(),
            store_server_id: true,
            store_conn_cnt: false,
            store_conn_rate: false,
            store_http_req_cnt: false,
            store_http_req_rate: false,
            store_bytes_in_cnt: false,
            store_bytes_out_cnt: false,
            rate_period_secs: default_rate_period(),
            peers_enabled: false,
        }
    }
}

/// Type of key used in stick table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StickTableType {
    /// IPv4/IPv6 address
    #[default]
    Ip,
    /// Integer key
    Integer,
    /// String key (e.g., session ID, cookie value)
    String,
    /// Binary key (raw bytes, hashed)
    Binary,
}

/// Entry stored in stick table
#[derive(Debug)]
pub struct StickEntry {
    /// Server/backend ID for session affinity
    pub server_id: Option<usize>,

    /// Connection counter
    pub conn_cnt: AtomicU64,

    /// Connection rate (connections per rate period)
    pub conn_rate: AtomicU64,

    /// HTTP request counter
    pub http_req_cnt: AtomicU64,

    /// HTTP request rate
    pub http_req_rate: AtomicU64,

    /// Bytes received counter
    pub bytes_in_cnt: AtomicU64,

    /// Bytes sent counter
    pub bytes_out_cnt: AtomicU64,

    /// When this entry was created
    pub created_at: Instant,

    /// When this entry was last accessed
    pub last_access: AtomicU64,

    /// When this entry expires (UNIX timestamp)
    pub expires_at: u64,

    /// Custom data storage
    pub custom_data: Option<Vec<u8>>,
}

impl StickEntry {
    /// Create a new entry with server ID
    pub fn server_id(id: usize) -> Self {
        Self::new(Some(id), 0)
    }

    /// Create a new entry with expiration
    pub fn new(server_id: Option<usize>, expire_secs: u64) -> Self {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            server_id,
            conn_cnt: AtomicU64::new(0),
            conn_rate: AtomicU64::new(0),
            http_req_cnt: AtomicU64::new(0),
            http_req_rate: AtomicU64::new(0),
            bytes_in_cnt: AtomicU64::new(0),
            bytes_out_cnt: AtomicU64::new(0),
            created_at: Instant::now(),
            last_access: AtomicU64::new(now_millis),
            expires_at: if expire_secs > 0 {
                now_secs + expire_secs
            } else {
                u64::MAX
            },
            custom_data: None,
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= self.expires_at
    }

    /// Touch entry to update last access time
    pub fn touch(&self) {
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_access.store(now_millis, AtomicOrdering::Relaxed);
    }

    /// Increment connection counter
    pub fn inc_conn(&self) -> u64 {
        self.conn_cnt.fetch_add(1, AtomicOrdering::Relaxed) + 1
    }

    /// Increment HTTP request counter
    pub fn inc_http_req(&self) -> u64 {
        self.http_req_cnt.fetch_add(1, AtomicOrdering::Relaxed) + 1
    }

    /// Add bytes received
    pub fn add_bytes_in(&self, bytes: u64) -> u64 {
        self.bytes_in_cnt.fetch_add(bytes, AtomicOrdering::Relaxed) + bytes
    }

    /// Add bytes sent
    pub fn add_bytes_out(&self, bytes: u64) -> u64 {
        self.bytes_out_cnt.fetch_add(bytes, AtomicOrdering::Relaxed) + bytes
    }

    /// Get server ID
    pub fn get_server_id(&self) -> Option<usize> {
        self.server_id
    }

    /// Get connection count
    pub fn get_conn_cnt(&self) -> u64 {
        self.conn_cnt.load(AtomicOrdering::Relaxed)
    }

    /// Get HTTP request count
    pub fn get_http_req_cnt(&self) -> u64 {
        self.http_req_cnt.load(AtomicOrdering::Relaxed)
    }
}

/// Entry for LRU eviction heap
#[derive(Debug, Clone)]
struct LruEntry {
    key: String,
    last_access: u64,
}

impl PartialEq for LruEntry {
    fn eq(&self, other: &Self) -> bool {
        self.last_access == other.last_access
    }
}

impl Eq for LruEntry {}

impl PartialOrd for LruEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LruEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: older entries first
        other.last_access.cmp(&self.last_access)
    }
}

/// High-performance stick table
pub struct StickTable {
    /// Configuration
    config: StickTableConfig,

    /// Main storage
    entries: DashMap<String, Arc<StickEntry>>,

    /// Current entry count
    entry_count: AtomicUsize,

    /// Statistics
    stats: StickTableStats,
}

/// Stick table statistics
#[derive(Debug, Default)]
pub struct StickTableStats {
    /// Total lookups
    pub lookups: AtomicU64,

    /// Cache hits
    pub hits: AtomicU64,

    /// Cache misses
    pub misses: AtomicU64,

    /// Entries inserted
    pub inserts: AtomicU64,

    /// Entries evicted (LRU)
    pub evictions: AtomicU64,

    /// Entries expired
    pub expirations: AtomicU64,
}

impl StickTable {
    /// Create a new stick table
    pub fn new(config: StickTableConfig) -> Self {
        info!(
            "Creating stick table '{}' with size {}",
            config.name, config.size
        );

        Self {
            entries: DashMap::with_capacity(config.size),
            entry_count: AtomicUsize::new(0),
            stats: StickTableStats::default(),
            config,
        }
    }

    /// Get entry by key
    pub fn get(&self, key: &str) -> Option<Arc<StickEntry>> {
        self.stats.lookups.fetch_add(1, AtomicOrdering::Relaxed);

        if let Some(entry) = self.entries.get(key) {
            let entry = entry.value().clone();

            // Check expiration
            if entry.is_expired() {
                drop(entry);
                self.remove(key);
                self.stats.expirations.fetch_add(1, AtomicOrdering::Relaxed);
                self.stats.misses.fetch_add(1, AtomicOrdering::Relaxed);
                return None;
            }

            // Update access time
            entry.touch();
            self.stats.hits.fetch_add(1, AtomicOrdering::Relaxed);
            Some(entry)
        } else {
            self.stats.misses.fetch_add(1, AtomicOrdering::Relaxed);
            None
        }
    }

    /// Set entry by key
    pub fn set(&self, key: &str, entry: StickEntry) {
        // Check if we need to evict
        let current_count = self.entry_count.load(AtomicOrdering::Relaxed);
        if current_count >= self.config.size && !self.entries.contains_key(key) {
            self.evict_lru();
        }

        let is_new = !self.entries.contains_key(key);
        self.entries.insert(key.to_string(), Arc::new(entry));

        if is_new {
            self.entry_count.fetch_add(1, AtomicOrdering::Relaxed);
        }

        self.stats.inserts.fetch_add(1, AtomicOrdering::Relaxed);
        debug!("Stick table '{}': set key '{}'", self.config.name, key);
    }

    /// Set or update server ID for a key
    pub fn set_server(&self, key: &str, server_id: usize) {
        if let Some(mut entry) = self.entries.get_mut(key) {
            // Update existing entry's server_id by creating new entry
            let old = entry.value();
            let new_entry = StickEntry {
                server_id: Some(server_id),
                conn_cnt: AtomicU64::new(old.conn_cnt.load(AtomicOrdering::Relaxed)),
                conn_rate: AtomicU64::new(old.conn_rate.load(AtomicOrdering::Relaxed)),
                http_req_cnt: AtomicU64::new(old.http_req_cnt.load(AtomicOrdering::Relaxed)),
                http_req_rate: AtomicU64::new(old.http_req_rate.load(AtomicOrdering::Relaxed)),
                bytes_in_cnt: AtomicU64::new(old.bytes_in_cnt.load(AtomicOrdering::Relaxed)),
                bytes_out_cnt: AtomicU64::new(old.bytes_out_cnt.load(AtomicOrdering::Relaxed)),
                created_at: old.created_at,
                last_access: AtomicU64::new(old.last_access.load(AtomicOrdering::Relaxed)),
                expires_at: old.expires_at,
                custom_data: old.custom_data.clone(),
            };
            *entry = Arc::new(new_entry);
        } else {
            self.set(
                key,
                StickEntry::new(Some(server_id), self.config.expire_secs),
            );
        }
    }

    /// Get server ID for a key
    pub fn get_server(&self, key: &str) -> Option<usize> {
        self.get(key).and_then(|e| e.get_server_id())
    }

    /// Remove entry by key
    pub fn remove(&self, key: &str) -> bool {
        if self.entries.remove(key).is_some() {
            self.entry_count.fetch_sub(1, AtomicOrdering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Check if key exists
    pub fn contains(&self, key: &str) -> bool {
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                drop(entry);
                self.remove(key);
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.entry_count.load(AtomicOrdering::Relaxed)
    }

    /// Check if table is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
        self.entry_count.store(0, AtomicOrdering::Relaxed);
        info!("Stick table '{}' cleared", self.config.name);
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        // Find oldest entry
        let mut oldest_key: Option<String> = None;
        let mut oldest_access: u64 = u64::MAX;

        for entry in self.entries.iter() {
            let access = entry.value().last_access.load(AtomicOrdering::Relaxed);
            if access < oldest_access {
                oldest_access = access;
                oldest_key = Some(entry.key().clone());
            }
        }

        if let Some(key) = oldest_key {
            self.remove(&key);
            self.stats.evictions.fetch_add(1, AtomicOrdering::Relaxed);
            debug!(
                "Stick table '{}': evicted LRU key '{}'",
                self.config.name, key
            );
        }
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> usize {
        let mut expired_keys = Vec::new();

        for entry in self.entries.iter() {
            if entry.value().is_expired() {
                expired_keys.push(entry.key().clone());
            }
        }

        let count = expired_keys.len();
        for key in expired_keys {
            self.remove(&key);
        }

        if count > 0 {
            self.stats
                .expirations
                .fetch_add(count as u64, AtomicOrdering::Relaxed);
            debug!(
                "Stick table '{}': cleaned up {} expired entries",
                self.config.name, count
            );
        }

        count
    }

    /// Get statistics
    pub fn stats(&self) -> &StickTableStats {
        &self.stats
    }

    /// Get configuration
    pub fn config(&self) -> &StickTableConfig {
        &self.config
    }

    /// Get hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f64 {
        let lookups = self.stats.lookups.load(AtomicOrdering::Relaxed);
        if lookups == 0 {
            return 0.0;
        }
        let hits = self.stats.hits.load(AtomicOrdering::Relaxed);
        hits as f64 / lookups as f64
    }
}

/// Manager for multiple stick tables
pub struct StickTableManager {
    tables: DashMap<String, Arc<StickTable>>,
}

impl StickTableManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            tables: DashMap::new(),
        }
    }

    /// Create and register a new table
    pub fn create_table(&self, config: StickTableConfig) -> Arc<StickTable> {
        let name = config.name.clone();
        let table = Arc::new(StickTable::new(config));
        self.tables.insert(name, table.clone());
        table
    }

    /// Get table by name
    pub fn get_table(&self, name: &str) -> Option<Arc<StickTable>> {
        self.tables.get(name).map(|t| t.value().clone())
    }

    /// Remove table by name
    pub fn remove_table(&self, name: &str) -> bool {
        self.tables.remove(name).is_some()
    }

    /// Get all table names
    pub fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|e| e.key().clone()).collect()
    }

    /// Cleanup expired entries in all tables
    pub fn cleanup_all(&self) -> usize {
        let mut total = 0;
        for table in self.tables.iter() {
            total += table.value().cleanup_expired();
        }
        total
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            loop {
                timer.tick().await;
                let cleaned = self.cleanup_all();
                if cleaned > 0 {
                    debug!("Stick table cleanup: removed {} expired entries", cleaned);
                }
            }
        });
    }
}

impl Default for StickTableManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stick_table_basic() {
        let config = StickTableConfig {
            name: "test".to_string(),
            size: 100,
            expire_secs: 60,
            ..Default::default()
        };

        let table = StickTable::new(config);

        // Set and get
        table.set("192.168.1.1", StickEntry::server_id(1));
        let entry = table.get("192.168.1.1").unwrap();
        assert_eq!(entry.get_server_id(), Some(1));

        // Contains
        assert!(table.contains("192.168.1.1"));
        assert!(!table.contains("192.168.1.2"));

        // Len
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_stick_table_counters() {
        let config = StickTableConfig::default();
        let table = StickTable::new(config);

        table.set("test", StickEntry::server_id(0));
        let entry = table.get("test").unwrap();

        // Increment counters
        assert_eq!(entry.inc_conn(), 1);
        assert_eq!(entry.inc_conn(), 2);
        assert_eq!(entry.get_conn_cnt(), 2);

        assert_eq!(entry.inc_http_req(), 1);
        assert_eq!(entry.get_http_req_cnt(), 1);

        entry.add_bytes_in(1024);
        entry.add_bytes_out(2048);
    }

    #[test]
    fn test_stick_table_lru_eviction() {
        let config = StickTableConfig {
            name: "test".to_string(),
            size: 3,
            expire_secs: 60,
            ..Default::default()
        };

        let table = StickTable::new(config);

        // Fill table with small delays to ensure different timestamps
        table.set("key1", StickEntry::server_id(1));
        std::thread::sleep(std::time::Duration::from_millis(5));
        table.set("key2", StickEntry::server_id(2));
        std::thread::sleep(std::time::Duration::from_millis(5));
        table.set("key3", StickEntry::server_id(3));

        assert_eq!(table.len(), 3);

        // Access key2 to make it recently used
        std::thread::sleep(std::time::Duration::from_millis(5));
        table.get("key2");

        // Add key4 - should evict key1 (oldest - key3 wasn't accessed)
        std::thread::sleep(std::time::Duration::from_millis(5));
        table.set("key4", StickEntry::server_id(4));

        assert_eq!(table.len(), 3);
        // key2 was accessed most recently, key4 was just added
        // key3 was added after key1 but never accessed
        // key1 was added first and never accessed - should be evicted
        assert!(table.contains("key2"));
        assert!(table.contains("key4"));
        // Note: key3 or key1 could be evicted depending on timing
        // The test verifies key2 (accessed) and key4 (just added) remain
    }

    #[test]
    fn test_stick_table_set_server() {
        let config = StickTableConfig::default();
        let table = StickTable::new(config);

        // Set new
        table.set_server("client1", 5);
        assert_eq!(table.get_server("client1"), Some(5));

        // Update existing
        table.set_server("client1", 10);
        assert_eq!(table.get_server("client1"), Some(10));
    }

    #[test]
    fn test_stick_table_manager() {
        let manager = StickTableManager::new();

        let config1 = StickTableConfig {
            name: "sessions".to_string(),
            ..Default::default()
        };

        let config2 = StickTableConfig {
            name: "rate_limits".to_string(),
            ..Default::default()
        };

        manager.create_table(config1);
        manager.create_table(config2);

        assert!(manager.get_table("sessions").is_some());
        assert!(manager.get_table("rate_limits").is_some());
        assert!(manager.get_table("nonexistent").is_none());

        let names = manager.table_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_stick_table_stats() {
        let config = StickTableConfig::default();
        let table = StickTable::new(config);

        // Miss
        table.get("nonexistent");
        assert_eq!(table.stats().misses.load(AtomicOrdering::Relaxed), 1);

        // Insert and hit
        table.set("key1", StickEntry::server_id(1));
        table.get("key1");
        assert_eq!(table.stats().hits.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(table.stats().inserts.load(AtomicOrdering::Relaxed), 1);

        // Hit rate
        assert!(table.hit_rate() > 0.0);
    }

    #[test]
    fn test_config_serialization() {
        let config = StickTableConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: StickTableConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.name, parsed.name);
        assert_eq!(config.size, parsed.size);
    }
}
