//! Web server configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Directory listing output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DirectoryListingFormat {
    /// HTML format (default, human-readable)
    #[default]
    Html,
    /// JSON format (machine-readable)
    Json,
}

fn default_listing_format() -> DirectoryListingFormat {
    DirectoryListingFormat::Html
}

/// Web server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebServerConfig {
    /// Enable static file serving
    #[serde(default)]
    pub enable_static_files: bool,

    /// Document root for static files
    pub document_root: Option<PathBuf>,

    /// Default index files
    #[serde(default = "default_index_files")]
    pub index_files: Vec<String>,

    /// Enable directory listing
    #[serde(default)]
    pub directory_listing: bool,

    /// Directory listing format (html or json)
    #[serde(default = "default_listing_format")]
    pub directory_listing_format: DirectoryListingFormat,

    /// Show hidden files in directory listings
    #[serde(default)]
    pub show_hidden_files: bool,

    /// Enable PHP-FPM support
    #[serde(default)]
    pub enable_php_fpm: bool,

    /// PHP-FPM configuration
    pub php_fpm: Option<PhpFpmConfig>,

    /// Enable range requests (partial content)
    #[serde(default = "default_true")]
    pub enable_range_requests: bool,

    /// Enable ETag generation
    #[serde(default = "default_true")]
    pub enable_etag: bool,

    /// Enable gzip compression for static files
    #[serde(default = "default_true")]
    pub enable_gzip: bool,

    /// Enable brotli compression
    #[serde(default)]
    pub enable_brotli: bool,

    /// Cache control headers
    pub cache_control: Option<CacheControlConfig>,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            enable_static_files: false,
            document_root: None,
            index_files: default_index_files(),
            directory_listing: false,
            directory_listing_format: DirectoryListingFormat::Html,
            show_hidden_files: false,
            enable_php_fpm: false,
            php_fpm: None,
            enable_range_requests: true,
            enable_etag: true,
            enable_gzip: true,
            enable_brotli: false,
            cache_control: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_index_files() -> Vec<String> {
    vec![
        "index.html".to_string(),
        "index.htm".to_string(),
        "index.php".to_string(),
    ]
}

/// PHP-FPM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpFpmConfig {
    /// PHP-FPM socket path (Unix socket or TCP address)
    pub socket: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub connect_timeout: u64,

    /// Read timeout in seconds
    #[serde(default = "default_read_timeout")]
    pub read_timeout: u64,

    /// Write timeout in seconds
    #[serde(default = "default_write_timeout")]
    pub write_timeout: u64,

    /// Keep-alive timeout in seconds
    #[serde(default = "default_keepalive")]
    pub keepalive_timeout: u64,

    /// PHP script extensions
    #[serde(default = "default_php_extensions")]
    pub script_extensions: Vec<String>,

    /// SCRIPT_FILENAME override (useful for path rewriting)
    pub script_filename_override: Option<String>,

    /// Additional FastCGI parameters
    #[serde(default)]
    pub fastcgi_params: std::collections::HashMap<String, String>,

    /// Document root path inside PHP-FPM container (for path translation)
    /// If set, translates host paths to container paths in SCRIPT_FILENAME
    pub document_root: Option<String>,
}

impl Default for PhpFpmConfig {
    fn default() -> Self {
        Self {
            socket: "/var/run/php/php-fpm.sock".to_string(),
            pool_size: default_pool_size(),
            connect_timeout: default_timeout(),
            read_timeout: default_read_timeout(),
            write_timeout: default_write_timeout(),
            keepalive_timeout: default_keepalive(),
            script_extensions: default_php_extensions(),
            script_filename_override: None,
            fastcgi_params: std::collections::HashMap::new(),
            document_root: None,
        }
    }
}

fn default_pool_size() -> usize {
    10
}

fn default_timeout() -> u64 {
    5
}

fn default_read_timeout() -> u64 {
    30
}

fn default_write_timeout() -> u64 {
    30
}

fn default_keepalive() -> u64 {
    60
}

fn default_php_extensions() -> Vec<String> {
    vec![".php".to_string(), ".php5".to_string(), ".php7".to_string()]
}

/// Cache control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControlConfig {
    /// Default max-age for static files (seconds)
    #[serde(default = "default_max_age")]
    pub default_max_age: u64,

    /// Cache control by file extension
    #[serde(default)]
    pub by_extension: std::collections::HashMap<String, u64>,

    /// Enable public cache control
    #[serde(default = "default_true")]
    pub public: bool,

    /// Enable immutable directive
    #[serde(default)]
    pub immutable: bool,
}

impl Default for CacheControlConfig {
    fn default() -> Self {
        Self {
            default_max_age: default_max_age(),
            by_extension: std::collections::HashMap::new(),
            public: true,
            immutable: false,
        }
    }
}

fn default_max_age() -> u64 {
    3600 // 1 hour
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WebServerConfig::default();
        assert!(!config.enable_static_files);
        assert!(!config.enable_php_fpm);
        assert!(config.enable_range_requests);
        assert!(config.enable_etag);
    }

    #[test]
    fn test_php_fpm_config() {
        let config = PhpFpmConfig::default();
        assert_eq!(config.pool_size, 10);
        assert_eq!(config.connect_timeout, 5);
        assert!(config.script_extensions.contains(&".php".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = WebServerConfig {
            enable_static_files: true,
            document_root: Some(PathBuf::from("/var/www/html")),
            index_files: vec!["index.html".to_string()],
            directory_listing: false,
            directory_listing_format: DirectoryListingFormat::Html,
            show_hidden_files: false,
            enable_php_fpm: true,
            php_fpm: Some(PhpFpmConfig::default()),
            enable_range_requests: true,
            enable_etag: true,
            enable_gzip: true,
            enable_brotli: false,
            cache_control: Some(CacheControlConfig::default()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WebServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enable_static_files, deserialized.enable_static_files);
        assert_eq!(config.enable_php_fpm, deserialized.enable_php_fpm);
    }
}
