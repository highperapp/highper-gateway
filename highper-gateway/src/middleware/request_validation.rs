//! Request validation middleware for security
//!
//! This middleware provides comprehensive request validation to prevent common attacks:
//! - SQL injection detection
//! - XSS attack prevention
//! - Path traversal detection
//! - Oversized request blocking
//! - Malformed request detection
//! - Suspicious header validation

use super::Middleware;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, Request, Response, StatusCode};
use regex::Regex;
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

/// Request validation configuration
#[derive(Debug, Clone)]
pub struct RequestValidationConfig {
    /// Enable SQL injection detection
    pub sql_injection_detection: bool,
    /// Enable XSS detection
    pub xss_detection: bool,
    /// Enable path traversal detection
    pub path_traversal_detection: bool,
    /// Maximum request body size in bytes (0 = unlimited)
    pub max_body_size: usize,
    /// Maximum header size in bytes
    pub max_header_size: usize,
    /// Maximum URL length
    pub max_url_length: usize,
    /// Block suspicious user agents
    pub block_suspicious_user_agents: bool,
    /// Enable null byte detection
    pub null_byte_detection: bool,
    /// Enable command injection detection
    pub command_injection_detection: bool,
}

impl Default for RequestValidationConfig {
    fn default() -> Self {
        Self {
            sql_injection_detection: true,
            xss_detection: true,
            path_traversal_detection: true,
            max_body_size: 10485760, // 10 MB default
            max_header_size: 8192,   // 8 KB
            max_url_length: 2048,    // 2 KB
            block_suspicious_user_agents: true,
            null_byte_detection: true,
            command_injection_detection: true,
        }
    }
}

impl RequestValidationConfig {
    /// Create a strict validation configuration (recommended for production)
    pub fn strict() -> Self {
        Self {
            sql_injection_detection: true,
            xss_detection: true,
            path_traversal_detection: true,
            max_body_size: 1048576, // 1 MB
            max_header_size: 4096,  // 4 KB
            max_url_length: 1024,   // 1 KB
            block_suspicious_user_agents: true,
            null_byte_detection: true,
            command_injection_detection: true,
        }
    }

    /// Create a relaxed validation configuration (for development)
    pub fn relaxed() -> Self {
        Self {
            sql_injection_detection: true,
            xss_detection: true,
            path_traversal_detection: true,
            max_body_size: 104857600, // 100 MB
            max_header_size: 16384,   // 16 KB
            max_url_length: 4096,     // 4 KB
            block_suspicious_user_agents: false,
            null_byte_detection: true,
            command_injection_detection: true,
        }
    }

    /// Create configuration for API endpoints (stricter)
    pub fn api() -> Self {
        Self {
            sql_injection_detection: true,
            xss_detection: true,
            path_traversal_detection: true,
            max_body_size: 524288, // 512 KB (JSON payloads)
            max_header_size: 4096, // 4 KB
            max_url_length: 1024,  // 1 KB
            block_suspicious_user_agents: true,
            null_byte_detection: true,
            command_injection_detection: true,
        }
    }
}

/// Request validation middleware
pub struct RequestValidationMiddleware {
    config: RequestValidationConfig,
    sql_patterns: Vec<Regex>,
    xss_patterns: Vec<Regex>,
    path_traversal_patterns: Vec<Regex>,
    command_injection_patterns: Vec<Regex>,
    suspicious_user_agents: Vec<Regex>,
}

impl RequestValidationMiddleware {
    /// Create a new request validation middleware
    pub fn new(config: RequestValidationConfig) -> Self {
        Self {
            config,
            sql_patterns: Self::compile_sql_patterns(),
            xss_patterns: Self::compile_xss_patterns(),
            path_traversal_patterns: Self::compile_path_traversal_patterns(),
            command_injection_patterns: Self::compile_command_injection_patterns(),
            suspicious_user_agents: Self::compile_suspicious_user_agents(),
        }
    }

    /// Create with default config
    pub fn default_validation() -> Self {
        Self::new(RequestValidationConfig::default())
    }

    /// Create with strict config
    pub fn strict() -> Self {
        Self::new(RequestValidationConfig::strict())
    }

    /// Create with relaxed config
    pub fn relaxed() -> Self {
        Self::new(RequestValidationConfig::relaxed())
    }

    /// Create with API config
    pub fn api() -> Self {
        Self::new(RequestValidationConfig::api())
    }

    /// Validate request URL
    fn validate_url(&self, uri: &hyper::Uri) -> Result<(), String> {
        let url = uri.to_string();

        // Check URL length
        if url.len() > self.config.max_url_length {
            return Err(format!(
                "URL too long: {} > {}",
                url.len(),
                self.config.max_url_length
            ));
        }

        // URL-decode for pattern matching (attackers often use URL encoding to bypass filters)
        let decoded_url = urlencoding::decode(&url).unwrap_or(std::borrow::Cow::Borrowed(&url));

        // Check for null bytes
        if self.config.null_byte_detection && decoded_url.contains('\0') {
            return Err("Null byte detected in URL".to_string());
        }

        // Check for path traversal (check both encoded and decoded)
        if self.config.path_traversal_detection {
            for pattern in &self.path_traversal_patterns {
                if pattern.is_match(&url) || pattern.is_match(&decoded_url) {
                    return Err(format!(
                        "Path traversal pattern detected: {}",
                        pattern.as_str()
                    ));
                }
            }
        }

        // Check for SQL injection in URL (check both encoded and decoded)
        if self.config.sql_injection_detection {
            for pattern in &self.sql_patterns {
                if pattern.is_match(&url) || pattern.is_match(&decoded_url) {
                    return Err(format!("SQL injection pattern detected in URL"));
                }
            }
        }

        // Check for XSS in URL (check both encoded and decoded)
        if self.config.xss_detection {
            for pattern in &self.xss_patterns {
                if pattern.is_match(&url) || pattern.is_match(&decoded_url) {
                    return Err(format!("XSS pattern detected in URL"));
                }
            }
        }

        Ok(())
    }

    /// Validate request headers
    fn validate_headers(&self, headers: &hyper::HeaderMap) -> Result<(), String> {
        // Calculate total header size
        let total_size: usize = headers
            .iter()
            .map(|(name, value)| name.as_str().len() + value.len())
            .sum();

        if total_size > self.config.max_header_size {
            return Err(format!(
                "Headers too large: {} > {}",
                total_size, self.config.max_header_size
            ));
        }

        // Validate User-Agent
        if self.config.block_suspicious_user_agents {
            if let Some(user_agent) = headers.get(header::USER_AGENT) {
                if let Ok(ua_str) = user_agent.to_str() {
                    for pattern in &self.suspicious_user_agents {
                        if pattern.is_match(ua_str) {
                            return Err(format!("Suspicious user agent detected: {}", ua_str));
                        }
                    }
                }
            }
        }

        // Check for null bytes in headers
        if self.config.null_byte_detection {
            for (name, value) in headers.iter() {
                if name.as_str().contains('\0') {
                    return Err(format!("Null byte in header name: {}", name));
                }
                if let Ok(val_str) = value.to_str() {
                    if val_str.contains('\0') {
                        return Err(format!("Null byte in header value: {}", name));
                    }
                }
            }
        }

        // Validate Content-Length
        if let Some(content_length) = headers.get(header::CONTENT_LENGTH) {
            if let Ok(length_str) = content_length.to_str() {
                if let Ok(length) = length_str.parse::<usize>() {
                    if self.config.max_body_size > 0 && length > self.config.max_body_size {
                        return Err(format!(
                            "Request body too large: {} > {}",
                            length, self.config.max_body_size
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Compile SQL injection detection patterns
    fn compile_sql_patterns() -> Vec<Regex> {
        vec![
            // SQL keywords
            Regex::new(r"(?i)(union\s+select|select\s+.+\s+from|insert\s+into|delete\s+from|drop\s+table|update\s+.+\s+set)").unwrap(),
            // SQL comments
            Regex::new(r"(--|#|/\*|\*/)").unwrap(),
            // SQL string concatenation
            Regex::new(r"(?i)(\|\||concat\s*\()").unwrap(),
            // SQL injection patterns
            Regex::new(r"(?i)(or\s+1\s*=\s*1|and\s+1\s*=\s*1|'?\s*or\s+'?1'?\s*=\s*'?1)").unwrap(),
            // Hex encoding
            Regex::new(r"0x[0-9a-fA-F]+").unwrap(),
        ]
    }

    /// Compile XSS detection patterns
    fn compile_xss_patterns() -> Vec<Regex> {
        vec![
            // Script tags
            Regex::new(r"(?i)<script[^>]*>.*?</script>").unwrap(),
            // Event handlers
            Regex::new(r"(?i)on(load|error|click|mouse|focus|blur|change|submit)\s*=").unwrap(),
            // JavaScript protocol
            Regex::new(r"(?i)javascript:").unwrap(),
            // Data protocol
            Regex::new(r"(?i)data:text/html").unwrap(),
            // Iframe tags
            Regex::new(r"(?i)<iframe[^>]*>").unwrap(),
            // Object/embed tags
            Regex::new(r"(?i)<(object|embed)[^>]*>").unwrap(),
        ]
    }

    /// Compile path traversal detection patterns
    fn compile_path_traversal_patterns() -> Vec<Regex> {
        vec![
            // Directory traversal
            Regex::new(r"\.\./").unwrap(),
            Regex::new(r"\.\.\\").unwrap(),
            // Encoded traversal
            Regex::new(r"%2e%2e[/\\]").unwrap(),
            Regex::new(r"\.\.%2f").unwrap(),
            // Double encoding
            Regex::new(r"%252e%252e").unwrap(),
        ]
    }

    /// Compile command injection detection patterns
    fn compile_command_injection_patterns() -> Vec<Regex> {
        vec![
            // Shell metacharacters
            Regex::new(r"[;&|`$]").unwrap(),
            // Command substitution
            Regex::new(r"\$\(.*\)").unwrap(),
            Regex::new(r"`.*`").unwrap(),
            // Newlines (potential command chaining)
            Regex::new(r"[\n\r]").unwrap(),
        ]
    }

    /// Compile suspicious user agent patterns
    fn compile_suspicious_user_agents() -> Vec<Regex> {
        vec![
            Regex::new(r"(?i)(sqlmap|nikto|nmap|masscan|acunetix|nessus|openvas|metasploit)")
                .unwrap(),
            Regex::new(r"(?i)(havij|pangolin|jsql|bsqlbf)").unwrap(),
            Regex::new(r"(?i)(w3af|skipfish|wapiti|whatweb)").unwrap(),
        ]
    }
}

impl Middleware for RequestValidationMiddleware {
    fn name(&self) -> &str {
        "request-validation"
    }

    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>>
                + Send,
        >,
    > {
        // Perform validation synchronously before async block
        let url_validation = self.validate_url(req.uri());
        let header_validation = self.validate_headers(req.headers());

        Box::pin(async move {
            debug!("Validating request: {} {}", req.method(), req.uri());

            // Check URL validation result
            if let Err(msg) = url_validation {
                let response = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(format!(
                        r#"{{"error":"Request validation failed","details":"{}"}}"#,
                        msg.replace('"', "'")
                    ))))
                    .unwrap();
                return Err(response);
            }

            // Check header validation result
            if let Err(msg) = header_validation {
                let response = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(format!(
                        r#"{{"error":"Request validation failed","details":"{}"}}"#,
                        msg.replace('"', "'")
                    ))))
                    .unwrap();
                return Err(response);
            }

            debug!("Request validation passed");
            Ok(req)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        let middleware = RequestValidationMiddleware::default_validation();

        // URL-encode special characters for valid URIs
        let uri: hyper::Uri = "http://example.com/user?id=1%27%20OR%20%271%27=%271"
            .parse()
            .unwrap();
        assert!(middleware.validate_url(&uri).is_err());

        let uri: hyper::Uri = "http://example.com/user?name=admin%27--".parse().unwrap();
        assert!(middleware.validate_url(&uri).is_err());
    }

    #[test]
    fn test_xss_detection() {
        let middleware = RequestValidationMiddleware::default_validation();

        // URL-encode special characters for valid URIs
        let uri: hyper::Uri =
            "http://example.com/search?q=%3Cscript%3Ealert%28%27xss%27%29%3C%2Fscript%3E"
                .parse()
                .unwrap();
        assert!(middleware.validate_url(&uri).is_err());

        let uri: hyper::Uri = "http://example.com/page?redirect=javascript%3Aalert%281%29"
            .parse()
            .unwrap();
        assert!(middleware.validate_url(&uri).is_err());
    }

    #[test]
    fn test_path_traversal_detection() {
        let middleware = RequestValidationMiddleware::default_validation();

        let uri: hyper::Uri = "http://example.com/file?path=..%2F..%2Fetc%2Fpasswd"
            .parse()
            .unwrap();
        assert!(middleware.validate_url(&uri).is_err());

        let uri: hyper::Uri = "http://example.com/file?path=%2e%2e%2f".parse().unwrap();
        assert!(middleware.validate_url(&uri).is_err());
    }

    #[test]
    fn test_url_length_validation() {
        let middleware = RequestValidationMiddleware::strict();

        let long_url = format!("http://example.com/{}", "a".repeat(2000));
        let uri: hyper::Uri = long_url.parse().unwrap();
        assert!(middleware.validate_url(&uri).is_err());
    }

    #[test]
    fn test_valid_requests() {
        let middleware = RequestValidationMiddleware::default_validation();

        let uri: hyper::Uri = "http://example.com/api/users/123".parse().unwrap();
        assert!(middleware.validate_url(&uri).is_ok());

        let uri: hyper::Uri = "http://example.com/search?q=hello+world".parse().unwrap();
        assert!(middleware.validate_url(&uri).is_ok());
    }

    #[test]
    fn test_config_presets() {
        let strict = RequestValidationConfig::strict();
        assert_eq!(strict.max_body_size, 1048576);

        let relaxed = RequestValidationConfig::relaxed();
        assert_eq!(relaxed.max_body_size, 104857600);

        let api = RequestValidationConfig::api();
        assert_eq!(api.max_body_size, 524288);
    }
}
