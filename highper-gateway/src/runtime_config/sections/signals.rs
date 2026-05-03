//! Signal-handling tunables (Workstream 0.J Stage 3).

use crate::config::env_override::env_bool;
use crate::runtime_config::RuntimeConfigError;

#[derive(Debug, Clone)]
pub struct SignalsRuntimeConfig {
    pub sighup_reload_enabled: bool,    // HIGHPER_SIGNALS_SIGHUP_RELOAD (default true; Restart)
    pub sigterm_drain_enabled: bool,    // HIGHPER_SIGNALS_SIGTERM_DRAIN (default true; Restart)
}

impl Default for SignalsRuntimeConfig {
    fn default() -> Self {
        Self {
            sighup_reload_enabled: true,
            sigterm_drain_enabled: true,
        }
    }
}

pub(crate) fn load() -> Result<SignalsRuntimeConfig, RuntimeConfigError> {
    Ok(SignalsRuntimeConfig {
        sighup_reload_enabled: env_bool("SIGNALS_SIGHUP_RELOAD").unwrap_or(true),
        sigterm_drain_enabled: env_bool("SIGNALS_SIGTERM_DRAIN").unwrap_or(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        std::env::remove_var("HIGHPER_SIGNALS_SIGHUP_RELOAD");
        std::env::remove_var("HIGHPER_SIGNALS_SIGTERM_DRAIN");
    }

    #[test]
    #[serial]
    fn defaults_both_enabled() {
        clear();
        let cfg = load().unwrap();
        assert!(cfg.sighup_reload_enabled);
        assert!(cfg.sigterm_drain_enabled);
    }

    #[test]
    #[serial]
    fn parses_disable() {
        clear();
        std::env::set_var("HIGHPER_SIGNALS_SIGHUP_RELOAD", "false");
        let cfg = load().unwrap();
        assert!(!cfg.sighup_reload_enabled);
        clear();
    }
}
