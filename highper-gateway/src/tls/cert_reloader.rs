//! Certificate hot reload orchestrator
//!
//! Watches certificate files and automatically reloads them when changed.

use crate::config::{CertificateConfig, TlsConfig};
use crate::tls::cert_watcher::CertificateWatcher;
use crate::tls::TlsManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Certificate hot reload manager
///
/// This manages file watchers for all configured certificates and
/// automatically reloads them when files change on disk.
pub struct CertificateReloader {
    tls_manager: Arc<RwLock<TlsManager>>,
    config: TlsConfig,
}

impl CertificateReloader {
    /// Create a new certificate reloader
    ///
    /// # Arguments
    /// * `tls_manager` - Shared TLS manager
    /// * `config` - TLS configuration
    pub fn new(tls_manager: Arc<RwLock<TlsManager>>, config: TlsConfig) -> Self {
        Self {
            tls_manager,
            config,
        }
    }

    /// Start watching all configured certificates
    ///
    /// This spawns a background task for each certificate domain.
    /// The tasks run until the program exits.
    pub async fn start_watching(self: Arc<Self>) -> anyhow::Result<()> {
        info!("Starting certificate hot reload for {} domain(s)", self.config.certificates.len());

        if self.config.certificates.is_empty() {
            info!("No certificates configured for hot reload");
            return Ok(());
        }

        // Group certificates by their file paths to avoid duplicate watchers
        let mut cert_groups: HashMap<(String, String), Vec<CertificateConfig>> = HashMap::new();

        for cert_config in &self.config.certificates {
            let key = (cert_config.cert_file.clone(), cert_config.key_file.clone());
            cert_groups.entry(key).or_default().push(cert_config.clone());
        }

        info!("Watching {} unique certificate file pair(s)", cert_groups.len());

        // Start a watcher for each unique cert/key pair
        for ((cert_path, key_path), domains) in cert_groups {
            let self_clone = Arc::clone(&self);
            let cert_path_clone = cert_path.clone();
            let key_path_clone = key_path.clone();

            tokio::spawn(async move {
                if let Err(e) = self_clone.watch_certificate_pair(
                    cert_path_clone,
                    key_path_clone,
                    domains,
                ).await {
                    error!("Certificate watcher failed: {}", e);
                }
            });
        }

        info!("Certificate hot reload watchers started");
        Ok(())
    }

    /// Watch a specific certificate/key pair
    async fn watch_certificate_pair(
        &self,
        cert_path: String,
        key_path: String,
        domains: Vec<CertificateConfig>,
    ) -> anyhow::Result<()> {
        info!(
            "Starting watcher for cert={}, key={}, domains={:?}",
            cert_path,
            key_path,
            domains.iter().map(|d| &d.domain).collect::<Vec<_>>()
        );

        // Create watcher
        let (mut watcher, mut event_rx) = CertificateWatcher::new(&cert_path, &key_path)?;
        watcher.watch()?;

        // Process events
        while let Some(event) = event_rx.recv().await {
            info!("Certificate file change detected: {:?}", event);

            // Small delay to ensure file write is complete
            // This handles atomic file operations (write to temp + rename)
            sleep(Duration::from_millis(500)).await;

            // Reload all domains using this cert/key pair
            for domain_config in &domains {
                let tls_manager = self.tls_manager.read().await;

                match tls_manager.reload_certificate(
                    &domain_config.domain,
                    &cert_path,
                    &key_path,
                ) {
                    Ok(()) => {
                        info!("Certificate reloaded successfully for domain: {}", domain_config.domain);
                    }
                    Err(e) => {
                        error!(
                            "Failed to reload certificate for domain {}: {}",
                            domain_config.domain, e
                        );
                        warn!("Old certificate will continue to be used for {}", domain_config.domain);
                    }
                }
            }
        }

        warn!("Certificate watcher stopped for cert={}, key={}", cert_path, key_path);
        Ok(())
    }

    /// Manually trigger a reload of all certificates
    ///
    /// This is useful for testing or manual operations.
    pub async fn reload_all(&self) -> anyhow::Result<()> {
        info!("Manually reloading all certificates");

        let tls_manager = self.tls_manager.read().await;
        tls_manager.reload_all_certificates()?;

        info!("Manual reload complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AcmeConfig, CertificateConfig, MtlsConfig, TlsConfig};

    fn create_test_tls_config() -> TlsConfig {
        TlsConfig {
            auto: false,
            certificates: vec![],
            acme: None,
            passthrough: None,
            min_version: "TLS1.2".to_string(),
            session_cache: crate::config::SessionCacheConfig::default(),
            mtls: None,
            ocsp_stapling: crate::config::OcspStaplingConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_certificate_reloader_creation() {
        let config = create_test_tls_config();
        let tls_manager = Arc::new(RwLock::new(TlsManager::new(config.clone()).unwrap()));
        let reloader = CertificateReloader::new(tls_manager, config);

        // Should not panic
        assert_eq!(reloader.config.certificates.len(), 0);
    }

    #[tokio::test]
    async fn test_start_watching_empty_certificates() {
        let config = create_test_tls_config();
        let tls_manager = Arc::new(RwLock::new(TlsManager::new(config.clone()).unwrap()));
        let reloader = Arc::new(CertificateReloader::new(tls_manager, config));

        // Should not fail with empty certificates
        let result = reloader.start_watching().await;
        assert!(result.is_ok());
    }
}
