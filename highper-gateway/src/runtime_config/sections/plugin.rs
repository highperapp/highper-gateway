//! Plugin lifecycle tunables.
//!
//! Migrates the previously-hardcoded values at `src/plugin/manager.rs:254`
//! (drain timeout) and `:265` (idle poll interval) into env-var-driven
//! defaults per `ROADMAP.md` §0.J line 727.

use std::time::Duration;

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct PluginRuntimeConfig {
    /// `HIGHPER_PLUGIN_DRAIN` — max time to wait for in-flight requests to
    /// finish before forcing plugin unload. Default 30s, validated >= 5s.
    pub drain: Reloadable<Duration>,

    /// `HIGHPER_PLUGIN_IDLE_POLL` — polling interval while waiting for the
    /// active-request count to reach zero. Default 100ms.
    pub idle_poll: Reloadable<Duration>,

    /// `HIGHPER_PLUGIN_HOT_RELOAD_SETTLE` — settle delay after detecting a
    /// plugin file change before triggering reload (lets the file write
    /// complete on slow filesystems). Default 100ms. Stage 2 migration of
    /// the previously-hardcoded `Duration::from_millis(100)` at
    /// `src/plugin/hot_reload.rs:181`.
    pub hot_reload_settle: Reloadable<Duration>,
}

impl Default for PluginRuntimeConfig {
    fn default() -> Self {
        Self {
            drain: Reloadable::new(Duration::from_secs(30)),
            idle_poll: Reloadable::new(Duration::from_millis(100)),
            hot_reload_settle: Reloadable::new(Duration::from_millis(100)),
        }
    }
}

pub(crate) fn load() -> Result<PluginRuntimeConfig, RuntimeConfigError> {
    let drain = match env_string("PLUGIN_DRAIN") {
        None => Duration::from_secs(30),
        Some(raw) => parse_duration("HIGHPER_PLUGIN_DRAIN", &raw)?,
    };
    if drain < Duration::from_secs(5) {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_PLUGIN_DRAIN".into(),
            value: format!("{drain:?}"),
            valid_range: ">= 5s",
        });
    }

    let idle_poll = match env_string("PLUGIN_IDLE_POLL") {
        None => Duration::from_millis(100),
        Some(raw) => parse_duration("HIGHPER_PLUGIN_IDLE_POLL", &raw)?,
    };

    let hot_reload_settle = match env_string("PLUGIN_HOT_RELOAD_SETTLE") {
        None => Duration::from_millis(100),
        Some(raw) => parse_duration("HIGHPER_PLUGIN_HOT_RELOAD_SETTLE", &raw)?,
    };

    Ok(PluginRuntimeConfig {
        drain: Reloadable::new(drain),
        idle_poll: Reloadable::new(idle_poll),
        hot_reload_settle: Reloadable::new(hot_reload_settle),
    })
}

/// Strict duration parser that surfaces parse errors. The existing
/// `env_override::env_duration` silently returns `None` on parse failure,
/// which would mask misconfiguration here.
fn parse_duration(env_var: &str, raw: &str) -> Result<Duration, RuntimeConfigError> {
    let s = raw.trim();
    let err = || RuntimeConfigError::ParseError {
        env_var: env_var.to_string(),
        value: raw.to_string(),
        expected: "duration like 30s, 100ms, 5m, 1h",
    };

    let (num_str, unit) = if let Some(n) = s.strip_suffix("ms") {
        (n, "ms")
    } else if let Some(n) = s.strip_suffix('s') {
        (n, "s")
    } else if let Some(n) = s.strip_suffix('m') {
        (n, "m")
    } else if let Some(n) = s.strip_suffix('h') {
        (n, "h")
    } else if let Some(n) = s.strip_suffix('d') {
        (n, "d")
    } else {
        return Err(err());
    };

    let num: u64 = num_str.trim().parse().map_err(|_| err())?;

    Ok(match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        _ => unreachable!("unit checked above"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        std::env::remove_var("HIGHPER_PLUGIN_DRAIN");
        std::env::remove_var("HIGHPER_PLUGIN_IDLE_POLL");
    }

    #[test]
    #[serial]
    fn default_drain_is_30s() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.drain.get(), Duration::from_secs(30));
        assert_eq!(*cfg.idle_poll.get(), Duration::from_millis(100));
    }

    #[test]
    #[serial]
    fn parses_plugin_drain_45s() {
        clear();
        std::env::set_var("HIGHPER_PLUGIN_DRAIN", "45s");
        let cfg = load().unwrap();
        assert_eq!(*cfg.drain.get(), Duration::from_secs(45));
        clear();
    }

    #[test]
    #[serial]
    fn rejects_drain_below_5s() {
        clear();
        std::env::set_var("HIGHPER_PLUGIN_DRAIN", "2s");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }

    #[test]
    #[serial]
    fn rejects_unparseable_drain() {
        clear();
        std::env::set_var("HIGHPER_PLUGIN_DRAIN", "abc");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::ParseError { .. })));
        clear();
    }
}
