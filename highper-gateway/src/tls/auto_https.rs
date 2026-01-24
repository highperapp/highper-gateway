//! Automatic HTTPS with ACME (Let's Encrypt) support
//!
//! Provides automatic certificate provisioning and renewal similar to Caddy.
//! Supports HTTP-01 challenge for domain validation.

use crate::config::AcmeConfig;
use crate::tls::acme::AcmeClient;
use crate::tls::challenge::ChallengeStore;
use crate::tls::storage::{Certificate, CertificateStorage, FileStorage};
use crate::Result;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};
use x509_parser::prelude::*;

/// Automatic HTTPS manager
///
/// Handles automatic certificate provisioning and renewal for configured domains.
pub struct AutoHttpsManager {
    /// ACME configuration
    config: AcmeConfig,
    /// Certificate storage
    storage: Arc<dyn CertificateStorage>,
    /// Challenge store for HTTP-01 validation
    challenge_store: ChallengeStore,
    /// Domains to manage
    domains: Arc<RwLock<HashSet<String>>>,
    /// Whether the manager is running
    running: Arc<RwLock<bool>>,
}

impl AutoHttpsManager {
    /// Create a new automatic HTTPS manager
    pub fn new(config: AcmeConfig) -> Result<Self> {
        info!(
            "Creating AutoHttpsManager: provider={}, email={}",
            config.provider, config.email
        );

        let storage: Arc<dyn CertificateStorage> = Arc::new(
            FileStorage::new(&config.storage.path)?
        );

        let challenge_store = ChallengeStore::new();

        Ok(Self {
            config,
            storage,
            challenge_store,
            domains: Arc::new(RwLock::new(HashSet::new())),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Get the challenge store for HTTP-01 handling
    pub fn challenge_store(&self) -> ChallengeStore {
        self.challenge_store.clone()
    }

    /// Add a domain to be managed
    pub async fn add_domain(&self, domain: String) {
        info!("Adding domain for automatic HTTPS: {}", domain);
        let mut domains = self.domains.write().await;
        domains.insert(domain);
    }

    /// Add multiple domains to be managed
    pub async fn add_domains(&self, domains: impl IntoIterator<Item = String>) {
        let mut managed = self.domains.write().await;
        for domain in domains {
            info!("Adding domain for automatic HTTPS: {}", domain);
            managed.insert(domain);
        }
    }

    /// Remove a domain from management
    pub async fn remove_domain(&self, domain: &str) {
        info!("Removing domain from automatic HTTPS: {}", domain);
        let mut domains = self.domains.write().await;
        domains.remove(domain);
    }

    /// Check if a domain has a valid certificate
    pub fn has_valid_certificate(&self, domain: &str) -> Result<bool> {
        match self.storage.get(domain)? {
            Some(cert) => {
                // Check if certificate is valid and not expiring soon
                let days_until_expiry = self.days_until_expiry(&cert)?;
                Ok(days_until_expiry > self.config.renewal_days as i64)
            }
            None => Ok(false),
        }
    }

    /// Get days until certificate expiry
    fn days_until_expiry(&self, cert: &Certificate) -> Result<i64> {
        // Parse the certificate to get expiry date
        let (_, pem) = x509_parser::pem::parse_x509_pem(&cert.cert_pem)
            .map_err(|e| anyhow::anyhow!("Failed to parse PEM: {:?}", e))?;

        let (_, x509) = X509Certificate::from_der(&pem.contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse X509: {:?}", e))?;

        let not_after = x509.validity().not_after.timestamp();
        let now = chrono::Utc::now().timestamp();

        let seconds_remaining = not_after - now;
        let days_remaining = seconds_remaining / 86400;

        debug!(
            "Certificate for {} expires in {} days",
            cert.domain, days_remaining
        );

        Ok(days_remaining)
    }

    /// Check if certificate needs renewal
    pub fn needs_renewal(&self, domain: &str) -> Result<bool> {
        match self.storage.get(domain)? {
            Some(cert) => {
                let days_until_expiry = self.days_until_expiry(&cert)?;
                let needs_renewal = days_until_expiry <= self.config.renewal_days as i64;

                if needs_renewal {
                    info!(
                        "Certificate for {} needs renewal ({} days until expiry)",
                        domain, days_until_expiry
                    );
                }

                Ok(needs_renewal)
            }
            None => Ok(true), // No certificate = needs one
        }
    }

    /// Provision a certificate for a domain
    pub async fn provision_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Provisioning certificate for domain: {}", domain);

        let mut client = AcmeClient::with_challenge_store(
            self.config.clone(),
            self.storage.clone(),
            Some(self.challenge_store.clone()),
        );

        let cert = client.request_certificate(domain).await?;

        info!("Certificate provisioned successfully for: {}", domain);
        crate::observability::metrics::record_acme_request(domain, true);

        Ok(cert)
    }

    /// Renew a certificate for a domain
    pub async fn renew_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Renewing certificate for domain: {}", domain);

        let mut client = AcmeClient::with_challenge_store(
            self.config.clone(),
            self.storage.clone(),
            Some(self.challenge_store.clone()),
        );

        let cert = client.renew_certificate(domain).await?;

        info!("Certificate renewed successfully for: {}", domain);
        crate::observability::metrics::record_acme_renewal(domain, true);

        Ok(cert)
    }

    /// Ensure all managed domains have valid certificates
    pub async fn ensure_certificates(&self) -> Result<()> {
        let domains = self.domains.read().await.clone();

        info!("Checking certificates for {} domain(s)", domains.len());

        for domain in domains {
            match self.ensure_certificate(&domain).await {
                Ok(()) => {
                    debug!("Certificate OK for: {}", domain);
                }
                Err(e) => {
                    error!("Failed to ensure certificate for {}: {}", domain, e);
                    crate::observability::metrics::record_acme_request(&domain, false);
                }
            }
        }

        Ok(())
    }

    /// Ensure a single domain has a valid certificate
    async fn ensure_certificate(&self, domain: &str) -> Result<()> {
        if self.has_valid_certificate(domain)? {
            debug!("Domain {} already has valid certificate", domain);
            return Ok(());
        }

        if self.needs_renewal(domain)? {
            // Has a certificate but needs renewal
            match self.storage.get(domain)? {
                Some(_) => {
                    self.renew_certificate(domain).await?;
                }
                None => {
                    self.provision_certificate(domain).await?;
                }
            }
        } else {
            // No certificate at all
            self.provision_certificate(domain).await?;
        }

        Ok(())
    }

    /// Start the automatic renewal background task
    pub async fn start_renewal_task(self: Arc<Self>) {
        let mut running = self.running.write().await;
        if *running {
            warn!("Renewal task already running");
            return;
        }
        *running = true;
        drop(running);

        info!(
            "Starting certificate renewal task (interval: {:?})",
            self.config.renew_check_interval
        );

        let manager = self.clone();
        tokio::spawn(async move {
            manager.renewal_loop().await;
        });
    }

    /// Stop the automatic renewal task
    pub async fn stop_renewal_task(&self) {
        info!("Stopping certificate renewal task");
        let mut running = self.running.write().await;
        *running = false;
    }

    /// The main renewal loop
    async fn renewal_loop(&self) {
        let mut ticker = interval(self.config.renew_check_interval);

        // Initial delay to allow server to start
        sleep(Duration::from_secs(10)).await;

        loop {
            ticker.tick().await;

            let running = *self.running.read().await;
            if !running {
                info!("Renewal task stopped");
                break;
            }

            info!("Running certificate renewal check");

            if let Err(e) = self.ensure_certificates().await {
                error!("Certificate renewal check failed: {}", e);
            }
        }
    }

    /// Get certificate for a domain (from storage)
    pub fn get_certificate(&self, domain: &str) -> Result<Option<Certificate>> {
        self.storage.get(domain)
    }

    /// Get list of managed domains
    pub async fn managed_domains(&self) -> Vec<String> {
        self.domains.read().await.iter().cloned().collect()
    }

    /// Get certificate status for all managed domains
    pub async fn certificate_status(&self) -> Vec<CertificateStatus> {
        let domains = self.domains.read().await.clone();
        let mut statuses = Vec::new();

        for domain in domains {
            let status = match self.storage.get(&domain) {
                Ok(Some(cert)) => {
                    let days_remaining = self.days_until_expiry(&cert).unwrap_or(-1);
                    let needs_renewal = days_remaining <= self.config.renewal_days as i64;

                    CertificateStatus {
                        domain,
                        has_certificate: true,
                        days_until_expiry: Some(days_remaining),
                        needs_renewal,
                        error: None,
                    }
                }
                Ok(None) => CertificateStatus {
                    domain,
                    has_certificate: false,
                    days_until_expiry: None,
                    needs_renewal: true,
                    error: None,
                },
                Err(e) => CertificateStatus {
                    domain,
                    has_certificate: false,
                    days_until_expiry: None,
                    needs_renewal: true,
                    error: Some(e.to_string()),
                },
            };

            statuses.push(status);
        }

        statuses
    }
}

/// Certificate status information
#[derive(Debug, Clone)]
pub struct CertificateStatus {
    /// Domain name
    pub domain: String,
    /// Whether a certificate exists
    pub has_certificate: bool,
    /// Days until certificate expires
    pub days_until_expiry: Option<i64>,
    /// Whether certificate needs renewal
    pub needs_renewal: bool,
    /// Error message if any
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use std::env::temp_dir;

    fn create_test_config() -> AcmeConfig {
        AcmeConfig {
            provider: "letsencrypt".to_string(),
            email: "test@example.com".to_string(),
            directory_url: "https://acme-staging-v02.api.letsencrypt.org/directory".to_string(),
            staging: true,
            domains: vec![],
            challenge_type: "http-01".to_string(),
            storage: StorageConfig {
                storage_type: "file".to_string(),
                path: temp_dir().join("highper-gateway-auto-https-test").to_string_lossy().to_string(),
            },
            renewal_days: 30,
            renew_check_interval: Duration::from_secs(3600),
        }
    }

    #[tokio::test]
    async fn test_add_domain() {
        let config = create_test_config();
        let manager = AutoHttpsManager::new(config).unwrap();

        manager.add_domain("example.com".to_string()).await;
        manager.add_domain("test.example.com".to_string()).await;

        let domains = manager.managed_domains().await;
        assert_eq!(domains.len(), 2);
        assert!(domains.contains(&"example.com".to_string()));
        assert!(domains.contains(&"test.example.com".to_string()));
    }

    #[tokio::test]
    async fn test_remove_domain() {
        let config = create_test_config();
        let manager = AutoHttpsManager::new(config).unwrap();

        manager.add_domain("example.com".to_string()).await;
        manager.add_domain("test.example.com".to_string()).await;
        manager.remove_domain("example.com").await;

        let domains = manager.managed_domains().await;
        assert_eq!(domains.len(), 1);
        assert!(domains.contains(&"test.example.com".to_string()));
    }

    #[tokio::test]
    async fn test_challenge_store() {
        let config = create_test_config();
        let manager = AutoHttpsManager::new(config).unwrap();

        let store = manager.challenge_store();

        store.store("test-token".to_string(), "test-key-auth".to_string());

        let result = store.get("test-token");
        assert_eq!(result, Some("test-key-auth".to_string()));
    }

    #[tokio::test]
    async fn test_has_valid_certificate_no_cert() {
        let config = create_test_config();
        let manager = AutoHttpsManager::new(config).unwrap();

        let result = manager.has_valid_certificate("nonexistent.example.com");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_certificate_status_no_cert() {
        let config = create_test_config();
        let manager = AutoHttpsManager::new(config).unwrap();

        manager.add_domain("example.com".to_string()).await;

        let statuses = manager.certificate_status().await;
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].domain, "example.com");
        assert!(!statuses[0].has_certificate);
        assert!(statuses[0].needs_renewal);
    }
}
