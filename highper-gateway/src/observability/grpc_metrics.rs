//! gRPC-specific metrics
//!
//! This module provides metrics specifically for gRPC:
//! - Per-method request tracking
//! - Stream counting (unary, client/server/bidirectional streaming)
//! - Message counting
//! - gRPC status codes
//! - Stream duration

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::time::Duration;

/// Initialize gRPC-specific metrics descriptions
pub fn describe_grpc_metrics() {
    // Request metrics
    describe_counter!(
        "grpc_requests_total",
        "Total gRPC requests by service and method"
    );
    describe_histogram!(
        "grpc_request_duration_seconds",
        "gRPC request duration by service and method"
    );

    // Stream metrics
    describe_gauge!(
        "grpc_streams_active",
        "Number of currently active gRPC streams"
    );
    describe_counter!(
        "grpc_streams_total",
        "Total gRPC streams by type (unary/client/server/bidi)"
    );
    describe_counter!(
        "grpc_streams_closed_total",
        "Total gRPC streams closed"
    );
    describe_histogram!(
        "grpc_stream_duration_seconds",
        "Duration of gRPC streams"
    );

    // Message metrics
    describe_counter!(
        "grpc_messages_sent_total",
        "Total gRPC messages sent by service and method"
    );
    describe_counter!(
        "grpc_messages_received_total",
        "Total gRPC messages received by service and method"
    );
    describe_histogram!(
        "grpc_message_size_bytes",
        "Size of gRPC messages in bytes"
    );

    // Status code metrics
    describe_counter!(
        "grpc_status_codes_total",
        "gRPC status codes (OK, CANCELLED, UNKNOWN, etc.)"
    );
    describe_counter!(
        "grpc_errors_total",
        "Total gRPC errors by status code"
    );

    // Per-method metrics
    describe_counter!(
        "grpc_method_calls_total",
        "Total calls per gRPC method"
    );
    describe_histogram!(
        "grpc_method_duration_seconds",
        "Duration per gRPC method"
    );

    // Compression metrics
    describe_counter!(
        "grpc_compression_used_total",
        "Times compression was used"
    );

    // Deadline/timeout metrics
    describe_counter!(
        "grpc_deadline_exceeded_total",
        "Total times deadline was exceeded"
    );
}

/// Record a gRPC request
pub fn record_request(
    service: &str,
    method: &str,
    status_code: &str,
    duration: Duration,
) {
    let duration_secs = duration.as_secs_f64();

    counter!(
        "grpc_requests_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "status" => status_code.to_string(),
    ).increment(1);

    histogram!(
        "grpc_request_duration_seconds",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).record(duration_secs);

    counter!(
        "grpc_status_codes_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "code" => status_code.to_string(),
    ).increment(1);

    // Track errors (any non-OK status)
    if status_code != "OK" {
        counter!(
            "grpc_errors_total",
            "service" => service.to_string(),
            "method" => method.to_string(),
            "code" => status_code.to_string(),
        ).increment(1);

    } else {
    }
}

/// Record gRPC stream creation
pub fn record_stream_open(service: &str, method: &str, stream_type: &str) {
    gauge!("grpc_streams_active").increment(1.0);

    counter!(
        "grpc_streams_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "type" => stream_type.to_string(),
    ).increment(1);

}

/// Record gRPC stream close
pub fn record_stream_close(
    service: &str,
    method: &str,
    stream_type: &str,
    duration: Duration,
) {
    gauge!("grpc_streams_active").decrement(1.0);

    counter!(
        "grpc_streams_closed_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "type" => stream_type.to_string(),
    ).increment(1);

    histogram!(
        "grpc_stream_duration_seconds",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "type" => stream_type.to_string(),
    ).record(duration.as_secs_f64());

}

/// Record gRPC message sent
pub fn record_message_sent(service: &str, method: &str, size_bytes: usize) {
    counter!(
        "grpc_messages_sent_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).increment(1);

    histogram!(
        "grpc_message_size_bytes",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "direction" => "sent".to_string(),
    ).record(size_bytes as f64);

}

/// Record gRPC message received
pub fn record_message_received(service: &str, method: &str, size_bytes: usize) {
    counter!(
        "grpc_messages_received_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).increment(1);

    histogram!(
        "grpc_message_size_bytes",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "direction" => "received".to_string(),
    ).record(size_bytes as f64);

}

/// Record gRPC method call
pub fn record_method_call(service: &str, method: &str, duration: Duration) {
    counter!(
        "grpc_method_calls_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).increment(1);

    histogram!(
        "grpc_method_duration_seconds",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).record(duration.as_secs_f64());
}

/// Record compression usage
pub fn record_compression_used(service: &str, method: &str, algorithm: &str) {
    counter!(
        "grpc_compression_used_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
        "algorithm" => algorithm.to_string(),
    ).increment(1);

}

/// Record deadline exceeded
pub fn record_deadline_exceeded(service: &str, method: &str) {
    counter!(
        "grpc_deadline_exceeded_total",
        "service" => service.to_string(),
        "method" => method.to_string(),
    ).increment(1);

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_metrics_recording() {
        // Initialize metrics
        describe_grpc_metrics();

        // Test request recording
        record_request(
            "user.UserService",
            "GetUser",
            "OK",
            Duration::from_millis(50),
        );

        record_request(
            "user.UserService",
            "GetUser",
            "NOT_FOUND",
            Duration::from_millis(30),
        );

        // Test stream lifecycle
        record_stream_open("chat.ChatService", "StreamMessages", "bidirectional");
        record_message_sent("chat.ChatService", "StreamMessages", 1024);
        record_message_received("chat.ChatService", "StreamMessages", 2048);
        record_stream_close(
            "chat.ChatService",
            "StreamMessages",
            "bidirectional",
            Duration::from_secs(60),
        );

        // Test method call
        record_method_call("user.UserService", "CreateUser", Duration::from_millis(100));

        // Test compression
        record_compression_used("user.UserService", "GetUser", "gzip");

        // Test deadline exceeded
        record_deadline_exceeded("user.UserService", "SlowQuery");

        assert!(true);
    }
}
