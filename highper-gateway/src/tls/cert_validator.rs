//! Certificate validation for hot reload
//!
//! Validates certificates and keys before they are loaded into the TLS acceptor.

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

/// Certificate validator
pub struct CertificateValidator;

impl CertificateValidator {
    /// Validate certificate and key pair
    ///
    /// This performs the following checks:
    /// - Certificate file is readable and contains valid PEM data
    /// - Private key file is readable and contains valid PEM data
    /// - Certificate is not expired (with warning if < 30 days)
    ///
    /// # Arguments
    /// * `cert_path` - Path to certificate file
    /// * `key_path` - Path to private key file
    ///
    /// # Returns
    /// * `Result<(Vec<CertificateDer>, PrivateKeyDer)>` - Certificate chain and private key
    pub fn validate<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
    ) -> anyhow::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let cert_path = cert_path.as_ref();
        let key_path = key_path.as_ref();

        debug!("Validating certificate: {:?}", cert_path);
        debug!("Validating private key: {:?}", key_path);

        // Load certificate
        let cert_file = fs::File::open(cert_path).map_err(|e| {
            anyhow::anyhow!("Failed to open certificate file {:?}: {}", cert_path, e)
        })?;
        let mut reader = std::io::BufReader::new(cert_file);
        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        if certs.is_empty() {
            return Err(anyhow::anyhow!(
                "No certificates found in file {:?}",
                cert_path
            ));
        }

        info!("Loaded {} certificate(s) from {:?}", certs.len(), cert_path);

        // Load private key
        let key_file = fs::File::open(key_path).map_err(|e| {
            anyhow::anyhow!("Failed to open private key file {:?}: {}", key_path, e)
        })?;
        let mut reader = std::io::BufReader::new(key_file);
        let key = rustls_pemfile::private_key(&mut reader)
            .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No private key found in file {:?}", key_path))?;

        debug!("Loaded private key from {:?}", key_path);

        // Check certificate expiration
        Self::check_expiration(&certs[0])?;

        // Verify cert and key match (basic validation)
        // Note: Full validation would require cryptographic verification
        Self::verify_cert_key_match(&certs[0], &key)?;

        info!("Certificate validation successful");

        Ok((certs, key))
    }

    /// Verify that certificate and key match
    ///
    /// This is a placeholder for proper cryptographic verification.
    /// In production, you would:
    /// 1. Extract the public key from the certificate
    /// 2. Verify it matches the private key
    /// 3. Use ring or openssl for the actual verification
    fn verify_cert_key_match(
        _cert: &CertificateDer,
        _key: &PrivateKeyDer,
    ) -> anyhow::Result<()> {
        // TODO: Implement proper cert/key matching using ring or openssl
        // For now, we trust rustls to catch mismatches when building ServerConfig
        debug!("Certificate/key matching validation skipped (delegated to rustls)");
        Ok(())
    }

    /// Check certificate expiration
    ///
    /// Issues a warning if the certificate expires within 30 days.
    fn check_expiration(cert: &CertificateDer) -> anyhow::Result<()> {
        use x509_parser::prelude::*;

        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate for expiration check: {}", e))?;

        let validity = parsed_cert.validity();
        let not_before = validity.not_before;
        let not_after = validity.not_after;

        debug!("Certificate validity: {:?} to {:?}", not_before, not_after);

        // Check if certificate is already expired
        let now = x509_parser::time::ASN1Time::now();
        if now > not_after {
            return Err(anyhow::anyhow!(
                "Certificate has expired (valid until: {:?})",
                not_after
            ));
        }

        if now < not_before {
            return Err(anyhow::anyhow!(
                "Certificate is not yet valid (valid from: {:?})",
                not_before
            ));
        }

        // Warning if expires in < 30 days
        // Calculate days until expiration
        let now_unix = now.timestamp();
        let expiry_unix = not_after.timestamp();
        let seconds_until_expiry = expiry_unix - now_unix;
        let days_until_expiry = seconds_until_expiry / (24 * 60 * 60);

        if days_until_expiry < 30 {
            warn!(
                "Certificate expires soon: {} days remaining (expires: {:?})",
                days_until_expiry, not_after
            );
        } else {
            info!(
                "Certificate valid for {} days (expires: {:?})",
                days_until_expiry, not_after
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_nonexistent_certificate() {
        let result = CertificateValidator::validate(
            "/nonexistent/cert.pem",
            "/nonexistent/key.pem",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_file() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("empty_cert.pem");
        let key_path = temp_dir.join("empty_key.pem");

        // Create empty files
        std::fs::write(&cert_path, "").unwrap();
        std::fs::write(&key_path, "").unwrap();

        let result = CertificateValidator::validate(&cert_path, &key_path);
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_file(&cert_path);
        let _ = std::fs::remove_file(&key_path);
    }

    #[test]
    fn test_validate_invalid_pem() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("invalid_cert.pem");
        let key_path = temp_dir.join("invalid_key.pem");

        // Create files with invalid PEM data
        std::fs::write(&cert_path, "not a valid certificate").unwrap();
        std::fs::write(&key_path, "not a valid key").unwrap();

        let result = CertificateValidator::validate(&cert_path, &key_path);
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_file(&cert_path);
        let _ = std::fs::remove_file(&key_path);
    }
}
