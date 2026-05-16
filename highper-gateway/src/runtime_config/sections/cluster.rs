//! Cluster-bootstrap configuration (per `ROADMAP.md` §11.2).
//!
//! Two independent on/off flags for the coordination layers (Type B and
//! Type C); the four cluster types from §11.1 emerge from their combination
//! (both none → Type 1 Stateless; only B → Type 2 +Valkey; only C → Type 3
//! +etcd; both → Type 4 +Valkey+etcd).
//!
//! Defaults per `OWNER_GATES_2026-05-03.md` §6 #4:
//! - Type B backend: `Valkey` is the recommended default when a Group B UC
//!   is enabled. Default at the env-var level is `none` so a vanilla startup
//!   without UCs doesn't bind to anything.
//! - Type C backend: `etcd` similarly; default at env level is `none`.
//!
//! Stage 1 enforces §11.2 rules 4 (peer-discovery sanity). Rules 1, 2, 3, 5
//! depend on the enabled UC list from `Config` (the user-facing config-file
//! struct), which `RuntimeConfig` doesn't see at load time. Stage 2 wires
//! `validate_cross_subsystem(&Config, &RuntimeConfig)` after `Config` loads.

use std::net::SocketAddr;

use crate::config::env_override::{env_bool, env_string};
use crate::runtime_config::{Reloadable, RuntimeConfigError, SecretRef};

#[derive(Debug, Clone, Default)]
pub struct ClusterRuntimeConfig {
    pub infra: ClusterInfra,
    pub typeb_backend: Reloadable<TypeBBackend>,
    pub typeb_addrs: Reloadable<Vec<SocketAddr>>,
    pub typeb_auth: Reloadable<Option<SecretRef>>,
    pub typeb_tls: Reloadable<bool>,
    pub typec_backend: Reloadable<TypeCBackend>,
    pub typec_addrs: Reloadable<Vec<SocketAddr>>,
    pub typec_client_cert: Reloadable<Option<SecretRef>>,
    pub typec_client_key: Reloadable<Option<SecretRef>>,
    pub typec_ca: Reloadable<Option<SecretRef>>,
    pub peer_discovery: PeerDiscoveryMode,
    pub peers: Vec<SocketAddr>,
    pub allow_single_node: bool,
    pub allow_insecure: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClusterInfra {
    #[default]
    Single,
    K8s,
    Vm,
    BareMetal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypeBBackend {
    #[default]
    None,
    Valkey,
    Redis,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypeCBackend {
    #[default]
    None,
    Etcd,
    Consul,
    Raft,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PeerDiscoveryMode {
    #[default]
    None,
    Static,
    K8sHeadless,
    Consul,
    Dns,
}

pub(crate) fn load() -> Result<ClusterRuntimeConfig, RuntimeConfigError> {
    let infra = parse_infra(env_string("CLUSTER_INFRA").as_deref())?;
    let typeb_backend = parse_typeb_backend(env_string("CLUSTER_TYPEB_BACKEND").as_deref())?;
    let typeb_addrs = parse_addr_list(
        "HIGHPER_CLUSTER_TYPEB_ADDRS",
        env_string("CLUSTER_TYPEB_ADDRS"),
    )?;
    let typeb_auth = env_string("CLUSTER_TYPEB_AUTH")
        .map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEB_AUTH", &v))
        .transpose()?;
    let typeb_tls = env_bool("CLUSTER_TYPEB_TLS").unwrap_or(false);
    let typec_backend = parse_typec_backend(env_string("CLUSTER_TYPEC_BACKEND").as_deref())?;
    let typec_addrs = parse_addr_list(
        "HIGHPER_CLUSTER_TYPEC_ADDRS",
        env_string("CLUSTER_TYPEC_ADDRS"),
    )?;
    let typec_client_cert = env_string("CLUSTER_TYPEC_CLIENT_CERT")
        .map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CLIENT_CERT", &v))
        .transpose()?;
    let typec_client_key = env_string("CLUSTER_TYPEC_CLIENT_KEY")
        .map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CLIENT_KEY", &v))
        .transpose()?;
    let typec_ca = env_string("CLUSTER_TYPEC_CA")
        .map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CA", &v))
        .transpose()?;
    let peer_discovery = parse_peer_discovery(env_string("CLUSTER_PEER_DISCOVERY").as_deref())?;
    let peers = parse_addr_list("HIGHPER_CLUSTER_PEERS", env_string("CLUSTER_PEERS"))?;
    let allow_single_node = env_bool("CLUSTER_ALLOW_SINGLE_NODE").unwrap_or(false);
    let allow_insecure = env_bool("CLUSTER_ALLOW_INSECURE").unwrap_or(false);

    let cfg = ClusterRuntimeConfig {
        infra,
        typeb_backend: Reloadable::new(typeb_backend),
        typeb_addrs: Reloadable::new(typeb_addrs),
        typeb_auth: Reloadable::new(typeb_auth),
        typeb_tls: Reloadable::new(typeb_tls),
        typec_backend: Reloadable::new(typec_backend),
        typec_addrs: Reloadable::new(typec_addrs),
        typec_client_cert: Reloadable::new(typec_client_cert),
        typec_client_key: Reloadable::new(typec_client_key),
        typec_ca: Reloadable::new(typec_ca),
        peer_discovery,
        peers,
        allow_single_node,
        allow_insecure,
    };

    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &ClusterRuntimeConfig) -> Result<(), RuntimeConfigError> {
    // §11.2 rule 4: k8s_headless peer discovery requires HIGHPER_CLUSTER_INFRA=k8s
    if cfg.peer_discovery == PeerDiscoveryMode::K8sHeadless && cfg.infra != ClusterInfra::K8s {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "k8s_headless peer discovery requires HIGHPER_CLUSTER_INFRA=k8s",
            details: format!("got infra={:?}", cfg.infra),
        });
    }
    // §11.2 rule 4: static peer discovery requires HIGHPER_CLUSTER_PEERS non-empty
    if cfg.peer_discovery == PeerDiscoveryMode::Static && cfg.peers.is_empty() {
        return Err(RuntimeConfigError::MissingRequired {
            env_var: "HIGHPER_CLUSTER_PEERS".into(),
            required_because: "peer_discovery=static",
        });
    }
    // Rules 1, 2, 3, 5 deferred to Stage 2 (need enabled-UC list from Config).
    Ok(())
}

fn parse_infra(value: Option<&str>) -> Result<ClusterInfra, RuntimeConfigError> {
    match value {
        None => Ok(ClusterInfra::Single),
        Some("single") => Ok(ClusterInfra::Single),
        Some("k8s") => Ok(ClusterInfra::K8s),
        Some("vm") => Ok(ClusterInfra::Vm),
        Some("baremetal") => Ok(ClusterInfra::BareMetal),
        Some(v) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_CLUSTER_INFRA".into(),
            value: v.to_string(),
            expected: "single|k8s|vm|baremetal",
        }),
    }
}

fn parse_typeb_backend(value: Option<&str>) -> Result<TypeBBackend, RuntimeConfigError> {
    match value {
        None | Some("none") => Ok(TypeBBackend::None),
        Some("valkey") => Ok(TypeBBackend::Valkey),
        Some("redis") => Ok(TypeBBackend::Redis),
        Some(v) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_CLUSTER_TYPEB_BACKEND".into(),
            value: v.to_string(),
            expected: "valkey|redis|none",
        }),
    }
}

fn parse_typec_backend(value: Option<&str>) -> Result<TypeCBackend, RuntimeConfigError> {
    match value {
        None | Some("none") => Ok(TypeCBackend::None),
        Some("etcd") => Ok(TypeCBackend::Etcd),
        Some("consul") => Ok(TypeCBackend::Consul),
        Some("raft") => Ok(TypeCBackend::Raft),
        Some(v) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_CLUSTER_TYPEC_BACKEND".into(),
            value: v.to_string(),
            expected: "etcd|consul|raft|none",
        }),
    }
}

fn parse_peer_discovery(value: Option<&str>) -> Result<PeerDiscoveryMode, RuntimeConfigError> {
    match value {
        None | Some("none") => Ok(PeerDiscoveryMode::None),
        Some("static") => Ok(PeerDiscoveryMode::Static),
        Some("k8s_headless") => Ok(PeerDiscoveryMode::K8sHeadless),
        Some("consul") => Ok(PeerDiscoveryMode::Consul),
        Some("dns") => Ok(PeerDiscoveryMode::Dns),
        Some(v) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_CLUSTER_PEER_DISCOVERY".into(),
            value: v.to_string(),
            expected: "static|k8s_headless|consul|dns|none",
        }),
    }
}

fn parse_addr_list(
    full_env_var: &str,
    value: Option<String>,
) -> Result<Vec<SocketAddr>, RuntimeConfigError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(|s| {
            let s = s.trim();
            s.parse::<SocketAddr>()
                .map_err(|_| RuntimeConfigError::ParseError {
                    env_var: full_env_var.to_string(),
                    value: s.to_string(),
                    expected: "host:port (comma-separated for multiple)",
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_cluster_env() {
        for k in [
            "HIGHPER_CLUSTER_INFRA",
            "HIGHPER_CLUSTER_TYPEB_BACKEND",
            "HIGHPER_CLUSTER_TYPEB_ADDRS",
            "HIGHPER_CLUSTER_TYPEB_AUTH",
            "HIGHPER_CLUSTER_TYPEB_TLS",
            "HIGHPER_CLUSTER_TYPEC_BACKEND",
            "HIGHPER_CLUSTER_TYPEC_ADDRS",
            "HIGHPER_CLUSTER_TYPEC_CLIENT_CERT",
            "HIGHPER_CLUSTER_TYPEC_CLIENT_KEY",
            "HIGHPER_CLUSTER_TYPEC_CA",
            "HIGHPER_CLUSTER_PEER_DISCOVERY",
            "HIGHPER_CLUSTER_PEERS",
            "HIGHPER_CLUSTER_ALLOW_SINGLE_NODE",
            "HIGHPER_CLUSTER_ALLOW_INSECURE",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_when_no_env_vars_set() {
        clear_cluster_env();
        let cfg = load().unwrap();
        assert_eq!(cfg.infra, ClusterInfra::Single);
        assert_eq!(*cfg.typeb_backend.get(), TypeBBackend::None);
        assert_eq!(*cfg.typec_backend.get(), TypeCBackend::None);
        assert_eq!(cfg.peer_discovery, PeerDiscoveryMode::None);
        assert!(cfg.peers.is_empty());
        assert!(!cfg.allow_single_node);
        assert!(!cfg.allow_insecure);
    }

    #[test]
    #[serial]
    fn parses_typeb_valkey_with_addrs() {
        clear_cluster_env();
        std::env::set_var("HIGHPER_CLUSTER_TYPEB_BACKEND", "valkey");
        std::env::set_var("HIGHPER_CLUSTER_TYPEB_ADDRS", "10.0.0.1:6379,10.0.0.2:6379");
        let cfg = load().unwrap();
        assert_eq!(*cfg.typeb_backend.get(), TypeBBackend::Valkey);
        assert_eq!(cfg.typeb_addrs.get().len(), 2);
        assert_eq!(
            cfg.typeb_addrs.get()[0].to_string(),
            "10.0.0.1:6379".to_string()
        );
        clear_cluster_env();
    }

    #[test]
    #[serial]
    fn rejects_invalid_typeb_backend() {
        clear_cluster_env();
        std::env::set_var("HIGHPER_CLUSTER_TYPEB_BACKEND", "memcached");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::ParseError { .. })));
        clear_cluster_env();
    }

    #[test]
    #[serial]
    fn rejects_k8s_headless_when_infra_is_vm() {
        clear_cluster_env();
        std::env::set_var("HIGHPER_CLUSTER_INFRA", "vm");
        std::env::set_var("HIGHPER_CLUSTER_PEER_DISCOVERY", "k8s_headless");
        let r = load();
        assert!(matches!(
            r,
            Err(RuntimeConfigError::InvalidCombination { .. })
        ));
        clear_cluster_env();
    }

    #[test]
    #[serial]
    fn rejects_static_discovery_with_empty_peers() {
        clear_cluster_env();
        std::env::set_var("HIGHPER_CLUSTER_PEER_DISCOVERY", "static");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::MissingRequired { .. })));
        clear_cluster_env();
    }
}
