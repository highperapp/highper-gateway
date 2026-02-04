//! WebSocket Keep-Alive (Ping/Pong) Management
//!
//! Provides periodic ping sending and pong tracking to detect dead connections.
//! Automatically cleans up connections that don't respond to pings.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tracing::{debug, info, warn};
use crate::websocket::{ConnectionTracker, ConnectionId, ConnectionState};

/// Keep-alive manager configuration
#[derive(Debug, Clone)]
pub struct KeepAliveConfig {
    /// Ping interval
    pub ping_interval: Duration,

    /// Pong timeout (how long to wait for pong response)
    pub pong_timeout: Duration,

    /// Maximum missed pongs before considering connection dead
    pub max_missed_pongs: u32,

    /// Enable keep-alive
    pub enabled: bool,
}

impl Default for KeepAliveConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(5),
            max_missed_pongs: 3,
            enabled: true,
        }
    }
}

/// Keep-alive manager for WebSocket connections
#[derive(Clone)]
pub struct KeepAliveManager {
    config: KeepAliveConfig,
    connection_tracker: Arc<ConnectionTracker>,
}

impl KeepAliveManager {
    /// Create a new keep-alive manager
    pub fn new(config: KeepAliveConfig, connection_tracker: Arc<ConnectionTracker>) -> Self {
        Self {
            config,
            connection_tracker,
        }
    }

    /// Start the keep-alive monitor task
    ///
    /// This spawns a background task that periodically:
    /// 1. Sends pings to all active connections
    /// 2. Checks for missing pong responses
    /// 3. Cleans up dead connections
    pub fn start_monitor(&self) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();

        tokio::spawn(async move {
            if !manager.config.enabled {
                info!("Keep-alive monitoring disabled");
                return;
            }

            info!(
                "Starting WebSocket keep-alive monitor (interval: {:?}, timeout: {:?})",
                manager.config.ping_interval,
                manager.config.pong_timeout
            );

            let mut interval = interval(manager.config.ping_interval);

            loop {
                interval.tick().await;

                // Check all connections for liveness
                manager.check_all_connections().await;
            }
        })
    }

    /// Check all connections and clean up dead ones
    async fn check_all_connections(&self) {
        let connection_ids = self.connection_tracker.all_connection_ids();
        let mut dead_connections = Vec::new();

        for conn_id in connection_ids {
            if let Some(conn_info) = self.connection_tracker.get(&conn_id) {
                // Skip non-active connections
                if !conn_info.state.is_active() {
                    continue;
                }

                // Check if connection needs a ping
                let needs_ping = conn_info.last_activity.elapsed() >= self.config.ping_interval;

                if needs_ping {
                    // Get actual metrics (not from snapshot)
                    if let Some(metrics) = self.connection_tracker.get_metrics(&conn_id) {
                        // Check pong history to detect dead connections
                        let missed_pongs = metrics.pings_sent.saturating_sub(metrics.pongs_received);

                        if missed_pongs >= self.config.max_missed_pongs as u64 {
                            warn!(
                                "Connection {} is dead (missed {} pongs), marking for cleanup",
                                conn_id, missed_pongs
                            );
                            dead_connections.push(conn_id);
                        } else {
                            // Connection is responsive, increment ping counter
                            self.connection_tracker.record_ping_sent(&conn_id);

                            debug!(
                                "Connection {} sent ping ({} missed pongs)",
                                conn_id, missed_pongs
                            );

                            // Note: Actual ping frame sending would happen in the connection handler
                            // This is just tracking - the handler should check metrics and send pings
                        }
                    }
                }
            }
        }

        // Clean up dead connections
        for conn_id in dead_connections {
            self.connection_tracker.update_state(&conn_id, ConnectionState::Closed);
            self.connection_tracker.unregister(&conn_id);
        }
    }

    /// Record a pong response for a connection
    ///
    /// Should be called by connection handlers when a pong frame is received
    pub fn record_pong(&self, conn_id: &ConnectionId) {
        self.connection_tracker.record_pong_received(conn_id);
        debug!("Connection {} received pong", conn_id);
    }

    /// Check if a connection should send a ping
    ///
    /// Connection handlers should call this to decide when to send ping frames
    pub fn should_send_ping(&self, conn_id: &ConnectionId) -> bool {
        if !self.config.enabled {
            return false;
        }

        if let Some(conn_info) = self.connection_tracker.get(conn_id) {
            conn_info.last_activity.elapsed() >= self.config.ping_interval
        } else {
            false
        }
    }

    /// Get keep-alive statistics
    pub fn get_stats(&self) -> KeepAliveStats {
        let connection_ids = self.connection_tracker.all_connection_ids();
        let mut total_pings = 0u64;
        let mut total_pongs = 0u64;
        let mut active_connections = 0;

        for conn_id in connection_ids {
            if let Some(metrics) = self.connection_tracker.get_metrics(&conn_id) {
                active_connections += 1;
                total_pings += metrics.pings_sent;
                total_pongs += metrics.pongs_received;
            }
        }

        KeepAliveStats {
            active_connections,
            total_pings_sent: total_pings,
            total_pongs_received: total_pongs,
            missed_pongs: total_pings.saturating_sub(total_pongs),
            config: self.config.clone(),
        }
    }
}

/// Keep-alive statistics
#[derive(Debug, Clone)]
pub struct KeepAliveStats {
    pub active_connections: usize,
    pub total_pings_sent: u64,
    pub total_pongs_received: u64,
    pub missed_pongs: u64,
    pub config: KeepAliveConfig,
}

/// Helper function to send a WebSocket ping frame
///
/// This is a low-level helper that actual connection handlers should use.
/// In a real implementation, this would encode a proper WebSocket ping frame (opcode 0x9).
pub async fn send_ping_frame<W>(
    stream: &mut W,
    payload: &[u8],
) -> Result<(), std::io::Error>
where
    W: AsyncWrite + Unpin,
{
    debug!("Sending WebSocket ping frame ({} bytes payload)", payload.len());

    // In a real implementation, this would:
    // 1. Create WebSocket frame header (FIN=1, opcode=0x9 for ping)
    // 2. Add payload length
    // 3. Write frame to stream
    //
    // For now, we'll just flush the stream as a placeholder
    stream.flush().await?;

    Ok(())
}

/// Helper function to send a WebSocket pong frame
///
/// Should be called when a ping is received to respond with pong (opcode 0xA).
pub async fn send_pong_frame<W>(
    stream: &mut W,
    payload: &[u8],
) -> Result<(), std::io::Error>
where
    W: AsyncWrite + Unpin,
{
    debug!("Sending WebSocket pong frame ({} bytes payload)", payload.len());

    // In a real implementation, this would:
    // 1. Create WebSocket frame header (FIN=1, opcode=0xA for pong)
    // 2. Echo the ping payload
    // 3. Write frame to stream
    //
    // For now, we'll just flush the stream as a placeholder
    stream.flush().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_keepalive_config_default() {
        let config = KeepAliveConfig::default();
        assert_eq!(config.ping_interval, Duration::from_secs(30));
        assert_eq!(config.pong_timeout, Duration::from_secs(5));
        assert_eq!(config.max_missed_pongs, 3);
        assert!(config.enabled);
    }

    #[test]
    fn test_keepalive_manager_creation() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig::default();
        let manager = KeepAliveManager::new(config, tracker);

        let stats = manager.get_stats();
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.total_pings_sent, 0);
        assert_eq!(stats.total_pongs_received, 0);
    }

    #[test]
    fn test_should_send_ping_disabled() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = KeepAliveManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        // Should not send ping when disabled
        assert!(!manager.should_send_ping(&conn_id));
    }

    #[test]
    fn test_should_send_ping_fresh_connection() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig::default();
        let manager = KeepAliveManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        // Fresh connection should not need ping yet
        assert!(!manager.should_send_ping(&conn_id));
    }

    #[tokio::test]
    async fn test_should_send_ping_after_interval() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig {
            ping_interval: Duration::from_millis(50),
            ..Default::default()
        };
        let manager = KeepAliveManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        // Fresh connection should not need ping
        assert!(!manager.should_send_ping(&conn_id));

        // Wait for ping interval to elapse
        sleep(Duration::from_millis(100)).await;

        // Now should need ping
        assert!(manager.should_send_ping(&conn_id));
    }

    #[test]
    fn test_record_pong() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig::default();
        let manager = KeepAliveManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        // Initially no pongs
        let metrics = tracker.get_metrics(&conn_id).unwrap();
        assert_eq!(metrics.pongs_received, 0);

        // Record a pong
        manager.record_pong(&conn_id);

        // Should increment pong counter
        let metrics = tracker.get_metrics(&conn_id).unwrap();
        assert_eq!(metrics.pongs_received, 1);
    }

    #[tokio::test]
    async fn test_check_dead_connection() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig {
            ping_interval: Duration::from_millis(10),
            max_missed_pongs: 2,
            ..Default::default()
        };
        let manager = KeepAliveManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        // Wait for ping interval
        sleep(Duration::from_millis(50)).await;

        // Simulate missed pongs by sending pings without receiving pongs
        for _ in 0..3 {
            tracker.record_ping_sent(&conn_id);
        }

        // Check connections - should mark as dead
        manager.check_all_connections().await;

        // Connection should be unregistered
        assert!(tracker.get_metrics(&conn_id).is_none());
    }

    #[test]
    fn test_keepalive_stats() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = KeepAliveConfig::default();
        let manager = KeepAliveManager::new(config, tracker.clone());

        // Register connections
        let conn1 = tracker.register(0, None, None);
        let conn2 = tracker.register(1, None, None);

        // Simulate some ping/pong activity for conn1 (5 pings, 4 pongs)
        for _ in 0..5 {
            tracker.record_ping_sent(&conn1);
        }
        for _ in 0..4 {
            tracker.record_pong_received(&conn1);
        }

        // Simulate some ping/pong activity for conn2 (3 pings, 3 pongs)
        for _ in 0..3 {
            tracker.record_ping_sent(&conn2);
        }
        for _ in 0..3 {
            tracker.record_pong_received(&conn2);
        }

        let stats = manager.get_stats();
        assert_eq!(stats.active_connections, 2);
        assert_eq!(stats.total_pings_sent, 8);
        assert_eq!(stats.total_pongs_received, 7);
        assert_eq!(stats.missed_pongs, 1);
    }
}
