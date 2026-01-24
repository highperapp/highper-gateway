//! ACME client for automatic certificate management

use crate::config::AcmeConfig;
use crate::tls::storage::{Certificate, CertificateStorage};
use crate::tls::ChallengeStore;
use crate::Result;
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier,
    NewAccount, NewOrder, OrderStatus,
};
use rcgen::{CertificateParams, KeyPair};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

/// ACME client for automatic certificate issuance
pub struct AcmeClient {
    config: AcmeConfig,
    storage: Arc<dyn CertificateStorage>,
    challenge_store: Option<ChallengeStore>,
    account: Option<Account>,
}

impl AcmeClient {
    /// Create a new ACME client
    pub fn new(config: AcmeConfig, storage: Arc<dyn CertificateStorage>) -> Self {
        Self::with_challenge_store(config, storage, None)
    }

    /// Create a new ACME client with challenge store
    pub fn with_challenge_store(
        config: AcmeConfig,
        storage: Arc<dyn CertificateStorage>,
        challenge_store: Option<ChallengeStore>,
    ) -> Self {
        Self {
            config,
            storage,
            challenge_store,
            account: None,
        }
    }

    /// Initialize ACME account
    pub async fn init_account(&mut self) -> Result<()> {
        info!("Initializing ACME account for {}", self.config.email);

        // Create new account
        let (account, _credentials) = Account::create(
            &NewAccount {
                contact: &[&format!("mailto:{}", self.config.email)],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            &self.config.directory_url,
            None,
        )
        .await?;

        info!("ACME account created successfully");
        self.account = Some(account);
        Ok(())
    }

    /// Request a certificate for a domain
    pub async fn request_certificate(&mut self, domain: &str) -> Result<Certificate> {
        info!("Requesting certificate for domain: {}", domain);

        // Ensure account is initialized
        if self.account.is_none() {
            self.init_account().await?;
        }

        let account = self.account.as_ref().unwrap();

        // Create new order
        let identifier = Identifier::Dns(domain.to_string());
        let mut order = account
            .new_order(&NewOrder {
                identifiers: &[identifier],
            })
            .await?;

        debug!("Created ACME order for {}", domain);

        // Get authorizations
        let authorizations = order.authorizations().await?;

        for authz in &authorizations {
            match authz.status {
                AuthorizationStatus::Pending => {
                    info!("Authorization pending for {}", domain);

                    // Find HTTP-01 challenge
                    let challenge = authz
                        .challenges
                        .iter()
                        .find(|c| c.r#type == ChallengeType::Http01)
                        .ok_or_else(|| anyhow::anyhow!("No HTTP-01 challenge found"))?;

                    let challenge_token = challenge.token.clone();
                    let key_authorization = order.key_authorization(challenge);

                    info!(
                        "HTTP-01 challenge: token={}, key_auth={}",
                        challenge_token,
                        key_authorization.as_str()
                    );

                    // Store challenge in the challenge store
                    if let Some(store) = &self.challenge_store {
                        store.store(challenge_token.clone(), key_authorization.as_str().to_string());
                        info!("Stored HTTP-01 challenge for token: {}", challenge_token);
                    } else {
                        warn!(
                            "No challenge store available. HTTP-01 challenge requires serving: /.well-known/acme-challenge/{} -> {}",
                            challenge_token,
                            key_authorization.as_str()
                        );
                    }

                    // Tell ACME server we're ready for validation
                    info!("Setting challenge ready for validation");
                    order.set_challenge_ready(&challenge.url).await?;

                    // Wait a bit for validation to complete
                    info!("Waiting for challenge validation...");
                    sleep(Duration::from_secs(5)).await;

                    // Clean up challenge from store
                    if let Some(store) = &self.challenge_store {
                        store.remove(&challenge_token);
                    }
                }
                AuthorizationStatus::Valid => {
                    info!("Authorization already valid for {}", domain);
                }
                other => {
                    warn!("Unexpected authorization status: {:?}", other);
                }
            }
        }

        // Generate certificate signing request (CSR)
        let params = CertificateParams::new(vec![domain.to_string()])?;

        let key_pair = KeyPair::generate()?;
        let csr = params.serialize_request(&key_pair)?;

        debug!("Generated CSR for {}", domain);

        // Finalize order
        order.finalize(csr.der()).await?;

        // Poll for certificate
        let mut tries = 0;
        let cert_chain = loop {
            tries += 1;
            if tries > 10 {
                return Err(anyhow::anyhow!("Certificate issuance timeout"));
            }

            sleep(Duration::from_secs(2)).await;

            let state = order.refresh().await?;
            match state.status {
                OrderStatus::Valid => {
                    info!("Order valid, downloading certificate");
                    break order.certificate().await?;
                }
                OrderStatus::Invalid => {
                    return Err(anyhow::anyhow!("Order became invalid"));
                }
                OrderStatus::Processing => {
                    debug!("Certificate still processing...");
                }
                other => {
                    warn!("Unexpected order status: {:?}", other);
                }
            }
        };

        let cert_pem = cert_chain.ok_or_else(|| anyhow::anyhow!("No certificate in response"))?.as_bytes().to_vec();
        let key_pem = key_pair.serialize_pem().as_bytes().to_vec();

        let certificate = Certificate {
            domain: domain.to_string(),
            cert_pem,
            key_pem,
        };

        // Store certificate
        self.storage.store(&certificate)?;

        info!("Certificate obtained and stored for {}", domain);
        Ok(certificate)
    }

    /// Check if certificate needs renewal (< 30 days remaining)
    pub fn needs_renewal(&self, _domain: &str) -> Result<bool> {
        // In production, check certificate expiry date
        // For now, always return false
        Ok(false)
    }

    /// Renew certificate for a domain
    pub async fn renew_certificate(&mut self, domain: &str) -> Result<Certificate> {
        info!("Renewing certificate for {}", domain);
        self.request_certificate(domain).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::tls::storage::FileStorage;
    use std::env::temp_dir;

    #[tokio::test]
    #[ignore] // Requires network access and ACME server
    async fn test_acme_client_init() {
        let temp_path = temp_dir().join("highper-gateway-acme-test");
        let storage = Arc::new(FileStorage::new(&temp_path).unwrap());

        let config = AcmeConfig {
            provider: "letsencrypt".to_string(),
            email: "test@example.com".to_string(),
            directory_url: "https://acme-staging-v02.api.letsencrypt.org/directory".to_string(),
            staging: true,
            domains: vec![],
            challenge_type: "http-01".to_string(),
            storage: StorageConfig::default(),
            renewal_days: 30,
            renew_check_interval: Duration::from_secs(3600),
        };

        let mut client = AcmeClient::new(config, storage);

        // This would require network access
        // let result = client.init_account().await;
        // assert!(result.is_ok());
    }
}
