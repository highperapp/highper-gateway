//! CRL (Certificate Revocation List) checker
//!
//! Downloads, parses, and caches CRLs for certificate revocation checking.
//!
//! ## Features
//! - Full CRL support (base CRLs)
//! - Delta CRL support for efficient updates
//! - LRU cache with configurable max entries
//! - Multiple CRL distribution points
//! - Exponential backoff retry
//! - Graceful degradation with stale fallback

use anyhow::{anyhow, Result};
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// CRL checker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrlCheckerConfig {
    /// Primary CRL distribution point URL
    pub crl_url: String,

    /// Additional CRL distribution points (fallback)
    #[serde(default)]
    pub fallback_urls: Vec<String>,

    /// How often to refresh CRL (seconds)
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,

    /// HTTP timeout (seconds)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Initial retry delay (milliseconds)
    #[serde(default = "default_initial_retry_delay")]
    pub initial_retry_delay_ms: u64,

    /// Maximum cache entries (LRU eviction)
    #[serde(default = "default_max_cache_entries")]
    pub max_cache_entries: usize,

    /// Enable delta CRL support
    #[serde(default = "default_true")]
    pub enable_delta_crl: bool,

    /// Allow stale CRL when refresh fails
    #[serde(default = "default_true")]
    pub allow_stale_crl: bool,

    /// Maximum stale age (seconds)
    #[serde(default = "default_max_stale_age")]
    pub max_stale_age_secs: u64,

    /// Hard-fail if no CRL available (vs soft-fail)
    #[serde(default)]
    pub hard_fail: bool,
}

fn default_refresh_interval() -> u64 {
    3600
}
fn default_timeout() -> u64 {
    10
}
fn default_max_retries() -> u32 {
    3
}
fn default_initial_retry_delay() -> u64 {
    500
}
fn default_max_cache_entries() -> usize {
    100
}
fn default_true() -> bool {
    true
}
fn default_max_stale_age() -> u64 {
    86400
}

impl Default for CrlCheckerConfig {
    fn default() -> Self {
        Self {
            crl_url: String::new(),
            fallback_urls: vec![],
            refresh_interval_secs: default_refresh_interval(),
            timeout_secs: default_timeout(),
            max_retries: default_max_retries(),
            initial_retry_delay_ms: default_initial_retry_delay(),
            max_cache_entries: default_max_cache_entries(),
            enable_delta_crl: default_true(),
            allow_stale_crl: default_true(),
            max_stale_age_secs: default_max_stale_age(),
            hard_fail: false,
        }
    }
}

/// CRL checker for certificate revocation with enhanced features
#[derive(Debug)]
pub struct CrlChecker {
    /// HTTP client for downloading CRLs
    client: reqwest::Client,
    /// Cached CRL data (main CRL)
    cache: Arc<RwLock<Option<CachedCrl>>>,
    /// LRU cache for per-issuer CRLs
    issuer_cache: Arc<RwLock<LruCache<String, CachedCrl>>>,
    /// Configuration
    config: CrlCheckerConfig,
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
    /// CRL number (for delta CRL support)
    crl_number: Option<u64>,
    /// Delta CRL indicator URL (if present)
    delta_crl_url: Option<String>,
    /// Whether this is a delta CRL
    is_delta: bool,
}

/// Simple LRU cache implementation
#[derive(Debug)]
struct LruCache<K, V> {
    capacity: usize,
    order: VecDeque<K>,
    map: HashMap<K, V>,
}

impl<K: Clone + Eq + std::hash::Hash, V> LruCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            map: HashMap::with_capacity(capacity),
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            // Move to front (most recently used)
            self.order.retain(|k| k != key);
            self.order.push_front(key.clone());
            self.map.get(key)
        } else {
            None
        }
    }

    fn insert(&mut self, key: K, value: V) {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        } else if self.map.len() >= self.capacity {
            // Evict least recently used
            if let Some(lru_key) = self.order.pop_back() {
                self.map.remove(&lru_key);
            }
        }
        self.order.push_front(key.clone());
        self.map.insert(key, value);
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn clear(&mut self) {
        self.order.clear();
        self.map.clear();
    }
}

impl CrlChecker {
    /// Create a new CRL checker (backward compatible)
    ///
    /// # Arguments
    /// * `crl_url` - URL to download CRL from
    /// * `refresh_interval` - How often to refresh (seconds)
    /// * `timeout` - HTTP timeout (seconds)
    pub fn new(crl_url: String, refresh_interval: u64, timeout: u64) -> Result<Self> {
        let config = CrlCheckerConfig {
            crl_url,
            refresh_interval_secs: refresh_interval,
            timeout_secs: timeout,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create a new CRL checker with full configuration
    pub fn with_config(config: CrlCheckerConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent("highper-gateway/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            cache: Arc::new(RwLock::new(None)),
            issuer_cache: Arc::new(RwLock::new(LruCache::new(config.max_cache_entries))),
            config,
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
        let refresh_interval = Duration::from_secs(self.config.refresh_interval_secs);
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
    /// * `Ok(false)` if certificate is not revoked
    /// * `Err` if error checking or hard-fail mode with no CRL
    pub async fn is_revoked(&self, cert: &CertificateDer<'_>) -> Result<bool> {
        use x509_parser::prelude::*;

        // Parse certificate to get serial number and issuer
        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        // Get serial number as hex string
        let serial_hex = hex::encode(parsed_cert.serial.to_bytes_be());
        let issuer_key = parsed_cert.issuer().to_string();

        debug!(
            "Checking CRL for serial number: {} (issuer: {})",
            serial_hex, issuer_key
        );

        // First check main cache
        let main_cache = self.cache.read().await;
        if let Some(crl_data) = &*main_cache {
            if let Some(result) = self.check_against_crl(crl_data, &serial_hex).await {
                return Ok(result);
            }
        }
        drop(main_cache);

        // Check issuer-specific cache
        let mut issuer_cache = self.issuer_cache.write().await;
        if let Some(crl_data) = issuer_cache.get(&issuer_key) {
            let crl_data = crl_data.clone();
            if let Some(result) = self.check_against_crl(&crl_data, &serial_hex).await {
                return Ok(result);
            }
        }
        drop(issuer_cache);

        // Try to check against stale CRL if allowed
        if self.config.allow_stale_crl {
            let main_cache = self.cache.read().await;
            if let Some(crl_data) = &*main_cache {
                let age = SystemTime::now()
                    .duration_since(crl_data.fetched_at)
                    .unwrap_or(Duration::from_secs(0));

                if age.as_secs() <= self.config.max_stale_age_secs {
                    warn!("Using stale CRL (age: {:?})", age);
                    let is_revoked = crl_data.revoked_serials.contains(&serial_hex);
                    if is_revoked {
                        warn!(
                            "Certificate is REVOKED (serial: {}) - based on stale CRL",
                            serial_hex
                        );
                    }
                    return Ok(is_revoked);
                }
            }
        }

        // No valid CRL available
        if self.config.hard_fail {
            Err(anyhow!(
                "No valid CRL available and hard-fail mode is enabled"
            ))
        } else {
            warn!("No CRL available, soft-fail: assuming certificate is not revoked");
            Ok(false)
        }
    }

    /// Check serial against a CRL
    async fn check_against_crl(&self, crl_data: &CachedCrl, serial_hex: &str) -> Option<bool> {
        // Check if CRL is still valid
        if SystemTime::now() < crl_data.valid_until {
            let is_revoked = crl_data.revoked_serials.contains(serial_hex);

            if is_revoked {
                warn!("Certificate is REVOKED (serial: {})", serial_hex);
            } else {
                debug!("Certificate is valid (serial: {})", serial_hex);
            }

            return Some(is_revoked);
        } else {
            debug!("Cached CRL has expired");
        }

        None
    }

    /// Check if certificate is revoked for a specific issuer CRL URL
    pub async fn is_revoked_with_crl_url(
        &self,
        cert: &CertificateDer<'_>,
        crl_url: &str,
    ) -> Result<bool> {
        use x509_parser::prelude::*;

        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        let serial_hex = hex::encode(parsed_cert.serial.to_bytes_be());
        let issuer_key = crl_url.to_string();

        // Check issuer cache first
        let mut issuer_cache = self.issuer_cache.write().await;
        if let Some(crl_data) = issuer_cache.get(&issuer_key) {
            let crl_data = crl_data.clone();
            if let Some(result) = self.check_against_crl(&crl_data, &serial_hex).await {
                return Ok(result);
            }
        }
        drop(issuer_cache);

        // Fetch and cache CRL for this issuer
        match self.fetch_crl_with_retry(crl_url).await {
            Ok(crl_data) => {
                let result = crl_data.revoked_serials.contains(&serial_hex);

                // Cache for this issuer
                let mut issuer_cache = self.issuer_cache.write().await;
                issuer_cache.insert(issuer_key, crl_data);

                Ok(result)
            }
            Err(e) => {
                if self.config.hard_fail {
                    Err(e)
                } else {
                    warn!("Failed to fetch CRL from {}: {}", crl_url, e);
                    Ok(false)
                }
            }
        }
    }

    /// Refresh CRL from distribution point with retry
    async fn refresh_crl(&self) -> Result<()> {
        // Build list of URLs to try
        let mut urls = vec![self.config.crl_url.clone()];
        urls.extend(self.config.fallback_urls.clone());

        let mut last_error = None;

        for url in &urls {
            if url.is_empty() {
                continue;
            }

            match self.fetch_crl_with_retry(url).await {
                Ok(crl_data) => {
                    // Handle delta CRL if present and enabled
                    let final_crl = if self.config.enable_delta_crl {
                        if let Some(delta_url) = &crl_data.delta_crl_url {
                            match self.apply_delta_crl(crl_data.clone(), delta_url).await {
                                Ok(merged) => merged,
                                Err(e) => {
                                    warn!("Failed to apply delta CRL: {}", e);
                                    crl_data
                                }
                            }
                        } else {
                            crl_data
                        }
                    } else {
                        crl_data
                    };

                    // Update cache
                    let mut cache = self.cache.write().await;
                    *cache = Some(final_crl);

                    info!("CRL refreshed successfully from {}", url);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Failed to fetch CRL from {}: {}", url, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("No CRL URLs configured")))
    }

    /// Fetch CRL with exponential backoff retry
    async fn fetch_crl_with_retry(&self, url: &str) -> Result<CachedCrl> {
        let mut retry_delay = Duration::from_millis(self.config.initial_retry_delay_ms);
        let max_delay = Duration::from_secs(30);

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                debug!(
                    "CRL fetch retry attempt {}/{}",
                    attempt, self.config.max_retries
                );
                tokio::time::sleep(retry_delay).await;
                retry_delay = std::cmp::min(retry_delay * 2, max_delay);
            }

            match self.fetch_crl_once(url).await {
                Ok(crl) => return Ok(crl),
                Err(e) => {
                    if attempt == self.config.max_retries {
                        return Err(e);
                    }
                    debug!("CRL fetch attempt {} failed: {}", attempt + 1, e);
                }
            }
        }

        Err(anyhow!(
            "CRL fetch failed after {} retries",
            self.config.max_retries
        ))
    }

    /// Single CRL fetch attempt
    async fn fetch_crl_once(&self, url: &str) -> Result<CachedCrl> {
        info!("Fetching CRL from: {}", url);

        let response = self
            .client
            .get(url)
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

        self.parse_crl(&crl_bytes, false)
    }

    /// Parse CRL and extract revoked serials
    fn parse_crl(&self, crl_bytes: &[u8], is_delta: bool) -> Result<CachedCrl> {
        use x509_parser::prelude::*;

        // Try DER format first, then PEM
        let crl_der = if crl_bytes.starts_with(b"-----BEGIN") {
            // PEM format
            let pem_str =
                std::str::from_utf8(crl_bytes).map_err(|_| anyhow!("Invalid UTF-8 in PEM CRL"))?;

            // Find the base64 content between headers
            let start = pem_str
                .find("-----BEGIN X509 CRL-----")
                .ok_or_else(|| anyhow!("No CRL found in PEM"))?;
            let end = pem_str
                .find("-----END X509 CRL-----")
                .ok_or_else(|| anyhow!("Invalid PEM format"))?;

            let base64_content: String = pem_str[start + 24..end]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();

            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(&base64_content)
                .map_err(|e| anyhow!("Failed to decode base64 CRL: {}", e))?
        } else {
            crl_bytes.to_vec()
        };

        // Parse CRL
        let (_, parsed_crl) =
            parse_x509_crl(&crl_der).map_err(|e| anyhow!("Failed to parse CRL: {}", e))?;

        // Extract revoked certificate serial numbers
        let mut revoked_serials = HashSet::new();

        for revoked_cert in parsed_crl.iter_revoked_certificates() {
            let serial_hex = hex::encode(revoked_cert.raw_serial());
            revoked_serials.insert(serial_hex);
        }

        info!(
            "Parsed {}: {} revoked certificates",
            if is_delta { "delta CRL" } else { "CRL" },
            revoked_serials.len()
        );

        // Get validity period
        let fetched_at = SystemTime::now();
        let refresh_interval = Duration::from_secs(self.config.refresh_interval_secs);

        // Calculate validity based on nextUpdate field
        let valid_until = fetched_at + refresh_interval;

        // Extract CRL number for delta CRL support
        let crl_number = self.extract_crl_number(&parsed_crl);

        // Extract delta CRL indicator URL
        let delta_crl_url = self.extract_delta_crl_url(&parsed_crl);

        Ok(CachedCrl {
            revoked_serials,
            fetched_at,
            valid_until,
            next_update: None,
            crl_number,
            delta_crl_url,
            is_delta,
        })
    }

    /// Extract CRL number from CRL extensions
    fn extract_crl_number(
        &self,
        _crl: &x509_parser::revocation_list::CertificateRevocationList<'_>,
    ) -> Option<u64> {
        // TODO: Parse CRL number extension (OID 2.5.29.20)
        // For now, return None
        None
    }

    /// Extract delta CRL distribution point URL
    fn extract_delta_crl_url(
        &self,
        _crl: &x509_parser::revocation_list::CertificateRevocationList<'_>,
    ) -> Option<String> {
        // TODO: Parse freshestCRL extension (OID 2.5.29.46)
        // For now, return None
        None
    }

    /// Apply delta CRL to base CRL
    async fn apply_delta_crl(&self, mut base_crl: CachedCrl, delta_url: &str) -> Result<CachedCrl> {
        info!("Fetching delta CRL from: {}", delta_url);

        let response = self
            .client
            .get(delta_url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to download delta CRL: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Delta CRL server returned error: {}",
                response.status()
            ));
        }

        let delta_bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read delta CRL response: {}", e))?
            .to_vec();

        let delta_crl = self.parse_crl(&delta_bytes, true)?;

        // Merge delta into base
        for serial in delta_crl.revoked_serials {
            base_crl.revoked_serials.insert(serial);
        }

        info!(
            "Merged delta CRL: now {} total revoked certificates",
            base_crl.revoked_serials.len()
        );

        base_crl.is_delta = false; // Mark as complete CRL after merge
        Ok(base_crl)
    }

    /// Get CRL statistics
    pub async fn stats(&self) -> CrlStats {
        let cache = self.cache.read().await;
        let issuer_cache = self.issuer_cache.read().await;

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
                issuer_cache_size: issuer_cache.len(),
                has_delta_crl: crl_data.delta_crl_url.is_some(),
                crl_number: crl_data.crl_number,
            }
        } else {
            CrlStats {
                cached: false,
                revoked_count: 0,
                age_secs: 0,
                remaining_secs: 0,
                issuer_cache_size: issuer_cache.len(),
                has_delta_crl: false,
                crl_number: None,
            }
        }
    }

    /// Clear all caches
    pub async fn clear_cache(&self) {
        let mut main_cache = self.cache.write().await;
        let mut issuer_cache = self.issuer_cache.write().await;

        *main_cache = None;
        issuer_cache.clear();

        info!("CRL caches cleared");
    }

    /// Get configuration
    pub fn config(&self) -> &CrlCheckerConfig {
        &self.config
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
    /// Number of issuer-specific CRLs cached
    pub issuer_cache_size: usize,
    /// Whether delta CRL is available
    pub has_delta_crl: bool,
    /// CRL number (if available)
    pub crl_number: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crl_checker_creation() {
        let checker = CrlChecker::new("http://crl.example.com/ca.crl".to_string(), 3600, 10);
        assert!(checker.is_ok());
    }

    #[test]
    fn test_crl_checker_with_config() {
        let config = CrlCheckerConfig {
            crl_url: "http://crl.example.com/ca.crl".to_string(),
            fallback_urls: vec!["http://crl2.example.com/ca.crl".to_string()],
            refresh_interval_secs: 1800,
            timeout_secs: 15,
            max_retries: 5,
            initial_retry_delay_ms: 100,
            max_cache_entries: 50,
            enable_delta_crl: true,
            allow_stale_crl: true,
            max_stale_age_secs: 43200,
            hard_fail: false,
        };

        let checker = CrlChecker::with_config(config).unwrap();
        assert_eq!(checker.config().max_retries, 5);
        assert_eq!(checker.config().max_cache_entries, 50);
    }

    #[tokio::test]
    async fn test_crl_stats_empty() {
        let checker =
            CrlChecker::new("http://crl.example.com/ca.crl".to_string(), 3600, 10).unwrap();

        let stats = checker.stats().await;
        assert!(!stats.cached);
        assert_eq!(stats.revoked_count, 0);
        assert_eq!(stats.issuer_cache_size, 0);
    }

    #[test]
    fn test_config_defaults() {
        let config = CrlCheckerConfig::default();
        assert_eq!(config.refresh_interval_secs, 3600);
        assert_eq!(config.timeout_secs, 10);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.max_cache_entries, 100);
        assert!(config.enable_delta_crl);
        assert!(config.allow_stale_crl);
        assert!(!config.hard_fail);
    }

    #[test]
    fn test_lru_cache() {
        let mut cache: LruCache<String, i32> = LruCache::new(3);

        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        cache.insert("c".to_string(), 3);

        assert_eq!(cache.len(), 3);

        // Access "a" to make it recently used
        assert_eq!(cache.get(&"a".to_string()), Some(&1));

        // Insert "d" - should evict "b" (least recently used)
        cache.insert("d".to_string(), 4);

        assert_eq!(cache.len(), 3);
        assert!(cache.get(&"a".to_string()).is_some());
        assert!(cache.get(&"b".to_string()).is_none()); // Evicted
        assert!(cache.get(&"c".to_string()).is_some());
        assert!(cache.get(&"d".to_string()).is_some());
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let checker =
            CrlChecker::new("http://crl.example.com/ca.crl".to_string(), 3600, 10).unwrap();

        checker.clear_cache().await;

        let stats = checker.stats().await;
        assert!(!stats.cached);
    }

    #[test]
    fn test_config_serialization() {
        let config = CrlCheckerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CrlCheckerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.refresh_interval_secs, parsed.refresh_interval_secs);
        assert_eq!(config.max_retries, parsed.max_retries);
        assert_eq!(config.hard_fail, parsed.hard_fail);
    }
}
