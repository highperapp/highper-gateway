//! Distributed cache system with adapter pattern
//!
//! Provides a unified caching interface that supports multiple backends:
//! - In-memory cache (local, fast)
//! - Redis (distributed, persistent)
//! - Memcached (distributed, simple)
//! - Multi-tier (combines local + distributed)
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
//! │  - RedisBackend                      │
//! │  - MemcachedBackend                  │
//! │  - MultiTierBackend                  │
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
//! // Or create Redis cache
//! let cache = CacheManager::new(
//!     CacheBackendType::Redis("redis://localhost:6379".to_string())
//! ).await?;
//!
//! // Use cache
//! cache.set("key", "value", Some(Duration::from_secs(300))).await?;
//! let value: Option<String> = cache.get("key").await?;
//! ```

pub mod backend;
pub mod manager;
pub mod backends;

pub use backend::{CacheBackend, CacheEntry, CacheError, CacheStats};
pub use manager::{CacheManager, CacheBackendType};
pub use backends::*;
