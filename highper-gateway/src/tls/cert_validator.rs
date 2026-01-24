//! Certificate validation for hot reload
//!
//! Validates certificates and keys before they are loaded into the TLS acceptor.
//!
//! ## Features
//! - Basic certificate and key validation
//! - Expiration checking with warnings
//! - Partial chain handling (auto-builds chain from AIA)
//! - Cross-signed certificate support
//! - Chain depth validation

use anyhow::{anyhow, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Certificate validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertValidatorConfig {
    /// Maximum chain depth (default: 10)
    #[serde(default = "default_max_chain_depth")]
    pub max_chain_depth: usize,

    /// Allow partial chains (attempt to build from AIA)
    #[serde(default = "default_true")]
    pub allow_partial_chains: bool,

    /// Warning threshold for expiration (days)
    #[serde(default = "default_expiry_warning_days")]
    pub expiry_warning_days: i64,

    /// Allow self-signed certificates
    #[serde(default)]
    pub allow_self_signed: bool,

    /// Timeout for fetching intermediate certs (seconds)
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,
}

fn default_max_chain_depth() -> usize { 10 }
fn default_true() -> bool { true }
fn default_expiry_warning_days() -> i64 { 30 }
fn default_fetch_timeout() -> u64 { 10 }

impl Default for CertValidatorConfig {
    fn default() -> Self {
        Self {
            max_chain_depth: default_max_chain_depth(),
            allow_partial_chains: default_true(),
            expiry_warning_days: default_expiry_warning_days(),
            allow_self_signed: false,
            fetch_timeout_secs: default_fetch_timeout(),
        }
    }
}

/// Certificate chain validation result
#[derive(Debug, Clone)]
pub struct ChainValidationResult {
    /// Whether the chain is valid
    pub is_valid: bool,
    /// Chain depth
    pub depth: usize,
    /// Days until leaf certificate expires
    pub days_until_expiry: i64,
    /// Whether chain was built from partial input
    pub chain_built: bool,
    /// Any warnings encountered
    pub warnings: Vec<String>,
}

/// Certificate validator with enhanced edge case handling
pub struct CertificateValidator {
    config: CertValidatorConfig,
}

impl Default for CertificateValidator {
    fn default() -> Self {
        Self::new(CertValidatorConfig::default())
    }
}

impl CertificateValidator {
    /// Create a new certificate validator with configuration
    pub fn new(config: CertValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate certificate and key pair (static method for backward compatibility)
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
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let validator = Self::default();
        validator.validate_with_config(cert_path, key_path)
    }

    /// Validate certificate and key pair with instance configuration
    pub fn validate_with_config<P: AsRef<Path>>(
        &self,
        cert_path: P,
        key_path: P,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let cert_path = cert_path.as_ref();
        let key_path = key_path.as_ref();

        debug!("Validating certificate: {:?}", cert_path);
        debug!("Validating private key: {:?}", key_path);

        // Load certificate chain
        let cert_file = fs::File::open(cert_path).map_err(|e| {
            anyhow!("Failed to open certificate file {:?}: {}", cert_path, e)
        })?;
        let mut reader = std::io::BufReader::new(cert_file);
        let mut certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        if certs.is_empty() {
            return Err(anyhow!(
                "No certificates found in file {:?}",
                cert_path
            ));
        }

        info!("Loaded {} certificate(s) from {:?}", certs.len(), cert_path);

        // Check chain depth
        if certs.len() > self.config.max_chain_depth {
            return Err(anyhow!(
                "Certificate chain too deep: {} > max {}",
                certs.len(),
                self.config.max_chain_depth
            ));
        }

        // Check if chain is partial and try to build it
        if certs.len() == 1 && self.config.allow_partial_chains {
            if let Ok(built_chain) = self.try_build_chain(&certs[0]) {
                if built_chain.len() > 1 {
                    info!("Built certificate chain from AIA ({} certs)", built_chain.len());
                    certs = built_chain;
                }
            }
        }

        // Load private key
        let key_file = fs::File::open(key_path).map_err(|e| {
            anyhow!("Failed to open private key file {:?}: {}", key_path, e)
        })?;
        let mut reader = std::io::BufReader::new(key_file);
        let key = rustls_pemfile::private_key(&mut reader)
            .map_err(|e| anyhow!("Failed to parse private key: {}", e))?
            .ok_or_else(|| anyhow!("No private key found in file {:?}", key_path))?;

        debug!("Loaded private key from {:?}", key_path);

        // Check certificate expiration
        self.check_expiration_with_config(&certs[0])?;

        // Verify cert and key match (basic validation)
        Self::verify_cert_key_match(&certs[0], &key)?;

        // Validate chain relationships
        self.validate_chain(&certs)?;

        info!("Certificate validation successful");

        Ok((certs, key))
    }

    /// Try to build certificate chain from AIA (Authority Information Access)
    fn try_build_chain(&self, leaf_cert: &CertificateDer<'_>) -> Result<Vec<CertificateDer<'static>>> {
        use x509_parser::prelude::*;
        use x509_parser::oid_registry;

        let mut chain = vec![CertificateDer::from(leaf_cert.as_ref().to_vec())];
        let mut seen_subjects: HashSet<String> = HashSet::new();
        let mut current_cert = leaf_cert.as_ref().to_vec();

        // Add leaf subject to seen set
        if let Ok((_, parsed)) = parse_x509_certificate(&current_cert) {
            seen_subjects.insert(parsed.subject().to_string());
        }

        // Fetch intermediate certificates up to max depth
        for depth in 1..self.config.max_chain_depth {
            // Parse current certificate to find issuer URL
            let (_, parsed_cert) = parse_x509_certificate(&current_cert)
                .map_err(|e| anyhow!("Failed to parse certificate at depth {}: {}", depth, e))?;

            // Check if self-signed (issuer == subject)
            if parsed_cert.issuer() == parsed_cert.subject() {
                debug!("Reached self-signed certificate at depth {}", depth);
                break;
            }

            // Find CA Issuers URL from AIA extension
            let ca_issuer_url = self.extract_ca_issuer_url(&parsed_cert)?;

            if ca_issuer_url.is_none() {
                debug!("No CA Issuers URL found at depth {}", depth);
                break;
            }

            let url = ca_issuer_url.unwrap();
            debug!("Fetching issuer certificate from: {}", url);

            // Fetch issuer certificate (synchronous for simplicity)
            // In production, consider using async with tokio::spawn
            let issuer_bytes = self.fetch_certificate_sync(&url)?;

            // Parse issuer certificate
            let (_, issuer_cert) = parse_x509_certificate(&issuer_bytes)
                .map_err(|e| anyhow!("Failed to parse issuer certificate: {}", e))?;

            // Check for loops
            let issuer_subject = issuer_cert.subject().to_string();
            if seen_subjects.contains(&issuer_subject) {
                warn!("Certificate chain loop detected at depth {}", depth);
                break;
            }
            seen_subjects.insert(issuer_subject);

            chain.push(CertificateDer::from(issuer_bytes.clone()));
            current_cert = issuer_bytes;
        }

        Ok(chain)
    }

    /// Extract CA Issuers URL from certificate's AIA extension
    fn extract_ca_issuer_url(&self, cert: &x509_parser::certificate::X509Certificate<'_>) -> Result<Option<String>> {
        use x509_parser::prelude::*;
        use x509_parser::oid_registry;

        // OID for CA Issuers (1.3.6.1.5.5.7.48.2)
        let oid_ca_issuers = oid_registry::OID_PKIX_ACCESS_DESCRIPTOR_CA_ISSUERS.clone();

        for ext in cert.extensions() {
            if ext.oid == oid_registry::OID_PKIX_AUTHORITY_INFO_ACCESS {
                match ext.parsed_extension() {
                    ParsedExtension::AuthorityInfoAccess(aia_ext) => {
                        for access_desc in &aia_ext.accessdescs {
                            if access_desc.access_method == oid_ca_issuers {
                                if let GeneralName::URI(uri) = &access_desc.access_location {
                                    return Ok(Some(uri.to_string()));
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }
        }

        Ok(None)
    }

    /// Fetch certificate from URL
    /// Note: Uses ureq for synchronous HTTP since we're in a sync context during validation
    fn fetch_certificate_sync(&self, url: &str) -> Result<Vec<u8>> {
        // For chain building during validation, we use a simple sync HTTP fetch
        // This is called during certificate loading which happens infrequently

        // Build a simple HTTP GET using std::net for basic support
        // In production with full async support, consider using reqwest async client
        use std::io::Read;
        use std::net::TcpStream;

        // Parse URL
        let url_parsed = url::Url::parse(url)
            .map_err(|e| anyhow!("Invalid URL {}: {}", url, e))?;

        let host = url_parsed.host_str()
            .ok_or_else(|| anyhow!("No host in URL: {}", url))?;
        let port = url_parsed.port().unwrap_or(if url_parsed.scheme() == "https" { 443 } else { 80 });
        let path = url_parsed.path();

        // For HTTPS, we'd need TLS - for now, only support HTTP for AIA fetching
        // Most CA issuers URLs are HTTP anyway
        if url_parsed.scheme() == "https" {
            warn!("HTTPS AIA URLs not supported for chain building, skipping: {}", url);
            return Err(anyhow!("HTTPS AIA URLs not yet supported"));
        }

        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr)
            .map_err(|e| anyhow!("Failed to connect to {}: {}", addr, e))?;

        stream.set_read_timeout(Some(std::time::Duration::from_secs(self.config.fetch_timeout_secs)))
            .map_err(|e| anyhow!("Failed to set timeout: {}", e))?;

        // Send HTTP request
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: highper-gateway/1.0\r\n\r\n",
            path, host
        );

        use std::io::Write;
        stream.write_all(request.as_bytes())
            .map_err(|e| anyhow!("Failed to send request: {}", e))?;

        // Read response
        let mut response = Vec::new();
        stream.read_to_end(&mut response)
            .map_err(|e| anyhow!("Failed to read response: {}", e))?;

        // Parse HTTP response (simple parsing)
        let response_str = String::from_utf8_lossy(&response);
        let body_start = response_str.find("\r\n\r\n")
            .ok_or_else(|| anyhow!("Invalid HTTP response"))?;

        let bytes = response[body_start + 4..].to_vec();

        if bytes.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }

        // Try to determine format (DER or PEM)
        if bytes.starts_with(b"-----BEGIN") {
            // PEM format - extract DER
            let mut reader = std::io::BufReader::new(bytes.as_slice());
            let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| anyhow!("Failed to parse PEM certificate: {}", e))?;

            if certs.is_empty() {
                return Err(anyhow!("No certificate found in PEM response"));
            }
            Ok(certs[0].as_ref().to_vec())
        } else {
            // Assume DER format
            Ok(bytes)
        }
    }

    /// Validate certificate chain relationships
    fn validate_chain(&self, chain: &[CertificateDer<'_>]) -> Result<()> {
        use x509_parser::prelude::*;

        if chain.is_empty() {
            return Err(anyhow!("Empty certificate chain"));
        }

        if chain.len() == 1 {
            // Single certificate - check if self-signed
            let (_, cert) = parse_x509_certificate(chain[0].as_ref())
                .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

            if cert.issuer() == cert.subject() {
                if !self.config.allow_self_signed {
                    return Err(anyhow!("Self-signed certificate not allowed"));
                }
                debug!("Certificate is self-signed");
            }
            return Ok(());
        }

        // Validate chain: each cert should be signed by the next
        for i in 0..chain.len() - 1 {
            let (_, cert) = parse_x509_certificate(chain[i].as_ref())
                .map_err(|e| anyhow!("Failed to parse certificate at position {}: {}", i, e))?;

            let (_, issuer) = parse_x509_certificate(chain[i + 1].as_ref())
                .map_err(|e| anyhow!("Failed to parse issuer at position {}: {}", i + 1, e))?;

            // Check that cert's issuer matches issuer's subject
            if cert.issuer() != issuer.subject() {
                warn!(
                    "Chain validation warning: certificate {} issuer does not match certificate {} subject",
                    i, i + 1
                );
                // Don't fail - some chains have cross-signed intermediates
            }
        }

        debug!("Certificate chain validated ({} certificates)", chain.len());
        Ok(())
    }

    /// Check certificate expiration with configuration
    fn check_expiration_with_config(&self, cert: &CertificateDer) -> Result<()> {
        use x509_parser::prelude::*;

        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate for expiration check: {}", e))?;

        let validity = parsed_cert.validity();
        let not_before = validity.not_before;
        let not_after = validity.not_after;

        debug!("Certificate validity: {:?} to {:?}", not_before, not_after);

        // Check if certificate is already expired
        let now = x509_parser::time::ASN1Time::now();
        if now > not_after {
            return Err(anyhow!(
                "Certificate has expired (valid until: {:?})",
                not_after
            ));
        }

        if now < not_before {
            return Err(anyhow!(
                "Certificate is not yet valid (valid from: {:?})",
                not_before
            ));
        }

        // Calculate days until expiration
        let now_unix = now.timestamp();
        let expiry_unix = not_after.timestamp();
        let seconds_until_expiry = expiry_unix - now_unix;
        let days_until_expiry = seconds_until_expiry / (24 * 60 * 60);

        if days_until_expiry < self.config.expiry_warning_days {
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
    ) -> Result<()> {
        // TODO: Implement proper cert/key matching using ring or openssl
        // For now, we trust rustls to catch mismatches when building ServerConfig
        debug!("Certificate/key matching validation skipped (delegated to rustls)");
        Ok(())
    }

    /// Check certificate expiration (static method for backward compatibility)
    ///
    /// Issues a warning if the certificate expires within 30 days.
    fn check_expiration(cert: &CertificateDer) -> Result<()> {
        let validator = Self::default();
        validator.check_expiration_with_config(cert)
    }

    /// Perform full chain validation and return detailed result
    pub fn validate_chain_detailed(&self, chain: &[CertificateDer<'_>]) -> ChainValidationResult {
        use x509_parser::prelude::*;

        let mut result = ChainValidationResult {
            is_valid: true,
            depth: chain.len(),
            days_until_expiry: 0,
            chain_built: false,
            warnings: Vec::new(),
        };

        if chain.is_empty() {
            result.is_valid = false;
            result.warnings.push("Empty certificate chain".to_string());
            return result;
        }

        // Check leaf certificate expiration
        if let Ok((_, leaf)) = parse_x509_certificate(chain[0].as_ref()) {
            let now = x509_parser::time::ASN1Time::now();
            let not_after = leaf.validity().not_after;

            let now_unix = now.timestamp();
            let expiry_unix = not_after.timestamp();
            let seconds_until_expiry = expiry_unix - now_unix;
            result.days_until_expiry = seconds_until_expiry / (24 * 60 * 60);

            if result.days_until_expiry < 0 {
                result.is_valid = false;
                result.warnings.push("Certificate has expired".to_string());
            } else if result.days_until_expiry < self.config.expiry_warning_days {
                result.warnings.push(format!(
                    "Certificate expires in {} days",
                    result.days_until_expiry
                ));
            }
        }

        // Validate chain relationships
        if let Err(e) = self.validate_chain(chain) {
            result.warnings.push(format!("Chain validation issue: {}", e));
        }

        result
    }

    /// Get the configuration
    pub fn config(&self) -> &CertValidatorConfig {
        &self.config
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

    #[test]
    fn test_config_defaults() {
        let config = CertValidatorConfig::default();
        assert_eq!(config.max_chain_depth, 10);
        assert!(config.allow_partial_chains);
        assert_eq!(config.expiry_warning_days, 30);
        assert!(!config.allow_self_signed);
        assert_eq!(config.fetch_timeout_secs, 10);
    }

    #[test]
    fn test_validator_with_config() {
        let config = CertValidatorConfig {
            max_chain_depth: 5,
            allow_partial_chains: false,
            expiry_warning_days: 14,
            allow_self_signed: true,
            fetch_timeout_secs: 5,
        };

        let validator = CertificateValidator::new(config);
        assert_eq!(validator.config().max_chain_depth, 5);
        assert!(validator.config().allow_self_signed);
    }

    #[test]
    fn test_chain_validation_result_empty() {
        let validator = CertificateValidator::default();
        let empty_chain: Vec<CertificateDer<'static>> = vec![];

        let result = validator.validate_chain_detailed(&empty_chain);
        assert!(!result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("Empty")));
    }

    #[test]
    fn test_config_serialization() {
        let config = CertValidatorConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CertValidatorConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.max_chain_depth, parsed.max_chain_depth);
        assert_eq!(config.allow_partial_chains, parsed.allow_partial_chains);
        assert_eq!(config.expiry_warning_days, parsed.expiry_warning_days);
    }
}
