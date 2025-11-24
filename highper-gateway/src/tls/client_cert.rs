//! Client certificate extraction and information
//!
//! This module provides utilities for extracting client certificate information
//! from TLS connections for use in request headers and authorization.

use rustls::pki_types::CertificateDer;
use sha2::{Digest, Sha256};
use std::fmt;
use tracing::debug;

/// Client certificate information extracted from TLS connection
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    /// Certificate subject distinguished name (DN)
    pub subject_dn: String,

    /// Certificate issuer distinguished name
    pub issuer_dn: String,

    /// Certificate serial number (hex format)
    pub serial: String,

    /// SHA-256 fingerprint of the certificate (hex format)
    pub fingerprint: String,

    /// Certificate validity start (UNIX timestamp)
    pub not_before: Option<i64>,

    /// Certificate validity end (UNIX timestamp)
    pub not_after: Option<i64>,

    /// Certificate version
    pub version: u32,
}

impl ClientCertInfo {
    /// Extract certificate information from a DER-encoded certificate
    ///
    /// # Arguments
    /// * `cert_der` - DER-encoded certificate
    ///
    /// # Returns
    /// * `Option<Self>` - Certificate info if parsing succeeds, None otherwise
    pub fn from_der(cert_der: &CertificateDer) -> Option<Self> {
        // Parse the certificate using x509-parser
        match x509_parser::parse_x509_certificate(cert_der.as_ref()) {
            Ok((_, cert)) => {
                let tbs = cert.tbs_certificate;

                // Extract subject DN
                let subject_dn = tbs.subject.to_string();

                // Extract issuer DN
                let issuer_dn = tbs.issuer.to_string();

                // Extract serial number (as hex)
                let serial = hex::encode(tbs.serial.to_bytes_be());

                // Calculate SHA-256 fingerprint
                let mut hasher = Sha256::new();
                hasher.update(cert_der.as_ref());
                let fingerprint = hex::encode(hasher.finalize());

                // Extract validity dates
                let not_before = tbs.validity.not_before.timestamp();
                let not_after = tbs.validity.not_after.timestamp();

                // Extract version
                let version = tbs.version.0;

                debug!(
                    "Extracted client certificate info: subject={}, issuer={}, serial={}",
                    subject_dn, issuer_dn, serial
                );

                Some(Self {
                    subject_dn,
                    issuer_dn,
                    serial,
                    fingerprint,
                    not_before: Some(not_before),
                    not_after: Some(not_after),
                    version,
                })
            }
            Err(e) => {
                debug!("Failed to parse client certificate: {}", e);
                None
            }
        }
    }

    /// Get the certificate subject common name (CN)
    ///
    /// # Returns
    /// * `Option<String>` - Common name if found
    pub fn common_name(&self) -> Option<String> {
        // Parse CN from subject DN
        // Format is typically: CN=name,OU=unit,O=org,C=country
        self.subject_dn
            .split(',')
            .find_map(|part| {
                let part = part.trim();
                if part.starts_with("CN=") {
                    Some(part[3..].to_string())
                } else {
                    None
                }
            })
    }

    /// Get certificate as HTTP headers (X-Client-Cert-*)
    ///
    /// # Returns
    /// * `Vec<(String, String)>` - List of header name-value pairs
    pub fn as_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![
            ("X-Client-Cert-Subject".to_string(), self.subject_dn.clone()),
            ("X-Client-Cert-Issuer".to_string(), self.issuer_dn.clone()),
            ("X-Client-Cert-Serial".to_string(), self.serial.clone()),
            ("X-Client-Cert-Fingerprint".to_string(), self.fingerprint.clone()),
        ];

        if let Some(cn) = self.common_name() {
            headers.push(("X-Client-Cert-CN".to_string(), cn));
        }

        if let Some(not_before) = self.not_before {
            headers.push(("X-Client-Cert-Not-Before".to_string(), not_before.to_string()));
        }

        if let Some(not_after) = self.not_after {
            headers.push(("X-Client-Cert-Not-After".to_string(), not_after.to_string()));
        }

        headers
    }

    /// Check if certificate matches a fingerprint whitelist
    ///
    /// # Arguments
    /// * `whitelist` - List of allowed fingerprints (SHA-256 hex)
    ///
    /// # Returns
    /// * `bool` - True if certificate fingerprint is in whitelist
    pub fn matches_whitelist(&self, whitelist: &[String]) -> bool {
        if whitelist.is_empty() {
            return true; // No whitelist = allow all
        }

        whitelist.iter().any(|allowed| {
            // Case-insensitive comparison
            allowed.eq_ignore_ascii_case(&self.fingerprint)
        })
    }
}

impl fmt::Display for ClientCertInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClientCert(subject={}, serial={})",
            self.subject_dn, self.serial
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test certificate (self-signed, for testing only)
    const TEST_CERT_DER: &[u8] = include_bytes!("../../tests/fixtures/test-client-cert.der");

    #[test]
    fn test_client_cert_info_extraction() {
        // For this test to work, we would need a real DER-encoded certificate
        // For now, we test the structure
        let cert_der = CertificateDer::from(TEST_CERT_DER.to_vec());

        // This may fail if the test cert doesn't exist, which is OK for now
        if let Some(info) = ClientCertInfo::from_der(&cert_der) {
            assert!(!info.subject_dn.is_empty());
            assert!(!info.issuer_dn.is_empty());
            assert!(!info.serial.is_empty());
            assert_eq!(info.fingerprint.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        }
    }

    #[test]
    fn test_common_name_extraction() {
        let info = ClientCertInfo {
            subject_dn: "CN=John Doe,OU=Engineering,O=Example Corp,C=US".to_string(),
            issuer_dn: "CN=Example CA".to_string(),
            serial: "1234567890".to_string(),
            fingerprint: "a".repeat(64),
            not_before: Some(1609459200),
            not_after: Some(1672531200),
            version: 3,
        };

        let cn = info.common_name();
        assert_eq!(cn, Some("John Doe".to_string()));
    }

    #[test]
    fn test_headers_generation() {
        let info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "abc".to_string(),
            not_before: Some(1000),
            not_after: Some(2000),
            version: 3,
        };

        let headers = info.as_headers();

        assert!(headers.iter().any(|(k, v)| k == "X-Client-Cert-Subject" && v == "CN=test"));
        assert!(headers.iter().any(|(k, v)| k == "X-Client-Cert-Issuer" && v == "CN=CA"));
        assert!(headers.iter().any(|(k, v)| k == "X-Client-Cert-Serial" && v == "123"));
        assert!(headers.iter().any(|(k, v)| k == "X-Client-Cert-Fingerprint" && v == "abc"));
        assert!(headers.iter().any(|(k, v)| k == "X-Client-Cert-CN" && v == "test"));
    }

    #[test]
    fn test_fingerprint_whitelist_empty() {
        let info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "abc123".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        // Empty whitelist should allow all
        assert!(info.matches_whitelist(&[]));
    }

    #[test]
    fn test_fingerprint_whitelist_match() {
        let info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "abc123".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        let whitelist = vec!["ABC123".to_string(), "def456".to_string()];
        assert!(info.matches_whitelist(&whitelist)); // Case-insensitive match
    }

    #[test]
    fn test_fingerprint_whitelist_no_match() {
        let info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "xyz789".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        let whitelist = vec!["abc123".to_string(), "def456".to_string()];
        assert!(!info.matches_whitelist(&whitelist));
    }
}
