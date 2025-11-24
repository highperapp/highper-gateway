//! MIME type detection and caching

use dashmap::DashMap;
use std::path::Path;
use std::sync::Arc;

/// MIME type cache (lock-free concurrent access)
pub struct MimeCache {
    cache: Arc<DashMap<String, String>>,
}

impl MimeCache {
    /// Create a new MIME cache
    pub fn new() -> Self {
        let cache = Self {
            cache: Arc::new(DashMap::new()),
        };

        // Pre-populate common MIME types
        cache.populate_defaults();
        cache
    }

    /// Get MIME type for file extension
    pub fn get_mime_type(&self, path: &Path) -> String {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();

            // Try cache first (O(1) lookup)
            if let Some(mime) = self.cache.get(&ext) {
                return mime.value().clone();
            }

            // Fallback to detection
            let mime = detect_mime_type(&ext);
            self.cache.insert(ext, mime.clone());
            mime
        } else {
            "application/octet-stream".to_string()
        }
    }

    /// Pre-populate cache with common MIME types
    fn populate_defaults(&self) {
        // HTML
        self.cache.insert("html".to_string(), "text/html; charset=utf-8".to_string());
        self.cache.insert("htm".to_string(), "text/html; charset=utf-8".to_string());

        // CSS
        self.cache.insert("css".to_string(), "text/css; charset=utf-8".to_string());

        // JavaScript
        self.cache.insert("js".to_string(), "application/javascript; charset=utf-8".to_string());
        self.cache.insert("mjs".to_string(), "application/javascript; charset=utf-8".to_string());
        self.cache.insert("json".to_string(), "application/json; charset=utf-8".to_string());

        // Images
        self.cache.insert("png".to_string(), "image/png".to_string());
        self.cache.insert("jpg".to_string(), "image/jpeg".to_string());
        self.cache.insert("jpeg".to_string(), "image/jpeg".to_string());
        self.cache.insert("gif".to_string(), "image/gif".to_string());
        self.cache.insert("svg".to_string(), "image/svg+xml".to_string());
        self.cache.insert("webp".to_string(), "image/webp".to_string());
        self.cache.insert("ico".to_string(), "image/x-icon".to_string());

        // Fonts
        self.cache.insert("woff".to_string(), "font/woff".to_string());
        self.cache.insert("woff2".to_string(), "font/woff2".to_string());
        self.cache.insert("ttf".to_string(), "font/ttf".to_string());
        self.cache.insert("eot".to_string(), "application/vnd.ms-fontobject".to_string());

        // Documents
        self.cache.insert("pdf".to_string(), "application/pdf".to_string());
        self.cache.insert("txt".to_string(), "text/plain; charset=utf-8".to_string());
        self.cache.insert("xml".to_string(), "application/xml; charset=utf-8".to_string());

        // Video
        self.cache.insert("mp4".to_string(), "video/mp4".to_string());
        self.cache.insert("webm".to_string(), "video/webm".to_string());
        self.cache.insert("ogv".to_string(), "video/ogg".to_string());

        // Audio
        self.cache.insert("mp3".to_string(), "audio/mpeg".to_string());
        self.cache.insert("ogg".to_string(), "audio/ogg".to_string());
        self.cache.insert("wav".to_string(), "audio/wav".to_string());

        // Archives
        self.cache.insert("zip".to_string(), "application/zip".to_string());
        self.cache.insert("gz".to_string(), "application/gzip".to_string());
        self.cache.insert("tar".to_string(), "application/x-tar".to_string());
    }

    /// Clear cache
    pub fn clear(&self) {
        self.cache.clear();
        self.populate_defaults();
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

impl Default for MimeCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect MIME type from file extension
fn detect_mime_type(extension: &str) -> String {
    match extension {
        // Text formats
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "txt" | "text" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",

        // JavaScript
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",

        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" | "svgz" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",

        // Fonts
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",

        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",

        // Video
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",

        // Audio
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",

        // Archives
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        "rar" => "application/x-rar-compressed",
        "7z" => "application/x-7z-compressed",

        // Default
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Check if MIME type is compressible
pub fn is_compressible(mime_type: &str) -> bool {
    let mime_lower = mime_type.to_lowercase();

    mime_lower.starts_with("text/")
        || mime_lower.contains("javascript")
        || mime_lower.contains("json")
        || mime_lower.contains("xml")
        || mime_lower.contains("svg")
}

/// Check if MIME type should have charset
pub fn needs_charset(mime_type: &str) -> bool {
    let mime_lower = mime_type.to_lowercase();

    mime_lower.starts_with("text/")
        || mime_lower.contains("javascript")
        || mime_lower.contains("json")
        || mime_lower.contains("xml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_cache() {
        let cache = MimeCache::new();

        let path = Path::new("test.html");
        assert_eq!(cache.get_mime_type(path), "text/html; charset=utf-8");

        let path = Path::new("image.png");
        assert_eq!(cache.get_mime_type(path), "image/png");

        let path = Path::new("script.js");
        assert_eq!(cache.get_mime_type(path), "application/javascript; charset=utf-8");
    }

    #[test]
    fn test_detect_mime_type() {
        assert_eq!(detect_mime_type("html"), "text/html; charset=utf-8");
        assert_eq!(detect_mime_type("css"), "text/css; charset=utf-8");
        assert_eq!(detect_mime_type("png"), "image/png");
        assert_eq!(detect_mime_type("mp4"), "video/mp4");
        assert_eq!(detect_mime_type("unknown"), "application/octet-stream");
    }

    #[test]
    fn test_is_compressible() {
        assert!(is_compressible("text/html"));
        assert!(is_compressible("application/javascript"));
        assert!(is_compressible("application/json"));
        assert!(is_compressible("text/css"));

        assert!(!is_compressible("image/png"));
        assert!(!is_compressible("video/mp4"));
        assert!(!is_compressible("application/zip"));
    }

    #[test]
    fn test_needs_charset() {
        assert!(needs_charset("text/html"));
        assert!(needs_charset("application/javascript"));
        assert!(needs_charset("application/json"));

        assert!(!needs_charset("image/png"));
        assert!(!needs_charset("video/mp4"));
    }

    #[test]
    fn test_cache_size() {
        let cache = MimeCache::new();
        let initial_size = cache.size();

        // Should have pre-populated types
        assert!(initial_size > 20);

        // Adding new type
        let path = Path::new("test.xyz");
        cache.get_mime_type(path);

        assert_eq!(cache.size(), initial_size + 1);
    }
}
