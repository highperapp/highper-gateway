//! Graceful shutdown / drain timeouts (B14 release blocker).
//!
//! Stage 2 lands the section. Consumers (the listener-drain logic + spawned
//! task supervisor) wire to `runtime_config::current().shutdown.*` in
//! Stage 3 / B14 workstream.

use std::time::Duration;

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct ShutdownRuntimeConfig {
    /// `HIGHPER_SHUTDOWN_DRAIN` — graceful drain window for in-flight requests
    /// (default 30s). Consumers begin rejecting new connections at SIGTERM
    /// and let in-flight finish within this window.
    pub drain_secs: Reloadable<Duration>,

    /// `HIGHPER_SHUTDOWN_FORCE_KILL` — total time before SIGKILL-equivalent
    /// (default 60s). Must be > drain_secs.
    pub force_kill_after_secs: Reloadable<Duration>,

    /// `HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN` — drain window for tokio::spawn'd
    /// background tasks (default 10s). B14 main concern.
    pub spawn_task_drain_secs: Reloadable<Duration>,
}

impl Default for ShutdownRuntimeConfig {
    fn default() -> Self {
        Self {
            drain_secs: Reloadable::new(Duration::from_secs(30)),
            force_kill_after_secs: Reloadable::new(Duration::from_secs(60)),
            spawn_task_drain_secs: Reloadable::new(Duration::from_secs(10)),
        }
    }
}

pub(crate) fn load() -> Result<ShutdownRuntimeConfig, RuntimeConfigError> {
    let drain_secs = parse_duration_seconds(
        "HIGHPER_SHUTDOWN_DRAIN",
        env_string("SHUTDOWN_DRAIN").as_deref(),
        30,
    )?;
    let force_kill_after_secs = parse_duration_seconds(
        "HIGHPER_SHUTDOWN_FORCE_KILL",
        env_string("SHUTDOWN_FORCE_KILL").as_deref(),
        60,
    )?;
    let spawn_task_drain_secs = parse_duration_seconds(
        "HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN",
        env_string("SHUTDOWN_SPAWN_TASK_DRAIN").as_deref(),
        10,
    )?;

    if force_kill_after_secs <= drain_secs {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "HIGHPER_SHUTDOWN_FORCE_KILL must be > HIGHPER_SHUTDOWN_DRAIN",
            details: format!(
                "got drain={drain_secs}s force_kill={force_kill_after_secs}s"
            ),
        });
    }

    Ok(ShutdownRuntimeConfig {
        drain_secs: Reloadable::new(Duration::from_secs(drain_secs)),
        force_kill_after_secs: Reloadable::new(Duration::from_secs(force_kill_after_secs)),
        spawn_task_drain_secs: Reloadable::new(Duration::from_secs(spawn_task_drain_secs)),
    })
}

fn parse_duration_seconds(
    env_var: &str,
    raw: Option<&str>,
    default: u64,
) -> Result<u64, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s.trim().parse::<u64>().map_err(|_| RuntimeConfigError::ParseError {
            env_var: env_var.into(),
            value: s.into(),
            expected: "u64 (seconds)",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_SHUTDOWN_DRAIN",
            "HIGHPER_SHUTDOWN_FORCE_KILL",
            "HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_drain_30_force_kill_60_spawn_10() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.drain_secs.get(), Duration::from_secs(30));
        assert_eq!(*cfg.force_kill_after_secs.get(), Duration::from_secs(60));
        assert_eq!(*cfg.spawn_task_drain_secs.get(), Duration::from_secs(10));
    }

    #[test]
    #[serial]
    fn rejects_force_kill_lte_drain() {
        clear();
        std::env::set_var("HIGHPER_SHUTDOWN_DRAIN", "60");
        std::env::set_var("HIGHPER_SHUTDOWN_FORCE_KILL", "60");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::InvalidCombination { .. })));
        clear();
    }

    #[test]
    #[serial]
    fn parses_custom_values() {
        clear();
        std::env::set_var("HIGHPER_SHUTDOWN_DRAIN", "45");
        std::env::set_var("HIGHPER_SHUTDOWN_FORCE_KILL", "90");
        let cfg = load().unwrap();
        assert_eq!(*cfg.drain_secs.get(), Duration::from_secs(45));
        assert_eq!(*cfg.force_kill_after_secs.get(), Duration::from_secs(90));
        clear();
    }
}
