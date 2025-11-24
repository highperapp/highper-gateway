//! Request/Response logging with correlation IDs
//!
//! Provides structured logging for HTTP requests and responses with:
//! - Correlation ID generation and propagation
//! - Request/response metadata capture
//! - Integration with distributed tracing
//! - Configurable log levels and formats

use hyper::{HeaderMap, Method, StatusCode, Uri, Version};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Correlation ID for request tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID using UUID v7 (timestamp-based, sortable)
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    /// Create from existing ID string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract correlation ID from request headers
    ///
    /// Checks multiple common header names:
    /// - X-Request-ID
    /// - X-Correlation-ID
    /// - X-Request-Id (case variation)
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        const CORRELATION_HEADERS: &[&str] = &[
            "x-request-id",
            "x-correlation-id",
            "x-request-id",
            "request-id",
        ];

        for header_name in CORRELATION_HEADERS {
            if let Some(value) = headers.get(*header_name) {
                if let Ok(id) = value.to_str() {
                    return Some(Self::from_string(id.to_string()));
                }
            }
        }

        None
    }

    /// Extract or generate correlation ID
    pub fn extract_or_generate(headers: &HeaderMap) -> Self {
        Self::from_headers(headers).unwrap_or_else(Self::new)
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Request metadata for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    /// Correlation ID for tracking
    pub correlation_id: String,

    /// Timestamp (Unix epoch milliseconds)
    pub timestamp: u64,

    /// HTTP method
    pub method: String,

    /// Request URI
    pub uri: String,

    /// HTTP version
    pub version: String,

    /// Client IP address
    pub client_ip: Option<String>,

    /// Request headers (optional, configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,

    /// Request body size in bytes
    pub body_size: Option<u64>,

    /// Upstream/backend selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}

impl RequestLog {
    /// Create request log from HTTP request
    pub fn from_request(
        correlation_id: &CorrelationId,
        method: &Method,
        uri: &Uri,
        version: Version,
        client_addr: Option<SocketAddr>,
        headers: Option<&HeaderMap>,
        upstream: Option<String>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let headers_vec = headers.map(|h| {
            h.iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect()
        });

        Self {
            correlation_id: correlation_id.to_string(),
            timestamp,
            method: method.to_string(),
            uri: uri.to_string(),
            version: format!("{:?}", version),
            client_ip: client_addr.map(|a| a.ip().to_string()),
            headers: headers_vec,
            body_size: None,
            upstream,
        }
    }

    /// Log the request
    pub fn log(&self) {
        info!(
            correlation_id = %self.correlation_id,
            method = %self.method,
            uri = %self.uri,
            client_ip = ?self.client_ip,
            upstream = ?self.upstream,
            "HTTP request received"
        );
    }

    /// Log the request with JSON format
    pub fn log_json(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            info!("{}", json);
        }
    }
}

/// Response metadata for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseLog {
    /// Correlation ID for tracking
    pub correlation_id: String,

    /// Timestamp (Unix epoch milliseconds)
    pub timestamp: u64,

    /// HTTP status code
    pub status: u16,

    /// Response duration in milliseconds
    pub duration_ms: f64,

    /// Response body size in bytes
    pub body_size: Option<u64>,

    /// Response headers (optional, configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,

    /// Error message if request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Cache status (hit, miss, bypass, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_status: Option<String>,

    /// Upstream response time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_duration_ms: Option<f64>,
}

impl ResponseLog {
    /// Create response log
    pub fn new(
        correlation_id: &CorrelationId,
        status: StatusCode,
        duration: Duration,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            correlation_id: correlation_id.to_string(),
            timestamp,
            status: status.as_u16(),
            duration_ms: duration.as_secs_f64() * 1000.0,
            body_size: None,
            headers: None,
            error: None,
            cache_status: None,
            upstream_duration_ms: None,
        }
    }

    /// Set body size
    pub fn with_body_size(mut self, size: u64) -> Self {
        self.body_size = Some(size);
        self
    }

    /// Set headers
    pub fn with_headers(mut self, headers: &HeaderMap) -> Self {
        self.headers = Some(
            headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect(),
        );
        self
    }

    /// Set error message
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Set cache status
    pub fn with_cache_status(mut self, status: String) -> Self {
        self.cache_status = Some(status);
        self
    }

    /// Set upstream duration
    pub fn with_upstream_duration(mut self, duration: Duration) -> Self {
        self.upstream_duration_ms = Some(duration.as_secs_f64() * 1000.0);
        self
    }

    /// Log the response
    pub fn log(&self) {
        let status = self.status;

        if status >= 500 {
            error!(
                correlation_id = %self.correlation_id,
                status = status,
                duration_ms = %self.duration_ms,
                error = ?self.error,
                "HTTP request failed (server error)"
            );
        } else if status >= 400 {
            warn!(
                correlation_id = %self.correlation_id,
                status = status,
                duration_ms = %self.duration_ms,
                "HTTP request failed (client error)"
            );
        } else {
            info!(
                correlation_id = %self.correlation_id,
                status = status,
                duration_ms = %self.duration_ms,
                cache_status = ?self.cache_status,
                upstream_duration_ms = ?self.upstream_duration_ms,
                "HTTP request completed"
            );
        }
    }

    /// Log the response with JSON format
    pub fn log_json(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            match self.status {
                s if s >= 500 => error!("{}", json),
                s if s >= 400 => warn!("{}", json),
                _ => info!("{}", json),
            }
        }
    }
}

/// Request/Response logger with correlation tracking
pub struct RequestLogger {
    correlation_id: CorrelationId,
    request_log: RequestLog,
    start_time: SystemTime,
}

impl RequestLogger {
    /// Create a new request logger
    pub fn new(
        method: &Method,
        uri: &Uri,
        version: Version,
        headers: &HeaderMap,
        client_addr: Option<SocketAddr>,
    ) -> Self {
        let correlation_id = CorrelationId::extract_or_generate(headers);
        let request_log = RequestLog::from_request(
            &correlation_id,
            method,
            uri,
            version,
            client_addr,
            None,
            None,
        );

        Self {
            correlation_id,
            request_log,
            start_time: SystemTime::now(),
        }
    }

    /// Get correlation ID
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    /// Log the request
    pub fn log_request(&self) {
        self.request_log.log();
    }

    /// Set upstream name
    pub fn set_upstream(&mut self, upstream: String) {
        self.request_log.upstream = Some(upstream);
    }

    /// Log the response
    pub fn log_response(
        &self,
        status: StatusCode,
        body_size: Option<u64>,
        cache_status: Option<String>,
        upstream_duration: Option<Duration>,
    ) {
        let duration = self.start_time.elapsed().unwrap_or_default();

        let mut response_log = ResponseLog::new(&self.correlation_id, status, duration);

        if let Some(size) = body_size {
            response_log = response_log.with_body_size(size);
        }

        if let Some(cache) = cache_status {
            response_log = response_log.with_cache_status(cache);
        }

        if let Some(upstream_dur) = upstream_duration {
            response_log = response_log.with_upstream_duration(upstream_dur);
        }

        response_log.log();
    }

    /// Log error response
    pub fn log_error(&self, status: StatusCode, error: String) {
        let duration = self.start_time.elapsed().unwrap_or_default();

        let response_log = ResponseLog::new(&self.correlation_id, status, duration)
            .with_error(error);

        response_log.log();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_id_generation() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();

        // IDs should be unique
        assert_ne!(id1, id2);

        // Should be valid UUIDs
        assert!(Uuid::parse_str(id1.as_str()).is_ok());
        assert!(Uuid::parse_str(id2.as_str()).is_ok());
    }

    #[test]
    fn test_correlation_id_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", "test-12345".parse().unwrap());

        let id = CorrelationId::from_headers(&headers).unwrap();
        assert_eq!(id.as_str(), "test-12345");
    }

    #[test]
    fn test_correlation_id_extract_or_generate() {
        // With header
        let mut headers = HeaderMap::new();
        headers.insert("x-correlation-id", "existing-id".parse().unwrap());

        let id = CorrelationId::extract_or_generate(&headers);
        assert_eq!(id.as_str(), "existing-id");

        // Without header - should generate
        let empty_headers = HeaderMap::new();
        let id = CorrelationId::extract_or_generate(&empty_headers);
        assert!(Uuid::parse_str(id.as_str()).is_ok());
    }

    #[test]
    fn test_request_log_creation() {
        let correlation_id = CorrelationId::new();
        let method = Method::GET;
        let uri: Uri = "/api/users".parse().unwrap();
        let version = Version::HTTP_11;
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        let request_log = RequestLog::from_request(
            &correlation_id,
            &method,
            &uri,
            version,
            Some(addr),
            None,
            Some("backend-1".to_string()),
        );

        assert_eq!(request_log.method, "GET");
        assert_eq!(request_log.uri, "/api/users");
        assert_eq!(request_log.client_ip.unwrap(), "127.0.0.1");
        assert_eq!(request_log.upstream.unwrap(), "backend-1");
    }

    #[test]
    fn test_response_log_creation() {
        let correlation_id = CorrelationId::new();
        let status = StatusCode::OK;
        let duration = Duration::from_millis(42);

        let response_log = ResponseLog::new(&correlation_id, status, duration)
            .with_body_size(1024)
            .with_cache_status("hit".to_string());

        assert_eq!(response_log.status, 200);
        assert_eq!(response_log.duration_ms, 42.0);
        assert_eq!(response_log.body_size.unwrap(), 1024);
        assert_eq!(response_log.cache_status.unwrap(), "hit");
    }

    #[test]
    fn test_request_logger_workflow() {
        let method = Method::POST;
        let uri: Uri = "/api/data".parse().unwrap();
        let headers = HeaderMap::new();
        let addr: SocketAddr = "192.168.1.1:12345".parse().unwrap();

        let mut logger = RequestLogger::new(&method, &uri, Version::HTTP_2, &headers, Some(addr));

        // Should have generated a correlation ID
        assert!(Uuid::parse_str(logger.correlation_id().as_str()).is_ok());

        // Set upstream
        logger.set_upstream("api-backend".to_string());

        // Log request (just verify it doesn't panic)
        logger.log_request();

        // Log response
        logger.log_response(StatusCode::CREATED, Some(512), None, None);

        // Log error
        logger.log_error(StatusCode::INTERNAL_SERVER_ERROR, "Database connection failed".to_string());
    }

    #[test]
    fn test_json_serialization() {
        let correlation_id = CorrelationId::new();
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();

        let request_log = RequestLog::from_request(
            &correlation_id,
            &method,
            &uri,
            Version::HTTP_11,
            None,
            None,
            None,
        );

        let json = serde_json::to_string(&request_log).unwrap();
        assert!(json.contains("correlation_id"));
        assert!(json.contains("GET"));
        assert!(json.contains("/test"));
    }
}
