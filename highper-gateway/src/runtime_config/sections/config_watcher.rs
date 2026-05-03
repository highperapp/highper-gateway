//! File-system config-watcher tunables (Workstream 0.J Stage 3).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct ConfigWatcherRuntimeConfig {
    pub poll_interval_secs: Reloadable<u64>,         // HIGHPER_CONFIG_WATCHER_POLL_INTERVAL (default 2)
    pub debounce_secs: Reloadable<u64>,              // HIGHPER_CONFIG_WATCHER_DEBOUNCE (default 1)
    pub max_reload_attempts: Reloadable<u32>,        // HIGHPER_CONFIG_WATCHER_MAX_RELOAD_ATTEMPTS (default 3)
}

impl Default for ConfigWatcherRuntimeConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: Reloadable::new(2),
            debounce_secs: Reloadable::new(1),
            max_reload_attempts: Reloadable::new(3),
        }
    }
}

pub(crate) fn load() -> Result<ConfigWatcherRuntimeConfig, RuntimeConfigError> {
    let poll_interval_secs = parse_u64(
        "HIGHPER_CONFIG_WATCHER_POLL_INTERVAL",
        env_string("CONFIG_WATCHER_POLL_INTERVAL").as_deref(),
        2,
    )?;
    let debounce_secs = parse_u64(
        "HIGHPER_CONFIG_WATCHER_DEBOUNCE",
        env_string("CONFIG_WATCHER_DEBOUNCE").as_deref(),
        1,
    )?;
    let max_reload_attempts = parse_u32(
        "HIGHPER_CONFIG_WATCHER_MAX_RELOAD_ATTEMPTS",
        env_string("CONFIG_WATCHER_MAX_RELOAD_ATTEMPTS").as_deref(),
        3,
    )?;

    Ok(ConfigWatcherRuntimeConfig {
        poll_interval_secs: Reloadable::new(poll_interval_secs),
        debounce_secs: Reloadable::new(debounce_secs),
        max_reload_attempts: Reloadable::new(max_reload_attempts),
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

fn parse_u32(env_var: &str, raw: Option<&str>, default: u32) -> Result<u32, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s.trim().parse::<u32>().map_err(|_| RuntimeConfigError::ParseError {
            env_var: env_var.into(),
            value: s.into(),
            expected: "u32",
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
        for k in ["HIGHPER_CONFIG_WATCHER_POLL_INTERVAL", "HIGHPER_CONFIG_WATCHER_DEBOUNCE", "HIGHPER_CONFIG_WATCHER_MAX_RELOAD_ATTEMPTS"] {
            std::env::remove_var(k);
        }
        let cfg = load().unwrap();
        assert_eq!(*cfg.poll_interval_secs.get(), 2);
        assert_eq!(*cfg.debounce_secs.get(), 1);
        assert_eq!(*cfg.max_reload_attempts.get(), 3);
    }
}
