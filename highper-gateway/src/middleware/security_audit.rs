//! Security audit logging middleware
//!
//! This middleware provides comprehensive security event logging:
//! - Authentication attempts (success/failure)
//! - Authorization decisions
//! - Suspicious request patterns
//! - Rate limit violations
//! - Input validation failures
//! - TLS/mTLS events
//! - Security header violations

use super::{Middleware, MiddlewareResult};
use crate::http::ResponseBody;
use hyper::{header, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Security event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityEventType {
    /// Authentication attempt
    AuthAttempt,
    /// Authorization decision
    AuthzDecision,
    /// Suspicious request pattern detected
    SuspiciousRequest,
    /// Rate limit violation
    RateLimitViolation,
    /// Input validation failure
    ValidationFailure,
    /// TLS handshake event
    TlsEvent,
    /// mTLS authentication
    MtlsAuth,
    /// WAF rule triggered
    WafTrigger,
    /// DDoS protection triggered
    DdosProtection,
    /// Security header violation
    SecurityHeaderViolation,
}

/// Security event severity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityEventSeverity {
    /// Informational event
    Info,
    /// Warning - potential security issue
    Warning,
    /// Error - security violation detected
    Error,
    /// Critical - active attack detected
    Critical,
}

/// Security audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEvent {
    /// Event timestamp (Unix epoch milliseconds)
    pub timestamp: u64,
    /// Event type
    pub event_type: SecurityEventType,
    /// Event severity
    pub severity: SecurityEventSeverity,
    /// Client IP address
    pub client_ip: Option<String>,
    /// Request method
    pub method: String,
    /// Request path
    pub path: String,
    /// User agent
    pub user_agent: Option<String>,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// Event message
    pub message: String,
    /// Additional context
    pub context: serde_json::Value,
}

impl SecurityAuditEvent {
    /// Create a new security audit event
    pub fn new(
        event_type: SecurityEventType,
        severity: SecurityEventSeverity,
        message: String,
    ) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            event_type,
            severity,
            client_ip: None,
            method: String::new(),
            path: String::new(),
            user_agent: None,
            user_id: None,
            message,
            context: serde_json::json!({}),
        }
    }

    /// Set client IP
    pub fn with_client_ip(mut self, ip: String) -> Self {
        self.client_ip = Some(ip);
        self
    }

    /// Set request details
    pub fn with_request(mut self, method: String, path: String) -> Self {
        self.method = method;
        self.path = path;
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, ua: String) -> Self {
        self.user_agent = Some(ua);
        self
    }

    /// Set user ID
    pub fn with_user_id(mut self, uid: String) -> Self {
        self.user_id = Some(uid);
        self
    }

    /// Set additional context
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    /// Log the event using tracing
    pub fn log(&self) {
        let json = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());

        match self.severity {
            SecurityEventSeverity::Info => {
                info!(
                    security_event = %json,
                    event_type = ?self.event_type,
                    "Security audit event"
                );
            }
            SecurityEventSeverity::Warning => {
                warn!(
                    security_event = %json,
                    event_type = ?self.event_type,
                    "Security audit event"
                );
            }
            SecurityEventSeverity::Error | SecurityEventSeverity::Critical => {
                error!(
                    security_event = %json,
                    event_type = ?self.event_type,
                    severity = ?self.severity,
                    "Security audit event"
                );
            }
        }
    }
}

/// Security audit logger configuration
#[derive(Debug, Clone)]
pub struct SecurityAuditConfig {
    /// Log authentication attempts
    pub log_auth_attempts: bool,
    /// Log authorization decisions
    pub log_authz_decisions: bool,
    /// Log suspicious requests
    pub log_suspicious_requests: bool,
    /// Log rate limit violations
    pub log_rate_limit_violations: bool,
    /// Log validation failures
    pub log_validation_failures: bool,
    /// Log TLS events
    pub log_tls_events: bool,
    /// Log all successful requests
    pub log_successful_requests: bool,
    /// Minimum severity to log
    pub min_severity: SecurityEventSeverity,
}

impl Default for SecurityAuditConfig {
    fn default() -> Self {
        Self {
            log_auth_attempts: true,
            log_authz_decisions: true,
            log_suspicious_requests: true,
            log_rate_limit_violations: true,
            log_validation_failures: true,
            log_tls_events: false, // Can be noisy
            log_successful_requests: false,
            min_severity: SecurityEventSeverity::Info,
        }
    }
}

impl SecurityAuditConfig {
    /// Create strict audit configuration (log everything)
    pub fn strict() -> Self {
        Self {
            log_auth_attempts: true,
            log_authz_decisions: true,
            log_suspicious_requests: true,
            log_rate_limit_violations: true,
            log_validation_failures: true,
            log_tls_events: true,
            log_successful_requests: true,
            min_severity: SecurityEventSeverity::Info,
        }
    }

    /// Create minimal audit configuration (critical events only)
    pub fn minimal() -> Self {
        Self {
            log_auth_attempts: true,
            log_authz_decisions: false,
            log_suspicious_requests: true,
            log_rate_limit_violations: false,
            log_validation_failures: true,
            log_tls_events: false,
            log_successful_requests: false,
            min_severity: SecurityEventSeverity::Warning,
        }
    }

    /// Create compliance audit configuration (for regulatory requirements)
    pub fn compliance() -> Self {
        Self {
            log_auth_attempts: true,
            log_authz_decisions: true,
            log_suspicious_requests: true,
            log_rate_limit_violations: true,
            log_validation_failures: true,
            log_tls_events: true,
            log_successful_requests: false,
            min_severity: SecurityEventSeverity::Info,
        }
    }
}

/// Security audit middleware
pub struct SecurityAuditMiddleware {
    config: SecurityAuditConfig,
}

impl SecurityAuditMiddleware {
    /// Create a new security audit middleware
    pub fn new(config: SecurityAuditConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn default_audit() -> Self {
        Self::new(SecurityAuditConfig::default())
    }

    /// Create with strict config
    pub fn strict() -> Self {
        Self::new(SecurityAuditConfig::strict())
    }

    /// Create with minimal config
    pub fn minimal() -> Self {
        Self::new(SecurityAuditConfig::minimal())
    }

    /// Create with compliance config
    pub fn compliance() -> Self {
        Self::new(SecurityAuditConfig::compliance())
    }

    /// Extract client IP from request
    fn extract_client_ip<B>(req: &Request<B>) -> Option<String> {
        // Try X-Forwarded-For header first
        if let Some(xff) = req.headers().get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                return xff_str.split(',').next().map(|s| s.trim().to_string());
            }
        }

        // Try X-Real-IP header
        if let Some(xri) = req.headers().get("x-real-ip") {
            if let Ok(xri_str) = xri.to_str() {
                return Some(xri_str.to_string());
            }
        }

        None
    }

    /// Extract user agent from request
    fn extract_user_agent<B>(req: &Request<B>) -> Option<String> {
        req.headers()
            .get(header::USER_AGENT)
            .and_then(|ua| ua.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Log security event for validation failure
    pub fn log_validation_failure<B>(&self, req: &Request<B>, failure_reason: &str) {
        if !self.config.log_validation_failures {
            return;
        }

        let event = SecurityAuditEvent::new(
            SecurityEventType::ValidationFailure,
            SecurityEventSeverity::Warning,
            format!("Request validation failed: {}", failure_reason),
        )
        .with_client_ip(Self::extract_client_ip(req).unwrap_or_else(|| "unknown".to_string()))
        .with_request(req.method().to_string(), req.uri().path().to_string())
        .with_user_agent(Self::extract_user_agent(req).unwrap_or_default())
        .with_context(serde_json::json!({
            "failure_reason": failure_reason,
            "uri": req.uri().to_string(),
        }));

        event.log();
    }

    /// Log security event for rate limit violation
    pub fn log_rate_limit_violation<B>(&self, req: &Request<B>, limit: usize, window: &str) {
        if !self.config.log_rate_limit_violations {
            return;
        }

        let event = SecurityAuditEvent::new(
            SecurityEventType::RateLimitViolation,
            SecurityEventSeverity::Warning,
            "Rate limit exceeded".to_string(),
        )
        .with_client_ip(Self::extract_client_ip(req).unwrap_or_else(|| "unknown".to_string()))
        .with_request(req.method().to_string(), req.uri().path().to_string())
        .with_user_agent(Self::extract_user_agent(req).unwrap_or_default())
        .with_context(serde_json::json!({
            "limit": limit,
            "window": window,
        }));

        event.log();
    }

    /// Log security event for suspicious request
    pub fn log_suspicious_request<B>(&self, req: &Request<B>, reason: &str) {
        if !self.config.log_suspicious_requests {
            return;
        }

        let event = SecurityAuditEvent::new(
            SecurityEventType::SuspiciousRequest,
            SecurityEventSeverity::Error,
            format!("Suspicious request pattern: {}", reason),
        )
        .with_client_ip(Self::extract_client_ip(req).unwrap_or_else(|| "unknown".to_string()))
        .with_request(req.method().to_string(), req.uri().path().to_string())
        .with_user_agent(Self::extract_user_agent(req).unwrap_or_default())
        .with_context(serde_json::json!({
            "reason": reason,
            "uri": req.uri().to_string(),
        }));

        event.log();
    }
}

impl Middleware for SecurityAuditMiddleware {
    fn name(&self) -> &str {
        "security-audit"
    }

    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        let config = self.config.clone();

        Box::pin(async move {
            // Log failed authentication attempts
            if response.status() == StatusCode::UNAUTHORIZED && config.log_auth_attempts {
                let event = SecurityAuditEvent::new(
                    SecurityEventType::AuthAttempt,
                    SecurityEventSeverity::Warning,
                    "Authentication failed".to_string(),
                );
                event.log();
            }

            // Log forbidden access attempts
            if response.status() == StatusCode::FORBIDDEN && config.log_authz_decisions {
                let event = SecurityAuditEvent::new(
                    SecurityEventType::AuthzDecision,
                    SecurityEventSeverity::Warning,
                    "Authorization denied".to_string(),
                );
                event.log();
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_event_creation() {
        let event = SecurityAuditEvent::new(
            SecurityEventType::AuthAttempt,
            SecurityEventSeverity::Warning,
            "Test event".to_string(),
        )
        .with_client_ip("192.168.1.100".to_string())
        .with_request("GET".to_string(), "/api/test".to_string())
        .with_user_id("user123".to_string());

        assert_eq!(event.client_ip, Some("192.168.1.100".to_string()));
        assert_eq!(event.method, "GET");
        assert_eq!(event.path, "/api/test");
        assert_eq!(event.user_id, Some("user123".to_string()));
    }

    #[test]
    fn test_config_presets() {
        let default = SecurityAuditConfig::default();
        assert!(default.log_auth_attempts);

        let strict = SecurityAuditConfig::strict();
        assert!(strict.log_successful_requests);

        let minimal = SecurityAuditConfig::minimal();
        assert!(!minimal.log_successful_requests);

        let compliance = SecurityAuditConfig::compliance();
        assert!(compliance.log_authz_decisions);
    }
}
