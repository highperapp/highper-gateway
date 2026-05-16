//! WebSocket Connection State Tracking
//!
//! Provides per-connection state management for monitoring and debugging
//! WebSocket connections, including metrics, lifecycle tracking, and error handling.

use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Connection ID type (UUID v7)
pub type ConnectionId = Uuid;

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,

    /// Connection is active and open
    Connected,

    /// Connection is closing gracefully
    Closing,

    /// Connection is closed
    Closed,
}

impl ConnectionState {
    /// Check if connection is active (connected or closing)
    pub fn is_active(&self) -> bool {
        matches!(self, ConnectionState::Connected | ConnectionState::Closing)
    }

    /// Check if connection is closed
    pub fn is_closed(&self) -> bool {
        matches!(self, ConnectionState::Closed)
    }
}

/// WebSocket connection metrics
#[derive(Debug)]
pub struct ConnectionMetrics {
    /// Number of messages sent to client
    pub messages_sent: AtomicU64,

    /// Number of messages received from client
    pub messages_received: AtomicU64,

    /// Total bytes sent to client
    pub bytes_sent: AtomicU64,

    /// Total bytes received from client
    pub bytes_received: AtomicU64,

    /// Number of ping frames sent
    pub pings_sent: AtomicU64,

    /// Number of pong frames received
    pub pongs_received: AtomicU64,

    /// Number of errors encountered
    pub errors: AtomicU64,
}

impl ConnectionMetrics {
    /// Create new connection metrics
    pub fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            pings_sent: AtomicU64::new(0),
            pongs_received: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    /// Record a message sent
    pub fn record_message_sent(&self, size: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
    }

    /// Record a message received
    pub fn record_message_received(&self, size: usize) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(size as u64, Ordering::Relaxed);
    }

    /// Record a ping sent
    pub fn record_ping_sent(&self) {
        self.pings_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a pong received
    pub fn record_pong_received(&self) {
        self.pongs_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get snapshot of current metrics
    pub fn snapshot(&self) -> ConnectionMetricsSnapshot {
        ConnectionMetricsSnapshot {
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            pings_sent: self.pings_sent.load(Ordering::Relaxed),
            pongs_received: self.pongs_received.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of connection metrics (non-atomic copy)
#[derive(Debug, Clone, Copy)]
pub struct ConnectionMetricsSnapshot {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub pings_sent: u64,
    pub pongs_received: u64,
    pub errors: u64,
}

/// WebSocket connection information
#[derive(Debug)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub id: ConnectionId,

    /// Current connection state
    pub state: ConnectionState,

    /// Backend index this connection is proxied to
    pub backend_index: usize,

    /// Session ID (if using sticky sessions)
    pub session_id: Option<Uuid>,

    /// Client IP address
    pub client_ip: Option<String>,

    /// Connection creation timestamp
    pub created_at: Instant,

    /// Last activity timestamp
    pub last_activity: Instant,

    /// Last message sent timestamp
    pub last_message_sent: Option<Instant>,

    /// Last message received timestamp
    pub last_message_received: Option<Instant>,

    /// Connection metrics
    pub metrics: ConnectionMetrics,

    /// Last error message (if any)
    pub last_error: Option<String>,

    /// Graceful shutdown flag
    pub graceful_shutdown: AtomicBool,
}

impl ConnectionInfo {
    /// Create new connection info
    pub fn new(backend_index: usize, session_id: Option<Uuid>, client_ip: Option<String>) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::now_v7(),
            state: ConnectionState::Connecting,
            backend_index,
            session_id,
            client_ip,
            created_at: now,
            last_activity: now,
            last_message_sent: None,
            last_message_received: None,
            metrics: ConnectionMetrics::new(),
            last_error: None,
            graceful_shutdown: AtomicBool::new(false),
        }
    }

    /// Update connection state
    pub fn set_state(&mut self, state: ConnectionState) {
        debug!(
            "Connection {} state: {:?} -> {:?}",
            self.id, self.state, state
        );
        self.state = state;
        self.touch();
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Record message sent
    pub fn record_message_sent(&mut self, size: usize) {
        self.metrics.record_message_sent(size);
        self.last_message_sent = Some(Instant::now());
        self.touch();
    }

    /// Record message received
    pub fn record_message_received(&mut self, size: usize) {
        self.metrics.record_message_received(size);
        self.last_message_received = Some(Instant::now());
        self.touch();
    }

    /// Record error
    pub fn record_error(&mut self, error: String) {
        self.metrics.record_error();
        self.last_error = Some(error);
        self.touch();
    }

    /// Get connection age
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get time since last activity
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Check if connection is idle for longer than timeout
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.idle_time() > timeout
    }

    /// Request graceful shutdown
    pub fn request_graceful_shutdown(&self) {
        self.graceful_shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if graceful shutdown was requested
    pub fn is_graceful_shutdown_requested(&self) -> bool {
        self.graceful_shutdown.load(Ordering::Relaxed)
    }
}

/// WebSocket connection tracker
#[derive(Clone)]
pub struct ConnectionTracker {
    /// Active connections
    connections: Arc<DashMap<ConnectionId, ConnectionInfo>>,

    /// Idle timeout
    idle_timeout: Duration,
}

impl ConnectionTracker {
    /// Create new connection tracker
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            idle_timeout,
        }
    }

    /// Register a new connection
    pub fn register(
        &self,
        backend_index: usize,
        session_id: Option<Uuid>,
        client_ip: Option<String>,
    ) -> ConnectionId {
        let mut conn = ConnectionInfo::new(backend_index, session_id, client_ip.clone());
        conn.set_state(ConnectionState::Connected);

        let conn_id = conn.id;

        info!(
            "Registered WebSocket connection {} (backend: {}, session: {:?}, client: {:?})",
            conn_id, backend_index, session_id, client_ip
        );

        self.connections.insert(conn_id, conn);
        conn_id
    }

    /// Get connection info
    pub fn get(&self, conn_id: &ConnectionId) -> Option<ConnectionInfo> {
        self.connections.get(conn_id).map(|entry| {
            // Clone the value - ConnectionInfo doesn't implement Clone directly
            // because it contains AtomicBool and other non-Clone fields
            // We'll need to create a snapshot instead
            let info = entry.value();
            ConnectionInfo {
                id: info.id,
                state: info.state,
                backend_index: info.backend_index,
                session_id: info.session_id,
                client_ip: info.client_ip.clone(),
                created_at: info.created_at,
                last_activity: info.last_activity,
                last_message_sent: info.last_message_sent,
                last_message_received: info.last_message_received,
                metrics: ConnectionMetrics::new(), // Create new metrics (snapshot will be separate)
                last_error: info.last_error.clone(),
                graceful_shutdown: AtomicBool::new(info.graceful_shutdown.load(Ordering::Relaxed)),
            }
        })
    }

    /// Update connection state
    pub fn update_state(&self, conn_id: &ConnectionId, state: ConnectionState) {
        if let Some(mut entry) = self.connections.get_mut(conn_id) {
            entry.set_state(state);
        }
    }

    /// Record message sent
    pub fn record_message_sent(&self, conn_id: &ConnectionId, size: usize) {
        if let Some(mut entry) = self.connections.get_mut(conn_id) {
            entry.record_message_sent(size);
        }
    }

    /// Record message received
    pub fn record_message_received(&self, conn_id: &ConnectionId, size: usize) {
        if let Some(mut entry) = self.connections.get_mut(conn_id) {
            entry.record_message_received(size);
        }
    }

    /// Record error
    pub fn record_error(&self, conn_id: &ConnectionId, error: String) {
        if let Some(mut entry) = self.connections.get_mut(conn_id) {
            entry.record_error(error);
        }
    }

    /// Record ping sent
    pub fn record_ping_sent(&self, conn_id: &ConnectionId) {
        if let Some(entry) = self.connections.get(conn_id) {
            entry
                .value()
                .metrics
                .pings_sent
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record pong received
    pub fn record_pong_received(&self, conn_id: &ConnectionId) {
        if let Some(entry) = self.connections.get(conn_id) {
            entry
                .value()
                .metrics
                .pongs_received
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get metrics snapshot for a connection
    pub fn get_metrics(&self, conn_id: &ConnectionId) -> Option<ConnectionMetricsSnapshot> {
        self.connections
            .get(conn_id)
            .map(|entry| entry.value().metrics.snapshot())
    }

    /// Unregister a connection
    pub fn unregister(&self, conn_id: &ConnectionId) {
        if let Some((_, conn)) = self.connections.remove(conn_id) {
            info!(
                "Unregistered WebSocket connection {} (age: {:?}, messages: sent={} recv={})",
                conn_id,
                conn.age(),
                conn.metrics.messages_sent.load(Ordering::Relaxed),
                conn.metrics.messages_received.load(Ordering::Relaxed)
            );
        }
    }

    /// Get total number of active connections
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get connections for a specific backend
    pub fn connections_for_backend(&self, backend_index: usize) -> Vec<ConnectionId> {
        self.connections
            .iter()
            .filter(|entry| entry.value().backend_index == backend_index)
            .map(|entry| *entry.key())
            .collect()
    }

    /// Clean up idle connections
    pub fn cleanup_idle(&self) -> usize {
        let idle: Vec<ConnectionId> = self
            .connections
            .iter()
            .filter(|entry| entry.value().is_idle(self.idle_timeout))
            .map(|entry| *entry.key())
            .collect();

        let count = idle.len();
        for conn_id in idle {
            if let Some((_, conn)) = self.connections.remove(&conn_id) {
                warn!(
                    "Cleaned up idle connection {} (idle: {:?})",
                    conn_id,
                    conn.idle_time()
                );
            }
        }

        if count > 0 {
            debug!("Cleaned up {} idle connections", count);
        }

        count
    }

    /// Request graceful shutdown for all connections
    pub fn request_graceful_shutdown_all(&self) {
        for entry in self.connections.iter() {
            entry.value().request_graceful_shutdown();
        }
        info!(
            "Requested graceful shutdown for {} connections",
            self.connections.len()
        );
    }

    /// Get all connection IDs
    pub fn all_connection_ids(&self) -> Vec<ConnectionId> {
        self.connections.iter().map(|entry| *entry.key()).collect()
    }

    /// Clear all connections
    pub fn clear(&self) {
        let count = self.connections.len();
        self.connections.clear();
        debug!("Cleared {} connections", count);
    }
}

impl Default for ConnectionTracker {
    fn default() -> Self {
        Self::new(Duration::from_secs(300)) // 5 minutes default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_connection_state_transitions() {
        let state = ConnectionState::Connecting;
        assert!(!state.is_active());
        assert!(!state.is_closed());

        let state = ConnectionState::Connected;
        assert!(state.is_active());
        assert!(!state.is_closed());

        let state = ConnectionState::Closing;
        assert!(state.is_active());
        assert!(!state.is_closed());

        let state = ConnectionState::Closed;
        assert!(!state.is_active());
        assert!(state.is_closed());
    }

    #[test]
    fn test_connection_metrics() {
        let metrics = ConnectionMetrics::new();

        metrics.record_message_sent(100);
        metrics.record_message_sent(200);
        metrics.record_message_received(50);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_sent, 2);
        assert_eq!(snapshot.bytes_sent, 300);
        assert_eq!(snapshot.messages_received, 1);
        assert_eq!(snapshot.bytes_received, 50);
    }

    #[test]
    fn test_connection_info_creation() {
        let conn = ConnectionInfo::new(0, None, Some("192.168.1.1".to_string()));

        assert_eq!(conn.state, ConnectionState::Connecting);
        assert_eq!(conn.backend_index, 0);
        assert_eq!(conn.client_ip, Some("192.168.1.1".to_string()));
        assert!(conn.age() < Duration::from_millis(100));
    }

    #[test]
    fn test_connection_info_state_update() {
        let mut conn = ConnectionInfo::new(0, None, None);

        assert_eq!(conn.state, ConnectionState::Connecting);

        conn.set_state(ConnectionState::Connected);
        assert_eq!(conn.state, ConnectionState::Connected);

        conn.set_state(ConnectionState::Closing);
        assert_eq!(conn.state, ConnectionState::Closing);
    }

    #[test]
    fn test_connection_info_activity() {
        let mut conn = ConnectionInfo::new(0, None, None);

        conn.record_message_sent(100);
        assert!(conn.last_message_sent.is_some());
        assert_eq!(conn.metrics.messages_sent.load(Ordering::Relaxed), 1);

        conn.record_message_received(50);
        assert!(conn.last_message_received.is_some());
        assert_eq!(conn.metrics.messages_received.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_connection_tracker_register() {
        let tracker = ConnectionTracker::new(Duration::from_secs(300));

        let conn_id = tracker.register(0, None, Some("192.168.1.1".to_string()));

        assert_eq!(tracker.connection_count(), 1);

        let conn = tracker.get(&conn_id);
        assert!(conn.is_some());
        let conn = conn.unwrap();
        assert_eq!(conn.backend_index, 0);
        assert_eq!(conn.state, ConnectionState::Connected);
    }

    #[test]
    fn test_connection_tracker_unregister() {
        let tracker = ConnectionTracker::new(Duration::from_secs(300));

        let conn_id = tracker.register(0, None, None);
        assert_eq!(tracker.connection_count(), 1);

        tracker.unregister(&conn_id);
        assert_eq!(tracker.connection_count(), 0);
    }

    #[test]
    fn test_connection_tracker_update_state() {
        let tracker = ConnectionTracker::new(Duration::from_secs(300));

        let conn_id = tracker.register(0, None, None);

        tracker.update_state(&conn_id, ConnectionState::Closing);

        let conn = tracker.get(&conn_id).unwrap();
        assert_eq!(conn.state, ConnectionState::Closing);
    }

    #[test]
    fn test_connection_tracker_record_messages() {
        let tracker = ConnectionTracker::new(Duration::from_secs(300));

        let conn_id = tracker.register(0, None, None);

        tracker.record_message_sent(&conn_id, 100);
        tracker.record_message_received(&conn_id, 50);

        let metrics = tracker.get_metrics(&conn_id).unwrap();
        assert_eq!(metrics.messages_sent, 1);
        assert_eq!(metrics.messages_received, 1);
    }

    #[tokio::test]
    async fn test_connection_tracker_cleanup_idle() {
        let tracker = ConnectionTracker::new(Duration::from_millis(50));

        let _conn_id = tracker.register(0, None, None);
        assert_eq!(tracker.connection_count(), 1);

        // Wait for connection to become idle
        sleep(Duration::from_millis(100)).await;

        let cleaned = tracker.cleanup_idle();
        assert_eq!(cleaned, 1);
        assert_eq!(tracker.connection_count(), 0);
    }

    #[test]
    fn test_connections_for_backend() {
        let tracker = ConnectionTracker::new(Duration::from_secs(300));

        tracker.register(0, None, None);
        tracker.register(0, None, None);
        tracker.register(1, None, None);

        let backend0_conns = tracker.connections_for_backend(0);
        assert_eq!(backend0_conns.len(), 2);

        let backend1_conns = tracker.connections_for_backend(1);
        assert_eq!(backend1_conns.len(), 1);
    }
}
