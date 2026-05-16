//! TLS Handshake Structured Logging
//!
//! Provides structured JSON logging for TLS/SSL:
//! - Handshake lifecycle events
//! - Certificate validation
//! - Protocol version and cipher suite negotiation
//! - Client certificate (mTLS) events
//! - Session resumption
//! - ALPN and SNI

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// TLS Session ID for handshake tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TlsSessionId(String);

impl TlsSessionId {
    /// Generate a new TLS session ID
    pub fn new() -> Self {
        Self(format!("tls-{}", Uuid::now_v7()))
    }

    /// Get the ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TlsSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TlsSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TLS handshake context for structured logging
#[derive(Debug, Clone, Serialize)]
pub struct TlsHandshakeContext {
    pub session_id: String,
    pub client_addr: String,
    pub server_name: String, // SNI
}

impl TlsHandshakeContext {
    pub fn new(session_id: TlsSessionId, client_addr: SocketAddr, server_name: String) -> Self {
        Self {
            session_id: session_id.to_string(),
            client_addr: client_addr.to_string(),
            server_name,
        }
    }
}

/// Log TLS handshake start
pub fn log_handshake_start(ctx: &TlsHandshakeContext, sni: Option<&str>) {
    debug!(
        event = "tls_handshake_start",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        sni = ?sni,
        "TLS handshake started"
    );
}

/// Log TLS handshake complete
pub fn log_handshake_complete(
    ctx: &TlsHandshakeContext,
    duration: Duration,
    protocol_version: &str,
    cipher_suite: &str,
    alpn_protocol: Option<&str>,
) {
    info!(
        event = "tls_handshake_complete",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        duration_ms = duration.as_millis() as u64,
        protocol_version = protocol_version,
        cipher_suite = cipher_suite,
        alpn_protocol = ?alpn_protocol,
        "TLS handshake completed"
    );
}

/// Log TLS handshake failure
pub fn log_handshake_failure(
    ctx: &TlsHandshakeContext,
    duration: Duration,
    error_type: &str,
    error_message: &str,
) {
    error!(
        event = "tls_handshake_failure",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        duration_ms = duration.as_millis() as u64,
        error_type = error_type,
        error_message = error_message,
        "TLS handshake failed"
    );
}

/// Log certificate validation
pub fn log_certificate_validation(
    ctx: &TlsHandshakeContext,
    cert_subject: &str,
    cert_issuer: &str,
    valid_from: &str,
    valid_until: &str,
    success: bool,
    error: Option<&str>,
) {
    if success {
        debug!(
            event = "tls_certificate_validated",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            cert_subject = cert_subject,
            cert_issuer = cert_issuer,
            valid_from = valid_from,
            valid_until = valid_until,
            "Server certificate validated"
        );
    } else {
        warn!(
            event = "tls_certificate_validation_failed",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            cert_subject = cert_subject,
            cert_issuer = cert_issuer,
            error = error.unwrap_or("unknown"),
            "Server certificate validation failed"
        );
    }
}

/// Log client certificate validation (mTLS)
pub fn log_client_certificate_validation(
    ctx: &TlsHandshakeContext,
    cert_subject: &str,
    cert_issuer: &str,
    success: bool,
    failure_reason: Option<&str>,
) {
    if success {
        info!(
            event = "tls_client_cert_validated",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            cert_subject = cert_subject,
            cert_issuer = cert_issuer,
            "Client certificate validated (mTLS)"
        );
    } else {
        warn!(
            event = "tls_client_cert_validation_failed",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            cert_subject = cert_subject,
            cert_issuer = cert_issuer,
            failure_reason = failure_reason.unwrap_or("unknown"),
            "Client certificate validation failed (mTLS)"
        );
    }
}

/// Log certificate expiry warning
pub fn log_certificate_expiring_soon(
    server_name: &str,
    cert_subject: &str,
    days_until_expiry: i64,
) {
    warn!(
        event = "tls_certificate_expiring_soon",
        server_name = server_name,
        cert_subject = cert_subject,
        days_until_expiry = days_until_expiry,
        "Certificate expiring soon"
    );
}

/// Log certificate reload
pub fn log_certificate_reload(
    server_name: &str,
    cert_subject: &str,
    success: bool,
    error: Option<&str>,
) {
    if success {
        info!(
            event = "tls_certificate_reloaded",
            server_name = server_name,
            cert_subject = cert_subject,
            "Certificate reloaded successfully"
        );
    } else {
        error!(
            event = "tls_certificate_reload_failed",
            server_name = server_name,
            error = error.unwrap_or("unknown"),
            "Certificate reload failed"
        );
    }
}

/// Log TLS session resumption
pub fn log_session_resumption(ctx: &TlsHandshakeContext, success: bool, session_ticket: bool) {
    if success {
        debug!(
            event = "tls_session_resumed",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            session_ticket = session_ticket,
            "TLS session resumed"
        );
    } else {
        trace!(
            event = "tls_session_resumption_failed",
            session_id = %ctx.session_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            "TLS session resumption failed"
        );
    }
}

/// Log ALPN negotiation
pub fn log_alpn_negotiated(ctx: &TlsHandshakeContext, protocol: &str, client_protocols: &[String]) {
    debug!(
        event = "tls_alpn_negotiated",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        negotiated_protocol = protocol,
        client_protocols = ?client_protocols,
        "ALPN protocol negotiated"
    );
}

/// Log SNI request
pub fn log_sni_request(session_id: &TlsSessionId, client_addr: SocketAddr, server_name: &str) {
    trace!(
        event = "tls_sni_request",
        session_id = %session_id,
        client_addr = %client_addr,
        server_name = server_name,
        "SNI (Server Name Indication) received"
    );
}

/// Log TLS protocol error
pub fn log_protocol_error(ctx: &TlsHandshakeContext, error_type: &str, error_message: &str) {
    error!(
        event = "tls_protocol_error",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        error_type = error_type,
        error_message = error_message,
        "TLS protocol error"
    );
}

/// Log unsupported TLS version
pub fn log_unsupported_version(
    ctx: &TlsHandshakeContext,
    requested_version: &str,
    supported_versions: &[String],
) {
    warn!(
        event = "tls_unsupported_version",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        requested_version = requested_version,
        supported_versions = ?supported_versions,
        "Client requested unsupported TLS version"
    );
}

/// Log cipher suite mismatch
pub fn log_cipher_suite_mismatch(
    ctx: &TlsHandshakeContext,
    client_suites: &[String],
    server_suites: &[String],
) {
    warn!(
        event = "tls_cipher_suite_mismatch",
        session_id = %ctx.session_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        client_cipher_suites = ?client_suites,
        server_cipher_suites = ?server_suites,
        "No common cipher suites found"
    );
}

/// Log ACME certificate request
pub fn log_acme_certificate_request(server_name: &str, challenge_type: &str) {
    info!(
        event = "tls_acme_certificate_request",
        server_name = server_name,
        challenge_type = challenge_type,
        "ACME certificate request initiated"
    );
}

/// Log ACME certificate issued
pub fn log_acme_certificate_issued(server_name: &str, valid_days: i64) {
    info!(
        event = "tls_acme_certificate_issued",
        server_name = server_name,
        valid_days = valid_days,
        "ACME certificate issued successfully"
    );
}

/// Log ACME certificate renewal
pub fn log_acme_certificate_renewal(server_name: &str, days_until_expiry: i64, success: bool) {
    if success {
        info!(
            event = "tls_acme_certificate_renewed",
            server_name = server_name,
            days_until_expiry = days_until_expiry,
            "ACME certificate renewed successfully"
        );
    } else {
        error!(
            event = "tls_acme_certificate_renewal_failed",
            server_name = server_name,
            days_until_expiry = days_until_expiry,
            "ACME certificate renewal failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_tls_session_id() {
        let id1 = TlsSessionId::new();
        let id2 = TlsSessionId::new();

        assert!(id1.as_str().starts_with("tls-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tls_logging() {
        let session_id = TlsSessionId::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 44300);
        let ctx = TlsHandshakeContext::new(
            session_id.clone(),
            client_addr,
            "api.example.com".to_string(),
        );

        // Test handshake lifecycle
        log_handshake_start(&ctx, Some("api.example.com"));
        log_handshake_complete(
            &ctx,
            Duration::from_millis(150),
            "TLSv1.3",
            "TLS_AES_256_GCM_SHA384",
            Some("h2"),
        );

        // Test certificate validation
        log_certificate_validation(
            &ctx,
            "CN=api.example.com",
            "CN=Let's Encrypt Authority X3",
            "2024-01-01",
            "2024-12-31",
            true,
            None,
        );

        // Test client certificate (mTLS)
        log_client_certificate_validation(
            &ctx,
            "CN=client.example.com",
            "CN=Example CA",
            true,
            None,
        );

        // Test session resumption
        log_session_resumption(&ctx, true, true);

        // Test ALPN
        log_alpn_negotiated(&ctx, "h2", &["h2".to_string(), "http/1.1".to_string()]);

        // Test SNI
        log_sni_request(&session_id, client_addr, "api.example.com");

        assert!(true);
    }
}
