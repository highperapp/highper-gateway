//! HTTP/3 / QUIC tunables (B8 surface; Workstream 0.J Stage 3).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct Http3RuntimeConfig {
    pub max_concurrent_streams: Reloadable<u32>,    // HIGHPER_HTTP3_MAX_CONCURRENT_STREAMS (default 256)
    pub max_field_section_size: Reloadable<u32>,    // HIGHPER_HTTP3_MAX_FIELD_SECTION_SIZE (default 16384 bytes)
    pub idle_timeout_secs: Reloadable<u64>,         // HIGHPER_HTTP3_IDLE_TIMEOUT (default 30s)
    pub migration_window_secs: Reloadable<u64>,     // HIGHPER_HTTP3_MIGRATION_WINDOW (default 5s) - B8
    pub initial_max_data: Reloadable<u64>,          // HIGHPER_HTTP3_INITIAL_MAX_DATA (default 10 MB)
    pub max_buffered_pkts: Reloadable<u32>,         // HIGHPER_HTTP3_MAX_BUFFERED_PKTS (default 1024) - B8 unbounded
}

impl Default for Http3RuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: Reloadable::new(256),
            max_field_section_size: Reloadable::new(16384),
            idle_timeout_secs: Reloadable::new(30),
            migration_window_secs: Reloadable::new(5),
            initial_max_data: Reloadable::new(10 * 1024 * 1024),
            max_buffered_pkts: Reloadable::new(1024),
        }
    }
}

pub(crate) fn load() -> Result<Http3RuntimeConfig, RuntimeConfigError> {
    Ok(Http3RuntimeConfig {
        max_concurrent_streams: Reloadable::new(parse_u32(
            "HIGHPER_HTTP3_MAX_CONCURRENT_STREAMS",
            env_string("HTTP3_MAX_CONCURRENT_STREAMS").as_deref(),
            256,
        )?),
        max_field_section_size: Reloadable::new(parse_u32(
            "HIGHPER_HTTP3_MAX_FIELD_SECTION_SIZE",
            env_string("HTTP3_MAX_FIELD_SECTION_SIZE").as_deref(),
            16384,
        )?),
        idle_timeout_secs: Reloadable::new(parse_u64(
            "HIGHPER_HTTP3_IDLE_TIMEOUT",
            env_string("HTTP3_IDLE_TIMEOUT").as_deref(),
            30,
        )?),
        migration_window_secs: Reloadable::new(parse_u64(
            "HIGHPER_HTTP3_MIGRATION_WINDOW",
            env_string("HTTP3_MIGRATION_WINDOW").as_deref(),
            5,
        )?),
        initial_max_data: Reloadable::new(parse_u64(
            "HIGHPER_HTTP3_INITIAL_MAX_DATA",
            env_string("HTTP3_INITIAL_MAX_DATA").as_deref(),
            10 * 1024 * 1024,
        )?),
        max_buffered_pkts: Reloadable::new(parse_u32(
            "HIGHPER_HTTP3_MAX_BUFFERED_PKTS",
            env_string("HTTP3_MAX_BUFFERED_PKTS").as_deref(),
            1024,
        )?),
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

    #[test]
    #[serial]
    fn defaults() {
        for k in ["HIGHPER_HTTP3_MAX_CONCURRENT_STREAMS", "HIGHPER_HTTP3_MAX_BUFFERED_PKTS"] {
            std::env::remove_var(k);
        }
        let cfg = load().unwrap();
        assert_eq!(*cfg.max_concurrent_streams.get(), 256);
        assert_eq!(*cfg.max_buffered_pkts.get(), 1024);
    }
}
