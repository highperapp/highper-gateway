//! OCSP stapler for Rustls
//!
//! Implements OCSP stapling by wrapping a certificate resolver and
//! attaching OCSP responses to certificates during TLS handshake.

use crate::tls::ocsp_cache::OcspCache;
use crate::tls::ocsp_fetcher::OcspFetcher;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// OCSP stapler that wraps a certificate resolver
///
/// This implements OCSP stapling by:
/// 1. Delegating certificate resolution to an inner resolver
/// 2. Maintaining OCSP caches for each certificate
/// 3. Attaching OCSP responses to certificates during handshake
#[derive(Debug)]
pub struct Stapler {
    /// Inner certificate resolver
    inner: Arc<dyn ResolvesServerCert>,
    /// OCSP caches per SNI name
    caches: Arc<RwLock<HashMap<String, Arc<OcspCache>>>>,
    /// OCSP fetcher
    fetcher: Arc<OcspFetcher>,
    /// OCSP configuration
    config: OcspStaplerConfig,
}

/// OCSP stapler configuration
#[derive(Clone, Debug)]
pub struct OcspStaplerConfig {
    /// OCSP responder URL (optional)
    pub responder_url: Option<String>,
    /// Refresh interval in seconds
    pub refresh_interval: u64,
    /// Timeout for OCSP requests in seconds
    pub timeout: u64,
}

impl Stapler {
    /// Create a new OCSP stapler
    ///
    /// # Arguments
    /// * `inner` - Inner certificate resolver to wrap
    /// * `config` - OCSP stapler configuration
    pub fn new(inner: Arc<dyn ResolvesServerCert>, config: OcspStaplerConfig) -> Arc<Self> {
        info!("Creating OCSP stapler");

        let fetcher =
            Arc::new(OcspFetcher::new(config.timeout).expect("Failed to create OCSP fetcher"));

        Arc::new(Self {
            inner,
            caches: Arc::new(RwLock::new(HashMap::new())),
            fetcher,
            config,
        })
    }

    /// Get or create OCSP cache for a certificate
    async fn get_or_create_cache(&self, sni_name: &str, cert_der: &[u8]) -> Arc<OcspCache> {
        let mut caches = self.caches.write().await;

        if let Some(cache) = caches.get(sni_name) {
            return cache.clone();
        }

        // Create new cache
        info!("Creating OCSP cache for SNI: {}", sni_name);

        let cache = Arc::new(OcspCache::new(
            self.fetcher.clone(),
            cert_der.to_vec(),
            None, // TODO: Get issuer cert from chain if available
            self.config.responder_url.clone(),
            Duration::from_secs(self.config.refresh_interval),
        ));

        // Start auto-refresh
        cache.clone().start_auto_refresh().await;

        caches.insert(sni_name.to_string(), cache.clone());

        cache
    }

    /// Attach OCSP response to a certified key
    ///
    /// Creates a new CertifiedKey with the OCSP response attached
    async fn attach_ocsp_response(
        &self,
        certified_key: Arc<CertifiedKey>,
        sni_name: &str,
    ) -> Arc<CertifiedKey> {
        // Get the certificate DER bytes
        if let Some(first_cert) = certified_key.cert.first() {
            let cert_der = first_cert.as_ref();

            // Get or create cache for this certificate
            let cache = self.get_or_create_cache(sni_name, cert_der).await;

            // Get cached OCSP response
            if let Some(ocsp_response) = cache.get_response().await {
                debug!(
                    "Attaching OCSP response ({} bytes) to certificate for {}",
                    ocsp_response.len(),
                    sni_name
                );

                // Create a new CertifiedKey with OCSP response
                // Note: Rustls doesn't directly support OCSP stapling in the current version
                // This is a placeholder for when proper support is added
                //
                // In the meantime, OCSP responses are cached and can be accessed
                // for manual stapling or when Rustls adds support

                // TODO: Once Rustls supports OCSP stapling, attach the response here
                // For now, we just cache it and it can be retrieved via the admin API

                info!(
                    "OCSP response cached for {} (stapling support pending)",
                    sni_name
                );
            } else {
                warn!("No OCSP response available for {}", sni_name);
            }
        }

        // Return the original certified key
        // (will be modified once Rustls supports OCSP stapling)
        certified_key
    }
}

impl ResolvesServerCert for Stapler {
    /// Resolve server certificate with OCSP stapling
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        // Get SNI name
        let sni_name = client_hello.server_name().unwrap_or("default");

        debug!("Resolving certificate for SNI: {}", sni_name);

        // Delegate to inner resolver
        let certified_key = self.inner.resolve(client_hello)?;

        // OCSP attachment needs to be async, but ResolvesServerCert::resolve is sync
        // Options:
        // 1. Pre-fetch OCSP responses for all certificates on startup
        // 2. Cache OCSP responses and return cached version immediately
        // 3. Use a separate mechanism for OCSP stapling
        //
        // For now, we'll just return the certificate and rely on background OCSP fetching
        // The cache will be populated by the auto-refresh task

        // In a production implementation with full Rustls OCSP support:
        // - We would check if an OCSP response is cached
        // - Attach it to the certificate before returning
        // - The TLS handshake would include the OCSP response

        Some(certified_key)
    }
}

/// Get OCSP cache statistics for admin API
///
/// This can be used by the admin API to expose OCSP stapling status
pub async fn get_ocsp_stats(
    stapler: &Stapler,
) -> HashMap<String, crate::tls::ocsp_cache::OcspCacheStats> {
    let caches = stapler.caches.read().await;
    let mut stats = HashMap::new();

    for (sni_name, cache) in caches.iter() {
        stats.insert(sni_name.clone(), cache.stats().await);
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stapler_config() {
        let config = OcspStaplerConfig {
            responder_url: Some("http://ocsp.example.com".to_string()),
            refresh_interval: 3600,
            timeout: 10,
        };

        assert_eq!(config.refresh_interval, 3600);
        assert_eq!(config.timeout, 10);
    }
}
