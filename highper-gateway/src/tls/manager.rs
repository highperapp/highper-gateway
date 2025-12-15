//! TLS certificate manager

use crate::config::TlsConfig;
use crate::tls::ca_manager::CaManager;
use crate::tls::cert_validator::CertificateValidator;
use crate::tls::client_verifier::MtlsClientVerifier;
use crate::tls::storage::{Certificate, CertificateStorage, FileStorage};
use crate::Result;
use rustls::ServerConfig;
use std::sync::Arc;
use tracing::{error, info, warn};

/// TLS certificate manager
pub struct TlsManager {
    config: TlsConfig,
    storage: Arc<dyn CertificateStorage>,
    server_config: Option<Arc<ServerConfig>>,
}

impl TlsManager {
    /// Create a new TLS manager
    pub fn new(config: TlsConfig) -> Result<Self> {
        info!("Initializing TLS manager");

        // Initialize storage
        let storage: Arc<dyn CertificateStorage> = match config.acme.as_ref() {
            Some(acme_config) => {
                info!("Using certificate storage: {}", acme_config.storage.storage_type);
                Arc::new(FileStorage::new(&acme_config.storage.path)?)
            }
            None => {
                // Default storage for manual certificates
                Arc::new(FileStorage::new("/tmp/highper-gateway-certs")?)
            }
        };

        Ok(Self {
            config,
            storage,
            server_config: None,
        })
    }

    /// Load manual certificates from configuration
    pub fn load_manual_certificates(&mut self) -> Result<()> {
        for cert_config in &self.config.certificates {
            info!("Loading manual certificate for domain: {}", cert_config.domain);

            let cert_pem = std::fs::read(&cert_config.cert_file)
                .map_err(|e| anyhow::anyhow!("Failed to read cert file {}: {}", cert_config.cert_file, e))?;

            let key_pem = std::fs::read(&cert_config.key_file)
                .map_err(|e| anyhow::anyhow!("Failed to read key file {}: {}", cert_config.key_file, e))?;

            let cert = Certificate {
                domain: cert_config.domain.clone(),
                cert_pem,
                key_pem,
            };

            self.storage.store(&cert)?;
        }

        Ok(())
    }

    /// Get certificate for a domain
    pub fn get_certificate(&self, domain: &str) -> Result<Option<Certificate>> {
        self.storage.get(domain)
    }

    /// Reload a certificate from disk
    ///
    /// Validates the new certificate before storing it.
    /// If validation fails, the old certificate remains in use.
    ///
    /// # Arguments
    /// * `domain` - Domain name for the certificate
    /// * `cert_path` - Path to certificate file
    /// * `key_path` - Path to private key file
    pub fn reload_certificate(
        &self,
        domain: &str,
        cert_path: &str,
        key_path: &str,
    ) -> Result<()> {
        info!("Reloading certificate for domain: {}", domain);

        // Validate new certificate first
        let (_certs, _key) = CertificateValidator::validate(cert_path, key_path)
            .map_err(|e| {
                error!("Certificate validation failed for {}: {}", domain, e);
                e
            })?;

        info!("Certificate validation successful for {}", domain);

        // Convert to PEM format for storage
        let cert_pem = std::fs::read(cert_path)?;
        let key_pem = std::fs::read(key_path)?;

        let cert = Certificate {
            domain: domain.to_string(),
            cert_pem,
            key_pem,
        };

        // Store the new certificate
        self.storage.store(&cert)?;

        info!("Certificate reloaded successfully for domain: {}", domain);
        Ok(())
    }

    /// Reload all manual certificates from configuration
    ///
    /// This will re-read all certificate files and update the storage.
    /// Invalid certificates are skipped with an error log.
    pub fn reload_all_certificates(&self) -> Result<()> {
        info!("Reloading all certificates");

        let mut errors = Vec::new();

        for cert_config in &self.config.certificates {
            match self.reload_certificate(
                &cert_config.domain,
                &cert_config.cert_file,
                &cert_config.key_file,
            ) {
                Ok(()) => {
                    info!("Reloaded certificate for {}", cert_config.domain);
                }
                Err(e) => {
                    error!("Failed to reload certificate for {}: {}", cert_config.domain, e);
                    errors.push((cert_config.domain.clone(), e));
                }
            }
        }

        if !errors.is_empty() {
            warn!(
                "Failed to reload {} certificate(s)",
                errors.len()
            );
        }

        Ok(())
    }

    /// Build Rustls ServerConfig
    pub fn build_server_config(&mut self) -> Result<Arc<ServerConfig>> {
        if let Some(config) = &self.server_config {
            return Ok(config.clone());
        }

        info!("Building TLS server configuration");

        // Check if mTLS is enabled
        let mut config = if let Some(mtls_config) = &self.config.mtls {
            if mtls_config.enabled {
                info!("mTLS enabled, configuring client certificate verification");

                // Load CA certificates
                let ca_manager = CaManager::new(
                    &mtls_config.ca_cert_path,
                    &mtls_config.additional_cas,
                )?;

                info!("Loaded {} CA certificate(s) for client verification", ca_manager.ca_count());

                // Build custom client verifier with our verification mode
                info!("Client certificate verification mode: {:?}", mtls_config.verification_mode);
                let client_verifier = MtlsClientVerifier::new(
                    ca_manager.root_store_arc(),
                    mtls_config.verification_mode,
                )?;

                // Create basic cert resolver
                let cert_resolver = Arc::new(DynamicCertResolver {
                    storage: self.storage.clone(),
                });

                // Wrap with OCSP stapler if enabled
                let cert_resolver: Arc<dyn rustls::server::ResolvesServerCert> = if self.config.ocsp_stapling.enabled {
                    info!("Enabling OCSP stapling");
                    let ocsp_config = crate::tls::ocsp_stapler::OcspStaplerConfig {
                        responder_url: self.config.ocsp_stapling.responder_url.clone(),
                        refresh_interval: self.config.ocsp_stapling.refresh_interval,
                        timeout: self.config.ocsp_stapling.timeout,
                    };
                    crate::tls::ocsp_stapler::Stapler::new(cert_resolver, ocsp_config)
                } else {
                    cert_resolver
                };

                // Create ServerConfig with client authentication
                ServerConfig::builder()
                    .with_client_cert_verifier(client_verifier)
                    .with_cert_resolver(cert_resolver)
            } else {
                // Create basic cert resolver
                let cert_resolver = Arc::new(DynamicCertResolver {
                    storage: self.storage.clone(),
                });

                // Wrap with OCSP stapler if enabled
                let cert_resolver: Arc<dyn rustls::server::ResolvesServerCert> = if self.config.ocsp_stapling.enabled {
                    info!("Enabling OCSP stapling");
                    let ocsp_config = crate::tls::ocsp_stapler::OcspStaplerConfig {
                        responder_url: self.config.ocsp_stapling.responder_url.clone(),
                        refresh_interval: self.config.ocsp_stapling.refresh_interval,
                        timeout: self.config.ocsp_stapling.timeout,
                    };
                    crate::tls::ocsp_stapler::Stapler::new(cert_resolver, ocsp_config)
                } else {
                    cert_resolver
                };

                // mTLS disabled
                ServerConfig::builder()
                    .with_no_client_auth()
                    .with_cert_resolver(cert_resolver)
            }
        } else {
            // Create basic cert resolver
            let cert_resolver = Arc::new(DynamicCertResolver {
                storage: self.storage.clone(),
            });

            // Wrap with OCSP stapler if enabled
            let cert_resolver: Arc<dyn rustls::server::ResolvesServerCert> = if self.config.ocsp_stapling.enabled {
                info!("Enabling OCSP stapling");
                Arc::new(ocsp_stapler::Stapler::new(cert_resolver))
            } else {
                cert_resolver
            };

            // No mTLS configuration
            ServerConfig::builder()
                .with_no_client_auth()
                .with_cert_resolver(cert_resolver)
        };

        // Configure ALPN protocols for HTTP/2 and HTTP/1.1
        config.alpn_protocols = vec![
            b"h2".to_vec(),       // HTTP/2
            b"http/1.1".to_vec(), // HTTP/1.1
        ];

        let config = Arc::new(config);
        self.server_config = Some(config.clone());

        Ok(config)
    }
}

/// Dynamic certificate resolver for SNI
#[derive(Clone)]
struct DynamicCertResolver {
    storage: Arc<dyn CertificateStorage>,
}

impl std::fmt::Debug for DynamicCertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicCertResolver").finish()
    }
}

impl rustls::server::ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<rustls::sign::CertifiedKey>> {
        // Get SNI hostname
        let sni_hostname = client_hello.server_name()?;

        info!("Resolving certificate for domain: {}", sni_hostname);

        // Look up certificate from storage
        match self.storage.get(sni_hostname) {
            Ok(Some(cert)) => {
                // Parse certificate chain
                let cert_chain: Vec<rustls::pki_types::CertificateDer> =
                    rustls_pemfile::certs(&mut cert.cert_pem.as_slice())
                        .filter_map(|c| c.ok())
                        .collect();

                if cert_chain.is_empty() {
                    warn!("No valid certificates found for domain: {}", sni_hostname);
                    return None;
                }

                // Parse private key
                let private_key = rustls_pemfile::private_key(&mut cert.key_pem.as_slice())
                    .ok()??;

                // Create signing key
                let signing_key = match rustls::crypto::ring::sign::any_supported_type(&private_key) {
                    Ok(key) => key,
                    Err(e) => {
                        warn!("Failed to create signing key for {}: {}", sni_hostname, e);
                        return None;
                    }
                };

                // Create CertifiedKey (OCSP stapling is handled by ocsp-stapler wrapper)
                let certified_key = rustls::sign::CertifiedKey::new(cert_chain, signing_key);
                info!("Successfully resolved certificate for domain: {}", sni_hostname);
                Some(Arc::new(certified_key))
            }
            Ok(None) => {
                warn!("No certificate found for domain: {}", sni_hostname);
                None
            }
            Err(e) => {
                warn!("Error looking up certificate for {}: {}", sni_hostname, e);
                None
            }
        }
    }
}
