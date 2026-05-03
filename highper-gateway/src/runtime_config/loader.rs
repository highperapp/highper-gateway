//! Top-level `RuntimeConfig` loader + cross-subsystem validators.
//!
//! Two validation entry points:
//! - `validate_cross_subsystem` runs at boot inside `load()`; checks
//!   invariants between `RuntimeConfig` sections only (e.g., AI cache=valkey
//!   requires Cluster Type B configured).
//! - `validate_against_config` runs in `main.rs` after `Config` loads;
//!   closes the §11.2 rules 1/2/3/5 deferred from Stage 1 (Group B/C UC
//!   requires backend) since they need the enabled-UC list from `Config`.

use crate::runtime_config::sections::ai::{AiCacheBackend, AiCooldownBackend};
use crate::runtime_config::sections::cluster::{TypeBBackend, TypeCBackend};
use crate::runtime_config::{
    sections::{ai, body, plugin, secrets, shutdown, cluster as cluster_section},
    RuntimeConfig, RuntimeConfigError,
};

pub fn load() -> Result<RuntimeConfig, RuntimeConfigError> {
    let cluster = cluster_section::load()?;
    let plugin = plugin::load()?;
    let ai = ai::load()?;
    let body = body::load()?;
    let shutdown = shutdown::load()?;
    let secrets = secrets::load()?;

    let cfg = RuntimeConfig {
        cluster,
        plugin,
        ai,
        body,
        shutdown,
        secrets,
    };
    validate_cross_subsystem(&cfg)?;
    Ok(cfg)
}

fn validate_cross_subsystem(cfg: &RuntimeConfig) -> Result<(), RuntimeConfigError> {
    // AI cache=valkey requires Cluster Type B configured (unless allow_single_node).
    if *cfg.ai.cache_backend.get() == AiCacheBackend::Valkey
        && *cfg.cluster.typeb_backend.get() == TypeBBackend::None
        && !cfg.cluster.allow_single_node
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "HIGHPER_AI_CACHE_BACKEND=valkey requires HIGHPER_CLUSTER_TYPEB_BACKEND set (or HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true)",
            details: "Type B cluster backend is needed for hot-path AI cache counters".into(),
        });
    }

    // AI cooldown_backend=valkey (forced) requires Cluster Type B configured.
    // `auto` resolves at runtime; `local` is unconditionally fine.
    if *cfg.ai.cooldown_backend.get() == AiCooldownBackend::Valkey
        && *cfg.cluster.typeb_backend.get() == TypeBBackend::None
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "HIGHPER_AI_COOLDOWN_BACKEND=valkey requires HIGHPER_CLUSTER_TYPEB_BACKEND set",
            details: "Forced Valkey cooldown without Type B cluster has no backend".into(),
        });
    }

    // §11.2 rules 1, 2, 3, 5 (Group B/C UC requires backend) need the
    // enabled-UC list from `Config`. They run via `validate_against_config`
    // from main.rs after `Config` loads.
    Ok(())
}

/// Closes §11.2 rules 1/2/3/5 once `Config` has loaded. Called from
/// `main.rs::start_server` after `validate_config(&config)`.
///
/// Stage 2 leaves the actual UC-enablement derivation as a stub
/// (`derive_enabled_ucs`); the helper currently returns an empty set so
/// the validations are no-ops. Phase 2 (UC16) and the existing Phase 0
/// workstreams will populate the helper as each UC's enablement signal
/// becomes derivable from `Config`.
pub fn validate_against_config(
    rt: &RuntimeConfig,
    config: &crate::config::Config,
) -> Result<(), RuntimeConfigError> {
    let enabled = derive_enabled_ucs(config);

    // §11.2 rule 1: any Group B UC enabled requires Type B backend (or
    // single-node opt-in).
    if enabled.any_group_b
        && *rt.cluster.typeb_backend.get() == TypeBBackend::None
        && !rt.cluster.allow_single_node
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "Group B UC enabled requires HIGHPER_CLUSTER_TYPEB_BACKEND set (or HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true)",
            details: format!("enabled Group B UCs: {:?}", enabled.group_b_ucs),
        });
    }

    // §11.2 rule 2: any Group C UC enabled requires Type C backend.
    if enabled.any_group_c
        && *rt.cluster.typec_backend.get() == TypeCBackend::None
        && !rt.cluster.allow_single_node
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "Group C UC enabled requires HIGHPER_CLUSTER_TYPEC_BACKEND set (or HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true)",
            details: format!("enabled Group C UCs: {:?}", enabled.group_c_ucs),
        });
    }

    // §11.2 rule 3 / rule 5 are subsumed by rules 1 + 2 above (independence
    // check is implicit since both flags are checked separately).
    Ok(())
}

/// Derive which UCs are enabled from the user-facing `Config`. Stage 2
/// stub — returns empty set. Each phase fills in its UC enablement signal.
fn derive_enabled_ucs(_config: &crate::config::Config) -> EnabledUcs {
    // TODO(Phase 0+): walk `_config` and determine which of UC1–UC15 + UC16
    // are enabled based on listener bindings, route protocols, upstream
    // types, etc. Until then, the §11.2 cross-config rules are no-ops; the
    // intra-RuntimeConfig invariants (validate_cross_subsystem) cover the
    // critical AI/Cluster cases.
    EnabledUcs::default()
}

#[derive(Debug, Default)]
struct EnabledUcs {
    any_group_b: bool,
    any_group_c: bool,
    group_b_ucs: Vec<&'static str>,
    group_c_ucs: Vec<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_all() {
        for k in [
            "HIGHPER_CLUSTER_INFRA",
            "HIGHPER_CLUSTER_TYPEB_BACKEND",
            "HIGHPER_CLUSTER_TYPEB_ADDRS",
            "HIGHPER_CLUSTER_TYPEC_BACKEND",
            "HIGHPER_CLUSTER_PEER_DISCOVERY",
            "HIGHPER_CLUSTER_PEERS",
            "HIGHPER_CLUSTER_ALLOW_SINGLE_NODE",
            "HIGHPER_PLUGIN_DRAIN",
            "HIGHPER_PLUGIN_IDLE_POLL",
            "HIGHPER_PLUGIN_HOT_RELOAD_SETTLE",
            "HIGHPER_AI_CACHE_BACKEND",
            "HIGHPER_AI_COOLDOWN_BACKEND",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn load_with_no_env_vars_returns_defaults() {
        clear_all();
        let cfg = load().unwrap();
        assert_eq!(
            *cfg.plugin.drain.get(),
            std::time::Duration::from_secs(30)
        );
    }

    #[test]
    #[serial]
    fn ai_cache_valkey_requires_cluster_typeb() {
        clear_all();
        // Default cache_backend is valkey; default typeb_backend is none;
        // default allow_single_node is false → must reject.
        let r = load();
        assert!(
            matches!(r, Err(RuntimeConfigError::InvalidCombination { .. })),
            "expected InvalidCombination, got {r:?}"
        );
        clear_all();
    }

    #[test]
    #[serial]
    fn ai_cache_valkey_with_allow_single_node_passes() {
        clear_all();
        std::env::set_var("HIGHPER_CLUSTER_ALLOW_SINGLE_NODE", "true");
        let r = load();
        assert!(r.is_ok(), "expected Ok, got {r:?}");
        clear_all();
    }

    #[test]
    #[serial]
    fn ai_cache_valkey_with_typeb_set_passes() {
        clear_all();
        std::env::set_var("HIGHPER_CLUSTER_TYPEB_BACKEND", "valkey");
        std::env::set_var("HIGHPER_CLUSTER_TYPEB_ADDRS", "127.0.0.1:6379");
        let r = load();
        assert!(r.is_ok(), "expected Ok, got {r:?}");
        clear_all();
    }

    #[test]
    #[serial]
    fn ai_cooldown_valkey_requires_cluster_typeb() {
        clear_all();
        std::env::set_var("HIGHPER_CLUSTER_ALLOW_SINGLE_NODE", "true");
        std::env::set_var("HIGHPER_AI_CACHE_BACKEND", "memory");
        std::env::set_var("HIGHPER_AI_COOLDOWN_BACKEND", "valkey");
        let r = load();
        assert!(
            matches!(r, Err(RuntimeConfigError::InvalidCombination { .. })),
            "expected InvalidCombination, got {r:?}"
        );
        clear_all();
    }
}
