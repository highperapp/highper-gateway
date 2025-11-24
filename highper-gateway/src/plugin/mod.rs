//! Plugin System - Hybrid WASM + FFI Architecture
//!
//! This module provides a flexible plugin system supporting:
//! - WebAssembly (WASM/WASI) plugins for untrusted, multi-language code
//! - FFI/C ABI plugins for trusted, high-performance Rust/C/C++ code
//! - Hot-reload without proxy restart
//! - Resource limits and sandboxing
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │      Plugin Manager                 │
//! │  ┌────────────┐  ┌────────────┐    │
//! │  │   WASM     │  │    FFI     │    │
//! │  │  Runtime   │  │  Loader    │    │
//! │  └────────────┘  └────────────┘    │
//! │  ┌────────────────────────────┐    │
//! │  │   Plugin Registry          │    │
//! │  └────────────────────────────┘    │
//! └─────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use highper_gateway::plugin::{PluginType, PluginLimits};
//!
//! // Configure plugin limits
//! let limits = PluginLimits::default();
//!
//! // Choose plugin type
//! let plugin_type = PluginType::Wasm; // Safe, sandboxed
//! assert_eq!(plugin_type, PluginType::Wasm);
//!
//! // Or use FFI for high performance
//! let ffi_type = PluginType::Ffi;
//! assert_eq!(ffi_type, PluginType::Ffi);
//! ```
//!
//! ## Plugin Types
//!
//! - **WASM**: Safe, sandboxed, multi-language (Rust, JS, Python, Go, etc.)
//! - **FFI**: High-performance, zero-copy, Rust/C/C++ only, requires trust

pub mod types;
pub mod trait_def;
pub mod manager;
pub mod registry;
pub mod wasm;
pub mod ffi;
pub mod config;
pub mod hot_reload;
pub mod host_functions;

pub use types::*;
pub use trait_def::{Plugin, PluginContext, FilterResult, BoxedPlugin};
pub use manager::PluginManager;
pub use registry::PluginRegistry;
pub use config::{PluginConfig, PluginType, PluginLimits, PluginCapabilities, PluginSystemConfig};

/// Plugin system error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Plugin already loaded: {0}")]
    AlreadyLoaded(String),

    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),

    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Plugin timeout: {0}")]
    Timeout(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Invalid plugin format: {0}")]
    InvalidFormat(String),

    #[error("WASM error: {0}")]
    WasmError(String),

    #[error("FFI error: {0}")]
    FfiError(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Load error: {0}")]
    LoadError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, PluginError>;
