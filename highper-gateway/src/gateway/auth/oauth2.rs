//! OAuth2 and OpenID Connect (OIDC) authentication
//!
//! Provides OAuth2 authorization code flow with PKCE and OIDC ID token validation.

use anyhow::{Context, Result};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse as OAuth2TokenResponse,
    TokenUrl,
};
use openidconnect::{
    core::CoreClient,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// OAuth2/OIDC configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuth2Config {
    /// OAuth2 provider (google, github, okta, auth0, custom)
    pub provider: String,

    /// OAuth2 client ID
    pub client_id: String,

    /// OAuth2 client secret
    pub client_secret: String,

    /// Redirect URI (callback URL)
    pub redirect_uri: String,

    /// OAuth2 scopes
    #[serde(default)]
    pub scopes: Vec<String>,

    /// OIDC issuer URL (for discovery)
    pub issuer_url: Option<String>,

    /// Manual authorization URL (if not using discovery)
    pub auth_url: Option<String>,

    /// Manual token URL (if not using discovery)
    pub token_url: Option<String>,

    /// Token validation enabled
    #[serde(default = "default_true")]
    pub validate_token: bool,

    /// Required claims for validation
    #[serde(default)]
    pub required_claims: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

impl Default for OAuth2Config {
    fn default() -> Self {
        Self {
            provider: "custom".to_string(),
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: String::new(),
            scopes: vec!["openid".to_string(), "profile".to_string()],
            issuer_url: None,
            auth_url: None,
            token_url: None,
            validate_token: true,
            required_claims: HashMap::new(),
        }
    }
}

/// OAuth2/OIDC handler
pub struct OAuth2Handler {
    config: OAuth2Config,
    oauth_client: BasicClient,
    oidc_client: Option<CoreClient>,
}

impl OAuth2Handler {
    /// Create new OAuth2 handler
    ///
    /// If issuer_url is provided, performs OIDC discovery.
    /// Otherwise, uses manually configured URLs.
    pub async fn new(config: OAuth2Config) -> Result<Self> {
        info!("Initializing OAuth2 handler for provider: {}", config.provider);

        // Create basic OAuth2 client
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = Some(ClientSecret::new(config.client_secret.clone()));

        // Determine auth and token URLs
        let (auth_url, token_url, oidc_client) = if let Some(_issuer_url) = &config.issuer_url {
            // TODO: OIDC discovery implementation
            // The openidconnect crate API varies by version
            // Full implementation requires:
            // 1. CoreProviderMetadata::discover_async()
            // 2. CoreClient::from_provider_metadata()
            // 3. Proper async_http_client configuration
            info!("OIDC discovery requested but not yet implemented - using manual config");

            let auth_url = AuthUrl::new(
                config
                    .auth_url
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("auth_url required"))?,
            )
            .context("Invalid auth URL")?;

            let token_url = config
                .token_url
                .as_ref()
                .map(|url| TokenUrl::new(url.clone()))
                .transpose()
                .context("Invalid token URL")?;

            (auth_url, token_url, None)
        } else {
            // Manual configuration
            info!("Using manual OAuth2 configuration");

            let auth_url = AuthUrl::new(
                config
                    .auth_url
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("auth_url required when issuer_url not provided"))?,
            )
            .context("Invalid auth URL")?;

            let token_url = config
                .token_url
                .as_ref()
                .map(|url| TokenUrl::new(url.clone()))
                .transpose()
                .context("Invalid token URL")?;

            (auth_url, token_url, None)
        };

        let oauth_client = BasicClient::new(client_id, client_secret, auth_url, token_url)
            .set_redirect_uri(
                RedirectUrl::new(config.redirect_uri.clone()).context("Invalid redirect URI")?,
            );

        info!("OAuth2 handler initialized successfully");

        Ok(Self {
            config,
            oauth_client,
            oidc_client,
        })
    }

    /// Generate authorization URL with PKCE
    ///
    /// Returns: (auth_url, csrf_token, pkce_verifier)
    pub fn authorize_url(&self) -> (String, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut auth_request = self
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        // Add scopes
        for scope in &self.config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        debug!("Generated authorization URL: {}", auth_url);

        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    /// Exchange authorization code for tokens
    ///
    /// Uses PKCE verifier to complete the flow.
    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<TokenResult> {
        info!("Exchanging authorization code for tokens");

        use oauth2::reqwest::async_http_client;

        let token_response = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .context("Token exchange failed")?;

        debug!("Token exchange successful");

        let access_token = token_response.access_token().secret().clone();
        let refresh_token = token_response
            .refresh_token()
            .map(|t| t.secret().clone());
        let expires_in = token_response.expires_in().map(|d| d.as_secs());

        // ID token extraction varies by OAuth2 crate version
        // For now, ID token is None (requires custom extra fields)
        let id_token = None;

        Ok(TokenResult {
            access_token,
            refresh_token,
            expires_in,
            id_token,
        })
    }

    /// Validate OIDC ID token
    ///
    /// TODO: Full OIDC ID token validation
    /// Requires proper openidconnect crate API integration
    pub async fn validate_id_token(&self, _id_token: &str) -> Result<OidcClaims> {
        // TODO: Implement ID token validation
        // This requires:
        // 1. Parsing ID token with CoreIdToken
        // 2. Verifying signature with JWKS
        // 3. Validating claims (iss, aud, exp, etc.)
        // 4. Extracting user claims

        Err(anyhow::anyhow!(
            "ID token validation not yet implemented - awaiting openidconnect API integration"
        ))
    }

    /// Get provider name
    pub fn provider(&self) -> &str {
        &self.config.provider
    }
}

/// Token exchange result
#[derive(Debug, Clone)]
pub struct TokenResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub id_token: Option<String>,
}

/// OIDC ID token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcClaims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> OAuth2Config {
        OAuth2Config {
            provider: "test".to_string(),
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            scopes: vec!["openid".to_string(), "profile".to_string()],
            issuer_url: None,
            auth_url: Some("https://provider.com/auth".to_string()),
            token_url: Some("https://provider.com/token".to_string()),
            validate_token: true,
            required_claims: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_oauth2_handler_creation() {
        let config = create_test_config();
        let result = OAuth2Handler::new(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_authorize_url_generation() {
        let config = create_test_config();
        let handler = OAuth2Handler::new(config).await.unwrap();

        let (auth_url, csrf_token, _pkce_verifier) = handler.authorize_url();

        assert!(auth_url.contains("https://provider.com/auth"));
        assert!(auth_url.contains("client_id=test-client-id"));
        assert!(auth_url.contains(csrf_token.secret()));
        assert!(auth_url.contains("code_challenge"));
    }

    #[test]
    fn test_oauth2_config_default() {
        let config = OAuth2Config::default();
        assert_eq!(config.provider, "custom");
        assert_eq!(config.validate_token, true);
        assert!(config.scopes.contains(&"openid".to_string()));
    }
}
