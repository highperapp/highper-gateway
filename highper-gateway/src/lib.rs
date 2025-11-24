//! Highper Gateway - High-performance reverse proxy and API gateway
//!
//! This is a production-grade reverse proxy built with io_uring for maximum performance.

pub mod config;
pub mod runtime;
pub mod proxy;
pub mod http;
pub mod tcp;
pub mod tls;
pub mod observability;
pub mod middleware;
pub mod gateway;
pub mod websocket;
pub mod grpc;
pub mod admin;
pub mod utils;
pub mod state;
pub mod discovery;
pub mod plugin;
pub mod webserver;
pub mod cache;

// Re-export commonly used types
pub use config::Config;

/// Result type alias using anyhow::Error
pub type Result<T> = anyhow::Result<T>;
