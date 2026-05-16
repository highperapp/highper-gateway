//! WebSocket Session Management
//!
//! Provides sticky session support for WebSocket connections to ensure
//! that all frames from a client are routed to the same backend server.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

/// Session ID type (UUID v4)
pub type SessionId = Uuid;

/// WebSocket session information
#[derive(Debug, Clone)]
pub struct WebSocketSession {
    /// Unique session identifier
    pub id: SessionId,

    /// Backend server index that this session is bound to
    pub backend_index: usize,

    /// Timestamp when the session was created
    pub created_at: Instant,

    /// Timestamp of last activity
    pub last_activity: Instant,

    /// Client identifier (IP address or similar)
    pub client_id: Option<String>,
}

impl WebSocketSession {
    /// Create a new WebSocket session
    pub fn new(backend_index: usize, client_id: Option<String>) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::now_v7(),
            backend_index,
            created_at: now,
            last_activity: now,
            client_id,
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if session has expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Get session age
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// WebSocket session manager with sticky session support
#[derive(Clone)]
pub struct SessionManager {
    /// Active sessions mapped by session ID
    sessions: Arc<DashMap<SessionId, WebSocketSession>>,

    /// Session timeout duration
    timeout: Duration,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            timeout,
        }
    }

    /// Create and register a new session
    pub fn create_session(
        &self,
        backend_index: usize,
        client_id: Option<String>,
    ) -> WebSocketSession {
        let session = WebSocketSession::new(backend_index, client_id.clone());

        debug!(
            "Created WebSocket session {} for backend {} (client: {:?})",
            session.id, backend_index, client_id
        );

        self.sessions.insert(session.id, session.clone());
        session
    }

    /// Get an existing session by ID
    pub fn get_session(&self, session_id: &SessionId) -> Option<WebSocketSession> {
        self.sessions.get(session_id).map(|entry| {
            let mut session = entry.value().clone();
            session.touch();
            // Update the stored session with new timestamp
            drop(entry);
            self.sessions.insert(*session_id, session.clone());
            session
        })
    }

    /// Remove a session
    pub fn remove_session(&self, session_id: &SessionId) -> Option<WebSocketSession> {
        self.sessions.remove(session_id).map(|(_, session)| {
            debug!("Removed WebSocket session {}", session_id);
            session
        })
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let expired: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|entry| entry.value().is_expired(self.timeout))
            .map(|entry| *entry.key())
            .collect();

        let count = expired.len();
        for session_id in expired {
            if let Some(session) = self.sessions.remove(&session_id) {
                warn!(
                    "Session {} expired after {:?} (backend: {})",
                    session_id,
                    session.1.age(),
                    session.1.backend_index
                );
            }
        }

        if count > 0 {
            debug!("Cleaned up {} expired WebSocket sessions", count);
        }

        count
    }

    /// Get total number of active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get session timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get all sessions for a specific backend
    pub fn sessions_for_backend(&self, backend_index: usize) -> Vec<WebSocketSession> {
        self.sessions
            .iter()
            .filter(|entry| entry.value().backend_index == backend_index)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Clear all sessions (useful for testing or shutdown)
    pub fn clear(&self) {
        let count = self.sessions.len();
        self.sessions.clear();
        debug!("Cleared {} WebSocket sessions", count);
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(300)) // 5 minutes default timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_session_creation() {
        let session = WebSocketSession::new(0, Some("192.168.1.1".to_string()));

        assert_eq!(session.backend_index, 0);
        assert_eq!(session.client_id, Some("192.168.1.1".to_string()));
        assert!(session.age() < Duration::from_millis(100));
    }

    #[test]
    fn test_session_expiration() {
        let mut session = WebSocketSession::new(0, None);

        // Fresh session should not be expired
        assert!(!session.is_expired(Duration::from_secs(1)));

        // Simulate old activity
        session.last_activity = Instant::now() - Duration::from_secs(2);

        // Should be expired with 1 second timeout
        assert!(session.is_expired(Duration::from_secs(1)));

        // Should not be expired with 5 second timeout
        assert!(!session.is_expired(Duration::from_secs(5)));
    }

    #[test]
    fn test_session_touch() {
        let mut session = WebSocketSession::new(0, None);
        let original_activity = session.last_activity;

        std::thread::sleep(Duration::from_millis(10));
        session.touch();

        assert!(session.last_activity > original_activity);
    }

    #[test]
    fn test_session_manager_create() {
        let manager = SessionManager::new(Duration::from_secs(300));

        let session = manager.create_session(0, Some("192.168.1.1".to_string()));

        assert_eq!(manager.session_count(), 1);
        assert_eq!(session.backend_index, 0);
    }

    #[test]
    fn test_session_manager_get() {
        let manager = SessionManager::new(Duration::from_secs(300));

        let created = manager.create_session(0, None);
        let retrieved = manager.get_session(&created.id);

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.backend_index, created.backend_index);
    }

    #[test]
    fn test_session_manager_remove() {
        let manager = SessionManager::new(Duration::from_secs(300));

        let session = manager.create_session(0, None);
        assert_eq!(manager.session_count(), 1);

        let removed = manager.remove_session(&session.id);
        assert!(removed.is_some());
        assert_eq!(manager.session_count(), 0);

        // Second remove should return None
        let removed_again = manager.remove_session(&session.id);
        assert!(removed_again.is_none());
    }

    #[tokio::test]
    async fn test_session_manager_cleanup() {
        let manager = SessionManager::new(Duration::from_millis(50));

        // Create sessions
        let _session1 = manager.create_session(0, None);
        let _session2 = manager.create_session(1, None);

        assert_eq!(manager.session_count(), 2);

        // Wait for sessions to expire
        sleep(Duration::from_millis(100)).await;

        // Cleanup should remove expired sessions
        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 2);
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_sessions_for_backend() {
        let manager = SessionManager::new(Duration::from_secs(300));

        manager.create_session(0, None);
        manager.create_session(0, None);
        manager.create_session(1, None);
        manager.create_session(2, None);

        let backend0_sessions = manager.sessions_for_backend(0);
        assert_eq!(backend0_sessions.len(), 2);

        let backend1_sessions = manager.sessions_for_backend(1);
        assert_eq!(backend1_sessions.len(), 1);

        let backend2_sessions = manager.sessions_for_backend(2);
        assert_eq!(backend2_sessions.len(), 1);
    }

    #[test]
    fn test_session_manager_clear() {
        let manager = SessionManager::new(Duration::from_secs(300));

        manager.create_session(0, None);
        manager.create_session(1, None);
        manager.create_session(2, None);

        assert_eq!(manager.session_count(), 3);

        manager.clear();
        assert_eq!(manager.session_count(), 0);
    }
}
