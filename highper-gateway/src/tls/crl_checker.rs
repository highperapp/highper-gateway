//! CRL (Certificate Revocation List) checker
//!
//! Downloads, parses, and caches CRLs for certificate revocation checking.

use anyhow::{anyhow, Result};
use rustls::pki_types::CertificateDer;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// CRL checker for certificate revocation
#[derive(Debug)]
pub struct CrlChecker {
    /// HTTP client for downloading CRLs
    client: reqwest::Client,
    /// Cached CRL data
    cache: Arc<RwLock<Option<CachedCrl>>>,
    /// CRL distribution point URL
    crl_url: String,
    /// How often to refresh CRL
    refresh_interval: Duration,
    /// Timeout for CRL downloads
    timeout: Duration,
}

/// Cached CRL with revoked serial numbers
#[derive(Clone, Debug)]
struct CachedCrl {
    /// Set of revoked certificate serial numbers (hex-encoded)
    revoked_serials: HashSet<String>,
    /// When this CRL was fetched
    fetched_at: SystemTime,
    /// When this CRL expires
    valid_until: SystemTime,
    /// Next update time from CRL
    next_update: Option<SystemTime>,
}

impl CrlChecker {
    /// Create a new CRL checker
    ///
    /// # Arguments
    /// * `crl_url` - URL to download CRL from
    /// * `refresh_interval` - How often to refresh (seconds)
    /// * `timeout` - HTTP timeout (seconds)
    pub fn new(crl_url: String, refresh_interval: u64, timeout: u64) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout))
            .user_agent("highper-gateway/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            cache: Arc::new(RwLock::new(None)),
            crl_url,
            refresh_interval: Duration::from_secs(refresh_interval),
            timeout: Duration::from_secs(timeout),
        })
    }

    /// Start auto-refresh background task
    pub async fn start_auto_refresh(self: Arc<Self>) {
        // Fetch immediately on startup
        if let Err(e) = self.refresh_crl().await {
            error!("Initial CRL fetch failed: {}", e);
            warn!("Certificate revocation checking will not be available until first successful CRL fetch");
        }

        // Schedule periodic refresh
        let refresh_interval = self.refresh_interval;
        let checker = self.clone();

        tokio::spawn(async move {
            let mut refresh_timer = interval(refresh_interval);

            loop {
                refresh_timer.tick().await;

                if let Err(e) = checker.refresh_crl().await {
                    error!("CRL refresh failed: {}", e);
                    // Continue trying - don't give up on transient errors
                }
            }
        });

        info!("CRL auto-refresh task started");
    }

    /// Check if a certificate is revoked
    ///
    /// # Arguments
    /// * `cert` - Certificate to check
    ///
    /// # Returns
    /// * `Ok(true)` if certificate is revoked
    /// * `Ok(false)` if certificate is not revoked or CRL not available
    /// * `Err` if error checking
    pub async fn is_revoked(&self, cert: &CertificateDer<'_>) -> Result<bool> {
        use x509_parser::prelude::*;

        // Parse certificate to get serial number
        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        // Get serial number as hex string
        let serial_hex = hex::encode(parsed_cert.serial.to_bytes_be());

        debug!("Checking CRL for serial number: {}", serial_hex);

        // Check against cached CRL
        let cache = self.cache.read().await;

        if let Some(crl_data) = &*cache {
            // Check if CRL is still valid
            if SystemTime::now() < crl_data.valid_until {
                let is_revoked = crl_data.revoked_serials.contains(&serial_hex);

                if is_revoked {
                    warn!("Certificate is REVOKED (serial: {})", serial_hex);
                } else {
                    debug!("Certificate is valid (serial: {})", serial_hex);
                }

                return Ok(is_revoked);
            } else {
                warn!("Cached CRL has expired, cannot verify revocation status");
            }
        } else {
            warn!("No CRL cached, cannot verify revocation status");
        }

        // No CRL available - assume not revoked (soft-fail)
        // Production systems may want to hard-fail here
        Ok(false)
    }

    /// Refresh CRL from distribution point
    async fn refresh_crl(&self) -> Result<()> {
        info!("Fetching CRL from: {}", self.crl_url);

        // Download CRL
        let response = self
            .client
            .get(&self.crl_url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to download CRL: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("CRL server returned error: {}", response.status()));
        }

        let crl_bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read CRL response: {}", e))?
            .to_vec();

        info!("Downloaded CRL ({} bytes)", crl_bytes.len());

        // Parse CRL
        let parsed_crl = self.parse_crl(&crl_bytes)?;

        // Update cache
        let mut cache = self.cache.write().await;
        *cache = Some(parsed_crl);

        info!("CRL refreshed successfully");

        Ok(())
    }

    /// Parse CRL and extract revoked serials
    fn parse_crl(&self, crl_bytes: &[u8]) -> Result<CachedCrl> {
        use x509_parser::prelude::*;

        // Parse CRL (DER format)
        // TODO: Add PEM format support using rustls_pemfile if needed
        let (_, parsed_crl) = parse_x509_crl(crl_bytes)
            .map_err(|e| anyhow!("Failed to parse CRL (expected DER format): {}", e))?;

        // Extract revoked certificate serial numbers
        let mut revoked_serials = HashSet::new();

        for revoked_cert in parsed_crl.iter_revoked_certificates() {
            let serial_hex = hex::encode(revoked_cert.raw_serial());
            revoked_serials.insert(serial_hex);
        }

        info!(
            "Parsed CRL: {} revoked certificates",
            revoked_serials.len()
        );

        // Get validity period
        let fetched_at = SystemTime::now();

        // Calculate validity based on nextUpdate field
        let next_update = parsed_crl.next_update();
        let valid_until = if let Some(next_update_time) = next_update {
            // Convert ASN.1 time to SystemTime
            // For now, use refresh interval as fallback
            fetched_at + self.refresh_interval
        } else {
            // No nextUpdate in CRL, use refresh interval
            fetched_at + self.refresh_interval
        };

        Ok(CachedCrl {
            revoked_serials,
            fetched_at,
            valid_until,
            next_update: None, // TODO: Parse nextUpdate from CRL properly
        })
    }

    /// Get CRL statistics
    pub async fn stats(&self) -> CrlStats {
        let cache = self.cache.read().await;

        if let Some(crl_data) = &*cache {
            let now = SystemTime::now();
            let age = now
                .duration_since(crl_data.fetched_at)
                .unwrap_or(Duration::from_secs(0));
            let remaining = crl_data
                .valid_until
                .duration_since(now)
                .unwrap_or(Duration::from_secs(0));

            CrlStats {
                cached: true,
                revoked_count: crl_data.revoked_serials.len(),
                age_secs: age.as_secs(),
                remaining_secs: remaining.as_secs(),
            }
        } else {
            CrlStats {
                cached: false,
                revoked_count: 0,
                age_secs: 0,
                remaining_secs: 0,
            }
        }
    }
}

/// CRL statistics
#[derive(Debug, Clone)]
pub struct CrlStats {
    /// Whether CRL is cached
    pub cached: bool,
    /// Number of revoked certificates
    pub revoked_count: usize,
    /// Age of cached CRL in seconds
    pub age_secs: u64,
    /// Remaining validity in seconds
    pub remaining_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crl_checker_creation() {
        let checker = CrlChecker::new(
            "http://crl.example.com/ca.crl".to_string(),
            3600,
            10,
        );
        assert!(checker.is_ok());
    }

    #[tokio::test]
    async fn test_crl_stats_empty() {
        let checker = CrlChecker::new(
            "http://crl.example.com/ca.crl".to_string(),
            3600,
            10,
        )
        .unwrap();

        let stats = checker.stats().await;
        assert!(!stats.cached);
        assert_eq!(stats.revoked_count, 0);
    }
}
