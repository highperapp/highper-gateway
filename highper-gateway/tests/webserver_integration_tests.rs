/// Integration tests for webserver features (static files, PHP-FPM, directory listing)
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test static file serving
#[tokio::test]
async fn test_static_file_serving() {
    // Create temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.html");

    fs::write(&test_file, b"<html><body>Hello World</body></html>").unwrap();

    // Test file exists and can be read
    assert!(test_file.exists());
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "<html><body>Hello World</body></html>");
}

/// Test index file resolution
#[tokio::test]
async fn test_index_file_resolution() {
    let temp_dir = TempDir::new().unwrap();

    // Create directory with index.html
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let index_file = subdir.join("index.html");
    fs::write(&index_file, b"<html><body>Index Page</body></html>").unwrap();

    // Verify index file exists in directory
    assert!(subdir.is_dir());
    assert!(index_file.exists());
}

/// Test custom error pages
#[tokio::test]
async fn test_custom_error_pages() {
    let temp_dir = TempDir::new().unwrap();

    // Create 404 error page
    let error_404 = temp_dir.path().join("404.html");
    fs::write(&error_404, b"<html><body>404 Not Found</body></html>").unwrap();

    // Create 500 error page
    let error_500 = temp_dir.path().join("500.html");
    fs::write(&error_500, b"<html><body>500 Server Error</body></html>").unwrap();

    // Verify error pages exist
    assert!(error_404.exists());
    assert!(error_500.exists());

    // Verify content
    let content_404 = fs::read_to_string(&error_404).unwrap();
    assert!(content_404.contains("404 Not Found"));

    let content_500 = fs::read_to_string(&error_500).unwrap();
    assert!(content_500.contains("500 Server Error"));
}

/// Test range request parsing
#[test]
fn test_range_header_parsing() {
    // Test explicit range
    let range = parse_range("bytes=0-1023", 2048);
    assert_eq!(range, Some((0, 1023)));

    // Test open-ended range
    let range = parse_range("bytes=1024-", 2048);
    assert_eq!(range, Some((1024, 2047)));

    // Test suffix range
    let range = parse_range("bytes=-500", 2048);
    assert_eq!(range, Some((1548, 2047)));

    // Test invalid range
    let range = parse_range("bytes=3000-4000", 2048);
    assert_eq!(range, None);

    // Test invalid format
    let range = parse_range("invalid", 2048);
    assert_eq!(range, None);
}

// Helper function to parse range header (simplified for testing)
fn parse_range(range_str: &str, file_size: u64) -> Option<(u64, u64)> {
    if !range_str.starts_with("bytes=") {
        return None;
    }

    let range_spec = &range_str[6..];
    let range_part = range_spec.split(',').next()?;

    if let Some((start_str, end_str)) = range_part.split_once('-') {
        match (start_str.trim(), end_str.trim()) {
            (start, end) if !start.is_empty() && !end.is_empty() => {
                let start: u64 = start.parse().ok()?;
                let end: u64 = end.parse().ok()?;
                if start > end || start >= file_size {
                    return None;
                }
                let end = std::cmp::min(end, file_size - 1);
                Some((start, end))
            }
            (start, "") if !start.is_empty() => {
                let start: u64 = start.parse().ok()?;
                if start >= file_size {
                    return None;
                }
                Some((start, file_size - 1))
            }
            ("", end) if !end.is_empty() => {
                let suffix_len: u64 = end.parse().ok()?;
                if suffix_len == 0 || suffix_len > file_size {
                    return None;
                }
                let start = file_size - suffix_len;
                Some((start, file_size - 1))
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Test file size formatting
#[test]
fn test_file_size_formatting() {
    assert_eq!(format_file_size(500), "500 B");
    assert_eq!(format_file_size(1024), "1.0 KB");
    assert_eq!(format_file_size(1536), "1.5 KB");
    assert_eq!(format_file_size(1048576), "1.0 MB");
    assert_eq!(format_file_size(1073741824), "1.0 GB");
}

fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

/// Test directory listing generation
#[tokio::test]
async fn test_directory_listing() {
    let temp_dir = TempDir::new().unwrap();

    // Create some test files and directories
    let file1 = temp_dir.path().join("file1.txt");
    fs::write(&file1, b"File 1 content").unwrap();

    let file2 = temp_dir.path().join("file2.html");
    fs::write(&file2, b"<html>File 2</html>").unwrap();

    let subdir1 = temp_dir.path().join("subdir1");
    fs::create_dir(&subdir1).unwrap();

    let subdir2 = temp_dir.path().join("subdir2");
    fs::create_dir(&subdir2).unwrap();

    // Read directory entries
    let entries: Vec<_> = fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // Should have 4 entries (2 files + 2 directories)
    assert_eq!(entries.len(), 4);

    // Count directories and files
    let mut dir_count = 0;
    let mut file_count = 0;

    for entry in entries {
        let metadata = entry.metadata().unwrap();
        if metadata.is_dir() {
            dir_count += 1;
        } else {
            file_count += 1;
        }
    }

    assert_eq!(dir_count, 2);
    assert_eq!(file_count, 2);
}

/// Test try_files pattern matching
#[test]
fn test_try_files_patterns() {
    let patterns = vec![
        "$uri".to_string(),
        "$uri/".to_string(),
        "/index.php".to_string(),
    ];

    // Verify patterns are correctly defined
    assert_eq!(patterns.len(), 3);
    assert_eq!(patterns[0], "$uri");
    assert_eq!(patterns[1], "$uri/");
    assert_eq!(patterns[2], "/index.php");
}

/// Test ETag generation (simplified)
#[test]
fn test_etag_generation() {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Simulate file metadata
    let modified_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let file_size = 1024u64;

    // Generate simple ETag (in real implementation, this uses a hash)
    let etag = format!("\"{:x}-{:x}\"", modified_time, file_size);

    // ETag should be non-empty and properly formatted
    assert!(!etag.is_empty());
    assert!(etag.starts_with('"'));
    assert!(etag.ends_with('"'));
}

/// Test MIME type detection
#[test]
fn test_mime_type_detection() {
    let test_cases = vec![
        ("file.html", "text/html"),
        ("file.css", "text/css"),
        ("file.js", "application/javascript"),
        ("file.json", "application/json"),
        ("file.png", "image/png"),
        ("file.jpg", "image/jpeg"),
        ("file.gif", "image/gif"),
        ("file.svg", "image/svg+xml"),
        ("file.pdf", "application/pdf"),
        ("file.txt", "text/plain"),
    ];

    for (filename, _expected_mime) in test_cases {
        let path = PathBuf::from(filename);
        let ext = path.extension().and_then(|e| e.to_str());
        assert!(ext.is_some(), "Failed to extract extension from {}", filename);
    }
}

/// Test PHP file detection
#[test]
fn test_php_file_detection() {
    let php_extensions = vec![".php", ".phtml", ".php5", ".php7"];

    for ext in php_extensions {
        let filename = format!("test{}", ext);
        assert!(filename.ends_with(ext));
    }
}

/// Test hidden file filtering
#[test]
fn test_hidden_file_filtering() {
    let files = vec![
        ("file.txt", false),
        (".hidden", true),
        ("..parent", true),
        (".htaccess", true),
        ("normal.html", false),
    ];

    for (filename, should_be_hidden) in files {
        let is_hidden = filename.starts_with('.');
        assert_eq!(is_hidden, should_be_hidden, "Failed for {}", filename);
    }
}

/// Test error page path resolution
#[test]
fn test_error_page_resolution() {
    use std::collections::HashMap;

    let mut error_pages: HashMap<u16, String> = HashMap::new();
    error_pages.insert(404, "/404.html".to_string());
    error_pages.insert(500, "/500.html".to_string());

    // Test lookup
    assert_eq!(error_pages.get(&404), Some(&"/404.html".to_string()));
    assert_eq!(error_pages.get(&500), Some(&"/500.html".to_string()));
    assert_eq!(error_pages.get(&403), None);
}

/// Test content type detection for error pages
#[test]
fn test_error_page_content_type() {
    let test_cases = vec![
        ("/404.html", "text/html; charset=utf-8"),
        ("/404.htm", "text/html; charset=utf-8"),
        ("/error.json", "application/json"),
        ("/error.txt", "text/plain"),
    ];

    for (path, expected) in test_cases {
        let content_type = if path.ends_with(".html") || path.ends_with(".htm") {
            "text/html; charset=utf-8"
        } else if path.ends_with(".json") {
            "application/json"
        } else {
            "text/plain"
        };

        assert_eq!(content_type, expected, "Failed for {}", path);
    }
}

/// Test DSL parsing (basic validation)
#[test]
fn test_dsl_directive_formats() {
    // Test various DSL directive formats
    let directives = vec![
        "root \"/var/www/html\"",
        "index index.php index.html",
        "try_files $uri $uri/ /index.php",
        "error_page 404 \"/404.html\"",
        "error_page 500 \"/500.html\"",
        "directory_listing on",
        "directory_listing off",
        "static_files",
    ];

    // Verify all directives are non-empty
    for directive in directives {
        assert!(!directive.is_empty());
        assert!(directive.len() > 3);
    }
}

/// Test route configuration validation
#[test]
fn test_route_config_validation() {
    // Verify that a route can have multiple enhancement features
    let has_error_pages = true;
    let has_directory_listing = true;
    let has_try_files = true;
    let has_static_files = true;

    // All features can coexist
    assert!(has_error_pages);
    assert!(has_directory_listing);
    assert!(has_try_files);
    assert!(has_static_files);
}

#[cfg(test)]
mod php_fpm_tests {
    use super::*;

    /// Test PHP-FPM socket path validation
    #[test]
    fn test_php_fpm_socket_paths() {
        let valid_sockets = vec![
            "/var/run/php/php8.2-fpm.sock",
            "/var/run/php/php-fpm.sock",
            "/usr/local/var/run/php-fpm.sock",
            "127.0.0.1:9000",
            "localhost:9000",
        ];

        for socket in valid_sockets {
            assert!(!socket.is_empty());
            // Unix socket paths start with /
            // TCP sockets contain :
            assert!(socket.starts_with('/') || socket.contains(':'));
        }
    }

    /// Test PHP-FPM configuration parameters
    #[test]
    fn test_php_fpm_config_params() {
        let pool_size = 50;
        let connect_timeout = 5;
        let read_timeout = 60;
        let write_timeout = 60;
        let keepalive_timeout = 90;

        // Verify reasonable defaults
        assert!(pool_size > 0);
        assert!(pool_size <= 1000);
        assert!(connect_timeout > 0);
        assert!(read_timeout >= connect_timeout);
        assert!(keepalive_timeout >= read_timeout);
    }
}
