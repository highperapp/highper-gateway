//! Circuit-breaker tunables (B7 partial — env vars only; trait-extraction
//! deferred to Phase 0.D per ROADMAP §4.4 row 3).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct CircuitBreakerRuntimeConfig {
    pub failure_threshold: Reloadable<u32>,         // HIGHPER_CIRCUIT_BREAKER_FAILURE_THRESHOLD (default 5)
    pub success_threshold: Reloadable<u32>,         // HIGHPER_CIRCUIT_BREAKER_SUCCESS_THRESHOLD (default 2)
    pub timeout_secs: Reloadable<u64>,              // HIGHPER_CIRCUIT_BREAKER_TIMEOUT (default 30)
    pub half_open_max_requests: Reloadable<u32>,    // HIGHPER_CIRCUIT_BREAKER_HALF_OPEN_MAX (default 3)
}

impl Default for CircuitBreakerRuntimeConfig {
    fn default() -> Self {
        Self {
            failure_threshold: Reloadable::new(5),
            success_threshold: Reloadable::new(2),
            timeout_secs: Reloadable::new(30),
            half_open_max_requests: Reloadable::new(3),
        }
    }
}

pub(crate) fn load() -> Result<CircuitBreakerRuntimeConfig, RuntimeConfigError> {
    let failure_threshold = parse_u32(
        "HIGHPER_CIRCUIT_BREAKER_FAILURE_THRESHOLD",
        env_string("CIRCUIT_BREAKER_FAILURE_THRESHOLD").as_deref(),
        5,
    )?;
    let success_threshold = parse_u32(
        "HIGHPER_CIRCUIT_BREAKER_SUCCESS_THRESHOLD",
        env_string("CIRCUIT_BREAKER_SUCCESS_THRESHOLD").as_deref(),
        2,
    )?;
    let timeout_secs = parse_u64(
        "HIGHPER_CIRCUIT_BREAKER_TIMEOUT",
        env_string("CIRCUIT_BREAKER_TIMEOUT").as_deref(),
        30,
    )?;
    let half_open_max_requests = parse_u32(
        "HIGHPER_CIRCUIT_BREAKER_HALF_OPEN_MAX",
        env_string("CIRCUIT_BREAKER_HALF_OPEN_MAX").as_deref(),
        3,
    )?;

    if failure_threshold == 0 {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_CIRCUIT_BREAKER_FAILURE_THRESHOLD".into(),
            value: "0".into(),
            valid_range: ">= 1",
        });
    }

    Ok(CircuitBreakerRuntimeConfig {
        failure_threshold: Reloadable::new(failure_threshold),
        success_threshold: Reloadable::new(success_threshold),
        timeout_secs: Reloadable::new(timeout_secs),
        half_open_max_requests: Reloadable::new(half_open_max_requests),
    })
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

    fn clear() {
        for k in [
            "HIGHPER_CIRCUIT_BREAKER_FAILURE_THRESHOLD",
            "HIGHPER_CIRCUIT_BREAKER_SUCCESS_THRESHOLD",
            "HIGHPER_CIRCUIT_BREAKER_TIMEOUT",
            "HIGHPER_CIRCUIT_BREAKER_HALF_OPEN_MAX",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.failure_threshold.get(), 5);
        assert_eq!(*cfg.timeout_secs.get(), 30);
    }

    #[test]
    #[serial]
    fn rejects_failure_threshold_zero() {
        clear();
        std::env::set_var("HIGHPER_CIRCUIT_BREAKER_FAILURE_THRESHOLD", "0");
        assert!(matches!(load(), Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }
}
