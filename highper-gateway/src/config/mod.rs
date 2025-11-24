//! Configuration management module

mod schema;
mod loader;
mod validator;
mod watcher;
mod reloader;

// Enhanced validation with detailed error reporting
pub mod validation;

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
