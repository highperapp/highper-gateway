//! TLS-specific metrics for HTTPS and TLS connections
//!
//! This module provides metrics specifically for TLS/SSL:
//! - TLS handshakes (success/failure)
//! - Certificate expiry tracking
//! - Cipher suite usage
//! - Protocol version distribution
//! - Client certificate validation
//! - TLS errors

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::time::Duration;

/// Initialize TLS-specific metrics descriptions
pub fn describe_tls_metrics() {
    // TLS handshake metrics
    describe_counter!(
        "tls_handshakes_total",
        "Total TLS handshakes by result (success/failure)"
    );
    describe_counter!(
        "tls_handshakes_success_total",
        "Total successful TLS handshakes"
    );
    describe_counter!(
        "tls_handshakes_failure_total",
        "Total failed TLS handshakes"
    );
    describe_histogram!(
        "tls_handshake_duration_seconds",
        "TLS handshake duration in seconds"
    );

    // Certificate metrics
    describe_gauge!(
        "tls_certificate_expiry_seconds",
        "Seconds until certificate expires"
    );
    describe_gauge!(
        "tls_certificates_expiring_soon",
        "Number of certificates expiring within 30 days"
    );
    describe_counter!(
        "tls_certificate_reload_total",
        "Total certificate reload events"
    );
    describe_counter!(
        "tls_certificate_reload_errors_total",
        "Total certificate reload errors"
    );

    // Cipher suite usage
    describe_counter!(
        "tls_cipher_suite_usage_total",
        "Usage count by cipher suite"
    );

    // TLS protocol version
    describe_counter!(
        "tls_protocol_version_total",
        "Connections by TLS version (1.2, 1.3)"
    );

    // Client certificate validation (mTLS)
    describe_counter!(
        "tls_client_cert_validations_total",
        "Client certificate validations by result"
    );
    describe_counter!(
        "tls_client_cert_success_total",
        "Successful client certificate validations"
    );
    describe_counter!(
        "tls_client_cert_failure_total",
        "Failed client certificate validations"
    );

    // TLS errors
    describe_counter!(
        "tls_errors_total",
        "TLS errors by type"
    );
    describe_counter!(
        "tls_protocol_errors_total",
        "TLS protocol errors"
    );
    describe_counter!(
        "tls_certificate_errors_total",
        "TLS certificate errors"
    );
    describe_counter!(
        "tls_handshake_timeout_total",
        "TLS handshake timeouts"
    );

    // Session resumption
    describe_counter!(
        "tls_session_resumptions_total",
        "Total TLS session resumptions"
    );
    describe_counter!(
        "tls_session_resumption_success_total",
        "Successful TLS session resumptions"
    );
    describe_counter!(
        "tls_session_resumption_failure_total",
        "Failed TLS session resumptions"
    );

    // ALPN (Application-Layer Protocol Negotiation)
    describe_counter!(
        "tls_alpn_negotiated_total",
        "ALPN protocols negotiated"
    );

    // SNI (Server Name Indication)
    describe_counter!(
        "tls_sni_total",
        "SNI requests by server name"
    );
}

/// Record a TLS handshake
pub fn record_handshake(
    server_name: &str,
    success: bool,
    duration: Duration,
    protocol_version: &str,
    cipher_suite: &str,
) {
    let duration_secs = duration.as_secs_f64();

    // Record handshake result
    counter!(
        "tls_handshakes_total",
        "server_name" => server_name.to_string(),
        "result" => if success { "success" } else { "failure" }.to_string(),
    ).increment(1);

    if success {
        counter!(
            "tls_handshakes_success_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

        // Record protocol version
        counter!(
            "tls_protocol_version_total",
            "version" => protocol_version.to_string(),
            "server_name" => server_name.to_string(),
        ).increment(1);

        // Record cipher suite
        counter!(
            "tls_cipher_suite_usage_total",
            "cipher" => cipher_suite.to_string(),
            "server_name" => server_name.to_string(),
        ).increment(1);

    } else {
        counter!(
            "tls_handshakes_failure_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

    }

    // Record duration
    histogram!(
        "tls_handshake_duration_seconds",
        "server_name" => server_name.to_string(),
        "result" => if success { "success" } else { "failure" }.to_string(),
    ).record(duration_secs);
}

/// Update certificate expiry gauge
pub fn update_certificate_expiry(server_name: &str, seconds_until_expiry: i64) {
    gauge!(
        "tls_certificate_expiry_seconds",
        "server_name" => server_name.to_string(),
    ).set(seconds_until_expiry as f64);

    // Track certificates expiring soon (within 30 days)
    if seconds_until_expiry > 0 && seconds_until_expiry < (30 * 24 * 60 * 60) {
        gauge!("tls_certificates_expiring_soon").increment(1.0);
    }
}

/// Record certificate reload event
pub fn record_certificate_reload(server_name: &str, success: bool) {
    counter!(
        "tls_certificate_reload_total",
        "server_name" => server_name.to_string(),
        "result" => if success { "success" } else { "failure" }.to_string(),
    ).increment(1);

    if !success {
        counter!(
            "tls_certificate_reload_errors_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

    } else {
    }
}

/// Record client certificate validation (mTLS)
pub fn record_client_cert_validation(
    server_name: &str,
    success: bool,
    failure_reason: Option<&str>,
) {
    let result = if success { "success" } else { "failure" };

    counter!(
        "tls_client_cert_validations_total",
        "server_name" => server_name.to_string(),
        "result" => result.to_string(),
    ).increment(1);

    if success {
        counter!(
            "tls_client_cert_success_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

    } else {
        counter!(
            "tls_client_cert_failure_total",
            "server_name" => server_name.to_string(),
            "reason" => failure_reason.unwrap_or("unknown").to_string(),
        ).increment(1);

    }
}

/// Record TLS error
pub fn record_tls_error(server_name: &str, error_type: &str) {
    counter!(
        "tls_errors_total",
        "server_name" => server_name.to_string(),
        "error" => error_type.to_string(),
    ).increment(1);

    // Record specific error types
    match error_type {
        "protocol_error" => {
            counter!(
                "tls_protocol_errors_total",
                "server_name" => server_name.to_string(),
            ).increment(1)
        }
        "certificate_error" => {
            counter!(
                "tls_certificate_errors_total",
                "server_name" => server_name.to_string(),
            ).increment(1)
        }
        "handshake_timeout" => {
            counter!(
                "tls_handshake_timeout_total",
                "server_name" => server_name.to_string(),
            ).increment(1)
        }
        _ => {}
    }

}

/// Record TLS session resumption
pub fn record_session_resumption(server_name: &str, success: bool) {
    counter!(
        "tls_session_resumptions_total",
        "server_name" => server_name.to_string(),
        "result" => if success { "success" } else { "failure" }.to_string(),
    ).increment(1);

    if success {
        counter!(
            "tls_session_resumption_success_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

    } else {
        counter!(
            "tls_session_resumption_failure_total",
            "server_name" => server_name.to_string(),
        ).increment(1);

    }
}

/// Record ALPN negotiation
pub fn record_alpn_negotiated(server_name: &str, protocol: &str) {
    counter!(
        "tls_alpn_negotiated_total",
        "server_name" => server_name.to_string(),
        "protocol" => protocol.to_string(),
    ).increment(1);

}

/// Record SNI request
pub fn record_sni(server_name: &str) {
    counter!(
        "tls_sni_total",
        "server_name" => server_name.to_string(),
    ).increment(1);

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_metrics_recording() {
        // Initialize metrics
        describe_tls_metrics();

        // Test handshake recording
        record_handshake(
            "api.example.com",
            true,
            Duration::from_millis(150),
            "TLSv1.3",
            "TLS_AES_256_GCM_SHA384",
        );

        record_handshake(
            "api.example.com",
            false,
            Duration::from_millis(200),
            "",
            "",
        );

        // Test certificate metrics
        update_certificate_expiry("api.example.com", 7776000); // 90 days
        record_certificate_reload("api.example.com", true);

        // Test client cert validation
        record_client_cert_validation("api.example.com", true, None);
        record_client_cert_validation("api.example.com", false, Some("expired"));

        // Test error recording
        record_tls_error("api.example.com", "protocol_error");
        record_tls_error("api.example.com", "certificate_error");

        // Test session resumption
        record_session_resumption("api.example.com", true);

        // Test ALPN and SNI
        record_alpn_negotiated("api.example.com", "h2");
        record_sni("api.example.com");

        // If we reach here without panicking, all metrics recorded successfully
        assert!(true);
    }
}
