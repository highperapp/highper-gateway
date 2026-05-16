//! Configuration management module

mod loader;
mod reloader;
mod schema;
mod validator;
mod watcher;

// Enhanced validation with detailed error reporting
pub mod validation;

// Environment variable configuration overrides (12-factor compliance)
pub mod env_override;

// Smart defaults by protocol
pub mod defaults;

// DSL support (Caddy-like configuration)
// Note: Not glob-exported to avoid conflicts with schema::Config
pub mod dsl_ast;
pub mod dsl_converter;
pub mod dsl_generator;
pub mod dsl_parser;

pub use defaults::ProtocolDefaults;
pub use loader::*;
pub use reloader::*;
pub use schema::*;
pub use validator::*;
pub use watcher::*;
