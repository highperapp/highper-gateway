//! WebSocket Proxying Support
//!
//! Provides transparent WebSocket proxying with ws:// and wss:// support.
//! Uses existing TLS infrastructure for secure connections.

pub mod handler;

use serde::{Deserialize, Serialize};

/// WebSocket configuration (minimal)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSocketConfig {
    /// Enable WebSocket support
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum message size in bytes
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Ping interval in seconds (for keep-alive)
    #[serde(default = "default_ping_interval")]
    pub ping_interval: u64,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_true() -> bool {
    true
}

fn default_max_message_size() -> usize {
    16 * 1024 * 1024 // 16 MB
}

fn default_ping_interval() -> u64 {
    30
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}
