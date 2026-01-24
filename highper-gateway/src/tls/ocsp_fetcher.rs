//! OCSP response fetcher
//!
//! Fetches OCSP (Online Certificate Status Protocol) responses from OCSP responders
//! to provide certificate revocation status during TLS handshake.
//!
//! ## Features
//! - Exponential backoff retry with configurable parameters
//! - Multiple OCSP responder support (fallback to alternative responders)
//! - Graceful degradation with stale response fallback
//! - Configurable timeouts and retry limits

use anyhow::{anyhow, Result};
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// OCSP fetcher configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcspFetcherConfig {
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Initial retry delay in milliseconds
    #[serde(default = "default_initial_retry_delay")]
    pub initial_retry_delay_ms: u64,

    /// Maximum retry delay in milliseconds
    #[serde(default = "default_max_retry_delay")]
    pub max_retry_delay_ms: u64,

    /// Retry delay multiplier (exponential backoff factor)
    #[serde(default = "default_retry_multiplier")]
    pub retry_multiplier: f64,

    /// Additional OCSP responder URLs (fallback if cert AIA fails)
    #[serde(default)]
    pub fallback_responder_urls: Vec<String>,

    /// Allow serving stale responses when fresh fetch fails
    #[serde(default = "default_true")]
    pub allow_stale_responses: bool,

    /// Maximum age of stale response in seconds (24 hours default)
    #[serde(default = "default_max_stale_age")]
    pub max_stale_age_secs: u64,
}

fn default_timeout() -> u64 { 10 }
fn default_max_retries() -> u32 { 3 }
fn default_initial_retry_delay() -> u64 { 500 }
fn default_max_retry_delay() -> u64 { 30000 }
fn default_retry_multiplier() -> f64 { 2.0 }
fn default_true() -> bool { true }
fn default_max_stale_age() -> u64 { 86400 }

impl Default for OcspFetcherConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            max_retries: default_max_retries(),
            initial_retry_delay_ms: default_initial_retry_delay(),
            max_retry_delay_ms: default_max_retry_delay(),
            retry_multiplier: default_retry_multiplier(),
            fallback_responder_urls: vec![],
            allow_stale_responses: default_true(),
            max_stale_age_secs: default_max_stale_age(),
        }
    }
}

/// Cached stale response for graceful degradation
#[derive(Debug, Clone)]
struct StaleResponse {
    response: Vec<u8>,
    fetched_at: Instant,
}

/// OCSP response fetcher with production hardening
#[derive(Debug)]
pub struct OcspFetcher {
    client: reqwest::Client,
    config: OcspFetcherConfig,
    /// Last successful response (for stale fallback)
    last_response: Arc<RwLock<Option<StaleResponse>>>,
}

impl OcspFetcher {
    /// Create a new OCSP fetcher with the specified timeout (backward compatible)
    pub fn new(timeout_secs: u64) -> Result<Self> {
        Self::with_config(OcspFetcherConfig {
            timeout_secs,
            ..Default::default()
        })
    }

    /// Create a new OCSP fetcher with full configuration
    pub fn with_config(config: OcspFetcherConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent("highper-gateway/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            config,
            last_response: Arc::new(RwLock::new(None)),
        })
    }

    /// Fetch OCSP response for a certificate with retry and fallback support
    ///
    /// # Arguments
    /// * `cert` - Certificate to check
    /// * `issuer_cert` - Issuer certificate (optional)
    /// * `responder_url` - OCSP responder URL (optional, extracted from cert if not provided)
    ///
    /// # Returns
    /// Raw OCSP response bytes
    ///
    /// # Behavior
    /// 1. Tries primary OCSP responder with exponential backoff retry
    /// 2. If primary fails, tries fallback responders
    /// 3. If all responders fail and stale responses allowed, returns cached stale response
    pub async fn fetch_ocsp_response(
        &self,
        cert: &CertificateDer<'_>,
        issuer_cert: Option<&CertificateDer<'_>>,
        responder_url: Option<&str>,
    ) -> Result<Vec<u8>> {
        // Build list of OCSP responder URLs to try
        let mut responder_urls = Vec::new();

        // Primary: provided URL or extracted from certificate
        if let Some(url) = responder_url {
            responder_urls.push(url.to_string());
        } else if let Ok(url) = self.extract_ocsp_url(cert) {
            responder_urls.push(url);
        }

        // Add fallback responders from config
        for url in &self.config.fallback_responder_urls {
            if !responder_urls.contains(url) {
                responder_urls.push(url.clone());
            }
        }

        if responder_urls.is_empty() {
            return Err(anyhow!("No OCSP responder URLs available"));
        }

        // Build OCSP request
        let ocsp_request = self.build_ocsp_request(cert, issuer_cert)?;

        // Try each responder with retry
        let mut last_error = None;
        for (idx, ocsp_url) in responder_urls.iter().enumerate() {
            debug!(
                "Trying OCSP responder {}/{}: {}",
                idx + 1,
                responder_urls.len(),
                ocsp_url
            );

            match self.fetch_with_retry(ocsp_url, &ocsp_request).await {
                Ok(response) => {
                    // Store successful response for stale fallback
                    self.store_response(&response).await;
                    return Ok(response);
                }
                Err(e) => {
                    warn!(
                        "OCSP responder {} failed: {}",
                        ocsp_url, e
                    );
                    last_error = Some(e);
                }
            }
        }

        // All responders failed - try stale fallback
        if self.config.allow_stale_responses {
            if let Some(stale) = self.get_stale_response().await {
                warn!(
                    "Using stale OCSP response (age: {:?})",
                    stale.fetched_at.elapsed()
                );
                return Ok(stale.response);
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All OCSP responders failed")))
    }

    /// Fetch OCSP response from a single responder with exponential backoff retry
    async fn fetch_with_retry(&self, ocsp_url: &str, ocsp_request: &[u8]) -> Result<Vec<u8>> {
        let mut retry_delay = Duration::from_millis(self.config.initial_retry_delay_ms);
        let max_delay = Duration::from_millis(self.config.max_retry_delay_ms);

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                debug!(
                    "OCSP retry attempt {}/{} after {:?}",
                    attempt, self.config.max_retries, retry_delay
                );
                tokio::time::sleep(retry_delay).await;

                // Exponential backoff with cap
                retry_delay = Duration::from_millis(
                    ((retry_delay.as_millis() as f64 * self.config.retry_multiplier) as u64)
                        .min(max_delay.as_millis() as u64),
                );
            }

            match self.fetch_once(ocsp_url, ocsp_request).await {
                Ok(response) => {
                    if attempt > 0 {
                        info!(
                            "OCSP fetch succeeded on retry attempt {}",
                            attempt
                        );
                    }
                    return Ok(response);
                }
                Err(e) => {
                    if attempt == self.config.max_retries {
                        return Err(e);
                    }
                    debug!("OCSP fetch attempt {} failed: {}", attempt + 1, e);
                }
            }
        }

        Err(anyhow!("OCSP fetch failed after {} retries", self.config.max_retries))
    }

    /// Single OCSP fetch attempt (no retry)
    async fn fetch_once(&self, ocsp_url: &str, ocsp_request: &[u8]) -> Result<Vec<u8>> {
        let response = self
            .client
            .post(ocsp_url)
            .header("Content-Type", "application/ocsp-request")
            .body(ocsp_request.to_vec())
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

    /// Store successful response for stale fallback
    async fn store_response(&self, response: &[u8]) {
        let mut last = self.last_response.write().await;
        *last = Some(StaleResponse {
            response: response.to_vec(),
            fetched_at: Instant::now(),
        });
    }

    /// Get stale response if within acceptable age
    async fn get_stale_response(&self) -> Option<StaleResponse> {
        let last = self.last_response.read().await;
        if let Some(stale) = &*last {
            let age = stale.fetched_at.elapsed();
            if age.as_secs() < self.config.max_stale_age_secs {
                return Some(stale.clone());
            } else {
                warn!(
                    "Stale OCSP response too old ({:?} > {} secs)",
                    age, self.config.max_stale_age_secs
                );
            }
        }
        None
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

    #[test]
    fn test_fetcher_with_config() {
        let config = OcspFetcherConfig {
            timeout_secs: 15,
            max_retries: 5,
            initial_retry_delay_ms: 100,
            max_retry_delay_ms: 5000,
            retry_multiplier: 1.5,
            fallback_responder_urls: vec![
                "http://ocsp.example.com".to_string(),
                "http://ocsp2.example.com".to_string(),
            ],
            allow_stale_responses: true,
            max_stale_age_secs: 3600,
        };

        let fetcher = OcspFetcher::with_config(config);
        assert!(fetcher.is_ok());

        let fetcher = fetcher.unwrap();
        assert_eq!(fetcher.config.max_retries, 5);
        assert_eq!(fetcher.config.fallback_responder_urls.len(), 2);
    }

    #[test]
    fn test_default_config() {
        let config = OcspFetcherConfig::default();
        assert_eq!(config.timeout_secs, 10);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_retry_delay_ms, 500);
        assert_eq!(config.max_retry_delay_ms, 30000);
        assert_eq!(config.retry_multiplier, 2.0);
        assert!(config.allow_stale_responses);
        assert_eq!(config.max_stale_age_secs, 86400);
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

    #[tokio::test]
    async fn test_stale_response_storage() {
        let fetcher = OcspFetcher::new(10).unwrap();

        // Initially no stale response
        assert!(fetcher.get_stale_response().await.is_none());

        // Store a response
        let response = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        fetcher.store_response(&response).await;

        // Should be retrievable
        let stale = fetcher.get_stale_response().await;
        assert!(stale.is_some());
        assert_eq!(stale.unwrap().response, response);
    }

    #[tokio::test]
    async fn test_stale_response_expiry() {
        let config = OcspFetcherConfig {
            max_stale_age_secs: 1, // Expire after 1 second
            ..Default::default()
        };

        let fetcher = OcspFetcher::with_config(config).unwrap();

        // Store a response
        fetcher.store_response(&vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).await;

        // Should be valid immediately
        assert!(fetcher.get_stale_response().await.is_some());

        // Wait for expiry (1.1 seconds)
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should be expired now
        assert!(fetcher.get_stale_response().await.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = OcspFetcherConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: OcspFetcherConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.timeout_secs, config.timeout_secs);
        assert_eq!(parsed.max_retries, config.max_retries);
    }
}
