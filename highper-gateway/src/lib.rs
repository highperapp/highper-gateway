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

pub mod admin;
pub mod cache;
pub mod config;
pub mod discovery;
pub mod gateway;
pub mod grpc;
pub mod http;
pub mod middleware;
pub mod observability;
pub mod plugin;
pub mod proxy;
pub mod runtime;
pub mod runtime_config;
pub mod state;
pub mod tcp;
pub mod tls;
pub mod utils;
pub mod webserver;
pub mod websocket;

// Re-export commonly used types
pub use config::Config;

/// Result type alias using anyhow::Error
pub type Result<T> = anyhow::Result<T>;
