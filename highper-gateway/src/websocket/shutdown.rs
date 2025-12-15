//! WebSocket Graceful Shutdown Handling
//!
//! Provides coordinated shutdown of WebSocket connections with proper close handshakes
//! and timeout-based forced closure.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::{timeout, sleep};
use tracing::{debug, info, warn};
use crate::websocket::{ConnectionTracker, ConnectionState};

/// Shutdown coordinator for WebSocket connections
#[derive(Clone)]
pub struct ShutdownCoordinator {
    /// Shutdown initiated flag
    shutdown_initiated: Arc<AtomicBool>,

    /// Notify for shutdown signal
    shutdown_notify: Arc<Notify>,

    /// Connection tracker
    connection_tracker: Arc<ConnectionTracker>,

    /// Graceful shutdown timeout
    graceful_timeout: Duration,

    /// Force close timeout (after graceful timeout expires)
    force_timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new(
        connection_tracker: Arc<ConnectionTracker>,
        graceful_timeout: Duration,
        force_timeout: Duration,
    ) -> Self {
        Self {
            shutdown_initiated: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            connection_tracker,
            graceful_timeout,
            force_timeout,
        }
    }

    /// Check if shutdown has been initiated
    pub fn is_shutdown_initiated(&self) -> bool {
        self.shutdown_initiated.load(Ordering::Relaxed)
    }

    /// Initiate graceful shutdown
    pub fn initiate_shutdown(&self) {
        if self.shutdown_initiated.swap(true, Ordering::SeqCst) {
            // Shutdown already initiated
            return;
        }

        info!("Initiating graceful WebSocket shutdown");

        // Request graceful shutdown for all connections
        self.connection_tracker.request_graceful_shutdown_all();

        // Notify all waiting tasks
        self.shutdown_notify.notify_waiters();
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&self) {
        if self.is_shutdown_initiated() {
            return;
        }

        self.shutdown_notify.notified().await;
    }

    /// Wait for all connections to close gracefully
    pub async fn wait_for_connections_to_close(&self) -> bool {
        let start = tokio::time::Instant::now();
        let graceful_deadline = start + self.graceful_timeout;

        info!(
            "Waiting up to {:?} for {} WebSocket connections to close gracefully",
            self.graceful_timeout,
            self.connection_tracker.connection_count()
        );

        loop {
            let active_count = self.connection_tracker.connection_count();

            if active_count == 0 {
                info!("All WebSocket connections closed gracefully");
                return true;
            }

            // Check if graceful timeout expired
            if tokio::time::Instant::now() >= graceful_deadline {
                warn!(
                    "{} WebSocket connections did not close within graceful timeout",
                    active_count
                );
                return false;
            }

            // Wait a bit before checking again
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// Force close all remaining connections
    pub async fn force_close_connections(&self) {
        let count = self.connection_tracker.connection_count();

        if count == 0 {
            return;
        }

        warn!(
            "Force closing {} remaining WebSocket connections after {:?} grace period",
            count, self.graceful_timeout
        );

        // Update all remaining connections to Closed state
        // The actual connection handlers will detect this and terminate
        for conn_id in self.connection_tracker.all_connection_ids() {
            self.connection_tracker.update_state(&conn_id, ConnectionState::Closed);
        }

        // Wait briefly for handlers to process the state change
        let force_deadline = tokio::time::Instant::now() + self.force_timeout;

        loop {
            let remaining = self.connection_tracker.connection_count();

            if remaining == 0 {
                info!("All connections force-closed successfully");
                break;
            }

            if tokio::time::Instant::now() >= force_deadline {
                warn!("{} connections remain after force close timeout", remaining);
                // Clear them from the tracker anyway
                self.connection_tracker.clear();
                break;
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    /// Perform complete shutdown sequence
    pub async fn shutdown(&self) {
        self.initiate_shutdown();

        // Wait for graceful closure
        let graceful_success = self.wait_for_connections_to_close().await;

        // Force close if needed
        if !graceful_success {
            self.force_close_connections().await;
        }

        info!("WebSocket shutdown complete");
    }

    /// Get shutdown statistics
    pub fn get_stats(&self) -> ShutdownStats {
        ShutdownStats {
            shutdown_initiated: self.is_shutdown_initiated(),
            active_connections: self.connection_tracker.connection_count(),
            graceful_timeout: self.graceful_timeout,
            force_timeout: self.force_timeout,
        }
    }
}

/// Shutdown statistics
#[derive(Debug, Clone)]
pub struct ShutdownStats {
    pub shutdown_initiated: bool,
    pub active_connections: usize,
    pub graceful_timeout: Duration,
    pub force_timeout: Duration,
}

/// Helper function to perform graceful close handshake
pub async fn graceful_close_handshake<S>(
    stream: &mut S,
    timeout_duration: Duration,
) -> Result<(), std::io::Error>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;

    debug!("Initiating graceful WebSocket close handshake");

    // In a real implementation, this would:
    // 1. Send WebSocket close frame (opcode 0x8)
    // 2. Wait for close frame response
    // 3. Close the underlying TCP connection
    //
    // For now, we'll just flush and shutdown the write side

    match timeout(timeout_duration, async {
        stream.flush().await?;
        stream.shutdown().await
    })
    .await
    {
        Ok(Ok(())) => {
            debug!("Graceful close handshake completed");
            Ok(())
        }
        Ok(Err(e)) => {
            warn!("Error during close handshake: {}", e);
            Err(e)
        }
        Err(_) => {
            warn!("Close handshake timed out after {:?}", timeout_duration);
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Close handshake timeout",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_coordinator_creation() {
        let tracker = Arc::new(ConnectionTracker::default());
        let coordinator = ShutdownCoordinator::new(
            tracker,
            Duration::from_secs(30),
            Duration::from_secs(5),
        );

        assert!(!coordinator.is_shutdown_initiated());
    }

    #[test]
    fn test_shutdown_initiation() {
        let tracker = Arc::new(ConnectionTracker::default());
        let coordinator = ShutdownCoordinator::new(
            tracker,
            Duration::from_secs(30),
            Duration::from_secs(5),
        );

        coordinator.initiate_shutdown();
        assert!(coordinator.is_shutdown_initiated());

        // Second call should be idempotent
        coordinator.initiate_shutdown();
        assert!(coordinator.is_shutdown_initiated());
    }

    #[tokio::test]
    async fn test_wait_for_shutdown() {
        let tracker = Arc::new(ConnectionTracker::default());
        let coordinator = ShutdownCoordinator::new(
            tracker,
            Duration::from_secs(30),
            Duration::from_secs(5),
        );

        let coordinator_clone = coordinator.clone();

        // Spawn task that initiates shutdown after delay
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            coordinator_clone.initiate_shutdown();
        });

        // This should wait for shutdown signal
        coordinator.wait_for_shutdown().await;
        assert!(coordinator.is_shutdown_initiated());
    }

    #[tokio::test]
    async fn test_wait_for_connections_empty() {
        let tracker = Arc::new(ConnectionTracker::default());
        let coordinator = ShutdownCoordinator::new(
            tracker,
            Duration::from_secs(1),
            Duration::from_secs(1),
        );

        coordinator.initiate_shutdown();

        // Should return immediately since no connections
        let success = coordinator.wait_for_connections_to_close().await;
        assert!(success);
    }

    #[tokio::test]
    async fn test_wait_for_connections_with_active() {
        let tracker = Arc::new(ConnectionTracker::default());

        // Register a connection
        tracker.register(0, None, None);
        assert_eq!(tracker.connection_count(), 1);

        let coordinator = ShutdownCoordinator::new(
            tracker.clone(),
            Duration::from_millis(100),
            Duration::from_secs(1),
        );

        coordinator.initiate_shutdown();

        // Spawn task to close connection
        let tracker_clone = tracker.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            tracker_clone.clear();
        });

        // Should wait for connection to close
        let success = coordinator.wait_for_connections_to_close().await;
        assert!(success);
        assert_eq!(tracker.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_wait_for_connections_timeout() {
        let tracker = Arc::new(ConnectionTracker::default());

        // Register a connection that won't be closed
        tracker.register(0, None, None);

        let coordinator = ShutdownCoordinator::new(
            tracker.clone(),
            Duration::from_millis(100), // Short timeout
            Duration::from_secs(1),
        );

        coordinator.initiate_shutdown();

        // Should timeout since connection is not closed
        let success = coordinator.wait_for_connections_to_close().await;
        assert!(!success);
        assert_eq!(tracker.connection_count(), 1);
    }

    #[tokio::test]
    async fn test_force_close_connections() {
        let tracker = Arc::new(ConnectionTracker::default());

        // Register multiple connections
        let conn1 = tracker.register(0, None, None);
        let conn2 = tracker.register(1, None, None);
        assert_eq!(tracker.connection_count(), 2);

        let coordinator = ShutdownCoordinator::new(
            tracker.clone(),
            Duration::from_millis(100),
            Duration::from_millis(200),
        );

        coordinator.force_close_connections().await;

        // Connections should be force-closed
        let conn1_state = tracker.get(&conn1).map(|c| c.state);
        let conn2_state = tracker.get(&conn2).map(|c| c.state);

        // States should be updated to Closed (or connections cleared)
        assert!(
            conn1_state.is_none() || conn1_state == Some(ConnectionState::Closed)
        );
        assert!(
            conn2_state.is_none() || conn2_state == Some(ConnectionState::Closed)
        );
    }

    #[tokio::test]
    async fn test_shutdown_stats() {
        let tracker = Arc::new(ConnectionTracker::default());
        tracker.register(0, None, None);
        tracker.register(1, None, None);

        let coordinator = ShutdownCoordinator::new(
            tracker,
            Duration::from_secs(30),
            Duration::from_secs(5),
        );

        let stats = coordinator.get_stats();
        assert!(!stats.shutdown_initiated);
        assert_eq!(stats.active_connections, 2);
        assert_eq!(stats.graceful_timeout, Duration::from_secs(30));
        assert_eq!(stats.force_timeout, Duration::from_secs(5));

        coordinator.initiate_shutdown();

        let stats = coordinator.get_stats();
        assert!(stats.shutdown_initiated);
    }
}
