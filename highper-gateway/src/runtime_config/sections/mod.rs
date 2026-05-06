//! Per-subsystem `RuntimeConfig` sub-structs.
//!
//! Stage 1 shipped `cluster` + `plugin`. Stage 2 added `ai`, `body`,
//! `shutdown`, `secrets`. Stage 3 closes Workstream 0.J with the
//! remaining 9 sections: `http3`, `tls`, `ratelimit`, `circuit_breaker`,
//! `geo`, `cache`, `signals`, `config_watcher`, `observability`.

pub mod cluster;
pub mod plugin;
pub mod ai;
pub mod body;
pub mod shutdown;
pub mod secrets;
pub mod http3;
pub mod tls;
pub mod ratelimit;
pub mod circuit_breaker;
pub mod geo;
pub mod cache;
pub mod signals;
pub mod config_watcher;
pub mod observability;
pub mod graphql;

pub use cluster::ClusterRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
pub use ai::AiRuntimeConfig;
pub use body::BodyRuntimeConfig;
pub use shutdown::ShutdownRuntimeConfig;
pub use secrets::SecretsRuntimeConfig;
pub use http3::Http3RuntimeConfig;
pub use tls::TlsRuntimeConfig;
pub use ratelimit::RatelimitRuntimeConfig;
pub use circuit_breaker::CircuitBreakerRuntimeConfig;
pub use geo::GeoRuntimeConfig;
pub use cache::CacheRuntimeConfig;
pub use signals::SignalsRuntimeConfig;
pub use config_watcher::ConfigWatcherRuntimeConfig;
pub use observability::ObservabilityRuntimeConfig;
pub use graphql::GraphqlRuntimeConfig;
