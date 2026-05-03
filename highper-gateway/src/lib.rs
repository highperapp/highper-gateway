// Copyright 2024-2026 Highper Gateway Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
pub mod runtime_config;

// Re-export commonly used types
pub use config::Config;

/// Result type alias using anyhow::Error
pub type Result<T> = anyhow::Result<T>;
