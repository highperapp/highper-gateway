//! Per-subsystem `RuntimeConfig` sub-structs.
//!
//! Stage 1 shipped `cluster` + `plugin`. Stage 2 adds `ai`, `body`,
//! `shutdown`, `secrets`. Stage 3 will add `http3`, `tls`, `ratelimit`,
//! `circuit_breaker`, `geo`, `cache`, `signals`, `config_watcher`,
//! `observability`.

pub mod cluster;
pub mod plugin;
pub mod ai;
pub mod body;
pub mod shutdown;
pub mod secrets;

pub use cluster::ClusterRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
pub use ai::AiRuntimeConfig;
pub use body::BodyRuntimeConfig;
pub use shutdown::ShutdownRuntimeConfig;
pub use secrets::SecretsRuntimeConfig;
