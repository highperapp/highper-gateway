//! TLS and certificate management

pub mod manager;
pub mod storage;
pub mod acme;
pub mod challenge;
pub mod auto_https;
pub mod acceptor;
pub mod passthrough;
pub mod ca_manager;
pub mod client_cert;
pub mod client_verifier;
pub mod cert_watcher;
pub mod cert_validator;
pub mod cert_reloader;

// OCSP stapling support
pub mod ocsp_fetcher;
pub mod ocsp_cache;
pub mod ocsp_stapler;

// CRL checking support
pub mod crl_checker;

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
pub use ocsp_fetcher::*;
pub use ocsp_cache::*;
pub use ocsp_stapler::*;
pub use crl_checker::*;
pub use auto_https::*;
