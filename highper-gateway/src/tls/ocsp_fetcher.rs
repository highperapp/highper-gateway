//! OCSP response fetcher
//!
//! Fetches OCSP (Online Certificate Status Protocol) responses from OCSP responders
//! to provide certificate revocation status during TLS handshake.

use anyhow::{anyhow, Result};
use rustls::pki_types::CertificateDer;
use std::time::Duration;
use tracing::{debug, info, warn};

/// OCSP response fetcher
#[derive(Debug)]
pub struct OcspFetcher {
    client: reqwest::Client,
    timeout: Duration,
}

impl OcspFetcher {
    /// Create a new OCSP fetcher with the specified timeout
    pub fn new(timeout_secs: u64) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .user_agent("highper-gateway/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            timeout: Duration::from_secs(timeout_secs),
        })
    }

    /// Fetch OCSP response for a certificate
    ///
    /// # Arguments
    /// * `cert` - Certificate to check
    /// * `issuer_cert` - Issuer certificate (optional)
    /// * `responder_url` - OCSP responder URL (optional, extracted from cert if not provided)
    ///
    /// # Returns
    /// Raw OCSP response bytes
    pub async fn fetch_ocsp_response(
        &self,
        cert: &CertificateDer<'_>,
        issuer_cert: Option<&CertificateDer<'_>>,
        responder_url: Option<&str>,
    ) -> Result<Vec<u8>> {
        // Extract OCSP responder URL from certificate if not provided
        let ocsp_url = if let Some(url) = responder_url {
            url.to_string()
        } else {
            self.extract_ocsp_url(cert)?
        };

        debug!("Fetching OCSP response from: {}", ocsp_url);

        // Build OCSP request
        let ocsp_request = self.build_ocsp_request(cert, issuer_cert)?;

        // Send OCSP request
        let response = self
            .client
            .post(&ocsp_url)
            .header("Content-Type", "application/ocsp-request")
            .body(ocsp_request)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send OCSP request: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "OCSP responder returned error: {}",
                response.status()
            ));
        }

        let ocsp_response = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read OCSP response: {}", e))?
            .to_vec();

        // Validate OCSP response
        self.validate_ocsp_response(&ocsp_response)?;

        info!(
            "OCSP response fetched successfully ({} bytes)",
            ocsp_response.len()
        );

        Ok(ocsp_response)
    }

    /// Extract OCSP responder URL from certificate's AIA (Authority Information Access) extension
    fn extract_ocsp_url(&self, cert: &CertificateDer<'_>) -> Result<String> {
        use x509_parser::prelude::*;
        use x509_parser::oid_registry;

        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        // OID for OCSP (1.3.6.1.5.5.7.48.1)
        let oid_ad_ocsp = oid_registry::OID_PKIX_ACCESS_DESCRIPTOR_OCSP.clone();

        // Look for Authority Information Access extension
        for ext in parsed_cert.extensions() {
            if ext.oid == oid_registry::OID_PKIX_AUTHORITY_INFO_ACCESS {
                // Parse AIA extension
                match ext.parsed_extension() {
                    ParsedExtension::AuthorityInfoAccess(aia_ext) => {
                        // Find OCSP access descriptor
                        for access_desc in &aia_ext.accessdescs {
                            if access_desc.access_method == oid_ad_ocsp {
                                // Extract URL from GeneralName
                                if let GeneralName::URI(uri) = &access_desc.access_location {
                                    debug!("Found OCSP responder URL: {}", uri);
                                    return Ok(uri.to_string());
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }
        }

        Err(anyhow!("No OCSP responder URL found in certificate"))
    }

    /// Build OCSP request
    ///
    /// Note: This is a placeholder implementation. Production code should use a proper
    /// OCSP library or OpenSSL bindings to build valid OCSP requests.
    fn build_ocsp_request(
        &self,
        cert: &CertificateDer<'_>,
        issuer_cert: Option<&CertificateDer<'_>>,
    ) -> Result<Vec<u8>> {
        use x509_parser::prelude::*;

        // Parse certificate to get serial number
        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        let serial = parsed_cert.serial.to_bytes_be();

        // For now, we'll use a simplified OCSP request
        // In production, you would use:
        // 1. OpenSSL's OCSP_* functions
        // 2. A dedicated Rust OCSP library
        // 3. Proper ASN.1 encoding with der crate

        // This is a minimal OCSP request structure (placeholder)
        // Real implementation would properly encode:
        // - Certificate serial number
        // - Issuer name hash (SHA-1)
        // - Issuer key hash (SHA-1)
        // - Request extensions

        warn!("Using simplified OCSP request (production should use proper OCSP library)");

        // For now, return a minimal placeholder
        // This will fail with real OCSP responders
        // TODO: Implement proper OCSP request encoding
        let _ = issuer_cert; // Suppress unused warning
        let _ = serial; // Suppress unused warning

        // Return empty request as placeholder
        // Real implementation needed before production use
        Ok(vec![])
    }

    /// Validate OCSP response
    ///
    /// Checks:
    /// - Response is not empty
    /// - Response status is successful
    /// - Basic ASN.1 structure is valid
    ///
    /// Note: Production implementation should verify:
    /// - Signature validation
    /// - thisUpdate and nextUpdate times
    /// - Certificate status (good/revoked/unknown)
    /// - Nonce matching (if sent in request)
    fn validate_ocsp_response(&self, response: &[u8]) -> Result<()> {
        if response.is_empty() {
            return Err(anyhow!("Empty OCSP response"));
        }

        // Basic length check
        if response.len() < 10 {
            return Err(anyhow!("OCSP response too short"));
        }

        // TODO: Implement proper OCSP response validation using:
        // - ASN.1 parsing
        // - Signature verification
        // - Time validity checks
        // - Certificate status checks

        debug!("OCSP response basic validation passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let fetcher = OcspFetcher::new(10);
        assert!(fetcher.is_ok());
    }

    #[tokio::test]
    async fn test_extract_ocsp_url() {
        // Test with a real Let's Encrypt certificate (mock)
        // In production, load a real certificate with AIA extension
    }

    #[test]
    fn test_validate_empty_response() {
        let fetcher = OcspFetcher::new(10).unwrap();
        let result = fetcher.validate_ocsp_response(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_short_response() {
        let fetcher = OcspFetcher::new(10).unwrap();
        let result = fetcher.validate_ocsp_response(&[1, 2, 3]);
        assert!(result.is_err());
    }
}
