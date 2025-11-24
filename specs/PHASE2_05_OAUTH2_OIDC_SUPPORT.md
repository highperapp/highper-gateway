# Phase 2.5: OAuth2/OIDC Support - Implementation Specification

**Duration:** 2 weeks
**Priority:** Medium
**Difficulty:** Medium
**Impact:** +2% API Gateway score

---

## Executive Summary

Implement OAuth2 and OpenID Connect (OIDC) authentication to enable standard identity federation and single sign-on (SSO) capabilities.

---

## Configuration

```yaml
routes:
  - path: "/api/protected"
    upstream: "backend"

    # OAuth2/OIDC authentication
    auth:
      type: "oauth2"
      provider: "google"  # google, github, okta, auth0, custom

      # OAuth2 settings
      client_id: "your-client-id"
      client_secret: "your-client-secret"
      redirect_uri: "https://example.com/callback"
      scopes:
        - "openid"
        - "profile"
        - "email"

      # OIDC discovery
      issuer_url: "https://accounts.google.com"
      # Or manual endpoints:
      # auth_url: "https://accounts.google.com/o/oauth2/v2/auth"
      # token_url: "https://oauth2.googleapis.com/token"
      # jwks_url: "https://www.googleapis.com/oauth2/v3/certs"

      # Token validation
      validate_token: true
      required_claims:
        email_verified: true

      # Session management
      cookie_name: "auth_session"
      cookie_secure: true
      session_ttl: 3600
```

---

## Implementation

```rust
// File: highper-gateway/src/gateway/auth/oauth2.rs

use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
    IssuerUrl, Nonce,
};

pub struct OAuth2Config {
    pub provider: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub issuer_url: Option<String>,
}

pub struct OAuth2Handler {
    client: BasicClient,
    oidc_client: Option<CoreClient>,
}

impl OAuth2Handler {
    pub async fn new(config: &OAuth2Config) -> anyhow::Result<Self> {
        // Create OAuth2 client
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = Some(ClientSecret::new(config.client_secret.clone()));
        let auth_url = AuthUrl::new(config.auth_url.clone())?;
        let token_url = Some(TokenUrl::new(config.token_url.clone())?);

        let client = BasicClient::new(
            client_id,
            client_secret,
            auth_url,
            token_url,
        )
        .set_redirect_uri(RedirectUrl::new(config.redirect_uri.clone())?);

        // Create OIDC client if issuer provided
        let oidc_client = if let Some(issuer_url) = &config.issuer_url {
            let issuer = IssuerUrl::new(issuer_url.clone())?;
            let metadata = CoreProviderMetadata::discover_async(issuer, async_http_client).await?;
            Some(CoreClient::from_provider_metadata(
                metadata,
                ClientId::new(config.client_id.clone()),
                Some(ClientSecret::new(config.client_secret.clone())),
            ))
        } else {
            None
        };

        Ok(Self { client, oidc_client })
    }

    /// Start authorization flow
    pub fn authorize_url(&self) -> (String, CsrfToken, Option<PkceCodeChallenge>) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_token) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .set_pkce_challenge(pkce_challenge.clone())
            .url();

        (auth_url.to_string(), csrf_token, Some(pkce_challenge))
    }

    /// Exchange code for token
    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: Option<PkceCodeVerifier>,
    ) -> anyhow::Result<TokenResponse> {
        let token_result = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier.unwrap())
            .request_async(async_http_client)
            .await?;

        Ok(TokenResponse {
            access_token: token_result.access_token().secret().clone(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            expires_in: token_result.expires_in().map(|d| d.as_secs()),
            id_token: token_result.extra_fields().id_token().map(|t| t.to_string()),
        })
    }

    /// Validate ID token (OIDC)
    pub async fn validate_id_token(&self, id_token: &str) -> anyhow::Result<Claims> {
        if let Some(oidc_client) = &self.oidc_client {
            let token = oidc_client.id_token_verifier().verify(id_token)?;

            Ok(Claims {
                sub: token.subject().to_string(),
                email: token.email().map(|e| e.to_string()),
                email_verified: token.email_verified(),
                name: token.name().map(|n| n.get(None).map(|l| l.to_string())).flatten(),
            })
        } else {
            Err(anyhow::anyhow!("OIDC not configured"))
        }
    }
}

#[derive(Debug)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub id_token: Option<String>,
}

#[derive(Debug)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
}
```

---

## OAuth2 Flow

```
1. User requests protected resource
   ↓
2. Proxy redirects to OAuth provider
   GET https://provider.com/authorize?client_id=...&redirect_uri=...
   ↓
3. User logs in at provider
   ↓
4. Provider redirects back with code
   GET https://example.com/callback?code=abc123&state=xyz
   ↓
5. Proxy exchanges code for token
   POST https://provider.com/token
   ↓
6. Proxy validates token (OIDC ID token)
   ↓
7. Proxy creates session cookie
   ↓
8. Proxy forwards request to backend with token
```

---

## Dependencies

```toml
[dependencies]
# OAuth2
oauth2 = "4.4"

# OpenID Connect
openidconnect = "3.5"

# JWT validation
jsonwebtoken = "9.2"
```

---

## Acceptance Criteria

- [ ] Authorization code flow works
- [ ] Token exchange works
- [ ] ID token validation works (OIDC)
- [ ] Token refresh works
- [ ] PKCE supported
- [ ] Multiple providers supported (Google, GitHub, etc.)
- [ ] Session management works

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
