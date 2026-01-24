// Copyright 2024-2026 Highper Gateway Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! OAuth2 Provider Presets
//!
//! Pre-configured settings for common OAuth2 providers.

use super::OAuth2Config;

/// Supported OAuth2 providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Google,
    GitHub,
    Microsoft,
    Okta { domain: String },
    Auth0 { domain: String },
    GitLab,
    Discord,
    Facebook,
    Twitter,
    Custom,
}

impl Provider {
    /// Get provider configuration preset
    ///
    /// Returns a partially configured OAuth2Config with provider-specific URLs and scopes.
    /// You must still provide: client_id, client_secret, redirect_uri
    pub fn get_config(&self) -> OAuth2Config {
        match self {
            Provider::Google => OAuth2Config {
                provider: "google".to_string(),
                auth_url: Some("https://accounts.google.com/o/oauth2/v2/auth".to_string()),
                token_url: Some("https://oauth2.googleapis.com/token".to_string()),
                revocation_url: Some("https://oauth2.googleapis.com/revoke".to_string()),
                issuer_url: Some("https://accounts.google.com".to_string()),
                scopes: vec![
                    "openid".to_string(),
                    "email".to_string(),
                    "profile".to_string(),
                ],
                ..Default::default()
            },

            Provider::GitHub => OAuth2Config {
                provider: "github".to_string(),
                auth_url: Some("https://github.com/login/oauth/authorize".to_string()),
                token_url: Some("https://github.com/login/oauth/access_token".to_string()),
                revocation_url: Some("https://api.github.com/applications/CLIENT_ID/token".to_string()),
                scopes: vec!["user".to_string(), "user:email".to_string()],
                ..Default::default()
            },

            Provider::Microsoft => OAuth2Config {
                provider: "microsoft".to_string(),
                auth_url: Some(
                    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
                ),
                token_url: Some(
                    "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
                ),
                revocation_url: Some(
                    "https://login.microsoftonline.com/common/oauth2/v2.0/logout".to_string(),
                ),
                issuer_url: Some("https://login.microsoftonline.com/common/v2.0".to_string()),
                scopes: vec![
                    "openid".to_string(),
                    "profile".to_string(),
                    "email".to_string(),
                ],
                ..Default::default()
            },

            Provider::Okta { domain } => OAuth2Config {
                provider: "okta".to_string(),
                auth_url: Some(format!("https://{}/oauth2/v1/authorize", domain)),
                token_url: Some(format!("https://{}/oauth2/v1/token", domain)),
                revocation_url: Some(format!("https://{}/oauth2/v1/revoke", domain)),
                issuer_url: Some(format!("https://{}", domain)),
                scopes: vec![
                    "openid".to_string(),
                    "profile".to_string(),
                    "email".to_string(),
                ],
                ..Default::default()
            },

            Provider::Auth0 { domain } => OAuth2Config {
                provider: "auth0".to_string(),
                auth_url: Some(format!("https://{}/authorize", domain)),
                token_url: Some(format!("https://{}/oauth/token", domain)),
                revocation_url: Some(format!("https://{}/oauth/revoke", domain)),
                issuer_url: Some(format!("https://{}", domain)),
                scopes: vec![
                    "openid".to_string(),
                    "profile".to_string(),
                    "email".to_string(),
                ],
                ..Default::default()
            },

            Provider::GitLab => OAuth2Config {
                provider: "gitlab".to_string(),
                auth_url: Some("https://gitlab.com/oauth/authorize".to_string()),
                token_url: Some("https://gitlab.com/oauth/token".to_string()),
                revocation_url: Some("https://gitlab.com/oauth/revoke".to_string()),
                scopes: vec!["read_user".to_string(), "email".to_string()],
                ..Default::default()
            },

            Provider::Discord => OAuth2Config {
                provider: "discord".to_string(),
                auth_url: Some("https://discord.com/api/oauth2/authorize".to_string()),
                token_url: Some("https://discord.com/api/oauth2/token".to_string()),
                revocation_url: Some("https://discord.com/api/oauth2/token/revoke".to_string()),
                scopes: vec!["identify".to_string(), "email".to_string()],
                ..Default::default()
            },

            Provider::Facebook => OAuth2Config {
                provider: "facebook".to_string(),
                auth_url: Some("https://www.facebook.com/v18.0/dialog/oauth".to_string()),
                token_url: Some("https://graph.facebook.com/v18.0/oauth/access_token".to_string()),
                scopes: vec!["public_profile".to_string(), "email".to_string()],
                ..Default::default()
            },

            Provider::Twitter => OAuth2Config {
                provider: "twitter".to_string(),
                auth_url: Some("https://twitter.com/i/oauth2/authorize".to_string()),
                token_url: Some("https://api.twitter.com/2/oauth2/token".to_string()),
                revocation_url: Some("https://api.twitter.com/2/oauth2/revoke".to_string()),
                scopes: vec!["tweet.read".to_string(), "users.read".to_string()],
                ..Default::default()
            },

            Provider::Custom => OAuth2Config::default(),
        }
    }

    /// Create provider from string name
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "google" => Provider::Google,
            "github" => Provider::GitHub,
            "microsoft" => Provider::Microsoft,
            "gitlab" => Provider::GitLab,
            "discord" => Provider::Discord,
            "facebook" => Provider::Facebook,
            "twitter" => Provider::Twitter,
            _ => Provider::Custom,
        }
    }

    /// Get provider name
    pub fn name(&self) -> &str {
        match self {
            Provider::Google => "google",
            Provider::GitHub => "github",
            Provider::Microsoft => "microsoft",
            Provider::Okta { .. } => "okta",
            Provider::Auth0 { .. } => "auth0",
            Provider::GitLab => "gitlab",
            Provider::Discord => "discord",
            Provider::Facebook => "facebook",
            Provider::Twitter => "twitter",
            Provider::Custom => "custom",
        }
    }
}

/// Helper to create OAuth2Config with provider preset
///
/// # Example
///
/// ```no_run
/// use highper_gateway::gateway::auth::oauth2_providers::*;
///
/// let config = oauth2_config_for_provider(
///     Provider::Google,
///     "your-client-id".to_string(),
///     "your-client-secret".to_string(),
///     "https://your-app.com/callback".to_string(),
/// );
/// ```
pub fn oauth2_config_for_provider(
    provider: Provider,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
) -> OAuth2Config {
    let mut config = provider.get_config();
    config.client_id = client_id;
    config.client_secret = client_secret;
    config.redirect_uri = redirect_uri;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_name() {
        assert_eq!(Provider::from_name("google"), Provider::Google);
        assert_eq!(Provider::from_name("GitHub"), Provider::GitHub);
        assert_eq!(Provider::from_name("MICROSOFT"), Provider::Microsoft);
        assert_eq!(Provider::from_name("unknown"), Provider::Custom);
    }

    #[test]
    fn test_provider_name() {
        assert_eq!(Provider::Google.name(), "google");
        assert_eq!(Provider::GitHub.name(), "github");
        assert_eq!(Provider::Microsoft.name(), "microsoft");
    }

    #[test]
    fn test_google_config() {
        let config = Provider::Google.get_config();
        assert_eq!(config.provider, "google");
        assert!(config.auth_url.is_some());
        assert!(config.token_url.is_some());
        assert!(config.revocation_url.is_some());
        assert!(config.scopes.contains(&"openid".to_string()));
        assert!(config.scopes.contains(&"email".to_string()));
    }

    #[test]
    fn test_github_config() {
        let config = Provider::GitHub.get_config();
        assert_eq!(config.provider, "github");
        assert!(config.auth_url.unwrap().contains("github.com"));
        assert!(config.scopes.contains(&"user".to_string()));
    }

    #[test]
    fn test_okta_config() {
        let config = Provider::Okta {
            domain: "dev-12345.okta.com".to_string(),
        }
        .get_config();
        assert_eq!(config.provider, "okta");
        assert!(config
            .auth_url
            .unwrap()
            .contains("dev-12345.okta.com"));
    }

    #[test]
    fn test_auth0_config() {
        let config = Provider::Auth0 {
            domain: "myapp.auth0.com".to_string(),
        }
        .get_config();
        assert_eq!(config.provider, "auth0");
        assert!(config.auth_url.unwrap().contains("myapp.auth0.com"));
    }

    #[test]
    fn test_oauth2_config_for_provider() {
        let config = oauth2_config_for_provider(
            Provider::Google,
            "test-client-id".to_string(),
            "test-client-secret".to_string(),
            "https://example.com/callback".to_string(),
        );

        assert_eq!(config.provider, "google");
        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(config.client_secret, "test-client-secret");
        assert_eq!(config.redirect_uri, "https://example.com/callback");
        assert!(config.auth_url.is_some());
    }

    #[test]
    fn test_all_providers_have_auth_url() {
        let providers = vec![
            Provider::Google,
            Provider::GitHub,
            Provider::Microsoft,
            Provider::GitLab,
            Provider::Discord,
            Provider::Facebook,
            Provider::Twitter,
        ];

        for provider in providers {
            let config = provider.get_config();
            assert!(
                config.auth_url.is_some(),
                "Provider {:?} missing auth_url",
                provider
            );
            assert!(
                config.token_url.is_some(),
                "Provider {:?} missing token_url",
                provider
            );
        }
    }
}
