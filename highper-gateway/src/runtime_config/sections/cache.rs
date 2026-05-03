//! Cache tunables (Workstream 0.J Stage 3).
//!
//! Resolves the 10 production-path `// allow: Stage 3 — CacheRuntimeConfig::*`
//! waivers landed in Stage 2 (commit 67bf863) across:
//! - `src/cache/backend.rs:208` (health_check_ttl_secs)
//! - `src/cache/backends.rs:76` (cleanup_interval_secs)
//! - `src/cache/backends.rs:364` (multi_tier_l1_ttl_secs)
//! - `src/cache/backends.rs:373` (multi_tier_l1_max_ttl_secs)
//! - `src/cache/disk.rs:54` (disk_cleanup_interval_secs)
//! - `src/cache/manager.rs` × 6 (disk_cleanup_interval_secs ×4 + tiered_hot_ttl_secs ×2)

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct CacheRuntimeConfig {
    pub default_ttl_secs: Reloadable<u64>,             // HIGHPER_CACHE_DEFAULT_TTL (default 300)
    pub health_check_ttl_secs: Reloadable<u64>,        // HIGHPER_CACHE_HEALTH_CHECK_TTL (default 5)
    pub cleanup_interval_secs: Reloadable<u64>,        // HIGHPER_CACHE_CLEANUP_INTERVAL (default 60)
    pub disk_cleanup_interval_secs: Reloadable<u64>,   // HIGHPER_CACHE_DISK_CLEANUP_INTERVAL (default 300)
    pub multi_tier_l1_ttl_secs: Reloadable<u64>,       // HIGHPER_CACHE_MULTI_TIER_L1_TTL (default 300)
    pub multi_tier_l1_max_ttl_secs: Reloadable<u64>,   // HIGHPER_CACHE_MULTI_TIER_L1_MAX_TTL (default 300)
    pub tiered_hot_ttl_secs: Reloadable<u64>,          // HIGHPER_CACHE_TIERED_HOT_TTL (default 300)
}

impl Default for CacheRuntimeConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: Reloadable::new(300),
            health_check_ttl_secs: Reloadable::new(5),
            cleanup_interval_secs: Reloadable::new(60),
            disk_cleanup_interval_secs: Reloadable::new(300),
            multi_tier_l1_ttl_secs: Reloadable::new(300),
            multi_tier_l1_max_ttl_secs: Reloadable::new(300),
            tiered_hot_ttl_secs: Reloadable::new(300),
        }
    }
}

pub(crate) fn load() -> Result<CacheRuntimeConfig, RuntimeConfigError> {
    Ok(CacheRuntimeConfig {
        default_ttl_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_DEFAULT_TTL",
            env_string("CACHE_DEFAULT_TTL").as_deref(),
            300,
        )?),
        health_check_ttl_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_HEALTH_CHECK_TTL",
            env_string("CACHE_HEALTH_CHECK_TTL").as_deref(),
            5,
        )?),
        cleanup_interval_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_CLEANUP_INTERVAL",
            env_string("CACHE_CLEANUP_INTERVAL").as_deref(),
            60,
        )?),
        disk_cleanup_interval_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_DISK_CLEANUP_INTERVAL",
            env_string("CACHE_DISK_CLEANUP_INTERVAL").as_deref(),
            300,
        )?),
        multi_tier_l1_ttl_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_MULTI_TIER_L1_TTL",
            env_string("CACHE_MULTI_TIER_L1_TTL").as_deref(),
            300,
        )?),
        multi_tier_l1_max_ttl_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_MULTI_TIER_L1_MAX_TTL",
            env_string("CACHE_MULTI_TIER_L1_MAX_TTL").as_deref(),
            300,
        )?),
        tiered_hot_ttl_secs: Reloadable::new(parse_u64(
            "HIGHPER_CACHE_TIERED_HOT_TTL",
            env_string("CACHE_TIERED_HOT_TTL").as_deref(),
            300,
        )?),
    })
}

fn parse_u64(env_var: &str, raw: Option<&str>, default: u64) -> Result<u64, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s.trim().parse::<u64>().map_err(|_| RuntimeConfigError::ParseError {
            env_var: env_var.into(),
            value: s.into(),
            expected: "u64",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn defaults() {
        for k in [
            "HIGHPER_CACHE_DEFAULT_TTL",
            "HIGHPER_CACHE_HEALTH_CHECK_TTL",
            "HIGHPER_CACHE_CLEANUP_INTERVAL",
            "HIGHPER_CACHE_DISK_CLEANUP_INTERVAL",
        ] {
            std::env::remove_var(k);
        }
        let cfg = load().unwrap();
        assert_eq!(*cfg.default_ttl_secs.get(), 300);
        assert_eq!(*cfg.health_check_ttl_secs.get(), 5);
        assert_eq!(*cfg.cleanup_interval_secs.get(), 60);
    }

    #[test]
    #[serial]
    fn parses_custom_ttl() {
        std::env::remove_var("HIGHPER_CACHE_DEFAULT_TTL");
        std::env::set_var("HIGHPER_CACHE_DEFAULT_TTL", "600");
        let cfg = load().unwrap();
        assert_eq!(*cfg.default_ttl_secs.get(), 600);
        std::env::remove_var("HIGHPER_CACHE_DEFAULT_TTL");
    }
}
