//! Authentication module for API gateway
//!
//! Provides JWT, API key, OAuth2, and basic authentication.

pub mod jwt;
pub mod api_key;
pub mod oauth2;

/// Authentication result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// Authentication succeeded
    Authenticated {
        /// User identifier
        user_id: String,
        /// Additional claims or metadata
        metadata: std::collections::HashMap<String, String>,
    },
    /// Authentication failed
    Failed {
        /// Reason for failure
        reason: String,
    },
}

impl AuthResult {
    /// Check if authentication succeeded
    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthResult::Authenticated { .. })
    }

    /// Get user ID if authenticated
    pub fn user_id(&self) -> Option<&str> {
        match self {
            AuthResult::Authenticated { user_id, .. } => Some(user_id),
            AuthResult::Failed { .. } => None,
        }
    }
}
