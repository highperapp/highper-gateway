//! Top-level `RuntimeConfig` loader.
//!
//! Each section's `load()` returns its sub-struct or a `RuntimeConfigError`.
//! Cross-subsystem invariants (e.g., AI cache=valkey requires Cluster Type B
//! configured) live in `validate_cross_subsystem` — Stage 1 is a skeleton;
//! Stage 2 wires the AI/Cluster invariants once `AiRuntimeConfig` lands.

use crate::runtime_config::{
    sections::{cluster, plugin},
    RuntimeConfig, RuntimeConfigError,
};

pub fn load() -> Result<RuntimeConfig, RuntimeConfigError> {
    let cluster = cluster::load()?;
    let plugin = plugin::load()?;

    let cfg = RuntimeConfig { cluster, plugin };
    validate_cross_subsystem(&cfg)?;
    Ok(cfg)
}

fn validate_cross_subsystem(_cfg: &RuntimeConfig) -> Result<(), RuntimeConfigError> {
    // Stage 1: only cluster + plugin are present; no inter-section invariants.
    // Stage 2 adds: AI cache=valkey requires Cluster Type B configured
    //   (unless allow_single_node=true).
    Ok(())
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
            "HIGHPER_PLUGIN_DRAIN",
            "HIGHPER_PLUGIN_IDLE_POLL",
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
}
