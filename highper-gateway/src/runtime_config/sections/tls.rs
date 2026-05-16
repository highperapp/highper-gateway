//! TLS tunables (Workstream 0.J Stage 3).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct TlsRuntimeConfig {
    pub session_cache_size: Reloadable<u32>, // HIGHPER_TLS_SESSION_CACHE_SIZE (default 4096)
    pub session_ticket_lifetime_secs: Reloadable<u64>, // HIGHPER_TLS_SESSION_TICKET_LIFETIME (default 86400)
    pub ocsp_cache_ttl_secs: Reloadable<u64>,          // HIGHPER_TLS_OCSP_CACHE_TTL (default 3600)
    pub acme_renew_check_secs: Reloadable<u64>, // HIGHPER_TLS_ACME_RENEW_CHECK (default 3600)
    pub min_version: TlsMinVersion,             // HIGHPER_TLS_MIN_VERSION (default 1.2; Restart)
    /// `HIGHPER_TLS_CERT_WATCHER_CHANNEL_CAPACITY` — bounded channel
    /// capacity for cert-file-modified events. Default 32; events drop on
    /// full (file-watch is idempotent — the next change re-triggers). B11
    /// migration of `src/tls/cert_watcher.rs:49` from `unbounded_channel`.
    pub cert_watcher_event_channel_capacity: Reloadable<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TlsMinVersion {
    #[default]
    V1_2,
    V1_3,
}

impl Default for TlsRuntimeConfig {
    fn default() -> Self {
        Self {
            session_cache_size: Reloadable::new(4096),
            session_ticket_lifetime_secs: Reloadable::new(86400),
            ocsp_cache_ttl_secs: Reloadable::new(3600),
            acme_renew_check_secs: Reloadable::new(3600),
            min_version: TlsMinVersion::V1_2,
            cert_watcher_event_channel_capacity: Reloadable::new(32),
        }
    }
}

pub(crate) fn load() -> Result<TlsRuntimeConfig, RuntimeConfigError> {
    let session_cache_size = parse_u32(
        "HIGHPER_TLS_SESSION_CACHE_SIZE",
        env_string("TLS_SESSION_CACHE_SIZE").as_deref(),
        4096,
    )?;
    let session_ticket_lifetime_secs = parse_u64(
        "HIGHPER_TLS_SESSION_TICKET_LIFETIME",
        env_string("TLS_SESSION_TICKET_LIFETIME").as_deref(),
        86400,
    )?;
    let ocsp_cache_ttl_secs = parse_u64(
        "HIGHPER_TLS_OCSP_CACHE_TTL",
        env_string("TLS_OCSP_CACHE_TTL").as_deref(),
        3600,
    )?;
    let acme_renew_check_secs = parse_u64(
        "HIGHPER_TLS_ACME_RENEW_CHECK",
        env_string("TLS_ACME_RENEW_CHECK").as_deref(),
        3600,
    )?;
    let min_version = match env_string("TLS_MIN_VERSION").as_deref() {
        None | Some("1.2") => TlsMinVersion::V1_2,
        Some("1.3") => TlsMinVersion::V1_3,
        Some(other) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_TLS_MIN_VERSION".into(),
                value: other.into(),
                expected: "1.2|1.3",
            });
        }
    };
    let cert_watcher_event_channel_capacity = parse_u32(
        "HIGHPER_TLS_CERT_WATCHER_CHANNEL_CAPACITY",
        env_string("TLS_CERT_WATCHER_CHANNEL_CAPACITY").as_deref(),
        32,
    )?;

    Ok(TlsRuntimeConfig {
        session_cache_size: Reloadable::new(session_cache_size),
        session_ticket_lifetime_secs: Reloadable::new(session_ticket_lifetime_secs),
        ocsp_cache_ttl_secs: Reloadable::new(ocsp_cache_ttl_secs),
        acme_renew_check_secs: Reloadable::new(acme_renew_check_secs),
        min_version,
        cert_watcher_event_channel_capacity: Reloadable::new(cert_watcher_event_channel_capacity),
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
            "HIGHPER_TLS_SESSION_CACHE_SIZE",
            "HIGHPER_TLS_SESSION_TICKET_LIFETIME",
            "HIGHPER_TLS_OCSP_CACHE_TTL",
            "HIGHPER_TLS_ACME_RENEW_CHECK",
            "HIGHPER_TLS_MIN_VERSION",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_v1_2() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(cfg.min_version, TlsMinVersion::V1_2);
        assert_eq!(*cfg.session_cache_size.get(), 4096);
    }

    #[test]
    #[serial]
    fn parses_v1_3() {
        clear();
        std::env::set_var("HIGHPER_TLS_MIN_VERSION", "1.3");
        let cfg = load().unwrap();
        assert_eq!(cfg.min_version, TlsMinVersion::V1_3);
        clear();
    }

    #[test]
    #[serial]
    fn rejects_invalid_min_version() {
        clear();
        std::env::set_var("HIGHPER_TLS_MIN_VERSION", "1.1");
        assert!(matches!(load(), Err(RuntimeConfigError::ParseError { .. })));
        clear();
    }
}
