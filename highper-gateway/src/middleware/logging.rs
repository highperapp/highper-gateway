//! Request logging middleware
//!
//! Provides comprehensive request/response logging with timing information.

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;
use tracing::info;

/// Access log format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Combined Apache format
    Combined,
    /// Common Apache format
    Common,
    /// JSON structured format
    Json,
}

/// Request logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log format
    pub format: LogFormat,
    /// Log request headers
    pub log_headers: bool,
    /// Log request body (be careful with sensitive data)
    pub log_body: bool,
    /// Maximum body size to log (bytes)
    pub max_body_size: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            format: LogFormat::Combined,
            log_headers: false,
            log_body: false,
            max_body_size: 1024,
        }
    }
}

/// Request logging middleware
pub struct LoggingMiddleware {
    config: LoggingConfig,
}

impl LoggingMiddleware {
    /// Create a new logging middleware
    pub fn new(config: LoggingConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn default_logging() -> Self {
        Self::new(LoggingConfig::default())
    }

    /// Create with specific format
    pub fn with_format(format: LogFormat) -> Self {
        Self::new(LoggingConfig {
            format,
            ..Default::default()
        })
    }

    /// Log in combined format (Apache)
    /// Format: remote_addr - remote_user [time] "request" status bytes "referer" "user_agent"
    fn log_combined(
        method: &str,
        uri: &str,
        version: &str,
        status: u16,
        duration_ms: f64,
        user_agent: Option<&str>,
        referer: Option<&str>,
    ) {
        info!(
            target: "access_log",
            method = method,
            uri = uri,
            version = version,
            status = status,
            duration_ms = format!("{:.2}", duration_ms),
            user_agent = user_agent.unwrap_or("-"),
            referer = referer.unwrap_or("-"),
            "{} {} {} {} {:.2}ms",
            method,
            uri,
            version,
            status,
            duration_ms
        );
    }

    /// Log in common format (Apache)
    /// Format: remote_addr - remote_user [time] "request" status bytes
    fn log_common(method: &str, uri: &str, version: &str, status: u16, duration_ms: f64) {
        info!(
            target: "access_log",
            method = method,
            uri = uri,
            version = version,
            status = status,
            duration_ms = format!("{:.2}", duration_ms),
            "{} {} {} {} {:.2}ms",
            method,
            uri,
            version,
            status,
            duration_ms
        );
    }

    /// Log in JSON format
    fn log_json(
        method: &str,
        uri: &str,
        version: &str,
        status: u16,
        duration_ms: f64,
        user_agent: Option<&str>,
        referer: Option<&str>,
    ) {
        info!(
            target: "access_log",
            method = method,
            uri = uri,
            version = version,
            status = status,
            duration_ms = format!("{:.3}", duration_ms),
            user_agent = user_agent.unwrap_or(""),
            referer = referer.unwrap_or(""),
            "request_completed"
        );
    }
}

impl Middleware for LoggingMiddleware {
    fn name(&self) -> &str {
        "logging"
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
        let config = self.config.clone();

        Box::pin(async move {
            let method = req.method().to_string();
            let uri = req.uri().to_string();
            let version = format!("{:?}", req.version());

            // Log request headers if enabled
            if config.log_headers {
                for (name, value) in req.headers() {
                    if let Ok(val_str) = value.to_str() {
                        tracing::debug!(
                            target: "request_headers",
                            header = name.as_str(),
                            value = val_str,
                            "Request header"
                        );
                    }
                }
            }

            tracing::debug!(
                target: "request_log",
                method = method.as_str(),
                uri = uri.as_str(),
                version = version.as_str(),
                "Incoming request"
            );

            Ok(req)
        })
    }

    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            // Note: In a real implementation, we'd need to pass request metadata
            // through the middleware chain. For now, we'll just log the response.

            let status = response.status();

            // Log response headers if enabled
            if config.log_headers {
                for (name, value) in response.headers() {
                    if let Ok(val_str) = value.to_str() {
                        tracing::debug!(
                            target: "response_headers",
                            header = name.as_str(),
                            value = val_str,
                            "Response header"
                        );
                    }
                }
            }

            tracing::debug!(
                target: "response_log",
                status = status.as_u16(),
                "Response sent"
            );

            Ok(response)
        })
    }
}

/// Request context for tracking timing and metadata
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub start_time: Instant,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

impl RequestContext {
    /// Create from a request
    pub fn from_request(req: &Request<hyper::body::Incoming>) -> Self {
        let user_agent = req
            .headers()
            .get(hyper::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let referer = req
            .headers()
            .get(hyper::header::REFERER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        Self {
            method: req.method().to_string(),
            uri: req.uri().to_string(),
            version: format!("{:?}", req.version()),
            start_time: Instant::now(),
            user_agent,
            referer,
        }
    }

    /// Log the completed request
    pub fn log_completed(&self, status: StatusCode, format: LogFormat) {
        let duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;

        match format {
            LogFormat::Combined => {
                LoggingMiddleware::log_combined(
                    &self.method,
                    &self.uri,
                    &self.version,
                    status.as_u16(),
                    duration_ms,
                    self.user_agent.as_deref(),
                    self.referer.as_deref(),
                );
            }
            LogFormat::Common => {
                LoggingMiddleware::log_common(
                    &self.method,
                    &self.uri,
                    &self.version,
                    status.as_u16(),
                    duration_ms,
                );
            }
            LogFormat::Json => {
                LoggingMiddleware::log_json(
                    &self.method,
                    &self.uri,
                    &self.version,
                    status.as_u16(),
                    duration_ms,
                    self.user_agent.as_deref(),
                    self.referer.as_deref(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.format, LogFormat::Combined);
        assert!(!config.log_headers);
        assert!(!config.log_body);
        assert_eq!(config.max_body_size, 1024);
    }

    #[test]
    fn test_log_formats() {
        // Just test that logging doesn't panic
        LoggingMiddleware::log_combined(
            "GET",
            "/test",
            "HTTP/1.1",
            200,
            12.34,
            Some("test-agent"),
            Some("http://example.com"),
        );

        LoggingMiddleware::log_common("GET", "/test", "HTTP/1.1", 200, 12.34);

        LoggingMiddleware::log_json(
            "GET",
            "/test",
            "HTTP/1.1",
            200,
            12.34,
            Some("test-agent"),
            Some("http://example.com"),
        );
    }

    #[test]
    fn test_middleware_creation() {
        let mw1 = LoggingMiddleware::default_logging();
        assert_eq!(mw1.name(), "logging");

        let mw2 = LoggingMiddleware::with_format(LogFormat::Json);
        assert_eq!(mw2.config.format, LogFormat::Json);
    }
}
