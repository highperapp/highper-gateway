//! QUIC/HTTP/3-specific metrics
//!
//! This module provides metrics specifically for QUIC and HTTP/3:
//! - QUIC connections
//! - 0-RTT usage
//! - Packet loss
//! - RTT measurements
//! - Stream multiplexing
//! - Connection migration

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::time::Duration;

/// Initialize QUIC-specific metrics descriptions
pub fn describe_quic_metrics() {
    // Connection metrics
    describe_counter!(
        "quic_connections_total",
        "Total QUIC connections established"
    );
    describe_gauge!(
        "quic_connections_active",
        "Number of currently active QUIC connections"
    );
    describe_counter!(
        "quic_connections_closed_total",
        "Total QUIC connections closed"
    );

    // 0-RTT metrics
    describe_counter!(
        "quic_zero_rtt_attempts_total",
        "Total 0-RTT connection attempts"
    );
    describe_counter!(
        "quic_zero_rtt_success_total",
        "Successful 0-RTT connections"
    );
    describe_counter!(
        "quic_zero_rtt_rejected_total",
        "Rejected 0-RTT connections"
    );

    // Packet loss metrics
    describe_gauge!(
        "quic_packet_loss_ratio",
        "Current packet loss ratio"
    );
    describe_counter!(
        "quic_packets_sent_total",
        "Total packets sent"
    );
    describe_counter!(
        "quic_packets_received_total",
        "Total packets received"
    );
    describe_counter!(
        "quic_packets_lost_total",
        "Total packets lost"
    );

    // RTT metrics
    describe_histogram!(
        "quic_rtt_seconds",
        "Round-trip time in seconds"
    );
    describe_gauge!(
        "quic_rtt_min_seconds",
        "Minimum RTT observed"
    );
    describe_gauge!(
        "quic_rtt_smoothed_seconds",
        "Smoothed RTT estimate"
    );

    // Stream metrics
    describe_gauge!(
        "quic_streams_active",
        "Number of currently active streams"
    );
    describe_counter!(
        "quic_streams_total",
        "Total streams created"
    );
    describe_counter!(
        "quic_streams_closed_total",
        "Total streams closed"
    );
    describe_counter!(
        "quic_streams_by_type_total",
        "Streams by type (unidirectional/bidirectional)"
    );

    // Connection migration
    describe_counter!(
        "quic_migration_events_total",
        "Total connection migration events"
    );
    describe_counter!(
        "quic_migration_success_total",
        "Successful connection migrations"
    );
    describe_counter!(
        "quic_migration_failure_total",
        "Failed connection migrations"
    );

    // Congestion control
    describe_counter!(
        "quic_congestion_events_total",
        "Total congestion control events"
    );
    describe_gauge!(
        "quic_congestion_window_bytes",
        "Current congestion window size in bytes"
    );
    describe_counter!(
        "quic_congestion_limited_total",
        "Times sending was congestion-limited"
    );

    // Flow control
    describe_counter!(
        "quic_flow_control_blocked_total",
        "Times blocked by flow control"
    );
    describe_gauge!(
        "quic_flow_control_window_bytes",
        "Current flow control window size"
    );

    // Errors
    describe_counter!(
        "quic_errors_total",
        "Total QUIC errors by type"
    );
    describe_counter!(
        "quic_protocol_violations_total",
        "Total QUIC protocol violations"
    );
    describe_counter!(
        "quic_timeouts_total",
        "Total QUIC connection timeouts"
    );

    // HTTP/3 specific
    describe_counter!(
        "http3_requests_total",
        "Total HTTP/3 requests"
    );
    describe_histogram!(
        "http3_request_duration_seconds",
        "HTTP/3 request duration"
    );
    describe_counter!(
        "http3_push_promises_total",
        "Total HTTP/3 server push promises"
    );
}

/// Record a new QUIC connection
pub fn record_connection_open(server_name: &str, zero_rtt: bool) {
    gauge!("quic_connections_active").increment(1.0);
    counter!("quic_connections_total", "server_name" => server_name.to_string()).increment(1);

    if zero_rtt {
        counter!("quic_zero_rtt_attempts_total", "server_name" => server_name.to_string()).increment(1);
    }
}

/// Record QUIC connection close
pub fn record_connection_close(server_name: &str, duration: Duration) {
    gauge!("quic_connections_active").decrement(1.0);
    counter!("quic_connections_closed_total", "server_name" => server_name.to_string()).increment(1);

}

/// Record 0-RTT result
pub fn record_zero_rtt_result(server_name: &str, success: bool) {
    if success {
        counter!("quic_zero_rtt_success_total", "server_name" => server_name.to_string()).increment(1);
    } else {
        counter!("quic_zero_rtt_rejected_total", "server_name" => server_name.to_string()).increment(1);
    }
}

/// Record packet loss
pub fn record_packet_stats(server_name: &str, sent: u64, received: u64, lost: u64) {
    counter!("quic_packets_sent_total", "server_name" => server_name.to_string()).increment(sent);
    counter!("quic_packets_received_total", "server_name" => server_name.to_string()).increment(received);
    counter!("quic_packets_lost_total", "server_name" => server_name.to_string()).increment(lost);

    // Calculate loss ratio
    if sent > 0 {
        let loss_ratio = (lost as f64) / (sent as f64);
        gauge!("quic_packet_loss_ratio", "server_name" => server_name.to_string()).set(loss_ratio);

        if loss_ratio > 0.05 {
            // More than 5% loss
        }
    }
}

/// Record RTT measurement
pub fn record_rtt(server_name: &str, rtt: Duration, min_rtt: Duration, smoothed_rtt: Duration) {
    let rtt_secs = rtt.as_secs_f64();

    histogram!("quic_rtt_seconds", "server_name" => server_name.to_string()).record(rtt_secs);
    gauge!("quic_rtt_min_seconds", "server_name" => server_name.to_string()).set(min_rtt.as_secs_f64());
    gauge!("quic_rtt_smoothed_seconds", "server_name" => server_name.to_string()).set(smoothed_rtt.as_secs_f64());

}

/// Record stream creation
pub fn record_stream_open(server_name: &str, stream_type: &str) {
    gauge!("quic_streams_active", "server_name" => server_name.to_string()).increment(1.0);
    counter!("quic_streams_total", "server_name" => server_name.to_string()).increment(1);
    counter!(
        "quic_streams_by_type_total",
        "server_name" => server_name.to_string(),
        "type" => stream_type.to_string(),
    ).increment(1);

}

/// Record stream close
pub fn record_stream_close(server_name: &str) {
    gauge!("quic_streams_active", "server_name" => server_name.to_string()).decrement(1.0);
    counter!("quic_streams_closed_total", "server_name" => server_name.to_string()).increment(1);

}

/// Record connection migration
pub fn record_migration(server_name: &str, success: bool) {
    counter!("quic_migration_events_total", "server_name" => server_name.to_string()).increment(1);

    if success {
        counter!("quic_migration_success_total", "server_name" => server_name.to_string()).increment(1);
    } else {
        counter!("quic_migration_failure_total", "server_name" => server_name.to_string()).increment(1);
    }
}

/// Record congestion event
pub fn record_congestion_event(server_name: &str, congestion_window: u64) {
    counter!("quic_congestion_events_total", "server_name" => server_name.to_string()).increment(1);
    gauge!("quic_congestion_window_bytes", "server_name" => server_name.to_string()).set(congestion_window as f64);

}

/// Record QUIC error
pub fn record_quic_error(server_name: &str, error_type: &str) {
    counter!(
        "quic_errors_total",
        "server_name" => server_name.to_string(),
        "error" => error_type.to_string(),
    ).increment(1);

    match error_type {
        "protocol_violation" => {
            counter!("quic_protocol_violations_total", "server_name" => server_name.to_string()).increment(1)
        }
        "timeout" => {
            counter!("quic_timeouts_total", "server_name" => server_name.to_string()).increment(1)
        }
        _ => {}
    }

}

/// Record HTTP/3 request
pub fn record_http3_request(server_name: &str, method: &str, status: u16, duration: Duration) {
    counter!(
        "http3_requests_total",
        "server_name" => server_name.to_string(),
        "method" => method.to_string(),
        "status" => status.to_string(),
    ).increment(1);

    histogram!(
        "http3_request_duration_seconds",
        "server_name" => server_name.to_string(),
        "method" => method.to_string(),
    ).record(duration.as_secs_f64());

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_metrics_recording() {
        // Initialize metrics
        describe_quic_metrics();

        // Test connection
        record_connection_open("h3.example.com", true);
        record_zero_rtt_result("h3.example.com", true);
        record_connection_close("h3.example.com", Duration::from_secs(120));

        // Test packet stats
        record_packet_stats("h3.example.com", 1000, 990, 10);

        // Test RTT
        record_rtt(
            "h3.example.com",
            Duration::from_millis(50),
            Duration::from_millis(40),
            Duration::from_millis(45),
        );

        // Test streams
        record_stream_open("h3.example.com", "bidirectional");
        record_stream_close("h3.example.com");

        // Test migration
        record_migration("h3.example.com", true);

        // Test congestion
        record_congestion_event("h3.example.com", 65536);

        // Test errors
        record_quic_error("h3.example.com", "protocol_violation");

        // Test HTTP/3
        record_http3_request("h3.example.com", "GET", 200, Duration::from_millis(100));

        assert!(true);
    }
}
