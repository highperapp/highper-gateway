//! JWT authentication
//!
//! Validates JSON Web Tokens for API authentication.

use super::AuthResult;
use dashmap::DashMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for HS* algorithms or public key for RS* algorithms
    pub secret: String,
    /// Algorithm (HS256, HS384, HS512, RS256, RS384, RS512, ES256, ES384)
    pub algorithm: Algorithm,
    /// Issuer to validate (optional)
    pub issuer: Option<String>,
    /// Audience to validate (optional)
    pub audience: Option<Vec<String>>,
    /// Leeway for time validation (seconds)
    pub leeway: u64,
    /// Enable token caching
    pub enable_cache: bool,
    /// Cache TTL (seconds)
    pub cache_ttl: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
            algorithm: Algorithm::HS256,
            issuer: None,
            audience: None,
            leeway: 60,
            enable_cache: true,
            cache_ttl: 300, // 5 minutes
        }
    }
}

/// Standard JWT claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Expiration time (UNIX timestamp)
    pub exp: u64,
    /// Issued at (UNIX timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<u64>,
    /// Not before (UNIX timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
    /// Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<Vec<String>>,
    /// Custom claims
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Cached token validation result
#[derive(Debug, Clone)]
struct CachedToken {
    user_id: String,
    metadata: HashMap<String, String>,
    cached_at: SystemTime,
    ttl: Duration,
}

impl CachedToken {
    fn is_expired(&self) -> bool {
        match self.cached_at.elapsed() {
            Ok(elapsed) => elapsed > self.ttl,
            Err(_) => true,
        }
    }
}

/// JWT authenticator
pub struct JwtAuthenticator {
    config: JwtConfig,
    decoding_key: DecodingKey,
    validation: Validation,
    cache: Arc<DashMap<String, CachedToken>>,
}

impl JwtAuthenticator {
    /// Create a new JWT authenticator
    pub fn new(config: JwtConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Create decoding key based on algorithm
        let decoding_key = match config.algorithm {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
                DecodingKey::from_secret(config.secret.as_bytes())
            }
            Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
                DecodingKey::from_rsa_pem(config.secret.as_bytes())?
            }
            Algorithm::ES256 | Algorithm::ES384 => {
                DecodingKey::from_ec_pem(config.secret.as_bytes())?
            }
            _ => {
                return Err("Unsupported algorithm".into());
            }
        };

        // Create validation
        let mut validation = Validation::new(config.algorithm);
        validation.leeway = config.leeway;

        if let Some(ref issuer) = config.issuer {
            validation.set_issuer(&[issuer]);
        }

        if let Some(ref audience) = config.audience {
            validation.set_audience(audience);
        }

        Ok(Self {
            config,
            decoding_key,
            validation,
            cache: Arc::new(DashMap::new()),
        })
    }

    /// Create with HS256 algorithm
    pub fn with_hs256(secret: String) -> Result<Self, Box<dyn std::error::Error>> {
        let config = JwtConfig {
            secret,
            algorithm: Algorithm::HS256,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Validate a JWT token
    pub fn validate(&self, token: &str) -> AuthResult {
        // Check cache first if enabled
        if self.config.enable_cache {
            if let Some(cached) = self.cache.get(token) {
                if !cached.is_expired() {
                    debug!("JWT cache hit for token");
                    return AuthResult::Authenticated {
                        user_id: cached.user_id.clone(),
                        metadata: cached.metadata.clone(),
                    };
                } else {
                    drop(cached);
                    self.cache.remove(token);
                    debug!("JWT cache expired, removing");
                }
            }
        }

        // Decode and validate token
        match decode::<Claims>(token, &self.decoding_key, &self.validation) {
            Ok(token_data) => {
                let claims = token_data.claims;
                debug!("JWT validation successful for user: {}", claims.sub);

                // Convert custom claims to metadata
                let mut metadata = HashMap::new();
                for (key, value) in claims.custom {
                    if let Some(str_value) = value.as_str() {
                        metadata.insert(key, str_value.to_string());
                    } else {
                        metadata.insert(key, value.to_string());
                    }
                }

                // Add standard claims to metadata
                if let Some(iat) = claims.iat {
                    metadata.insert("iat".to_string(), iat.to_string());
                }
                if let Some(iss) = claims.iss {
                    metadata.insert("iss".to_string(), iss);
                }

                let result = AuthResult::Authenticated {
                    user_id: claims.sub.clone(),
                    metadata: metadata.clone(),
                };

                // Cache the result if enabled
                if self.config.enable_cache {
                    self.cache.insert(
                        token.to_string(),
                        CachedToken {
                            user_id: claims.sub,
                            metadata,
                            cached_at: SystemTime::now(),
                            ttl: Duration::from_secs(self.config.cache_ttl),
                        },
                    );
                }

                result
            }
            Err(e) => {
                warn!("JWT validation failed: {}", e);
                AuthResult::Failed {
                    reason: format!("Invalid token: {}", e),
                }
            }
        }
    }

    /// Extract token from Authorization header
    pub fn extract_bearer_token(header_value: &str) -> Option<String> {
        header_value.strip_prefix("Bearer ").map(|s| s.to_string())
    }

    /// Clear token cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Cleanup expired cache entries
    pub fn cleanup_cache(&self) {
        self.cache.retain(|_, cached| !cached.is_expired());
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);
            loop {
                cleanup_interval.tick().await;
                self.cleanup_cache();
            }
        });
    }
}

/// Generate a JWT token (for testing)
pub fn generate_token(
    secret: &str,
    user_id: &str,
    ttl_seconds: u64,
    custom_claims: Option<HashMap<String, serde_json::Value>>,
) -> Result<String, Box<dyn std::error::Error>> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let claims = Claims {
        sub: user_id.to_string(),
        exp: now + ttl_seconds,
        iat: Some(now),
        nbf: Some(now),
        iss: None,
        aud: None,
        custom: custom_claims.unwrap_or_default(),
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_hs256_validation() {
        let secret = "test-secret-key-123";
        let auth = JwtAuthenticator::with_hs256(secret.to_string()).unwrap();

        // Generate a valid token
        let token = generate_token(secret, "user123", 3600, None).unwrap();

        // Validate token
        let result = auth.validate(&token);
        assert!(result.is_authenticated());
        assert_eq!(result.user_id(), Some("user123"));
    }

    #[test]
    fn test_jwt_expired_token() {
        let secret = "test-secret-key-456";

        // Create authenticator with 0 leeway to test actual expiration
        let config = JwtConfig {
            secret: secret.to_string(),
            algorithm: Algorithm::HS256,
            leeway: 0, // No leeway for this test
            ..Default::default()
        };
        let auth = JwtAuthenticator::new(config).unwrap();

        // Create token that expired 10 seconds ago (clearly expired)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: "user456".to_string(),
            exp: now - 10,         // Expired 10 seconds ago
            iat: Some(now - 3610), // Issued 1 hour + 10 seconds ago
            nbf: None,
            iss: None,
            aud: None,
            custom: std::collections::HashMap::new(),
        };

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
        let token = jsonwebtoken::encode(&header, &claims, &key).unwrap();

        // Should fail validation (token is already expired)
        let result = auth.validate(&token);
        assert!(
            !result.is_authenticated(),
            "Expired token should be rejected"
        );
    }

    #[test]
    fn test_jwt_invalid_signature() {
        let auth = JwtAuthenticator::with_hs256("correct-secret".to_string()).unwrap();

        // Generate token with different secret
        let token = generate_token("wrong-secret", "user789", 3600, None).unwrap();

        // Should fail validation
        let result = auth.validate(&token);
        assert!(!result.is_authenticated());
    }

    #[test]
    fn test_jwt_custom_claims() {
        let secret = "test-secret";
        let auth = JwtAuthenticator::with_hs256(secret.to_string()).unwrap();

        let mut custom = HashMap::new();
        custom.insert(
            "role".to_string(),
            serde_json::Value::String("admin".to_string()),
        );
        custom.insert(
            "level".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );

        let token = generate_token(secret, "admin-user", 3600, Some(custom)).unwrap();

        let result = auth.validate(&token);
        assert!(result.is_authenticated());

        if let AuthResult::Authenticated { metadata, .. } = result {
            assert_eq!(metadata.get("role"), Some(&"admin".to_string()));
            assert!(metadata.contains_key("level"));
        }
    }

    #[test]
    fn test_extract_bearer_token() {
        let header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...";
        let token = JwtAuthenticator::extract_bearer_token(header);
        assert!(token.is_some());
        assert!(token.unwrap().starts_with("eyJhbG"));

        let invalid = "Basic user:pass";
        assert!(JwtAuthenticator::extract_bearer_token(invalid).is_none());
    }

    #[test]
    fn test_jwt_cache() {
        let secret = "test-cache-secret";
        let auth = JwtAuthenticator::with_hs256(secret.to_string()).unwrap();

        let token = generate_token(secret, "cache-user", 3600, None).unwrap();

        // First validation (cache miss)
        let result1 = auth.validate(&token);
        assert!(result1.is_authenticated());
        assert_eq!(auth.cache_size(), 1);

        // Second validation (cache hit)
        let result2 = auth.validate(&token);
        assert!(result2.is_authenticated());
        assert_eq!(auth.cache_size(), 1);

        // Clear cache
        auth.clear_cache();
        assert_eq!(auth.cache_size(), 0);
    }
}
