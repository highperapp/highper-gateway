/// Security validation and hardening for webserver requests
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};

/// Maximum file size for static files (100 MB default)
pub const MAX_STATIC_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum request body size for PHP-FPM (10 MB default)
pub const MAX_PHP_REQUEST_BODY: usize = 10 * 1024 * 1024;

/// Maximum path depth to prevent deep directory traversal
pub const MAX_PATH_DEPTH: usize = 32;

/// Dangerous file extensions that should never be served as static files
const DANGEROUS_EXTENSIONS: &[&str] = &[
    ".sh", ".bash", ".zsh", ".fish",  // Shell scripts
    ".exe", ".dll", ".so", ".dylib",   // Executables
    ".bat", ".cmd", ".ps1",            // Windows scripts
    ".py", ".rb", ".pl", ".lua",       // Script languages (unless explicitly PHP)
];

/// Sensitive filenames that should be blocked
const SENSITIVE_FILES: &[&str] = &[
    ".env", ".env.local", ".env.production",
    ".git", ".gitignore", ".gitconfig",
    ".htaccess", ".htpasswd",
    "id_rsa", "id_dsa", "id_ecdsa", "id_ed25519",
    ".ssh", "authorized_keys", "known_hosts",
    "web.config", "app.config",
    ".npmrc", ".yarnrc",
    "composer.json", "composer.lock",
];

/// Path security validator
pub struct PathValidator {
    document_root: PathBuf,
}

impl PathValidator {
    /// Create a new path validator with the given document root
    pub fn new(document_root: impl AsRef<Path>) -> Self {
        Self {
            document_root: document_root.as_ref().to_path_buf(),
        }
    }

    /// Validate a request path for security issues
    pub fn validate_path(&self, request_path: &str) -> Result<PathBuf> {
        // Decode URL encoding
        let decoded_path = percent_encoding::percent_decode_str(request_path)
            .decode_utf8()
            .map_err(|e| anyhow!("Invalid UTF-8 in path: {}", e))?;

        // Check for null bytes (path traversal attack)
        if decoded_path.contains('\0') {
            return Err(anyhow!("Null byte detected in path"));
        }

        // Remove leading slash
        let clean_path = decoded_path.trim_start_matches('/');

        // Build full path
        let full_path = self.document_root.join(clean_path);

        // Canonicalize to resolve .. and . components
        let canonical_path = match full_path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Path doesn't exist yet - validate without canonicalization
                self.validate_path_components(clean_path)?;
                return Ok(full_path);
            }
            Err(e) => return Err(anyhow!("Path canonicalization failed: {}", e)),
        };

        // Ensure the canonical path is still within document root
        let canonical_root = self.document_root.canonicalize()
            .map_err(|e| anyhow!("Document root canonicalization failed: {}", e))?;

        if !canonical_path.starts_with(&canonical_root) {
            return Err(anyhow!("Path traversal attempt detected: path outside document root"));
        }

        // Check path depth
        if canonical_path.components().count() > MAX_PATH_DEPTH {
            return Err(anyhow!("Path depth exceeds maximum allowed"));
        }

        Ok(canonical_path)
    }

    /// Validate path components without file system access
    fn validate_path_components(&self, path: &str) -> Result<()> {
        // Check for directory traversal patterns
        if path.contains("..") {
            return Err(anyhow!("Directory traversal pattern detected"));
        }

        // Check for absolute paths (shouldn't happen after trim, but be safe)
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(anyhow!("Absolute path not allowed"));
        }

        // Check path depth
        let depth = path.split('/').count();
        if depth > MAX_PATH_DEPTH {
            return Err(anyhow!("Path depth exceeds maximum allowed"));
        }

        Ok(())
    }

    /// Check if a file should be blocked from being served
    pub fn is_file_allowed(&self, file_path: &Path) -> Result<()> {
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid file name"))?;

        // Check for sensitive files
        for sensitive in SENSITIVE_FILES {
            if file_name.eq_ignore_ascii_case(sensitive) {
                return Err(anyhow!("Access to sensitive file '{}' is forbidden", file_name));
            }
        }

        // Check for dangerous extensions (unless it's a PHP file which is handled separately)
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext);
            for dangerous in DANGEROUS_EXTENSIONS {
                if ext_with_dot.eq_ignore_ascii_case(dangerous) {
                    return Err(anyhow!("Files with extension '{}' cannot be served directly", ext_with_dot));
                }
            }
        }

        // Check for hidden files (starting with .)
        if file_name.starts_with('.') && file_name != "." && file_name != ".." {
            return Err(anyhow!("Access to hidden files is forbidden"));
        }

        Ok(())
    }

    /// Validate file size before serving
    pub fn validate_file_size(&self, size: u64) -> Result<()> {
        if size > MAX_STATIC_FILE_SIZE {
            return Err(anyhow!("File size ({} bytes) exceeds maximum allowed ({} bytes)",
                size, MAX_STATIC_FILE_SIZE));
        }
        Ok(())
    }

    /// Validate PHP request body size
    pub fn validate_request_body_size(&self, size: usize) -> Result<()> {
        if size > MAX_PHP_REQUEST_BODY {
            return Err(anyhow!("Request body size ({} bytes) exceeds maximum allowed ({} bytes)",
                size, MAX_PHP_REQUEST_BODY));
        }
        Ok(())
    }
}

/// Rate limiter for file access (prevent abuse)
pub struct FileAccessLimiter {
    // Could use a DashMap<PathBuf, AtomicU64> for per-file access tracking
    // For now, this is a placeholder for future enhancement
}

impl FileAccessLimiter {
    pub fn new() -> Self {
        Self {}
    }

    /// Check if file access should be allowed
    pub fn check_access(&self, _path: &Path) -> Result<()> {
        // TODO: Implement per-file rate limiting
        // For now, always allow
        Ok(())
    }
}

/// Validate PHP script path
pub fn validate_php_script(script_path: &Path) -> Result<()> {
    // Check if file exists
    if !script_path.exists() {
        return Err(anyhow!("PHP script not found: {:?}", script_path));
    }

    // Check if it's a file
    if !script_path.is_file() {
        return Err(anyhow!("PHP script path is not a file: {:?}", script_path));
    }

    // Check if it has a PHP extension
    let has_php_ext = script_path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(ext.to_lowercase().as_str(), "php" | "phtml" | "php5" | "php7" | "php8")
        })
        .unwrap_or(false);

    if !has_php_ext {
        return Err(anyhow!("File does not have a valid PHP extension: {:?}", script_path));
    }

    Ok(())
}

/// Sanitize FastCGI parameters to prevent injection
pub fn sanitize_fastcgi_param(value: &str) -> String {
    // Remove null bytes and control characters
    value.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_path_traversal_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test directory traversal attempts
        assert!(validator.validate_path("../etc/passwd").is_err());
        assert!(validator.validate_path("../../secret").is_err());
        assert!(validator.validate_path("./../../etc").is_err());

        // Test valid paths
        assert!(validator.validate_path("index.html").is_ok());
        assert!(validator.validate_path("assets/css/style.css").is_ok());
    }

    #[test]
    fn test_null_byte_detection() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Null byte in path
        assert!(validator.validate_path("index.html\0.php").is_err());
    }

    #[test]
    fn test_sensitive_file_blocking() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test sensitive files
        assert!(validator.is_file_allowed(Path::new(".env")).is_err());
        assert!(validator.is_file_allowed(Path::new(".htaccess")).is_err());
        assert!(validator.is_file_allowed(Path::new("id_rsa")).is_err());

        // Test allowed files
        assert!(validator.is_file_allowed(Path::new("index.html")).is_ok());
        assert!(validator.is_file_allowed(Path::new("style.css")).is_ok());
    }

    #[test]
    fn test_dangerous_extension_blocking() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test dangerous extensions
        assert!(validator.is_file_allowed(Path::new("script.sh")).is_err());
        assert!(validator.is_file_allowed(Path::new("program.exe")).is_err());
        assert!(validator.is_file_allowed(Path::new("library.dll")).is_err());

        // Test allowed extensions
        assert!(validator.is_file_allowed(Path::new("index.html")).is_ok());
        assert!(validator.is_file_allowed(Path::new("script.js")).is_ok());
    }

    #[test]
    fn test_hidden_file_blocking() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test hidden files
        assert!(validator.is_file_allowed(Path::new(".hidden")).is_err());
        assert!(validator.is_file_allowed(Path::new(".secret")).is_err());

        // Test normal files
        assert!(validator.is_file_allowed(Path::new("visible.txt")).is_ok());
    }

    #[test]
    fn test_file_size_validation() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test valid size
        assert!(validator.validate_file_size(1024).is_ok());
        assert!(validator.validate_file_size(10 * 1024 * 1024).is_ok());

        // Test oversized file
        assert!(validator.validate_file_size(200 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_request_body_size_validation() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Test valid size
        assert!(validator.validate_request_body_size(1024).is_ok());
        assert!(validator.validate_request_body_size(1 * 1024 * 1024).is_ok());

        // Test oversized body
        assert!(validator.validate_request_body_size(20 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_path_depth_limit() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::new(temp_dir.path());

        // Create a very deep path
        let deep_path = (0..40).map(|i| format!("dir{}", i)).collect::<Vec<_>>().join("/");
        assert!(validator.validate_path(&deep_path).is_err());

        // Normal depth should be ok
        let normal_path = "a/b/c/d/index.html";
        assert!(validator.validate_path(normal_path).is_ok());
    }

    #[test]
    fn test_php_script_validation() {
        let temp_dir = TempDir::new().unwrap();
        let php_file = temp_dir.path().join("index.php");
        fs::write(&php_file, "<?php echo 'test'; ?>").unwrap();

        // Valid PHP file
        assert!(validate_php_script(&php_file).is_ok());

        // Non-existent file
        assert!(validate_php_script(Path::new("/nonexistent.php")).is_err());

        // Non-PHP file
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "test").unwrap();
        assert!(validate_php_script(&txt_file).is_err());
    }

    #[test]
    fn test_sanitize_fastcgi_param() {
        // Test null byte removal
        assert_eq!(sanitize_fastcgi_param("test\0value"), "testvalue");

        // Test control character removal (except newline, tab, CR)
        assert_eq!(sanitize_fastcgi_param("test\x01\x02value"), "testvalue");
        assert_eq!(sanitize_fastcgi_param("test\nvalue"), "test\nvalue");
        assert_eq!(sanitize_fastcgi_param("test\tvalue"), "test\tvalue");
    }
}
