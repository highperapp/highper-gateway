//! UC16 (AI/LLM gateway) runtime tunables.
//!
//! Sources:
//! - `docs/planning/USECASE_16_AI_LLM_GATEWAY.md` §3.4 (routing), §3.5
//!   (streaming), §3.6 (cluster behaviour), §5.5 (pricing), §6.0 (cache),
//!   §7.1.3 (virtual-key pepper).
//! - `docs/planning/ROADMAP.md` §5 Phase 0.J task lines 726–733.
//! - `docs/planning/SETTINGS_SCAFFOLD.md` §3.2 (sub-struct shape).
//!
//! 25 env vars across 5 logical groups: state/storage, caching/vector,
//! routing, streaming, cluster behaviour. Plus a nested
//! `AiPricingRuntimeConfig` (5 fields).
//!
//! Cross-subsystem invariants ("AI cache=valkey requires Cluster Type B")
//! live in `loader::validate_cross_subsystem` per
//! `docs/planning/RUNTIME_CONFIG_STAGE2_PR_PLAN.md` §3.2.

use std::net::SocketAddr;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError, SecretRef};

#[derive(Debug, Clone, Default)]
pub struct AiRuntimeConfig {
    // ── secrets / state ───────────────────────────────────────────
    pub key_pepper: Reloadable<Option<SecretRef>>,
    pub state_backend: AiStateBackend,
    pub state_path: PathBuf,

    // ── caching / vector ──────────────────────────────────────────
    pub cache_backend: Reloadable<AiCacheBackend>,
    pub vector_backend: Reloadable<AiVectorBackend>,
    pub vector_addrs: Reloadable<Vec<SocketAddr>>,
    pub vector_auth: Reloadable<Option<SecretRef>>,

    // ── routing ───────────────────────────────────────────────────
    pub retry_budget: Reloadable<u32>,
    pub cooldown_backend: Reloadable<AiCooldownBackend>,
    pub default_backoff_ms_min: Reloadable<u64>,
    pub default_backoff_ms_max: Reloadable<u64>,
    pub default_cooldown_secs_no_header: Reloadable<u64>,

    // ── streaming ─────────────────────────────────────────────────
    pub stream_buffer_depth: Reloadable<u32>,
    pub stream_buffer_overflow: Reloadable<StreamOverflow>,
    pub default_cancel_on_close: Reloadable<bool>,
    pub default_tpm_hard_stop: Reloadable<bool>,

    // ── cluster behaviour ─────────────────────────────────────────
    pub valkey_fail_mode: Reloadable<ValkeyFailMode>,
    pub token_quota_key_shards: u32,
    pub key_cache_ttl_secs: Reloadable<u64>,
    pub pricing_override_cache_ttl_secs: Reloadable<u64>,

    // ── nested pricing ────────────────────────────────────────────
    pub pricing: AiPricingRuntimeConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiStateBackend {
    #[default]
    Redb,
    RocksDb,
    ScyllaDb,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiCacheBackend {
    #[default]
    Valkey,
    Redis,
    Memory,
    Disk,
    MultiTier,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiVectorBackend {
    #[default]
    None,
    Qdrant,
    RedisStack,
    PgVector,
    Hnsw,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiCooldownBackend {
    #[default]
    Auto,
    Valkey,
    Local,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamOverflow {
    #[default]
    DropOldest,
    Block,
    Error,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValkeyFailMode {
    #[default]
    LocalFallback,
    FailOpen,
    FailClosed,
}

#[derive(Debug, Clone, Default)]
pub struct AiPricingRuntimeConfig {
    pub feed_url: Reloadable<Option<String>>,
    pub feed_sign_key: Reloadable<Option<SecretRef>>,
    pub refresh_interval: Reloadable<Duration>,
    pub refresh_fail_mode: Reloadable<PricingFailMode>,
    pub allow_free_tier: Reloadable<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PricingFailMode {
    #[default]
    LastKnownGood,
    FailClosed,
}

pub(crate) fn load() -> Result<AiRuntimeConfig, RuntimeConfigError> {
    let key_pepper = env_string("AI_KEY_PEPPER")
        .map(|v| SecretRef::parse("HIGHPER_AI_KEY_PEPPER", &v))
        .transpose()?;
    let state_backend = parse_state_backend(env_string("AI_STATE_BACKEND").as_deref())?;
    let state_path = env_string("AI_STATE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./data/highper-ai"));

    let cache_backend = parse_cache_backend(env_string("AI_CACHE_BACKEND").as_deref())?;
    let vector_backend = parse_vector_backend(env_string("AI_VECTOR_BACKEND").as_deref())?;
    let vector_addrs = parse_addr_list(
        "HIGHPER_AI_VECTOR_ADDRS",
        env_string("AI_VECTOR_ADDRS"),
    )?;
    let vector_auth = env_string("AI_VECTOR_AUTH")
        .map(|v| SecretRef::parse("HIGHPER_AI_VECTOR_AUTH", &v))
        .transpose()?;

    let retry_budget = parse_u32_in_range(
        "HIGHPER_AI_RETRY_BUDGET",
        env_string("AI_RETRY_BUDGET").as_deref(),
        3,
        1..=10,
    )?;
    let cooldown_backend = parse_cooldown_backend(env_string("AI_COOLDOWN_BACKEND").as_deref())?;
    let default_backoff_ms_min = parse_u64(
        "HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN",
        env_string("AI_DEFAULT_BACKOFF_MS_MIN").as_deref(),
        50,
    )?;
    let default_backoff_ms_max = parse_u64(
        "HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX",
        env_string("AI_DEFAULT_BACKOFF_MS_MAX").as_deref(),
        200,
    )?;
    let default_cooldown_secs_no_header = parse_u64(
        "HIGHPER_AI_DEFAULT_COOLDOWN_SECS_NO_HEADER",
        env_string("AI_DEFAULT_COOLDOWN_SECS_NO_HEADER").as_deref(),
        30,
    )?;

    let stream_buffer_depth = parse_u32_in_range(
        "HIGHPER_AI_STREAM_BUFFER_DEPTH",
        env_string("AI_STREAM_BUFFER_DEPTH").as_deref(),
        64,
        1..=4096,
    )?;
    let stream_buffer_overflow =
        parse_stream_overflow(env_string("AI_STREAM_BUFFER_OVERFLOW_POLICY").as_deref())?;
    let default_cancel_on_close = parse_bool_default_true(
        "HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE",
        env_string("AI_DEFAULT_CANCEL_ON_CLOSE").as_deref(),
    )?;
    let default_tpm_hard_stop = parse_bool_default_false(
        "HIGHPER_AI_DEFAULT_TPM_HARD_STOP",
        env_string("AI_DEFAULT_TPM_HARD_STOP").as_deref(),
    )?;

    let valkey_fail_mode = parse_valkey_fail_mode(env_string("AI_VALKEY_FAIL_MODE").as_deref())?;
    let token_quota_key_shards = parse_u32_in_range(
        "HIGHPER_AI_TOKEN_QUOTA_KEY_SHARDS",
        env_string("AI_TOKEN_QUOTA_KEY_SHARDS").as_deref(),
        1,
        1..=1024,
    )?;
    let key_cache_ttl_secs = parse_u64(
        "HIGHPER_AI_KEY_CACHE_TTL_SECS",
        env_string("AI_KEY_CACHE_TTL_SECS").as_deref(),
        300,
    )?;
    let pricing_override_cache_ttl_secs = parse_u64(
        "HIGHPER_AI_PRICING_OVERRIDE_CACHE_TTL_SECS",
        env_string("AI_PRICING_OVERRIDE_CACHE_TTL_SECS").as_deref(),
        60,
    )?;

    let pricing = load_pricing()?;

    let cfg = AiRuntimeConfig {
        key_pepper: Reloadable::new(key_pepper),
        state_backend,
        state_path,
        cache_backend: Reloadable::new(cache_backend),
        vector_backend: Reloadable::new(vector_backend),
        vector_addrs: Reloadable::new(vector_addrs),
        vector_auth: Reloadable::new(vector_auth),
        retry_budget: Reloadable::new(retry_budget),
        cooldown_backend: Reloadable::new(cooldown_backend),
        default_backoff_ms_min: Reloadable::new(default_backoff_ms_min),
        default_backoff_ms_max: Reloadable::new(default_backoff_ms_max),
        default_cooldown_secs_no_header: Reloadable::new(default_cooldown_secs_no_header),
        stream_buffer_depth: Reloadable::new(stream_buffer_depth),
        stream_buffer_overflow: Reloadable::new(stream_buffer_overflow),
        default_cancel_on_close: Reloadable::new(default_cancel_on_close),
        default_tpm_hard_stop: Reloadable::new(default_tpm_hard_stop),
        valkey_fail_mode: Reloadable::new(valkey_fail_mode),
        token_quota_key_shards,
        key_cache_ttl_secs: Reloadable::new(key_cache_ttl_secs),
        pricing_override_cache_ttl_secs: Reloadable::new(pricing_override_cache_ttl_secs),
        pricing,
    };

    validate_ai(&cfg)?;
    Ok(cfg)
}

fn load_pricing() -> Result<AiPricingRuntimeConfig, RuntimeConfigError> {
    let feed_url = env_string("AI_PRICING_FEED_URL");
    let feed_sign_key = env_string("AI_PRICING_FEED_SIGN_KEY")
        .map(|v| SecretRef::parse("HIGHPER_AI_PRICING_FEED_SIGN_KEY", &v))
        .transpose()?;
    let refresh_interval_secs = parse_u64(
        "HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS",
        env_string("AI_PRICING_REFRESH_INTERVAL_SECS").as_deref(),
        604_800, // 7 days
    )?;
    let refresh_fail_mode =
        parse_pricing_fail_mode(env_string("AI_PRICING_REFRESH_FAIL_MODE").as_deref())?;
    let allow_free_tier = parse_bool_default_false(
        "HIGHPER_AI_ALLOW_FREE_TIER",
        env_string("AI_ALLOW_FREE_TIER").as_deref(),
    )?;

    Ok(AiPricingRuntimeConfig {
        feed_url: Reloadable::new(feed_url),
        feed_sign_key: Reloadable::new(feed_sign_key),
        refresh_interval: Reloadable::new(Duration::from_secs(refresh_interval_secs)),
        refresh_fail_mode: Reloadable::new(refresh_fail_mode),
        allow_free_tier: Reloadable::new(allow_free_tier),
    })
}

fn validate_ai(cfg: &AiRuntimeConfig) -> Result<(), RuntimeConfigError> {
    // Vector backend != None requires addrs non-empty (HNSW is local; skip).
    let vb = *cfg.vector_backend.get();
    let needs_addrs =
        matches!(vb, AiVectorBackend::Qdrant | AiVectorBackend::RedisStack | AiVectorBackend::PgVector);
    if needs_addrs && cfg.vector_addrs.get().is_empty() {
        return Err(RuntimeConfigError::MissingRequired {
            env_var: "HIGHPER_AI_VECTOR_ADDRS".into(),
            required_because: "vector_backend is qdrant/redis-stack/pgvector",
        });
    }

    // backoff_ms_min < backoff_ms_max
    if *cfg.default_backoff_ms_min.get() >= *cfg.default_backoff_ms_max.get() {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "AI default_backoff_ms_min must be < default_backoff_ms_max",
            details: format!(
                "got min={} max={}",
                cfg.default_backoff_ms_min.get(),
                cfg.default_backoff_ms_max.get()
            ),
        });
    }

    // pricing.refresh_interval >= 3600s
    if *cfg.pricing.refresh_interval.get() < Duration::from_secs(3600) {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS".into(),
            value: format!("{:?}", cfg.pricing.refresh_interval.get()),
            valid_range: ">= 3600 (1h)",
        });
    }

    Ok(())
}

// ── parsers ──────────────────────────────────────────────────────────────

fn parse_state_backend(v: Option<&str>) -> Result<AiStateBackend, RuntimeConfigError> {
    match v {
        None | Some("redb") => Ok(AiStateBackend::Redb),
        Some("rocksdb") => Ok(AiStateBackend::RocksDb),
        Some("scylladb") => Ok(AiStateBackend::ScyllaDb),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_STATE_BACKEND".into(),
            value: other.into(),
            expected: "redb|rocksdb|scylladb",
        }),
    }
}

fn parse_cache_backend(v: Option<&str>) -> Result<AiCacheBackend, RuntimeConfigError> {
    match v {
        None | Some("valkey") => Ok(AiCacheBackend::Valkey),
        Some("redis") => Ok(AiCacheBackend::Redis),
        Some("memory") => Ok(AiCacheBackend::Memory),
        Some("disk") => Ok(AiCacheBackend::Disk),
        Some("multi-tier") => Ok(AiCacheBackend::MultiTier),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_CACHE_BACKEND".into(),
            value: other.into(),
            expected: "valkey|redis|memory|disk|multi-tier",
        }),
    }
}

fn parse_vector_backend(v: Option<&str>) -> Result<AiVectorBackend, RuntimeConfigError> {
    match v {
        None | Some("none") => Ok(AiVectorBackend::None),
        Some("qdrant") => Ok(AiVectorBackend::Qdrant),
        Some("redis-stack") => Ok(AiVectorBackend::RedisStack),
        Some("pgvector") => Ok(AiVectorBackend::PgVector),
        Some("hnsw") => Ok(AiVectorBackend::Hnsw),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_VECTOR_BACKEND".into(),
            value: other.into(),
            expected: "none|qdrant|redis-stack|pgvector|hnsw",
        }),
    }
}

fn parse_cooldown_backend(v: Option<&str>) -> Result<AiCooldownBackend, RuntimeConfigError> {
    match v {
        None | Some("auto") => Ok(AiCooldownBackend::Auto),
        Some("valkey") => Ok(AiCooldownBackend::Valkey),
        Some("local") => Ok(AiCooldownBackend::Local),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_COOLDOWN_BACKEND".into(),
            value: other.into(),
            expected: "auto|valkey|local",
        }),
    }
}

fn parse_stream_overflow(v: Option<&str>) -> Result<StreamOverflow, RuntimeConfigError> {
    match v {
        None | Some("drop_oldest") => Ok(StreamOverflow::DropOldest),
        Some("block") => Ok(StreamOverflow::Block),
        Some("error") => Ok(StreamOverflow::Error),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY".into(),
            value: other.into(),
            expected: "drop_oldest|block|error",
        }),
    }
}

fn parse_valkey_fail_mode(v: Option<&str>) -> Result<ValkeyFailMode, RuntimeConfigError> {
    match v {
        None | Some("local_fallback") => Ok(ValkeyFailMode::LocalFallback),
        Some("fail_open") => Ok(ValkeyFailMode::FailOpen),
        Some("fail_closed") => Ok(ValkeyFailMode::FailClosed),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_VALKEY_FAIL_MODE".into(),
            value: other.into(),
            expected: "local_fallback|fail_open|fail_closed",
        }),
    }
}

fn parse_pricing_fail_mode(v: Option<&str>) -> Result<PricingFailMode, RuntimeConfigError> {
    match v {
        None | Some("last_known_good") => Ok(PricingFailMode::LastKnownGood),
        Some("fail_closed") => Ok(PricingFailMode::FailClosed),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_AI_PRICING_REFRESH_FAIL_MODE".into(),
            value: other.into(),
            expected: "last_known_good|fail_closed",
        }),
    }
}

fn parse_u32_in_range(
    env_var: &str,
    raw: Option<&str>,
    default: u32,
    range: RangeInclusive<u32>,
) -> Result<u32, RuntimeConfigError> {
    let value = match raw {
        None => default,
        Some(s) => s.parse::<u32>().map_err(|_| RuntimeConfigError::ParseError {
            env_var: env_var.into(),
            value: s.into(),
            expected: "u32",
        })?,
    };
    if !range.contains(&value) {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: env_var.into(),
            value: value.to_string(),
            valid_range: Box::leak(format!("{}..={}", range.start(), range.end()).into_boxed_str()),
        });
    }
    Ok(value)
}

fn parse_u64(
    env_var: &str,
    raw: Option<&str>,
    default: u64,
) -> Result<u64, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s.parse::<u64>().map_err(|_| RuntimeConfigError::ParseError {
            env_var: env_var.into(),
            value: s.into(),
            expected: "u64",
        }),
    }
}

fn parse_bool_default_true(
    env_var: &str,
    raw: Option<&str>,
) -> Result<bool, RuntimeConfigError> {
    match raw {
        None => Ok(true),
        Some(s) => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(RuntimeConfigError::ParseError {
                env_var: env_var.into(),
                value: s.into(),
                expected: "true|false",
            }),
        },
    }
}

fn parse_bool_default_false(
    env_var: &str,
    raw: Option<&str>,
) -> Result<bool, RuntimeConfigError> {
    match raw {
        None => Ok(false),
        Some(s) => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(RuntimeConfigError::ParseError {
                env_var: env_var.into(),
                value: s.into(),
                expected: "true|false",
            }),
        },
    }
}

fn parse_addr_list(
    env_var: &str,
    raw: Option<String>,
) -> Result<Vec<SocketAddr>, RuntimeConfigError> {
    let Some(value) = raw else { return Ok(Vec::new()) };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(|s| {
            let s = s.trim();
            s.parse::<SocketAddr>().map_err(|_| RuntimeConfigError::ParseError {
                env_var: env_var.into(),
                value: s.into(),
                expected: "host:port (comma-separated)",
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_ai_env() {
        for k in [
            "HIGHPER_AI_KEY_PEPPER",
            "HIGHPER_AI_STATE_BACKEND",
            "HIGHPER_AI_STATE_PATH",
            "HIGHPER_AI_CACHE_BACKEND",
            "HIGHPER_AI_VECTOR_BACKEND",
            "HIGHPER_AI_VECTOR_ADDRS",
            "HIGHPER_AI_VECTOR_AUTH",
            "HIGHPER_AI_RETRY_BUDGET",
            "HIGHPER_AI_COOLDOWN_BACKEND",
            "HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN",
            "HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX",
            "HIGHPER_AI_DEFAULT_COOLDOWN_SECS_NO_HEADER",
            "HIGHPER_AI_STREAM_BUFFER_DEPTH",
            "HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY",
            "HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE",
            "HIGHPER_AI_DEFAULT_TPM_HARD_STOP",
            "HIGHPER_AI_VALKEY_FAIL_MODE",
            "HIGHPER_AI_TOKEN_QUOTA_KEY_SHARDS",
            "HIGHPER_AI_KEY_CACHE_TTL_SECS",
            "HIGHPER_AI_PRICING_OVERRIDE_CACHE_TTL_SECS",
            "HIGHPER_AI_PRICING_FEED_URL",
            "HIGHPER_AI_PRICING_FEED_SIGN_KEY",
            "HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS",
            "HIGHPER_AI_PRICING_REFRESH_FAIL_MODE",
            "HIGHPER_AI_ALLOW_FREE_TIER",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_when_no_env_vars() {
        clear_ai_env();
        let cfg = load().unwrap();
        assert_eq!(cfg.state_backend, AiStateBackend::Redb);
        assert_eq!(*cfg.cache_backend.get(), AiCacheBackend::Valkey);
        assert_eq!(*cfg.vector_backend.get(), AiVectorBackend::None);
        assert_eq!(*cfg.retry_budget.get(), 3);
        assert_eq!(*cfg.cooldown_backend.get(), AiCooldownBackend::Auto);
        assert_eq!(*cfg.stream_buffer_depth.get(), 64);
        assert_eq!(*cfg.stream_buffer_overflow.get(), StreamOverflow::DropOldest);
        assert!(*cfg.default_cancel_on_close.get());
        assert!(!*cfg.default_tpm_hard_stop.get());
        assert_eq!(cfg.token_quota_key_shards, 1);
        assert_eq!(cfg.state_path, PathBuf::from("./data/highper-ai"));
        assert_eq!(
            *cfg.pricing.refresh_interval.get(),
            Duration::from_secs(604_800)
        );
    }

    #[test]
    #[serial]
    fn parses_state_backend_rocksdb() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_STATE_BACKEND", "rocksdb");
        let cfg = load().unwrap();
        assert_eq!(cfg.state_backend, AiStateBackend::RocksDb);
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn rejects_invalid_state_backend() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_STATE_BACKEND", "leveldb");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::ParseError { .. })));
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn rejects_retry_budget_above_10() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_RETRY_BUDGET", "11");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::OutOfRange { .. })));
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn rejects_vector_qdrant_without_addrs() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_VECTOR_BACKEND", "qdrant");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::MissingRequired { .. })));
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn accepts_vector_hnsw_without_addrs() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_VECTOR_BACKEND", "hnsw");
        let cfg = load().unwrap();
        assert_eq!(*cfg.vector_backend.get(), AiVectorBackend::Hnsw);
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn rejects_backoff_min_gte_max() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN", "200");
        std::env::set_var("HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX", "100");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::InvalidCombination { .. })));
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn rejects_pricing_refresh_below_1h() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS", "1800");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::OutOfRange { .. })));
        clear_ai_env();
    }

    #[test]
    #[serial]
    fn parses_pricing_fail_mode_fail_closed() {
        clear_ai_env();
        std::env::set_var("HIGHPER_AI_PRICING_REFRESH_FAIL_MODE", "fail_closed");
        let cfg = load().unwrap();
        assert_eq!(*cfg.pricing.refresh_fail_mode.get(), PricingFailMode::FailClosed);
        clear_ai_env();
    }
}
