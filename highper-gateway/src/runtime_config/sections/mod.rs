//! Per-subsystem `RuntimeConfig` sub-structs.
//!
//! Stage 1 ships `cluster` + `plugin`. Stage 2 adds `ai`, `body`, `shutdown`,
//! `secrets`. Stage 3 adds `http3`, `tls`, `ratelimit`, `circuit_breaker`,
//! `geo`, `cache`, `signals`, `config_watcher`, `observability`.

pub mod cluster;
pub mod plugin;

pub use cluster::ClusterRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
