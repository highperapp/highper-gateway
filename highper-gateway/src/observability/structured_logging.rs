/// Structured logging for 12-factor app compliance
///
/// Provides structured JSON logging with consistent fields for
/// log aggregation systems (ELK, Splunk, CloudWatch, etc.)
///
/// All logs include:
/// - timestamp (ISO 8601)
/// - level (trace, debug, info, warn, error)
/// - target (module path)
/// - message
/// - structured fields (request_id, client_ip, duration, etc.)

use std::time::Instant;
use tracing::{info, warn, error, debug, Span};

/// Request context for structured logging
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub client_ip: String,
    pub method: String,
    pub path: String,
    pub user_agent: Option<String>,
}

impl RequestContext {
    pub fn new(
        request_id: String,
        client_ip: String,
        method: String,
        path: String,
    ) -> Self {
        Self {
            request_id,
            client_ip,
            method,
            path,
            user_agent: None,
        }
    }

    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Create a tracing span with all request context
    pub fn create_span(&self) -> Span {
        tracing::info_span!(
            "request",
            request_id = %self.request_id,
            client_ip = %self.client_ip,
            method = %self.method,
            path = %self.path,
            user_agent = ?self.user_agent,
        )
    }
}

/// Log a request completion with structured fields
pub fn log_request_completed(
    ctx: &RequestContext,
    status_code: u16,
    duration: std::time::Duration,
    bytes_sent: u64,
) {
    info!(
        request_id = %ctx.request_id,
        client_ip = %ctx.client_ip,
        method = %ctx.method,
        path = %ctx.path,
        status_code = status_code,
        duration_ms = duration.as_millis() as u64,
        bytes_sent = bytes_sent,
        "Request completed"
    );
}

/// Log a request error with structured fields
pub fn log_request_error(
    ctx: &RequestContext,
    status_code: u16,
    error: &str,
    duration: std::time::Duration,
) {
    error!(
        request_id = %ctx.request_id,
        client_ip = %ctx.client_ip,
        method = %ctx.method,
        path = %ctx.path,
        status_code = status_code,
        duration_ms = duration.as_millis() as u64,
        error = error,
        "Request failed"
    );
}

/// Log a security event with structured fields
pub fn log_security_event(
    event_type: &str,
    client_ip: &str,
    path: &str,
    reason: &str,
) {
    warn!(
        event_type = "security",
        security_event = event_type,
        client_ip = client_ip,
        path = path,
        reason = reason,
        "Security event detected"
    );
}

/// Log a rate limit event with structured fields
pub fn log_rate_limit(
    client_ip: &str,
    limit_type: &str,
    current: u64,
    max: u64,
) {
    warn!(
        event_type = "rate_limit",
        client_ip = client_ip,
        limit_type = limit_type,
        current = current,
        max = max,
        "Rate limit exceeded"
    );
}

/// Log a resource limit event with structured fields
pub fn log_resource_limit(
    resource_type: &str,
    client_ip: Option<&str>,
    current: usize,
    max: usize,
) {
    warn!(
        event_type = "resource_limit",
        client_ip = ?client_ip,
        resource_type = resource_type,
        current = current,
        max = max,
        "Resource limit exceeded"
    );
}

/// Log a backend health event with structured fields
pub fn log_backend_health(
    backend: &str,
    healthy: bool,
    reason: Option<&str>,
) {
    if healthy {
        info!(
            event_type = "backend_health",
            backend = backend,
            healthy = true,
            "Backend health restored"
        );
    } else {
        error!(
            event_type = "backend_health",
            backend = backend,
            healthy = false,
            reason = ?reason,
            "Backend health check failed"
        );
    }
}

/// Log a configuration event with structured fields
pub fn log_config_event(
    event: &str,
    config_path: &str,
    success: bool,
    error: Option<&str>,
) {
    if success {
        info!(
            event_type = "config",
            event = event,
            config_path = config_path,
            success = true,
            "Configuration event"
        );
    } else {
        error!(
            event_type = "config",
            event = event,
            config_path = config_path,
            success = false,
            error = ?error,
            "Configuration event failed"
        );
    }
}

/// Log a cache event with structured fields
pub fn log_cache_event(
    event_type: &str,
    key: &str,
    hit: bool,
    ttl_secs: Option<u64>,
) {
    debug!(
        event_type = "cache",
        cache_event = event_type,
        key = key,
        hit = hit,
        ttl_secs = ?ttl_secs,
        "Cache event"
    );
}

/// Log a TLS event with structured fields
pub fn log_tls_event(
    event: &str,
    domain: &str,
    success: bool,
    error: Option<&str>,
) {
    if success {
        info!(
            event_type = "tls",
            event = event,
            domain = domain,
            success = true,
            "TLS event"
        );
    } else {
        error!(
            event_type = "tls",
            event = event,
            domain = domain,
            success = false,
            error = ?error,
            "TLS event failed"
        );
    }
}

/// Performance timer for measuring operation duration
pub struct PerfTimer {
    start: Instant,
    operation: String,
}

impl PerfTimer {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.into(),
        }
    }

    pub fn finish(self) {
        let duration = self.start.elapsed();
        debug!(
            event_type = "performance",
            operation = %self.operation,
            duration_ms = duration.as_millis() as u64,
            "Operation completed"
        );
    }

    pub fn finish_with_result(self, success: bool, error: Option<&str>) {
        let duration = self.start.elapsed();
        if success {
            info!(
                event_type = "performance",
                operation = %self.operation,
                duration_ms = duration.as_millis() as u64,
                success = true,
                "Operation completed"
            );
        } else {
            error!(
                event_type = "performance",
                operation = %self.operation,
                duration_ms = duration.as_millis() as u64,
                success = false,
                error = ?error,
                "Operation failed"
            );
        }
    }
}

/// Log server startup event
pub fn log_server_startup(
    version: &str,
    bind_address: &str,
    hot_reload: bool,
) {
    info!(
        event_type = "lifecycle",
        event = "startup",
        version = version,
        bind_address = bind_address,
        hot_reload = hot_reload,
        "Server starting"
    );
}

/// Log server shutdown event
pub fn log_server_shutdown(
    graceful: bool,
    active_connections: usize,
) {
    info!(
        event_type = "lifecycle",
        event = "shutdown",
        graceful = graceful,
        active_connections = active_connections,
        "Server shutting down"
    );
}

/// Log metrics snapshot
pub fn log_metrics_snapshot(
    total_requests: u64,
    error_rate: f64,
    avg_response_time_ms: u64,
) {
    info!(
        event_type = "metrics",
        total_requests = total_requests,
        error_rate = error_rate,
        avg_response_time_ms = avg_response_time_ms,
        "Metrics snapshot"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_context_creation() {
        let ctx = RequestContext::new(
            "req-123".to_string(),
            "192.168.1.1".to_string(),
            "GET".to_string(),
            "/api/test".to_string(),
        );

        assert_eq!(ctx.request_id, "req-123");
        assert_eq!(ctx.client_ip, "192.168.1.1");
        assert_eq!(ctx.method, "GET");
        assert_eq!(ctx.path, "/api/test");
        assert_eq!(ctx.user_agent, None);
    }

    #[test]
    fn test_request_context_with_user_agent() {
        let ctx = RequestContext::new(
            "req-123".to_string(),
            "192.168.1.1".to_string(),
            "GET".to_string(),
            "/api/test".to_string(),
        )
        .with_user_agent("Mozilla/5.0".to_string());

        assert_eq!(ctx.user_agent, Some("Mozilla/5.0".to_string()));
    }

    #[test]
    fn test_perf_timer() {
        let timer = PerfTimer::new("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.finish();
        // If this doesn't panic, the test passes
    }
}
