//! Static file serving with zero-copy sendfile

use super::{DirectoryListingFormat, MimeCache, WebServerConfig};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[cfg(target_os = "linux")]
use crate::tls::ktls::sendfile_ktls;

/// Static file handler
pub struct StaticFileHandler {
    /// Document root directory
    document_root: PathBuf,

    /// MIME type cache
    mime_cache: MimeCache,

    /// Configuration
    config: WebServerConfig,
}

impl StaticFileHandler {
    /// Create a new static file handler
    pub fn new(document_root: PathBuf, config: WebServerConfig) -> Self {
        Self {
            document_root,
            mime_cache: MimeCache::new(),
            config,
        }
    }

    /// Resolve file path from request URI
    pub fn resolve_path(&self, uri: &str) -> io::Result<PathBuf> {
        // Remove query string
        let path_part = uri.split('?').next().unwrap_or(uri);

        // URL decode
        let decoded = percent_encoding::percent_decode_str(path_part)
            .decode_utf8()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid UTF-8 in path"))?;

        // Security: prevent directory traversal
        let requested_path = decoded.trim_start_matches('/');
        let full_path = self.document_root.join(requested_path);

        // Canonicalize to resolve .. and symlinks
        let canonical = full_path.canonicalize()?;

        // Ensure path is under document root
        if !canonical.starts_with(&self.document_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Path traversal attempt detected",
            ));
        }

        Ok(canonical)
    }

    /// Get file metadata and handle directories
    pub fn get_file_info(&self, path: &Path) -> io::Result<FileInfo> {
        let metadata = std::fs::metadata(path)?;

        if metadata.is_dir() {
            // Try index files
            for index_file in &self.config.index_files {
                let index_path = path.join(index_file);
                if index_path.exists() && index_path.is_file() {
                    let index_metadata = std::fs::metadata(&index_path)?;
                    return Ok(FileInfo {
                        path: index_path,
                        metadata: index_metadata,
                        is_php: index_file.ends_with(".php"),
                        is_directory: false,
                    });
                }
            }

            // No index file found - return directory listing marker
            if self.config.directory_listing {
                return Ok(FileInfo {
                    path: path.to_path_buf(),
                    metadata,
                    is_php: false,
                    is_directory: true,
                });
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Directory listing disabled",
                ));
            }
        }

        Ok(FileInfo {
            path: path.to_path_buf(),
            metadata,
            is_php: path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "php")
                .unwrap_or(false),
            is_directory: false,
        })
    }

    /// Generate directory listing
    ///
    /// Returns the listing content and content-type header value.
    pub fn generate_directory_listing(
        &self,
        path: &Path,
        request_uri: &str,
    ) -> io::Result<DirectoryListing> {
        let entries = fs::read_dir(path)?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files if configured
            if !self.config.show_hidden_files && file_name.starts_with('.') {
                continue;
            }

            let metadata = entry.metadata()?;
            let modified = metadata
                .modified()
                .unwrap_or(UNIX_EPOCH)
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let entry_info = DirectoryEntry {
                name: file_name,
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
                modified,
                is_directory: metadata.is_dir(),
            };

            if metadata.is_dir() {
                dirs.push(entry_info);
            } else {
                files.push(entry_info);
            }
        }

        // Sort: directories first, then files, both alphabetically
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(DirectoryListing {
            path: request_uri.to_string(),
            directories: dirs,
            files,
        })
    }

    /// Render directory listing to HTML or JSON based on config
    pub fn render_directory_listing(&self, listing: &DirectoryListing) -> DirectoryListingResponse {
        match self.config.directory_listing_format {
            DirectoryListingFormat::Html => {
                let content = self.render_html_listing(listing);
                DirectoryListingResponse {
                    content,
                    content_type: "text/html; charset=utf-8".to_string(),
                }
            }
            DirectoryListingFormat::Json => {
                let content =
                    serde_json::to_string_pretty(listing).unwrap_or_else(|_| "{}".to_string());
                DirectoryListingResponse {
                    content,
                    content_type: "application/json".to_string(),
                }
            }
        }
    }

    /// Render HTML directory listing
    fn render_html_listing(&self, listing: &DirectoryListing) -> String {
        let mut html = String::with_capacity(4096);

        // HTML header
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<meta charset=\"utf-8\">\n");
        html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        html.push_str(&format!(
            "<title>Index of {}</title>\n",
            escape_html(&listing.path)
        ));
        html.push_str("<style>\n");
        html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; ");
        html.push_str("margin: 40px; background: #f5f5f5; }\n");
        html.push_str("h1 { color: #333; border-bottom: 1px solid #ddd; padding-bottom: 10px; }\n");
        html.push_str("table { width: 100%; border-collapse: collapse; background: white; ");
        html.push_str("box-shadow: 0 1px 3px rgba(0,0,0,0.1); }\n");
        html.push_str(
            "th, td { text-align: left; padding: 12px 15px; border-bottom: 1px solid #eee; }\n",
        );
        html.push_str("th { background: #f8f9fa; font-weight: 600; color: #555; }\n");
        html.push_str("tr:hover { background: #f8f9fa; }\n");
        html.push_str("a { color: #0066cc; text-decoration: none; }\n");
        html.push_str("a:hover { text-decoration: underline; }\n");
        html.push_str(".icon { margin-right: 8px; }\n");
        html.push_str(".size { color: #666; font-family: monospace; }\n");
        html.push_str(".date { color: #888; font-size: 0.9em; }\n");
        html.push_str("</style>\n</head>\n<body>\n");

        // Heading
        html.push_str(&format!(
            "<h1>Index of {}</h1>\n",
            escape_html(&listing.path)
        ));

        // Table
        html.push_str("<table>\n");
        html.push_str("<thead><tr><th>Name</th><th>Size</th><th>Modified</th></tr></thead>\n");
        html.push_str("<tbody>\n");

        // Parent directory link (if not root)
        if listing.path != "/" {
            let parent = if listing.path.ends_with('/') {
                format!("{}../", listing.path)
            } else {
                format!(
                    "{}/",
                    listing.path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/")
                )
            };
            html.push_str(&format!(
                "<tr><td><span class=\"icon\">\u{1F4C1}</span><a href=\"{}\">../</a></td><td>-</td><td>-</td></tr>\n",
                escape_html(&parent)
            ));
        }

        // Build base path with trailing slash
        let base_path = if listing.path.ends_with('/') {
            listing.path.clone()
        } else {
            format!("{}/", listing.path)
        };

        // Directories
        for dir in &listing.directories {
            let href = format!("{}{}/", base_path, encode_uri_component(&dir.name));
            html.push_str(&format!(
                "<tr><td><span class=\"icon\">\u{1F4C1}</span><a href=\"{}\">{}/</a></td><td>-</td><td class=\"date\">{}</td></tr>\n",
                escape_html(&href),
                escape_html(&dir.name),
                format_timestamp(dir.modified)
            ));
        }

        // Files
        for file in &listing.files {
            let href = format!("{}{}", base_path, encode_uri_component(&file.name));
            let size_str = file
                .size
                .map(format_size)
                .unwrap_or_else(|| "-".to_string());
            html.push_str(&format!(
                "<tr><td><span class=\"icon\">\u{1F4C4}</span><a href=\"{}\">{}</a></td><td class=\"size\">{}</td><td class=\"date\">{}</td></tr>\n",
                escape_html(&href),
                escape_html(&file.name),
                size_str,
                format_timestamp(file.modified)
            ));
        }

        html.push_str("</tbody>\n</table>\n");

        // Footer
        html.push_str("<p style=\"color: #888; font-size: 0.9em; margin-top: 20px;\">");
        html.push_str("Highper Gateway</p>\n");
        html.push_str("</body>\n</html>");

        html
    }

    /// Get MIME type for file
    pub fn get_mime_type(&self, path: &Path) -> String {
        self.mime_cache.get_mime_type(path)
    }

    /// Generate ETag for file
    pub fn generate_etag(&self, metadata: &Metadata) -> String {
        use std::time::UNIX_EPOCH;

        // Simple ETag: mtime-size
        let mtime = metadata
            .modified()
            .unwrap_or(UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let size = metadata.len();

        format!("\"{:x}-{:x}\"", mtime, size)
    }

    /// Check if file is compressible
    pub fn is_compressible(&self, path: &Path) -> bool {
        let mime = self.get_mime_type(path);
        super::mime::is_compressible(&mime)
    }

    /// Serve file using zero-copy sendfile
    #[cfg(target_os = "linux")]
    pub fn serve_file_sendfile<S: std::os::unix::io::AsRawFd>(
        &self,
        socket: &S,
        file: &File,
        offset: u64,
        count: usize,
    ) -> io::Result<usize> {
        sendfile_ktls(socket, file, offset, count)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn serve_file_sendfile<S>(
        &self,
        _socket: &S,
        _file: &File,
        _offset: u64,
        _count: usize,
    ) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "sendfile is only supported on Linux",
        ))
    }
}

/// File information
pub struct FileInfo {
    pub path: PathBuf,
    pub metadata: Metadata,
    pub is_php: bool,
    pub is_directory: bool,
}

/// Directory listing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryListing {
    /// Request path
    pub path: String,
    /// Subdirectories
    pub directories: Vec<DirectoryEntry>,
    /// Files in directory
    pub files: Vec<DirectoryEntry>,
}

/// Directory entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    /// File or directory name
    pub name: String,
    /// File size in bytes (None for directories)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,
    /// Is this a directory
    pub is_directory: bool,
}

/// Directory listing response
pub struct DirectoryListingResponse {
    /// Content body
    pub content: String,
    /// Content-Type header value
    pub content_type: String,
}

// Helper functions for directory listing

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Encode URI component
fn encode_uri_component(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format timestamp for display
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, SystemTime};

    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp);

    // Simple ISO 8601-like format
    if let Ok(duration) = time.duration_since(SystemTime::UNIX_EPOCH) {
        let secs = duration.as_secs();
        let days_since_epoch = secs / 86400;
        let secs_of_day = secs % 86400;

        // Very simple date calculation (not accounting for leap years accurately)
        let year = 1970 + (days_since_epoch / 365);
        let day_of_year = days_since_epoch % 365;
        let month = (day_of_year / 30) + 1;
        let day = (day_of_year % 30) + 1;

        let hour = secs_of_day / 3600;
        let minute = (secs_of_day % 3600) / 60;

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            year,
            month.min(12),
            day.min(31),
            hour,
            minute
        )
    } else {
        "-".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_path_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let config = WebServerConfig::default();
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        // Create test file
        let test_file = temp_dir.path().join("test.html");
        fs::write(&test_file, "test").unwrap();

        let resolved = handler.resolve_path("/test.html").unwrap();
        assert!(resolved.exists());
    }

    #[test]
    fn test_directory_traversal_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let config = WebServerConfig::default();
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        // Attempt directory traversal
        let result = handler.resolve_path("/../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_mime_type_detection() {
        let temp_dir = TempDir::new().unwrap();
        let config = WebServerConfig::default();
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        let html_path = Path::new("test.html");
        assert_eq!(handler.get_mime_type(html_path), "text/html; charset=utf-8");

        let css_path = Path::new("style.css");
        assert_eq!(handler.get_mime_type(css_path), "text/css; charset=utf-8");
    }

    #[test]
    fn test_etag_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WebServerConfig::default();
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let metadata = fs::metadata(&test_file).unwrap();
        let etag = handler.generate_etag(&metadata);

        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert!(etag.contains('-'));
    }

    #[test]
    fn test_directory_listing_enabled() {
        let temp_dir = TempDir::new().unwrap();

        // Create test directory structure
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.html"), "content2").unwrap();
        fs::write(subdir.join("nested.txt"), "nested content").unwrap();

        let mut config = WebServerConfig::default();
        config.directory_listing = true;
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        // Get directory info - should return is_directory: true
        let info = handler.get_file_info(temp_dir.path()).unwrap();
        assert!(info.is_directory);

        // Generate listing
        let listing = handler
            .generate_directory_listing(temp_dir.path(), "/")
            .unwrap();
        assert_eq!(listing.directories.len(), 1);
        assert_eq!(listing.directories[0].name, "subdir");
        assert_eq!(listing.files.len(), 2);
    }

    #[test]
    fn test_directory_listing_html_format() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let mut config = WebServerConfig::default();
        config.directory_listing = true;
        config.directory_listing_format = DirectoryListingFormat::Html;
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        let listing = handler
            .generate_directory_listing(temp_dir.path(), "/")
            .unwrap();
        let response = handler.render_directory_listing(&listing);

        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(response.content.contains("<!DOCTYPE html>"));
        assert!(response.content.contains("Index of /"));
        assert!(response.content.contains("test.txt"));
    }

    #[test]
    fn test_directory_listing_json_format() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let mut config = WebServerConfig::default();
        config.directory_listing = true;
        config.directory_listing_format = DirectoryListingFormat::Json;
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        let listing = handler
            .generate_directory_listing(temp_dir.path(), "/test")
            .unwrap();
        let response = handler.render_directory_listing(&listing);

        assert_eq!(response.content_type, "application/json");

        let parsed: DirectoryListing = serde_json::from_str(&response.content).unwrap();
        assert_eq!(parsed.path, "/test");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].name, "test.txt");
    }

    #[test]
    fn test_directory_listing_hidden_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".hidden"), "hidden").unwrap();
        fs::write(temp_dir.path().join("visible.txt"), "visible").unwrap();

        // Without show_hidden_files
        let mut config = WebServerConfig::default();
        config.directory_listing = true;
        config.show_hidden_files = false;
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        let listing = handler
            .generate_directory_listing(temp_dir.path(), "/")
            .unwrap();
        assert_eq!(listing.files.len(), 1);
        assert_eq!(listing.files[0].name, "visible.txt");

        // With show_hidden_files
        let mut config2 = WebServerConfig::default();
        config2.directory_listing = true;
        config2.show_hidden_files = true;
        let handler2 = StaticFileHandler::new(temp_dir.path().to_path_buf(), config2);

        let listing2 = handler2
            .generate_directory_listing(temp_dir.path(), "/")
            .unwrap();
        assert_eq!(listing2.files.len(), 2);
    }

    #[test]
    fn test_directory_listing_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let config = WebServerConfig::default(); // directory_listing is false by default
        let handler = StaticFileHandler::new(temp_dir.path().to_path_buf(), config);

        // Should fail when trying to get directory info without index file
        let result = handler.get_file_info(&subdir);
        assert!(result.is_err());
    }

    #[test]
    fn test_helper_functions() {
        // Test escape_html
        assert_eq!(
            escape_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"
        );

        // Test format_size
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");

        // Test encode_uri_component
        assert!(encode_uri_component("hello world").contains("%20"));
    }
}
