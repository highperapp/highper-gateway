//! Distributed cache system with adapter pattern
//!
//! Provides a unified caching interface that supports multiple backends:
//! - In-memory cache (local, fast)
//! - Disk cache (persistent, survives restarts)
//! - Redis (distributed, persistent)
//! - Memcached (distributed, simple)
//! - Multi-tier (combines local + distributed)
//! - Tiered (memory hot + disk warm)
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │  Application                         │
//! ├──────────────────────────────────────┤
//! │  CacheManager (unified interface)    │
//! ├──────────────────────────────────────┤
//! │  CacheBackend trait                  │
//! ├──────────────────────────────────────┤
//! │  Backend Implementations:            │
//! │  - InMemoryBackend                   │
//! │  - DiskBackend (new!)                │
//! │  - RedisBackend                      │
//! │  - MemcachedBackend                  │
//! │  - MultiTierBackend                  │
//! │  - TieredBackend (memory + disk)     │
//! └──────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use highper_gateway::cache::{CacheManager, CacheBackendType};
//!
//! // Create in-memory cache
//! let cache = CacheManager::new(CacheBackendType::InMemory).await?;
//!
//! // Create tiered cache (memory hot + disk warm)
//! let cache = CacheManager::new(
//!     CacheBackendType::Tiered {
//!         disk_path: "/var/cache/highper-gateway".into(),
//!         disk_size: 10 * 1024 * 1024 * 1024, // 10GB
//!         hot_max_size: 1024 * 1024, // 1MB per entry in hot tier
//!     }
//! ).await?;
//!
//! // Use cache
//! cache.set("key", "value", Some(Duration::from_secs(300))).await?; // allow: doc-comment example
//! let value: Option<String> = cache.get("key").await?;
//! ```

pub mod backend;
pub mod backends;
pub mod disk;
pub mod manager;

pub use backend::{CacheBackend, CacheEntry, CacheError, CacheStats};
pub use backends::*;
pub use disk::{DiskBackend, DiskCacheConfig, TieredBackend};
pub use manager::{CacheBackendType, CacheManager};
