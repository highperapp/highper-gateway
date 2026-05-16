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
pub use reload::{compute_diff, install_sighup_handler, reload_now, ReloadDiff, Reloadable};
pub use secret_ref::{SecretRef, SecretValue};

// `try_current` is available as `runtime_config::try_current()` directly
// (not re-exported here since it's defined in this module's body).
pub use sections::{
    AiRuntimeConfig, BodyRuntimeConfig, CacheRuntimeConfig, CircuitBreakerRuntimeConfig,
    ClusterRuntimeConfig, ConfigWatcherRuntimeConfig, GeoRuntimeConfig, GraphqlRuntimeConfig,
    Http3RuntimeConfig, ObservabilityRuntimeConfig, PluginRuntimeConfig, RatelimitRuntimeConfig,
    SecretsRuntimeConfig, ShutdownRuntimeConfig, SignalsRuntimeConfig, TlsRuntimeConfig,
};
// Re-export inner enums + types that consumers reference. Add new ones
// here when consumer code outside `runtime_config` needs to match against
// the variants.
pub use sections::ratelimit::{RatelimitMode, RedisFailMode, XffTrustMode};

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
    pub http3: Http3RuntimeConfig,
    pub tls: TlsRuntimeConfig,
    pub ratelimit: RatelimitRuntimeConfig,
    pub circuit_breaker: CircuitBreakerRuntimeConfig,
    pub geo: GeoRuntimeConfig,
    pub cache: CacheRuntimeConfig,
    pub signals: SignalsRuntimeConfig,
    pub config_watcher: ConfigWatcherRuntimeConfig,
    pub observability: ObservabilityRuntimeConfig,
    pub graphql: GraphqlRuntimeConfig,
}

impl RuntimeConfig {
    /// Defaults-only constructor for unit tests. Does not read env vars.
    pub fn for_test() -> Self {
        Self::default()
    }
}

static CURRENT: OnceLock<ArcSwap<RuntimeConfig>> = OnceLock::new();
static LATEST_DIFF: OnceLock<ArcSwap<ReloadDiff>> = OnceLock::new();

/// Install the loaded `RuntimeConfig` as the process-global. Call once from
/// `main()` after `load()`. Panics if called twice.
pub fn install(c: RuntimeConfig) {
    CURRENT
        .set(ArcSwap::from_pointee(c))
        .map_err(|_| ())
        .expect("runtime_config::install() called twice");
    let _ = LATEST_DIFF.set(ArcSwap::from_pointee(ReloadDiff::empty()));
}

/// Read the current `RuntimeConfig`.
///
/// Workers that need a consistent view across multiple field reads should
/// call `current()` *once* per request and reuse the returned `Arc`. A
/// later SIGHUP reload (`reload_now()`) will atomically swap the inner
/// pointer; in-flight workers keep their previously-loaded snapshot, and
/// subsequent calls see the new struct.
///
/// Panics if `install()` was not called.
pub fn current() -> Arc<RuntimeConfig> {
    CURRENT
        .get()
        .expect("runtime_config not initialized — call runtime_config::install(load()?) in main()")
        .load_full()
}

/// Read the current `RuntimeConfig`, returning `None` if `install()` has
/// not been called yet. Use only for code paths that may run before
/// `main()` wires the global (e.g., unit tests that exercise modules in
/// isolation, or library uses that don't go through the binary's
/// `start_server`). Production hot paths should use `current()`.
pub fn try_current() -> Option<Arc<RuntimeConfig>> {
    CURRENT.get().map(|a| a.load_full())
}

/// Internal: the underlying `ArcSwap` for atomic store on reload. Used by
/// `reload::reload_now`; not part of the public API.
pub(crate) fn current_arcswap() -> &'static ArcSwap<RuntimeConfig> {
    CURRENT
        .get()
        .expect("runtime_config not initialized — call install() first")
}

/// Internal: persist the latest reload diff (Stage 3c admin endpoint
/// reads via `latest_diff()`).
pub(crate) fn store_latest_diff(diff: ReloadDiff) {
    if let Some(slot) = LATEST_DIFF.get() {
        slot.store(Arc::new(diff));
    }
}

/// The latest reload diff, or an empty diff if no reload has occurred.
pub fn latest_diff() -> Arc<ReloadDiff> {
    LATEST_DIFF
        .get()
        .map(|a| a.load_full())
        .unwrap_or_else(|| Arc::new(ReloadDiff::empty()))
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
        assert_eq!(*c.plugin.drain.get(), std::time::Duration::from_secs(30));
    }
}
