//! Rate-limit tunables (B4 surface — distributed mode + X-Forwarded-For trust).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone, Default)]
pub struct RatelimitRuntimeConfig {
    pub mode: RatelimitMode, // HIGHPER_RATELIMIT_MODE = local|distributed (Restart)
    pub key_shards: u32,     // HIGHPER_RATELIMIT_KEY_SHARDS (default 1; Restart)
    pub redis_fail_mode: Reloadable<RedisFailMode>, // HIGHPER_RATELIMIT_REDIS_FAIL_MODE
    pub xff_trust_mode: Reloadable<XffTrustMode>, // HIGHPER_RATELIMIT_XFF_TRUST = none|first|last
    pub default_burst: Reloadable<u32>, // HIGHPER_RATELIMIT_DEFAULT_BURST (default 100)
    pub default_window_secs: Reloadable<u64>, // HIGHPER_RATELIMIT_DEFAULT_WINDOW (default 60)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RatelimitMode {
    #[default]
    Local,
    Distributed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RedisFailMode {
    #[default]
    LocalFallback,
    FailOpen,
    FailClosed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum XffTrustMode {
    /// Don't trust X-Forwarded-For at all (use the direct peer address).
    #[default]
    None,
    /// Trust the *first* hop in `X-Forwarded-For` (matches "trust the
    /// outermost proxy"; common when behind a single trusted load balancer).
    First,
    /// Trust the *last* hop (matches "trust the innermost proxy"; useful
    /// when only the last hop is trusted, e.g., a service mesh sidecar).
    Last,
}

pub(crate) fn load() -> Result<RatelimitRuntimeConfig, RuntimeConfigError> {
    let mode = match env_string("RATELIMIT_MODE").as_deref() {
        None | Some("local") => RatelimitMode::Local,
        Some("distributed") => RatelimitMode::Distributed,
        Some(o) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_RATELIMIT_MODE".into(),
                value: o.into(),
                expected: "local|distributed",
            });
        }
    };
    let key_shards = parse_u32(
        "HIGHPER_RATELIMIT_KEY_SHARDS",
        env_string("RATELIMIT_KEY_SHARDS").as_deref(),
        1,
    )?;
    if key_shards == 0 {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_RATELIMIT_KEY_SHARDS".into(),
            value: "0".into(),
            valid_range: ">= 1",
        });
    }
    let redis_fail_mode = match env_string("RATELIMIT_REDIS_FAIL_MODE").as_deref() {
        None | Some("local_fallback") => RedisFailMode::LocalFallback,
        Some("fail_open") => RedisFailMode::FailOpen,
        Some("fail_closed") => RedisFailMode::FailClosed,
        Some(o) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_RATELIMIT_REDIS_FAIL_MODE".into(),
                value: o.into(),
                expected: "local_fallback|fail_open|fail_closed",
            });
        }
    };
    let xff_trust_mode = match env_string("RATELIMIT_XFF_TRUST").as_deref() {
        None | Some("none") => XffTrustMode::None,
        Some("first") => XffTrustMode::First,
        Some("last") => XffTrustMode::Last,
        Some(o) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_RATELIMIT_XFF_TRUST".into(),
                value: o.into(),
                expected: "none|first|last",
            });
        }
    };
    let default_burst = parse_u32(
        "HIGHPER_RATELIMIT_DEFAULT_BURST",
        env_string("RATELIMIT_DEFAULT_BURST").as_deref(),
        100,
    )?;
    let default_window_secs = parse_u64(
        "HIGHPER_RATELIMIT_DEFAULT_WINDOW",
        env_string("RATELIMIT_DEFAULT_WINDOW").as_deref(),
        60,
    )?;

    Ok(RatelimitRuntimeConfig {
        mode,
        key_shards,
        redis_fail_mode: Reloadable::new(redis_fail_mode),
        xff_trust_mode: Reloadable::new(xff_trust_mode),
        default_burst: Reloadable::new(default_burst),
        default_window_secs: Reloadable::new(default_window_secs),
    })
}

fn parse_u32(env_var: &str, raw: Option<&str>, default: u32) -> Result<u32, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s
            .trim()
            .parse::<u32>()
            .map_err(|_| RuntimeConfigError::ParseError {
                env_var: env_var.into(),
                value: s.into(),
                expected: "u32",
            }),
    }
}

fn parse_u64(env_var: &str, raw: Option<&str>, default: u64) -> Result<u64, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s
            .trim()
            .parse::<u64>()
            .map_err(|_| RuntimeConfigError::ParseError {
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

    fn clear() {
        for k in [
            "HIGHPER_RATELIMIT_MODE",
            "HIGHPER_RATELIMIT_KEY_SHARDS",
            "HIGHPER_RATELIMIT_REDIS_FAIL_MODE",
            "HIGHPER_RATELIMIT_XFF_TRUST",
            "HIGHPER_RATELIMIT_DEFAULT_BURST",
            "HIGHPER_RATELIMIT_DEFAULT_WINDOW",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_local_xff_none() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(cfg.mode, RatelimitMode::Local);
        assert_eq!(*cfg.xff_trust_mode.get(), XffTrustMode::None);
        assert_eq!(cfg.key_shards, 1);
    }

    #[test]
    #[serial]
    fn parses_distributed_xff_first() {
        clear();
        std::env::set_var("HIGHPER_RATELIMIT_MODE", "distributed");
        std::env::set_var("HIGHPER_RATELIMIT_XFF_TRUST", "first");
        let cfg = load().unwrap();
        assert_eq!(cfg.mode, RatelimitMode::Distributed);
        assert_eq!(*cfg.xff_trust_mode.get(), XffTrustMode::First);
        clear();
    }

    #[test]
    #[serial]
    fn rejects_key_shards_zero() {
        clear();
        std::env::set_var("HIGHPER_RATELIMIT_KEY_SHARDS", "0");
        assert!(matches!(load(), Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }
}
