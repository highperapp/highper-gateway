//! Content negotiation for compression
//!
//! Implements HTTP Accept-Encoding header parsing and content negotiation
//! according to RFC 7231.

use super::compressor::Compressor;
use super::registry::GLOBAL_COMPRESSOR_REGISTRY;
use std::sync::Arc;
use tracing::{debug, trace};

/// Encoding preference with quality value
#[derive(Debug, Clone, PartialEq)]
pub struct EncodingPreference {
    pub encoding: String,
    pub quality: f32,
}

impl EncodingPreference {
    pub fn new(encoding: impl Into<String>, quality: f32) -> Self {
        Self {
            encoding: encoding.into(),
            quality: quality.clamp(0.0, 1.0),
        }
    }
}

/// Parse Accept-Encoding header with full HTTP spec compliance
///
/// Parses quality values (q) and handles wildcards according to RFC 7231.
///
/// # Example
/// ```
/// # use highper_gateway::middleware::compression::parse_accept_encoding;
/// let prefs = parse_accept_encoding("gzip, deflate, br;q=0.8");
/// assert_eq!(prefs.len(), 3);
/// ```
pub fn parse_accept_encoding(header: &str) -> Vec<EncodingPreference> {
    let mut preferences = Vec::new();

    trace!("Parsing Accept-Encoding: {}", header);

    for part in header.split(',') {
        let part = part.trim();

        if part.is_empty() {
            continue;
        }

        // Handle quality parameter (e.g., "gzip;q=0.8")
        let (encoding, quality) = if let Some((enc, q_str)) = part.split_once(";q=") {
            let quality = q_str.trim().parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
            (enc.trim(), quality)
        } else if let Some((enc, _)) = part.split_once(';') {
            // Handle other parameters (ignore them for now)
            (enc.trim(), 1.0)
        } else {
            (part, 1.0)
        };

        // Skip if quality is 0 (explicitly not accepted)
        if quality > 0.0 && !encoding.is_empty() {
            preferences.push(EncodingPreference::new(encoding.to_lowercase(), quality));
            trace!("  Parsed: {} (q={})", encoding, quality);
        }
    }

    // Sort by quality (descending)
    preferences.sort_by(|a, b| {
        b.quality
            .partial_cmp(&a.quality)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    debug!("Parsed {} encoding preferences", preferences.len());
    preferences
}

/// Select best compressor based on Accept-Encoding and server preferences
///
/// # Arguments
/// * `accept_encoding` - Client's Accept-Encoding header value
/// * `server_preferences` - Server's preferred encodings in priority order
///
/// # Returns
/// The best matching compressor, or None if no acceptable match found
///
/// # Example
/// ```ignore
/// let compressor = select_compressor(
///     "gzip, deflate, br;q=0.9",
///     &["br", "zstd", "gzip"]
/// );
/// ```
pub fn select_compressor(
    accept_encoding: &str,
    server_preferences: &[&str],
) -> Option<Arc<dyn Compressor>> {
    let client_prefs = parse_accept_encoding(accept_encoding);

    debug!(
        "Selecting compressor: client accepts {:?}, server prefers {:?}",
        client_prefs.iter().map(|p| &p.encoding).collect::<Vec<_>>(),
        server_preferences
    );

    // Try to match server preferences first
    for &server_pref in server_preferences {
        // Check if client accepts this encoding
        for client_pref in &client_prefs {
            if client_pref.quality == 0.0 {
                continue;
            }

            // Direct match
            if client_pref.encoding == server_pref {
                if let Some(compressor) = GLOBAL_COMPRESSOR_REGISTRY.get(server_pref) {
                    if compressor.is_available() {
                        debug!(
                            "Selected {} (server pref: {}, client q: {})",
                            compressor.name(),
                            server_pref,
                            client_pref.quality
                        );
                        return Some(compressor);
                    }
                }
            }

            // Wildcard match
            if client_pref.encoding == "*" {
                if let Some(compressor) = GLOBAL_COMPRESSOR_REGISTRY.get(server_pref) {
                    if compressor.is_available() {
                        debug!(
                            "Selected {} (via wildcard, server pref: {})",
                            compressor.name(),
                            server_pref
                        );
                        return Some(compressor);
                    }
                }
            }
        }
    }

    // Check for explicit "identity" (no compression)
    for pref in &client_prefs {
        if pref.encoding == "identity" && pref.quality > 0.0 {
            debug!("Client accepts identity (no compression)");
            return None;
        }
    }

    debug!("No acceptable compressor found");
    None
}

/// Check if content type should be compressed
///
/// Returns true for text-based content types that benefit from compression.
///
/// # Example
/// ```
/// # use highper_gateway::middleware::compression::is_compressible;
/// assert!(is_compressible("text/html"));
/// assert!(is_compressible("application/json"));
/// assert!(!is_compressible("image/png"));
/// ```
pub fn is_compressible(content_type: &str) -> bool {
    const COMPRESSIBLE_TYPES: &[&str] = &[
        "text/",
        "application/json",
        "application/javascript",
        "application/xml",
        "application/xhtml",
        "application/rss+xml",
        "application/atom+xml",
        "application/ld+json",
        "application/manifest+json",
        "application/x-javascript",
        "application/graphql",
        "application/graphql+json",
        "image/svg+xml",
        "image/x-icon",
        "font/",
    ];

    let content_type_lower = content_type.to_lowercase();
    COMPRESSIBLE_TYPES
        .iter()
        .any(|t| content_type_lower.starts_with(t))
}

/// Check if content should NOT be compressed based on Content-Encoding
///
/// Returns true if content is already compressed or should not be compressed.
pub fn is_already_compressed(content_encoding: Option<&str>) -> bool {
    if let Some(encoding) = content_encoding {
        let encoding_lower = encoding.to_lowercase();
        !encoding_lower.is_empty() && encoding_lower != "identity"
    } else {
        false
    }
}

/// Get default server preferences
///
/// Returns the default compression preference order.
pub fn default_server_preferences() -> Vec<&'static str> {
    vec![
        "br",      // Brotli (best compression)
        "zstd",    // Zstandard (balanced)
        "gzip",    // Gzip (widely supported)
        "deflate", // Deflate (legacy)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_accept_encoding_simple() {
        let prefs = parse_accept_encoding("gzip, deflate, br");
        assert_eq!(prefs.len(), 3);
        assert!(prefs
            .iter()
            .any(|p| p.encoding == "gzip" && p.quality == 1.0));
        assert!(prefs
            .iter()
            .any(|p| p.encoding == "deflate" && p.quality == 1.0));
        assert!(prefs.iter().any(|p| p.encoding == "br" && p.quality == 1.0));
    }

    #[test]
    fn test_parse_accept_encoding_with_quality() {
        let prefs = parse_accept_encoding("gzip, deflate;q=0.8, br;q=0.9");
        assert_eq!(prefs.len(), 3);

        // Should be sorted by quality
        assert_eq!(prefs[0].encoding, "gzip");
        assert_eq!(prefs[0].quality, 1.0);
        assert_eq!(prefs[1].encoding, "br");
        assert_eq!(prefs[1].quality, 0.9);
        assert_eq!(prefs[2].encoding, "deflate");
        assert_eq!(prefs[2].quality, 0.8);
    }

    #[test]
    fn test_parse_accept_encoding_with_zero_quality() {
        let prefs = parse_accept_encoding("gzip, deflate;q=0, br");
        assert_eq!(prefs.len(), 2);
        assert!(!prefs.iter().any(|p| p.encoding == "deflate"));
    }

    #[test]
    fn test_parse_accept_encoding_wildcard() {
        let prefs = parse_accept_encoding("gzip, *;q=0.5");
        assert_eq!(prefs.len(), 2);
        assert!(prefs.iter().any(|p| p.encoding == "gzip"));
        assert!(prefs.iter().any(|p| p.encoding == "*" && p.quality == 0.5));
    }

    #[test]
    fn test_parse_accept_encoding_identity() {
        let prefs = parse_accept_encoding("gzip, identity;q=0.5");
        assert_eq!(prefs.len(), 2);
        assert!(prefs.iter().any(|p| p.encoding == "identity"));
    }

    #[test]
    fn test_parse_accept_encoding_empty() {
        let prefs = parse_accept_encoding("");
        assert_eq!(prefs.len(), 0);
    }

    #[test]
    fn test_is_compressible() {
        // Should compress
        assert!(is_compressible("text/html"));
        assert!(is_compressible("text/plain; charset=utf-8"));
        assert!(is_compressible("application/json"));
        assert!(is_compressible("application/javascript"));
        assert!(is_compressible("application/graphql"));
        assert!(is_compressible("image/svg+xml"));

        // Should not compress
        assert!(!is_compressible("image/png"));
        assert!(!is_compressible("image/jpeg"));
        assert!(!is_compressible("video/mp4"));
        assert!(!is_compressible("application/octet-stream"));
    }

    #[test]
    fn test_is_already_compressed() {
        assert!(is_already_compressed(Some("gzip")));
        assert!(is_already_compressed(Some("br")));
        assert!(!is_already_compressed(Some("identity")));
        assert!(!is_already_compressed(Some("")));
        assert!(!is_already_compressed(None));
    }

    #[test]
    fn test_default_server_preferences() {
        let prefs = default_server_preferences();
        assert_eq!(prefs.len(), 4);
        assert_eq!(prefs[0], "br");
        assert_eq!(prefs[1], "zstd");
        assert_eq!(prefs[2], "gzip");
        assert_eq!(prefs[3], "deflate");
    }
}
