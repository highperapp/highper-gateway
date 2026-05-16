//! Per-subsystem `RuntimeConfig` sub-structs.
//!
//! Stage 1 shipped `cluster` + `plugin`. Stage 2 added `ai`, `body`,
//! `shutdown`, `secrets`. Stage 3 closes Workstream 0.J with the
//! remaining 9 sections: `http3`, `tls`, `ratelimit`, `circuit_breaker`,
//! `geo`, `cache`, `signals`, `config_watcher`, `observability`.

pub mod ai;
pub mod body;
pub mod cache;
pub mod circuit_breaker;
pub mod cluster;
pub mod config_watcher;
pub mod geo;
pub mod graphql;
pub mod http3;
pub mod observability;
pub mod plugin;
pub mod ratelimit;
pub mod secrets;
pub mod shutdown;
pub mod signals;
pub mod tls;

pub use ai::AiRuntimeConfig;
pub use body::BodyRuntimeConfig;
pub use cache::CacheRuntimeConfig;
pub use circuit_breaker::CircuitBreakerRuntimeConfig;
pub use cluster::ClusterRuntimeConfig;
pub use config_watcher::ConfigWatcherRuntimeConfig;
pub use geo::GeoRuntimeConfig;
pub use graphql::GraphqlRuntimeConfig;
pub use http3::Http3RuntimeConfig;
pub use observability::ObservabilityRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
pub use ratelimit::RatelimitRuntimeConfig;
pub use secrets::SecretsRuntimeConfig;
pub use shutdown::ShutdownRuntimeConfig;
pub use signals::SignalsRuntimeConfig;
pub use tls::TlsRuntimeConfig;
