//! WebSocket Proxying Support
//!
//! Provides transparent WebSocket proxying with ws:// and wss:// support.
//! Uses existing TLS infrastructure for secure connections.
//! Supports sticky sessions for proper load balancing and per-connection state tracking.

pub mod connection;
pub mod handler;
pub mod keepalive;
pub mod recovery;
pub mod session;
pub mod shutdown;

pub use connection::{
    ConnectionId, ConnectionInfo, ConnectionMetrics, ConnectionMetricsSnapshot, ConnectionState,
    ConnectionTracker,
};
pub use keepalive::{
    send_ping_frame, send_pong_frame, KeepAliveConfig, KeepAliveManager, KeepAliveStats,
};
pub use recovery::{
    BackendRecoveryStats, CircuitState, RecoveryConfig, RecoveryManager, WebSocketError,
};
use serde::{Deserialize, Serialize};
pub use session::{SessionId, SessionManager, WebSocketSession};
pub use shutdown::{graceful_close_handshake, ShutdownCoordinator, ShutdownStats};

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Enable sticky sessions (session affinity)
    #[serde(default = "default_true")]
    pub sticky_sessions: bool,

    /// Cookie name for session ID
    #[serde(default = "default_cookie_name")]
    pub session_cookie_name: String,

    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub session_timeout: u64,

    /// Enable per-connection state tracking
    #[serde(default = "default_true")]
    pub track_connections: bool,

    /// Idle connection timeout in seconds (for cleanup)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_message_size: default_max_message_size(),
            ping_interval: default_ping_interval(),
            timeout: default_timeout(),
            sticky_sessions: default_true(),
            session_cookie_name: default_cookie_name(),
            session_timeout: default_session_timeout(),
            track_connections: default_true(),
            idle_timeout: default_idle_timeout(),
        }
    }
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

fn default_cookie_name() -> String {
    "HPGW_WS_SESSION".to_string()
}

fn default_session_timeout() -> u64 {
    3600 // 1 hour
}

fn default_idle_timeout() -> u64 {
    600 // 10 minutes
}
