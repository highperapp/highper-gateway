//! Global compressor registry for managing compression algorithms
//!
//! This module provides a thread-safe registry for discovering and managing
//! compression algorithms at runtime.

use super::compressor::Compressor;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Global compressor registry
///
/// Manages registered compression algorithms and provides lookup functionality.
/// Thread-safe using RwLock for concurrent read access.
pub struct CompressorRegistry {
    compressors: RwLock<HashMap<String, Arc<dyn Compressor>>>,
}

impl CompressorRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            compressors: RwLock::new(HashMap::new()),
        }
    }

    /// Create new registry with default compressors
    ///
    /// This will register gzip, brotli, zstd, and deflate compressors
    /// if they are available on the platform.
    pub fn with_defaults() -> Self {
        let registry = Self::new();

        info!("Initializing compressor registry with default algorithms");

        // Note: Actual compressor instances will be registered after
        // the individual compressor modules are implemented

        registry
    }

    /// Register a compressor
    ///
    /// # Example
    /// ```ignore
    /// let mut registry = CompressorRegistry::new();
    /// registry.register(Arc::new(GzipCompressor::new()));
    /// ```
    pub fn register(&self, compressor: Arc<dyn Compressor>) {
        let encoding = compressor.encoding().to_string();
        let name = compressor.name();

        let mut compressors = self.compressors.write();

        if compressors.contains_key(&encoding) {
            warn!(
                "Overwriting existing compressor for encoding '{}' (was: {})",
                encoding,
                compressors
                    .get(&encoding)
                    .map(|c| c.name())
                    .unwrap_or("unknown")
            );
        }

        info!(
            "Registered compressor: {} (encoding: {}, quality: {:.2})",
            name,
            encoding,
            compressor.quality()
        );

        compressors.insert(encoding, compressor);
    }

    /// Unregister a compressor by encoding name
    ///
    /// Returns the unregistered compressor if it existed.
    pub fn unregister(&self, encoding: &str) -> Option<Arc<dyn Compressor>> {
        let mut compressors = self.compressors.write();
        let removed = compressors.remove(encoding);

        if let Some(ref compressor) = removed {
            info!(
                "Unregistered compressor: {} (encoding: {})",
                compressor.name(),
                encoding
            );
        } else {
            debug!(
                "Attempted to unregister non-existent compressor: {}",
                encoding
            );
        }

        removed
    }

    /// Get compressor by encoding name
    ///
    /// # Example
    /// ```ignore
    /// if let Some(compressor) = registry.get("gzip") {
    ///     // Use compressor
    /// }
    /// ```
    pub fn get(&self, encoding: &str) -> Option<Arc<dyn Compressor>> {
        let compressors = self.compressors.read();
        compressors.get(encoding).cloned()
    }

    /// List all registered compressor encodings
    pub fn list(&self) -> Vec<String> {
        let compressors = self.compressors.read();
        compressors.keys().cloned().collect()
    }

    /// Get all registered compressors with their metadata
    pub fn list_detailed(&self) -> Vec<CompressorInfo> {
        let compressors = self.compressors.read();
        compressors
            .values()
            .map(|c| CompressorInfo {
                name: c.name().to_string(),
                encoding: c.encoding().to_string(),
                quality: c.quality(),
                available: c.is_available(),
            })
            .collect()
    }

    /// Check if a specific encoding is registered
    pub fn has(&self, encoding: &str) -> bool {
        let compressors = self.compressors.read();
        compressors.contains_key(encoding)
    }

    /// Get compressor count
    pub fn count(&self) -> usize {
        let compressors = self.compressors.read();
        compressors.len()
    }

    /// Select best compressor based on Accept-Encoding and preferences
    ///
    /// # Arguments
    /// * `accept_encoding` - Client's Accept-Encoding header value
    /// * `preferences` - Server's preferred encodings in priority order
    ///
    /// # Returns
    /// The best matching compressor, or None if no acceptable match found
    pub fn select_best(
        &self,
        accept_encoding: &str,
        preferences: &[&str],
    ) -> Option<Arc<dyn Compressor>> {
        // Parse Accept-Encoding with quality values
        let accepted = parse_accept_encoding(accept_encoding);

        debug!(
            "Selecting compressor for Accept-Encoding: {}, preferences: {:?}",
            accept_encoding, preferences
        );

        // Try each preference in order
        for pref in preferences {
            // Check if client accepts this encoding
            if let Some(quality) = accepted.get(*pref) {
                if *quality > 0.0 {
                    // Try to get the compressor
                    if let Some(compressor) = self.get(pref) {
                        if compressor.is_available() {
                            info!(
                                "Selected compressor: {} (quality: {:.2}, client q: {:.2})",
                                compressor.name(),
                                compressor.quality(),
                                quality
                            );
                            return Some(compressor);
                        } else {
                            warn!("Compressor {} not available on this platform", pref);
                        }
                    }
                }
            }
        }

        // Check for wildcard (*) acceptance
        if let Some(quality) = accepted.get("*") {
            if *quality > 0.0 {
                // Return first available from preferences
                for pref in preferences {
                    if let Some(compressor) = self.get(pref) {
                        if compressor.is_available() {
                            info!(
                                "Selected compressor (via wildcard): {} (quality: {:.2})",
                                compressor.name(),
                                compressor.quality()
                            );
                            return Some(compressor);
                        }
                    }
                }
            }
        }

        debug!("No acceptable compressor found");
        None
    }

    /// Clear all registered compressors
    pub fn clear(&self) {
        let mut compressors = self.compressors.write();
        info!("Clearing {} registered compressors", compressors.len());
        compressors.clear();
    }
}

impl Default for CompressorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Compressor information for listing
#[derive(Debug, Clone)]
pub struct CompressorInfo {
    pub name: String,
    pub encoding: String,
    pub quality: f32,
    pub available: bool,
}

/// Parse Accept-Encoding header with quality values
///
/// Parses HTTP Accept-Encoding header according to RFC 7231.
/// Supports quality values (q) and wildcard (*).
///
/// # Example
/// ```ignore
/// let accepted = parse_accept_encoding("gzip, deflate;q=0.8, br;q=0.9");
/// assert_eq!(accepted.get("gzip"), Some(&1.0));
/// assert_eq!(accepted.get("br"), Some(&0.9));
/// ```
fn parse_accept_encoding(header: &str) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    for part in header.split(',') {
        let part = part.trim();

        // Handle quality parameter (e.g., "gzip;q=0.8")
        let (encoding, quality) = if let Some((enc, q_str)) = part.split_once(";q=") {
            let quality = q_str.trim().parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
            (enc.trim(), quality)
        } else if let Some((enc, _)) = part.split_once(';') {
            // Handle other parameters (ignore them)
            (enc.trim(), 1.0)
        } else {
            (part, 1.0)
        };

        // Skip empty encodings
        if !encoding.is_empty() {
            result.insert(encoding.to_lowercase(), quality);
        }
    }

    result
}

/// Global registry instance
///
/// Use this singleton to access the global compressor registry.
/// It's initialized lazily on first access.
///
/// # Example
/// ```ignore
/// use highper_gateway::middleware::compression::GLOBAL_COMPRESSOR_REGISTRY;
///
/// // Get a compressor
/// if let Some(compressor) = GLOBAL_COMPRESSOR_REGISTRY.get("gzip") {
///     // Use it
/// }
/// ```
use once_cell::sync::Lazy;

pub static GLOBAL_COMPRESSOR_REGISTRY: Lazy<CompressorRegistry> = Lazy::new(|| {
    let registry = CompressorRegistry::with_defaults();
    info!("Global compressor registry initialized");
    registry
});

#[cfg(test)]
mod tests {
    use super::*;

    // Mock compressor for testing
    struct MockCompressor {
        name: &'static str,
        encoding: &'static str,
        quality: f32,
    }

    #[async_trait::async_trait]
    impl Compressor for MockCompressor {
        fn name(&self) -> &'static str {
            self.name
        }

        fn encoding(&self) -> &'static str {
            self.encoding
        }

        fn quality(&self) -> f32 {
            self.quality
        }

        fn compress(
            &self,
            _data: &[u8],
            _config: &super::super::compressor::CompressorConfig,
        ) -> Result<
            super::super::compressor::CompressionResult,
            super::super::compressor::CompressionError,
        > {
            Ok(super::super::compressor::CompressionResult {
                data: vec![],
                original_size: 0,
                compressed_size: 0,
                algorithm: self.name,
                compression_ratio: 1.0,
            })
        }
    }

    #[test]
    fn test_registry_basic() {
        let registry = CompressorRegistry::new();
        assert_eq!(registry.count(), 0);

        let mock = Arc::new(MockCompressor {
            name: "mock",
            encoding: "mock",
            quality: 0.8,
        });

        registry.register(mock.clone());
        assert_eq!(registry.count(), 1);
        assert!(registry.has("mock"));
        assert!(registry.get("mock").is_some());
    }

    #[test]
    fn test_registry_unregister() {
        let registry = CompressorRegistry::new();
        let mock = Arc::new(MockCompressor {
            name: "mock",
            encoding: "mock",
            quality: 0.8,
        });

        registry.register(mock);
        assert_eq!(registry.count(), 1);

        registry.unregister("mock");
        assert_eq!(registry.count(), 0);
        assert!(!registry.has("mock"));
    }

    #[test]
    fn test_parse_accept_encoding() {
        let parsed = parse_accept_encoding("gzip, deflate;q=0.8, br;q=0.9");
        assert_eq!(parsed.get("gzip"), Some(&1.0));
        assert_eq!(parsed.get("deflate"), Some(&0.8));
        assert_eq!(parsed.get("br"), Some(&0.9));
    }

    #[test]
    fn test_parse_accept_encoding_with_zero() {
        let parsed = parse_accept_encoding("gzip, deflate;q=0");
        assert_eq!(parsed.get("gzip"), Some(&1.0));
        assert_eq!(parsed.get("deflate"), Some(&0.0));
    }

    #[test]
    fn test_parse_accept_encoding_wildcard() {
        let parsed = parse_accept_encoding("gzip, *;q=0.5");
        assert_eq!(parsed.get("gzip"), Some(&1.0));
        assert_eq!(parsed.get("*"), Some(&0.5));
    }

    #[test]
    fn test_registry_list() {
        let registry = CompressorRegistry::new();

        registry.register(Arc::new(MockCompressor {
            name: "gzip",
            encoding: "gzip",
            quality: 0.7,
        }));

        registry.register(Arc::new(MockCompressor {
            name: "brotli",
            encoding: "br",
            quality: 0.9,
        }));

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"gzip".to_string()));
        assert!(list.contains(&"br".to_string()));
    }

    #[test]
    fn test_registry_select_best() {
        let registry = CompressorRegistry::new();

        registry.register(Arc::new(MockCompressor {
            name: "gzip",
            encoding: "gzip",
            quality: 0.7,
        }));

        registry.register(Arc::new(MockCompressor {
            name: "brotli",
            encoding: "br",
            quality: 0.9,
        }));

        // Should select br (first in preferences and accepted)
        let compressor = registry.select_best("gzip, br", &["br", "gzip"]);
        assert!(compressor.is_some());
        assert_eq!(compressor.unwrap().name(), "brotli");

        // Should select gzip (only one accepted)
        let compressor = registry.select_best("gzip", &["br", "gzip"]);
        assert!(compressor.is_some());
        assert_eq!(compressor.unwrap().name(), "gzip");

        // Should return None (nothing accepted)
        let compressor = registry.select_best("identity", &["br", "gzip"]);
        assert!(compressor.is_none());
    }
}
