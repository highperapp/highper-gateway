//! Static file serving with zero-copy sendfile

use super::{MimeCache, WebServerConfig};
use std::fs::{File, Metadata};
use std::io;
use std::path::{Path, PathBuf};

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
                "Path traversal attempt detected"
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
                    });
                }
            }

            // No index file found
            if self.config.directory_listing {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Directory listing not yet implemented"
                ));
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Directory listing disabled"
                ));
            }
        }

        Ok(FileInfo {
            path: path.to_path_buf(),
            metadata,
            is_php: path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "php")
                .unwrap_or(false),
        })
    }

    /// Get MIME type for file
    pub fn get_mime_type(&self, path: &Path) -> String {
        self.mime_cache.get_mime_type(path)
    }

    /// Generate ETag for file
    pub fn generate_etag(&self, metadata: &Metadata) -> String {
        use std::time::UNIX_EPOCH;

        // Simple ETag: mtime-size
        let mtime = metadata.modified()
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
            "sendfile is only supported on Linux"
        ))
    }
}

/// File information
pub struct FileInfo {
    pub path: PathBuf,
    pub metadata: Metadata,
    pub is_php: bool,
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
}
