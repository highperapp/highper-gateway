//! OCSP response cache with auto-refresh
//!
//! Caches OCSP responses and automatically refreshes them before expiration.

use crate::tls::ocsp_fetcher::OcspFetcher;
use anyhow::Result;
use rustls::pki_types::CertificateDer;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};

/// Cached OCSP response with expiration metadata
#[derive(Clone, Debug)]
struct CachedOcspResponse {
    /// The raw OCSP response bytes
    response: Vec<u8>,
    /// When this response was fetched
    fetched_at: SystemTime,
    /// When this response expires
    valid_until: SystemTime,
}

/// OCSP response cache with automatic refresh
///
/// Manages a single OCSP response for a certificate, automatically
/// refreshing it before expiration.
#[derive(Debug)]
pub struct OcspCache {
    /// Cached response
    cache: Arc<RwLock<Option<CachedOcspResponse>>>,
    /// OCSP fetcher
    fetcher: Arc<OcspFetcher>,
    /// Certificate to fetch OCSP response for
    cert: Vec<u8>,
    /// Issuer certificate (optional)
    issuer_cert: Option<Vec<u8>>,
    /// OCSP responder URL (optional)
    responder_url: Option<String>,
    /// How often to refresh
    refresh_interval: Duration,
}

impl OcspCache {
    /// Create a new OCSP cache
    ///
    /// # Arguments
    /// * `fetcher` - OCSP fetcher to use
    /// * `cert` - Certificate bytes (DER format)
    /// * `issuer_cert` - Issuer certificate bytes (optional)
    /// * `responder_url` - OCSP responder URL (optional, uses cert's AIA if not provided)
    /// * `refresh_interval` - How often to refresh the OCSP response
    pub fn new(
        fetcher: Arc<OcspFetcher>,
        cert: Vec<u8>,
        issuer_cert: Option<Vec<u8>>,
        responder_url: Option<String>,
        refresh_interval: Duration,
    ) -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            fetcher,
            cert,
            issuer_cert,
            responder_url,
            refresh_interval,
        }
    }

    /// Get current OCSP response
    ///
    /// Returns None if:
    /// - No response has been cached yet
    /// - The cached response has expired
    pub async fn get_response(&self) -> Option<Vec<u8>> {
        let cache = self.cache.read().await;

        if let Some(cached) = &*cache {
            // Check if still valid
            if SystemTime::now() < cached.valid_until {
                return Some(cached.response.clone());
            } else {
                warn!("Cached OCSP response expired");
            }
        }

        None
    }

    /// Start auto-refresh background task
    ///
    /// This spawns a Tokio task that:
    /// 1. Immediately fetches an OCSP response
    /// 2. Periodically refreshes the response
    ///
    /// The task runs indefinitely in the background.
    pub async fn start_auto_refresh(self: Arc<Self>) {
        // Fetch immediately on startup
        if let Err(e) = self.refresh().await {
            error!("Initial OCSP fetch failed: {}", e);
            warn!("OCSP stapling will not be available until first successful fetch");
        }

        // Schedule periodic refresh
        let refresh_interval = self.refresh_interval;
        let cache = self.clone();

        tokio::spawn(async move {
            let mut refresh_timer = interval(refresh_interval);

            loop {
                refresh_timer.tick().await;

                if let Err(e) = cache.refresh().await {
                    error!("OCSP refresh failed: {}", e);
                    // Continue trying - don't give up on transient errors
                }
            }
        });

        info!("OCSP auto-refresh task started");
    }

    /// Refresh OCSP response
    ///
    /// Fetches a new OCSP response and updates the cache.
    async fn refresh(&self) -> Result<()> {
        info!("Refreshing OCSP response...");

        let cert = CertificateDer::from(self.cert.clone());
        let issuer_cert = self
            .issuer_cert
            .as_ref()
            .map(|c| CertificateDer::from(c.clone()));

        let response = self
            .fetcher
            .fetch_ocsp_response(&cert, issuer_cert.as_ref(), self.responder_url.as_deref())
            .await?;

        // Calculate validity period
        let fetched_at = SystemTime::now();
        let valid_until = fetched_at + self.refresh_interval;

        info!(
            "OCSP response will be valid until: {:?}",
            valid_until
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );

        // Update cache
        let mut cache = self.cache.write().await;
        *cache = Some(CachedOcspResponse {
            response,
            fetched_at,
            valid_until,
        });

        info!("OCSP response refreshed successfully");

        Ok(())
    }

    /// Force a refresh now
    ///
    /// Useful for testing or manual refresh operations
    pub async fn force_refresh(&self) -> Result<()> {
        self.refresh().await
    }

    /// Get cache statistics
    pub async fn stats(&self) -> OcspCacheStats {
        let cache = self.cache.read().await;

        if let Some(cached) = &*cache {
            let now = SystemTime::now();
            let age = now
                .duration_since(cached.fetched_at)
                .unwrap_or(Duration::from_secs(0));
            let remaining = cached
                .valid_until
                .duration_since(now)
                .unwrap_or(Duration::from_secs(0));

            OcspCacheStats {
                cached: true,
                age_secs: age.as_secs(),
                remaining_secs: remaining.as_secs(),
                size_bytes: cached.response.len(),
            }
        } else {
            OcspCacheStats {
                cached: false,
                age_secs: 0,
                remaining_secs: 0,
                size_bytes: 0,
            }
        }
    }
}

/// OCSP cache statistics
#[derive(Debug, Clone)]
pub struct OcspCacheStats {
    /// Whether a response is cached
    pub cached: bool,
    /// Age of cached response in seconds
    pub age_secs: u64,
    /// Remaining validity in seconds
    pub remaining_secs: u64,
    /// Size of cached response in bytes
    pub size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_creation() {
        let fetcher = Arc::new(OcspFetcher::new(10).unwrap());
        let cache = OcspCache::new(
            fetcher,
            vec![1, 2, 3],
            None,
            None,
            Duration::from_secs(3600),
        );

        // Initially no response cached
        assert!(cache.get_response().await.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats_empty() {
        let fetcher = Arc::new(OcspFetcher::new(10).unwrap());
        let cache = OcspCache::new(
            fetcher,
            vec![1, 2, 3],
            None,
            None,
            Duration::from_secs(3600),
        );

        let stats = cache.stats().await;
        assert!(!stats.cached);
        assert_eq!(stats.size_bytes, 0);
    }
}
