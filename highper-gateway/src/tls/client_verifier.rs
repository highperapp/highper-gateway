//! Client certificate verifier for mTLS
//!
//! Implements rustls ClientCertVerifier trait to verify client certificates
//! during TLS handshake according to configured verification mode.

use crate::config::{CertVerificationMode, RouteMtlsPolicy};
use crate::tls::client_cert::ClientCertInfo;
use anyhow::Result;
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::server::danger::ClientCertVerifier;
use rustls::{DistinguishedName, RootCertStore, SignatureScheme};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Client certificate verifier for mTLS
#[derive(Debug)]
pub struct MtlsClientVerifier {
    /// Root certificate store for verifying client certificates
    root_store: Arc<RootCertStore>,

    /// Client certificate verification mode
    verification_mode: CertVerificationMode,

    /// Optional per-route mTLS policy
    route_policy: Option<RouteMtlsPolicy>,

    /// Underlying rustls client verifier
    inner_verifier: Arc<dyn ClientCertVerifier>,
}

impl MtlsClientVerifier {
    /// Create a new client certificate verifier
    ///
    /// # Arguments
    /// * `root_store` - Root certificate store containing trusted CAs
    /// * `verification_mode` - Verification mode (required, optional, optional_no_ca)
    ///
    /// # Returns
    /// * `Arc<Self>` - Verifier wrapped in Arc for use with rustls
    pub fn new(
        root_store: Arc<RootCertStore>,
        verification_mode: CertVerificationMode,
    ) -> Result<Arc<Self>> {
        // Create rustls WebPki client verifier
        let inner_verifier = rustls::server::WebPkiClientVerifier::builder(root_store.clone())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build WebPki client verifier: {:?}", e))?;

        Ok(Arc::new(Self {
            root_store,
            verification_mode,
            route_policy: None,
            inner_verifier,
        }))
    }

    /// Create a new verifier with per-route policy
    ///
    /// # Arguments
    /// * `root_store` - Root certificate store containing trusted CAs
    /// * `verification_mode` - Global verification mode
    /// * `route_policy` - Per-route mTLS policy (overrides global)
    ///
    /// # Returns
    /// * `Arc<Self>` - Verifier wrapped in Arc
    pub fn with_route_policy(
        root_store: Arc<RootCertStore>,
        verification_mode: CertVerificationMode,
        route_policy: RouteMtlsPolicy,
    ) -> Result<Arc<Self>> {
        let inner_verifier = rustls::server::WebPkiClientVerifier::builder(root_store.clone())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build WebPki client verifier: {:?}", e))?;

        Ok(Arc::new(Self {
            root_store,
            verification_mode,
            route_policy: Some(route_policy),
            inner_verifier,
        }))
    }

    /// Get the effective verification mode (considering route policy)
    fn effective_verification_mode(&self) -> CertVerificationMode {
        if let Some(policy) = &self.route_policy {
            policy.verification_mode.unwrap_or(self.verification_mode)
        } else {
            self.verification_mode
        }
    }

    /// Verify client certificate against route policy
    ///
    /// Checks allowed subjects, issuers, and serial numbers
    fn verify_route_policy(&self, cert: &CertificateDer) -> Result<(), rustls::Error> {
        let policy = match &self.route_policy {
            Some(p) => p,
            None => return Ok(()), // No policy to check
        };

        // Extract certificate information
        let cert_info = ClientCertInfo::from_der(cert).ok_or_else(|| {
            warn!("Failed to parse client certificate for policy check");
            rustls::Error::General("Failed to parse client certificate".to_string())
        })?;

        // Check allowed subjects
        if !policy.allowed_subjects.is_empty() {
            let subject_match = policy
                .allowed_subjects
                .iter()
                .any(|allowed| self.matches_dn(&cert_info.subject_dn, allowed));

            if !subject_match {
                warn!(
                    "Client certificate subject '{}' not in allowed list",
                    cert_info.subject_dn
                );
                return Err(rustls::Error::General(
                    "Certificate subject not allowed".to_string(),
                ));
            }
            debug!("Client certificate subject matches policy");
        }

        // Check allowed issuers
        if !policy.allowed_issuers.is_empty() {
            let issuer_match = policy
                .allowed_issuers
                .iter()
                .any(|allowed| self.matches_dn(&cert_info.issuer_dn, allowed));

            if !issuer_match {
                warn!(
                    "Client certificate issuer '{}' not in allowed list",
                    cert_info.issuer_dn
                );
                return Err(rustls::Error::General(
                    "Certificate issuer not allowed".to_string(),
                ));
            }
            debug!("Client certificate issuer matches policy");
        }

        // Check allowed serial numbers
        if !policy.allowed_serials.is_empty() {
            let serial_match = policy
                .allowed_serials
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&cert_info.serial));

            if !serial_match {
                warn!(
                    "Client certificate serial '{}' not in allowed list",
                    cert_info.serial
                );
                return Err(rustls::Error::General(
                    "Certificate serial number not allowed".to_string(),
                ));
            }
            debug!("Client certificate serial matches policy");
        }

        // Check allowed fingerprints
        if !policy.allowed_fingerprints.is_empty() {
            let fingerprint_match = policy
                .allowed_fingerprints
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&cert_info.fingerprint));

            if !fingerprint_match {
                warn!(
                    "Client certificate fingerprint '{}' not in allowed list",
                    cert_info.fingerprint
                );
                return Err(rustls::Error::General(
                    "Certificate fingerprint not allowed".to_string(),
                ));
            }
            debug!("Client certificate fingerprint matches policy");
        }

        Ok(())
    }

    /// Check if a DN matches a pattern (simple substring match)
    ///
    /// # Arguments
    /// * `dn` - Distinguished name to check
    /// * `pattern` - Pattern to match (supports * wildcard)
    ///
    /// # Returns
    /// * `bool` - True if DN matches pattern
    fn matches_dn(&self, dn: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // Simple wildcard matching
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return dn.starts_with(prefix) && dn.ends_with(suffix);
            }
        }

        // Exact match (case-insensitive)
        dn.eq_ignore_ascii_case(pattern)
    }
}

impl ClientCertVerifier for MtlsClientVerifier {
    /// Verify client certificate
    ///
    /// This is called by rustls during the TLS handshake when a client presents a certificate.
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        now: UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        let mode = self.effective_verification_mode();

        debug!(
            "Verifying client certificate (mode: {:?}, intermediates: {})",
            mode,
            intermediates.len()
        );

        // For OptionalNoCA mode, we accept any certificate without verification
        if mode == CertVerificationMode::OptionalNoCA {
            info!("Client certificate accepted without CA verification (OptionalNoCA mode)");
            // Check route policy even in OptionalNoCA mode
            self.verify_route_policy(end_entity)?;
            return Ok(rustls::server::danger::ClientCertVerified::assertion());
        }

        // Use rustls's built-in WebPki verifier for chain verification
        self.inner_verifier
            .verify_client_cert(end_entity, intermediates, now)?;

        debug!("Client certificate chain verification successful");

        // Check route policy if configured
        self.verify_route_policy(end_entity)?;

        info!("Client certificate verified successfully");
        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    /// Verify TLS 1.2 signature
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // Delegate to inner verifier
        self.inner_verifier
            .verify_tls12_signature(message, cert, dss)
    }

    /// Verify TLS 1.3 signature
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // Delegate to inner verifier
        self.inner_verifier
            .verify_tls13_signature(message, cert, dss)
    }

    /// Get supported signature verification algorithms
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner_verifier.supported_verify_schemes()
    }

    /// Hint for client about what CAs we trust
    ///
    /// Returns empty by default for privacy (don't leak CA information)
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // Don't send CA list to client for privacy
        // Client should know which certificate to present
        &[]
    }

    /// Whether to offer client certificate authentication
    fn offer_client_auth(&self) -> bool {
        // Always offer client auth if verifier is configured
        true
    }

    /// Whether client certificate is required
    fn client_auth_mandatory(&self) -> bool {
        let mode = self.effective_verification_mode();
        mode == CertVerificationMode::Required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_root_store() -> Arc<RootCertStore> {
        // Install default crypto provider for tests
        let _ = rustls::crypto::ring::default_provider().install_default();
        Arc::new(RootCertStore::empty())
    }

    #[test]
    fn test_verifier_creation() {
        let root_store = create_test_root_store();

        // Creating verifier with empty root store should fail
        let result = MtlsClientVerifier::new(root_store.clone(), CertVerificationMode::Required);
        assert!(result.is_err());
    }

    #[test]
    fn test_verifier_optional_mode() {
        let root_store = create_test_root_store();

        // Creating verifier with empty root store should fail
        let result = MtlsClientVerifier::new(root_store.clone(), CertVerificationMode::Optional);
        assert!(result.is_err());
    }

    #[test]
    fn test_dn_matching_exact() {
        let root_store = create_test_root_store();
        // Use a direct construction to test DN matching logic without requiring root anchors
        let inner_verifier = rustls::server::WebPkiClientVerifier::no_client_auth();
        let verifier = MtlsClientVerifier {
            root_store,
            verification_mode: CertVerificationMode::Required,
            route_policy: None,
            inner_verifier,
        };

        assert!(verifier.matches_dn("CN=test.example.com", "CN=test.example.com"));
        assert!(verifier.matches_dn("CN=Test.Example.Com", "cn=test.example.com")); // Case-insensitive
    }

    #[test]
    fn test_dn_matching_wildcard() {
        let root_store = create_test_root_store();
        let inner_verifier = rustls::server::WebPkiClientVerifier::no_client_auth();
        let verifier = MtlsClientVerifier {
            root_store,
            verification_mode: CertVerificationMode::Required,
            route_policy: None,
            inner_verifier,
        };

        assert!(verifier.matches_dn("CN=test.example.com,O=Example", "CN=test*"));
        assert!(verifier.matches_dn("CN=test.example.com,O=Example", "*O=Example"));
        assert!(verifier.matches_dn("CN=test.example.com,O=Example", "*"));
    }

    #[test]
    fn test_dn_matching_no_match() {
        let root_store = create_test_root_store();
        let inner_verifier = rustls::server::WebPkiClientVerifier::no_client_auth();
        let verifier = MtlsClientVerifier {
            root_store,
            verification_mode: CertVerificationMode::Required,
            route_policy: None,
            inner_verifier,
        };

        assert!(!verifier.matches_dn("CN=test.example.com", "CN=other.example.com"));
        assert!(!verifier.matches_dn("CN=test.example.com", "CN=test.other*"));
    }

    #[test]
    fn test_effective_verification_mode_without_policy() {
        let root_store = create_test_root_store();
        let inner_verifier = rustls::server::WebPkiClientVerifier::no_client_auth();
        let verifier = MtlsClientVerifier {
            root_store,
            verification_mode: CertVerificationMode::Required,
            route_policy: None,
            inner_verifier,
        };

        assert_eq!(
            verifier.effective_verification_mode(),
            CertVerificationMode::Required
        );
    }

    #[test]
    fn test_effective_verification_mode_with_policy() {
        let root_store = create_test_root_store();
        let inner_verifier = rustls::server::WebPkiClientVerifier::no_client_auth();
        let policy = RouteMtlsPolicy {
            verification_mode: Some(CertVerificationMode::Optional),
            allowed_subjects: vec![],
            allowed_issuers: vec![],
            allowed_serials: vec![],
            allowed_fingerprints: vec![],
        };

        let verifier = MtlsClientVerifier {
            root_store,
            verification_mode: CertVerificationMode::Required,
            route_policy: Some(policy),
            inner_verifier,
        };

        assert_eq!(
            verifier.effective_verification_mode(),
            CertVerificationMode::Optional
        );
    }
}
