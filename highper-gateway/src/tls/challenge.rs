//! ACME HTTP-01 challenge handling

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// HTTP-01 challenge token and key authorization
#[derive(Debug, Clone)]
pub struct Challenge {
    /// The challenge token
    pub token: String,
    /// The key authorization
    pub key_authorization: String,
    /// When the challenge was stored
    pub stored_at: Instant,
}

/// Challenge storage for ACME HTTP-01 validation
#[derive(Clone)]
pub struct ChallengeStore {
    /// Map of token -> challenge
    challenges: Arc<DashMap<String, Challenge>>,
    /// How long to keep challenges (default: 1 hour)
    ttl: Duration,
}

impl ChallengeStore {
    /// Create a new challenge store
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(3600))
    }

    /// Create a new challenge store with custom TTL
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            challenges: Arc::new(DashMap::new()),
            ttl,
        }
    }

    /// Store a challenge for validation
    pub fn store(&self, token: String, key_authorization: String) {
        info!("Storing HTTP-01 challenge for token: {}", token);

        let challenge = Challenge {
            token: token.clone(),
            key_authorization,
            stored_at: Instant::now(),
        };

        self.challenges.insert(token, challenge);

        // Clean up expired challenges
        self.cleanup_expired();
    }

    /// Retrieve a challenge by token
    pub fn get(&self, token: &str) -> Option<String> {
        debug!("Looking up HTTP-01 challenge for token: {}", token);

        if let Some(entry) = self.challenges.get(token) {
            let challenge = entry.value();

            // Check if expired
            if challenge.stored_at.elapsed() > self.ttl {
                warn!("Challenge expired for token: {}", token);
                drop(entry);
                self.challenges.remove(token);
                return None;
            }

            debug!("Found valid challenge for token: {}", token);
            Some(challenge.key_authorization.clone())
        } else {
            debug!("No challenge found for token: {}", token);
            None
        }
    }

    /// Remove a challenge
    pub fn remove(&self, token: &str) {
        info!("Removing HTTP-01 challenge for token: {}", token);
        self.challenges.remove(token);
    }

    /// Clean up expired challenges
    fn cleanup_expired(&self) {
        let now = Instant::now();
        let ttl = self.ttl;

        self.challenges
            .retain(|_token, challenge| now.duration_since(challenge.stored_at) <= ttl);
    }

    /// Get the number of stored challenges
    pub fn len(&self) -> usize {
        self.cleanup_expired();
        self.challenges.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all challenges
    pub fn clear(&self) {
        info!("Clearing all HTTP-01 challenges");
        self.challenges.clear();
    }
}

impl Default for ChallengeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_store_and_retrieve() {
        let store = ChallengeStore::new();

        store.store("test-token".to_string(), "test-key-auth".to_string());

        let result = store.get("test-token");
        assert_eq!(result, Some("test-key-auth".to_string()));
    }

    #[test]
    fn test_missing_challenge() {
        let store = ChallengeStore::new();

        let result = store.get("nonexistent");
        assert_eq!(result, None);
    }

    #[test]
    fn test_remove_challenge() {
        let store = ChallengeStore::new();

        store.store("test-token".to_string(), "test-key-auth".to_string());

        store.remove("test-token");

        let result = store.get("test-token");
        assert_eq!(result, None);
    }

    #[test]
    fn test_expiration() {
        let store = ChallengeStore::with_ttl(Duration::from_millis(100));

        store.store("test-token".to_string(), "test-key-auth".to_string());

        // Should be available immediately
        assert!(store.get("test-token").is_some());

        // Wait for expiration
        thread::sleep(Duration::from_millis(150));

        // Should be expired
        assert!(store.get("test-token").is_none());
    }

    #[test]
    fn test_len_and_is_empty() {
        let store = ChallengeStore::new();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.store("token1".to_string(), "key-auth1".to_string());

        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);

        store.store("token2".to_string(), "key-auth2".to_string());

        assert_eq!(store.len(), 2);

        store.clear();
        assert!(store.is_empty());
    }
}
