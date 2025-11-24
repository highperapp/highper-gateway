//! TLS and certificate management

pub mod manager;
pub mod storage;
pub mod acme;
pub mod challenge;
pub mod acceptor;
pub mod passthrough;
pub mod ca_manager;
pub mod client_cert;
pub mod client_verifier;
pub mod cert_watcher;
pub mod cert_validator;
pub mod cert_reloader;

// Kernel TLS support (Linux-only feature)
#[cfg(target_os = "linux")]
pub mod ktls;

pub use manager::*;
pub use storage::*;
pub use acme::*;
pub use challenge::*;
pub use acceptor::*;
pub use passthrough::*;
pub use ca_manager::*;
pub use client_cert::*;
pub use client_verifier::*;
pub use cert_watcher::*;
pub use cert_validator::*;
pub use cert_reloader::*;
