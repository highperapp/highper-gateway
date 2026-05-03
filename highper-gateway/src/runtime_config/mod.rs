//! Central env-var-driven runtime configuration.
//!
//! Per `ROADMAP.md` §0.1: every operator-tunable value loads from a
//! `HIGHPER_*`-prefixed env var at startup. Hot paths must not call
//! `std::env::var`. Design: `docs/planning/SETTINGS_SCAFFOLD.md`. Stage 1
//! plan: `docs/planning/RUNTIME_CONFIG_STAGE1_PR_PLAN.md`.
//!
//! Singleton via `OnceLock<ArcSwap<RuntimeConfig>>` enforces "no
//! `std::env::var` on hot paths" through the type system: there is no other
//! way to reach the values. The `ArcSwap` wrapper supports the Tier 1 SIGHUP
//! reload runtime (lands in Stage 3).

mod error;
mod loader;
mod reload;
mod secret_ref;
mod sections;

pub use error::RuntimeConfigError;
pub use loader::{load, validate_against_config};
pub use reload::Reloadable;
pub use secret_ref::{SecretRef, SecretValue};
pub use sections::{
    AiRuntimeConfig, BodyRuntimeConfig, ClusterRuntimeConfig, PluginRuntimeConfig,
    SecretsRuntimeConfig, ShutdownRuntimeConfig,
};

use arc_swap::ArcSwap;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub cluster: ClusterRuntimeConfig,
    pub plugin: PluginRuntimeConfig,
    pub ai: AiRuntimeConfig,
    pub body: BodyRuntimeConfig,
    pub shutdown: ShutdownRuntimeConfig,
    pub secrets: SecretsRuntimeConfig,
    // Stage 3: http3, tls, ratelimit, circuit_breaker, geo, cache,
    //          signals, config_watcher, observability
}

impl RuntimeConfig {
    /// Defaults-only constructor for unit tests. Does not read env vars.
    pub fn for_test() -> Self {
        Self::default()
    }
}

static CURRENT: OnceLock<ArcSwap<RuntimeConfig>> = OnceLock::new();

/// Install the loaded `RuntimeConfig` as the process-global. Call once from
/// `main()` after `load()`. Panics if called twice.
pub fn install(c: RuntimeConfig) {
    CURRENT
        .set(ArcSwap::from_pointee(c))
        .map_err(|_| ())
        .expect("runtime_config::install() called twice");
}

/// Read the current `RuntimeConfig`. Panics if `install()` was not called.
pub fn current() -> Arc<RuntimeConfig> {
    CURRENT
        .get()
        .expect(
            "runtime_config not initialized — call runtime_config::install(load()?) in main()",
        )
        .load_full()
}

/// Test-only: install a `RuntimeConfig` if no install has happened yet, or
/// silently no-op if one has. Use to bootstrap unit tests that touch
/// `current()`.
#[cfg(test)]
pub(crate) fn install_for_test(c: RuntimeConfig) {
    let _ = CURRENT.set(ArcSwap::from_pointee(c));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn install_for_test_then_current_round_trips() {
        install_for_test(RuntimeConfig::for_test());
        let c = current();
        assert_eq!(
            *c.plugin.drain.get(),
            std::time::Duration::from_secs(30)
        );
    }
}
