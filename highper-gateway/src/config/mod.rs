//! Configuration management module

mod schema;
mod loader;
mod validator;
mod watcher;
mod reloader;

// Enhanced validation with detailed error reporting
pub mod validation;

// Environment variable configuration overrides (12-factor compliance)
pub mod env_override;

// Smart defaults by protocol
pub mod defaults;

// DSL support (Caddy-like configuration)
// Note: Not glob-exported to avoid conflicts with schema::Config
pub mod dsl_ast;
pub mod dsl_parser;
pub mod dsl_converter;
pub mod dsl_generator;

pub use schema::*;
pub use loader::*;
pub use validator::*;
pub use watcher::*;
pub use reloader::*;
pub use defaults::ProtocolDefaults;
