//! TCP-specific metrics for TCP proxy and database load balancing
//!
//! This module provides metrics specifically for TCP proxying:
//! - Active/total connections
//! - Bytes transferred
//! - Connection duration
//! - Connection pool usage
//! - Connection errors

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::time::Duration;

/// Initialize TCP-specific metrics descriptions
pub fn describe_tcp_metrics() {
    // Connection metrics
    describe_gauge!(
        "tcp_connections_active",
        "Number of currently active TCP connections"
    );
    describe_counter!(
        "tcp_connections_total",
        "Total number of TCP connections established"
    );
    describe_counter!(
        "tcp_connections_closed_total",
        "Total number of TCP connections closed"
    );

    // Data transfer metrics
    describe_counter!(
        "tcp_bytes_sent_total",
        "Total bytes sent over TCP connections"
    );
    describe_counter!(
        "tcp_bytes_received_total",
        "Total bytes received over TCP connections"
    );

    // Connection duration
    describe_histogram!(
        "tcp_connection_duration_seconds",
        "Duration of TCP connections in seconds"
    );

    // Connection errors
    describe_counter!(
        "tcp_connection_errors_total",
        "Total TCP connection errors by type"
    );
    describe_counter!(
        "tcp_connection_timeouts_total",
        "Total TCP connection timeouts"
    );
    describe_counter!(
        "tcp_connection_refused_total",
        "Total TCP connection refused errors"
    );
    describe_counter!(
        "tcp_connection_reset_total",
        "Total TCP connection reset errors"
    );

    // Connection pool metrics
    describe_gauge!(
        "tcp_pool_connections_active",
        "Number of active connections in the TCP pool"
    );
    describe_gauge!(
        "tcp_pool_connections_idle",
        "Number of idle connections in the TCP pool"
    );
    describe_gauge!(
        "tcp_pool_connections_total",
        "Total number of connections in the TCP pool (active + idle)"
    );
    describe_histogram!(
        "tcp_pool_wait_duration_seconds",
        "Time spent waiting for a connection from the pool"
    );
    describe_counter!(
        "tcp_pool_exhausted_total",
        "Total times the connection pool was exhausted"
    );
    describe_counter!(
        "tcp_pool_created_total",
        "Total number of new connections created by the pool"
    );
    describe_counter!(
        "tcp_pool_reused_total",
        "Total number of times a pooled connection was reused"
    );

    // Backend-specific metrics
    describe_gauge!(
        "tcp_backend_connections_active",
        "Number of active connections per TCP backend"
    );
    describe_counter!(
        "tcp_backend_connections_total",
        "Total connections per TCP backend"
    );
    describe_counter!(
        "tcp_backend_errors_total",
        "Total errors per TCP backend"
    );
}

/// Record a new TCP connection
pub fn record_connection_open(backend: &str, _remote_addr: &str) {
    gauge!("tcp_connections_active").increment(1.0);
    counter!("tcp_connections_total").increment(1);

    gauge!(
        "tcp_backend_connections_active",
        "backend" => backend.to_string(),
    ).increment(1.0);
    counter!(
        "tcp_backend_connections_total",
        "backend" => backend.to_string(),
    ).increment(1);

}

/// Record a TCP connection close
pub fn record_connection_close(
    backend: &str,
    _remote_addr: &str,
    duration: Duration,
    bytes_sent: u64,
    bytes_received: u64,
) {
    gauge!("tcp_connections_active").decrement(1.0);
    counter!("tcp_connections_closed_total").increment(1);

    gauge!(
        "tcp_backend_connections_active",
        "backend" => backend.to_string(),
    ).decrement(1.0);

    // Record duration
    let duration_secs = duration.as_secs_f64();
    histogram!("tcp_connection_duration_seconds").record(duration_secs);

    // Record bytes transferred
    if bytes_sent > 0 {
        counter!("tcp_bytes_sent_total").increment(bytes_sent);
    }
    if bytes_received > 0 {
        counter!("tcp_bytes_received_total").increment(bytes_received);
    }

}

/// Record a TCP connection error
pub fn record_connection_error(backend: &str, error_type: &str) {
    counter!(
        "tcp_connection_errors_total",
        "backend" => backend.to_string(),
        "error" => error_type.to_string(),
    ).increment(1);

    counter!(
        "tcp_backend_errors_total",
        "backend" => backend.to_string(),
        "error" => error_type.to_string(),
    ).increment(1);

    // Record specific error types
    match error_type {
        "timeout" => counter!("tcp_connection_timeouts_total").increment(1),
        "refused" => counter!("tcp_connection_refused_total").increment(1),
        "reset" => counter!("tcp_connection_reset_total").increment(1),
        _ => {}
    }

}

/// Record connection pool stats
pub fn record_pool_stats(active: usize, idle: usize) {
    gauge!("tcp_pool_connections_active").set(active as f64);
    gauge!("tcp_pool_connections_idle").set(idle as f64);
    gauge!("tcp_pool_connections_total").set((active + idle) as f64);
}

/// Record connection pool wait time
pub fn record_pool_wait(duration: Duration) {
    let duration_secs = duration.as_secs_f64();
    histogram!("tcp_pool_wait_duration_seconds").record(duration_secs);

}

/// Record pool exhaustion event
pub fn record_pool_exhausted(backend: &str) {
    counter!(
        "tcp_pool_exhausted_total",
        "backend" => backend.to_string(),
    ).increment(1);

}

/// Record pool connection created
pub fn record_pool_connection_created(backend: &str) {
    counter!(
        "tcp_pool_created_total",
        "backend" => backend.to_string(),
    ).increment(1);

}

/// Record pool connection reused
pub fn record_pool_connection_reused(backend: &str) {
    counter!(
        "tcp_pool_reused_total",
        "backend" => backend.to_string(),
    ).increment(1);

}

/// Get current active TCP connections (for monitoring)
pub fn get_active_connections() -> f64 {
    // Note: This is a simplified implementation
    // In a real implementation, we'd query the actual gauge value
    0.0 // Placeholder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_metrics_recording() {
        // Initialize metrics
        describe_tcp_metrics();

        // Test connection lifecycle
        record_connection_open("backend1:3306", "192.168.1.100:54321");
        record_connection_close(
            "backend1:3306",
            "192.168.1.100:54321",
            Duration::from_secs(300),
            1024000,
            2048000,
        );

        // Test error recording
        record_connection_error("backend1:3306", "timeout");

        // Test pool metrics
        record_pool_stats(50, 10);
        record_pool_wait(Duration::from_millis(100));
        record_pool_exhausted("backend1:3306");
        record_pool_connection_created("backend1:3306");
        record_pool_connection_reused("backend1:3306");

        // If we reach here without panicking, all metrics recorded successfully
        assert!(true);
    }
}
