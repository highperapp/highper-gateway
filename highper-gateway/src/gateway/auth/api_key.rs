//! API key authentication
//!
//! Simple API key validation for API gateway.

use super::AuthResult;
use dashmap::DashMap;
use std::sync::Arc;

/// API key configuration
#[derive(Debug, Clone)]
pub struct ApiKeyConfig {
    /// Header name for API key (e.g., "X-API-Key")
    pub header_name: String,
    /// Whether to allow query parameter fallback
    pub allow_query: bool,
    /// Query parameter name if allowed
    pub query_name: String,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            header_name: "X-API-Key".to_string(),
            allow_query: false,
            query_name: "api_key".to_string(),
        }
    }
}

/// API key entry
#[derive(Debug, Clone)]
pub struct ApiKey {
    /// The API key
    pub key: String,
    /// User/client identifier
    pub user_id: String,
    /// Whether the key is active
    pub active: bool,
    /// Optional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// API key authenticator
pub struct ApiKeyAuthenticator {
    config: ApiKeyConfig,
    keys: Arc<DashMap<String, ApiKey>>,
}

impl ApiKeyAuthenticator {
    /// Create a new API key authenticator
    pub fn new(config: ApiKeyConfig) -> Self {
        Self {
            config,
            keys: Arc::new(DashMap::new()),
        }
    }

    /// Create with default configuration
    pub fn default_authenticator() -> Self {
        Self::new(ApiKeyConfig::default())
    }

    /// Add an API key
    pub fn add_key(&self, key: ApiKey) {
        self.keys.insert(key.key.clone(), key);
    }

    /// Remove an API key
    pub fn remove_key(&self, key: &str) {
        self.keys.remove(key);
    }

    /// Validate an API key
    pub fn validate(&self, key: &str) -> AuthResult {
        match self.keys.get(key) {
            Some(api_key) => {
                if api_key.active {
                    AuthResult::Authenticated {
                        user_id: api_key.user_id.clone(),
                        metadata: api_key.metadata.clone(),
                    }
                } else {
                    AuthResult::Failed {
                        reason: "API key is inactive".to_string(),
                    }
                }
            }
            None => AuthResult::Failed {
                reason: "Invalid API key".to_string(),
            },
        }
    }

    /// Get key count
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_validation() {
        let auth = ApiKeyAuthenticator::default_authenticator();

        let api_key = ApiKey {
            key: "test-key-123".to_string(),
            user_id: "user1".to_string(),
            active: true,
            metadata: std::collections::HashMap::new(),
        };

        auth.add_key(api_key);

        // Valid key should authenticate
        let result = auth.validate("test-key-123");
        assert!(result.is_authenticated());
        assert_eq!(result.user_id(), Some("user1"));

        // Invalid key should fail
        let result = auth.validate("wrong-key");
        assert!(!result.is_authenticated());
    }

    #[test]
    fn test_inactive_key() {
        let auth = ApiKeyAuthenticator::default_authenticator();

        let api_key = ApiKey {
            key: "inactive-key".to_string(),
            user_id: "user2".to_string(),
            active: false,
            metadata: std::collections::HashMap::new(),
        };

        auth.add_key(api_key);

        let result = auth.validate("inactive-key");
        assert!(!result.is_authenticated());
    }

    #[test]
    fn test_remove_key() {
        let auth = ApiKeyAuthenticator::default_authenticator();

        let api_key = ApiKey {
            key: "temp-key".to_string(),
            user_id: "user3".to_string(),
            active: true,
            metadata: std::collections::HashMap::new(),
        };

        auth.add_key(api_key);
        assert_eq!(auth.key_count(), 1);

        auth.remove_key("temp-key");
        assert_eq!(auth.key_count(), 0);

        let result = auth.validate("temp-key");
        assert!(!result.is_authenticated());
    }
}
