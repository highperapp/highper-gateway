//! Body-size limits (B12 release blocker — body-size centralization).
//!
//! Centralizes the scattered `* 1024 * 1024` literals across
//! `src/middleware/`, `src/proxy/`, `src/http/` (Stage 3 migration).
//! Stage 2 lands the section + loader; Stage 3 wires the consumers.

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct BodyRuntimeConfig {
    /// `HIGHPER_BODY_MAX_REQUEST` — max request body in bytes (default 10 MB).
    pub max_request_body: Reloadable<u64>,
    /// `HIGHPER_BODY_MAX_STREAMING` — max streaming body in bytes (default 100 MB).
    pub max_streaming_body: Reloadable<u64>,
    /// `HIGHPER_BODY_MAX_FORM` — max form body in bytes (default 1 MB).
    pub max_form_body: Reloadable<u64>,
}

impl Default for BodyRuntimeConfig {
    fn default() -> Self {
        Self {
            max_request_body: Reloadable::new(10 * 1024 * 1024),
            max_streaming_body: Reloadable::new(100 * 1024 * 1024),
            max_form_body: Reloadable::new(1024 * 1024),
        }
    }
}

pub(crate) fn load() -> Result<BodyRuntimeConfig, RuntimeConfigError> {
    let max_request_body = parse_byte_size(
        "HIGHPER_BODY_MAX_REQUEST",
        env_string("BODY_MAX_REQUEST").as_deref(),
        10 * 1024 * 1024,
    )?;
    let max_streaming_body = parse_byte_size(
        "HIGHPER_BODY_MAX_STREAMING",
        env_string("BODY_MAX_STREAMING").as_deref(),
        100 * 1024 * 1024,
    )?;
    let max_form_body = parse_byte_size(
        "HIGHPER_BODY_MAX_FORM",
        env_string("BODY_MAX_FORM").as_deref(),
        1024 * 1024,
    )?;

    Ok(BodyRuntimeConfig {
        max_request_body: Reloadable::new(max_request_body),
        max_streaming_body: Reloadable::new(max_streaming_body),
        max_form_body: Reloadable::new(max_form_body),
    })
}

/// Parses byte sizes with `KB`/`MB`/`GB` suffixes (case-sensitive). No suffix
/// is interpreted as raw bytes. Strict — rejects unparseable input.
fn parse_byte_size(
    env_var: &str,
    raw: Option<&str>,
    default: u64,
) -> Result<u64, RuntimeConfigError> {
    let Some(s) = raw else { return Ok(default) };
    let s = s.trim();
    let err = || RuntimeConfigError::ParseError {
        env_var: env_var.to_string(),
        value: s.to_string(),
        expected: "byte size like 100, 5KB, 10MB, 1GB",
    };

    let (num_str, mult): (&str, u64) = if let Some(n) = s.strip_suffix("GB") {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix('B') {
        (n, 1)
    } else {
        (s, 1)
    };

    let num: u64 = num_str.trim().parse().map_err(|_| err())?;
    num.checked_mul(mult).ok_or_else(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_BODY_MAX_REQUEST",
            "HIGHPER_BODY_MAX_STREAMING",
            "HIGHPER_BODY_MAX_FORM",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_10mb_100mb_1mb() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.max_request_body.get(), 10 * 1024 * 1024);
        assert_eq!(*cfg.max_streaming_body.get(), 100 * 1024 * 1024);
        assert_eq!(*cfg.max_form_body.get(), 1024 * 1024);
    }

    #[test]
    #[serial]
    fn parses_50mb() {
        clear();
        std::env::set_var("HIGHPER_BODY_MAX_REQUEST", "50MB");
        let cfg = load().unwrap();
        assert_eq!(*cfg.max_request_body.get(), 50 * 1024 * 1024);
        clear();
    }

    #[test]
    #[serial]
    fn parses_2gb() {
        clear();
        std::env::set_var("HIGHPER_BODY_MAX_STREAMING", "2GB");
        let cfg = load().unwrap();
        assert_eq!(*cfg.max_streaming_body.get(), 2u64 * 1024 * 1024 * 1024);
        clear();
    }

    #[test]
    #[serial]
    fn rejects_unparseable() {
        clear();
        std::env::set_var("HIGHPER_BODY_MAX_REQUEST", "huge");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::ParseError { .. })));
        clear();
    }
}
