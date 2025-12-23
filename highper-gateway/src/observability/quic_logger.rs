//! QUIC Connection Structured Logging
//!
//! Provides structured JSON logging for QUIC/HTTP/3:
//! - Connection lifecycle events
//! - 0-RTT connection attempts
//! - Connection migration
//! - Stream lifecycle
//! - Packet loss and congestion events
//! - Protocol errors

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// QUIC Connection ID for tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuicConnectionId(String);

impl QuicConnectionId {
    /// Generate a new QUIC connection ID
    pub fn new() -> Self {
        Self(format!("quic-{}", Uuid::now_v7()))
    }

    /// Get the ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for QuicConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for QuicConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// QUIC connection context for structured logging
#[derive(Debug, Clone, Serialize)]
pub struct QuicConnectionContext {
    pub connection_id: String,
    pub client_addr: String,
    pub server_name: String,
    pub protocol_version: String, // e.g., "h3-29"
}

impl QuicConnectionContext {
    pub fn new(
        connection_id: QuicConnectionId,
        client_addr: SocketAddr,
        server_name: String,
        protocol_version: String,
    ) -> Self {
        Self {
            connection_id: connection_id.to_string(),
            client_addr: client_addr.to_string(),
            server_name,
            protocol_version,
        }
    }
}

/// Log QUIC connection established
pub fn log_connection_established(
    ctx: &QuicConnectionContext,
    zero_rtt: bool,
    initial_rtt: Option<Duration>,
) {
    info!(
        event = "quic_connection_established",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        protocol_version = %ctx.protocol_version,
        zero_rtt = zero_rtt,
        initial_rtt_ms = initial_rtt.map(|d| d.as_millis() as u64),
        "QUIC connection established"
    );
}

/// Log QUIC connection closed
pub fn log_connection_closed(
    ctx: &QuicConnectionContext,
    duration: Duration,
    bytes_sent: u64,
    bytes_received: u64,
    streams_opened: u64,
    error: Option<&str>,
) {
    let duration_ms = duration.as_millis() as u64;

    if let Some(err) = error {
        warn!(
            event = "quic_connection_closed",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            duration_ms = duration_ms,
            bytes_sent = bytes_sent,
            bytes_received = bytes_received,
            streams_opened = streams_opened,
            error = err,
            "QUIC connection closed with error"
        );
    } else {
        info!(
            event = "quic_connection_closed",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            duration_ms = duration_ms,
            bytes_sent = bytes_sent,
            bytes_received = bytes_received,
            streams_opened = streams_opened,
            "QUIC connection closed"
        );
    }
}

/// Log 0-RTT connection attempt
pub fn log_zero_rtt_attempt(
    ctx: &QuicConnectionContext,
    session_ticket_age: Duration,
) {
    debug!(
        event = "quic_zero_rtt_attempt",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        session_ticket_age_ms = session_ticket_age.as_millis() as u64,
        "0-RTT connection attempt"
    );
}

/// Log 0-RTT result
pub fn log_zero_rtt_result(
    ctx: &QuicConnectionContext,
    accepted: bool,
    rejection_reason: Option<&str>,
) {
    if accepted {
        info!(
            event = "quic_zero_rtt_accepted",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            "0-RTT connection accepted"
        );
    } else {
        warn!(
            event = "quic_zero_rtt_rejected",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            reason = rejection_reason.unwrap_or("unknown"),
            "0-RTT connection rejected"
        );
    }
}

/// Log connection migration
pub fn log_connection_migration(
    ctx: &QuicConnectionContext,
    old_addr: SocketAddr,
    new_addr: SocketAddr,
    success: bool,
) {
    if success {
        info!(
            event = "quic_connection_migrated",
            connection_id = %ctx.connection_id,
            server_name = %ctx.server_name,
            old_client_addr = %old_addr,
            new_client_addr = %new_addr,
            "QUIC connection migrated to new address"
        );
    } else {
        warn!(
            event = "quic_connection_migration_failed",
            connection_id = %ctx.connection_id,
            server_name = %ctx.server_name,
            old_client_addr = %old_addr,
            new_client_addr = %new_addr,
            "QUIC connection migration failed"
        );
    }
}

/// Log QUIC stream opened
pub fn log_stream_opened(
    ctx: &QuicConnectionContext,
    stream_id: u64,
    stream_type: &str, // "unidirectional" or "bidirectional"
    initiated_by: &str, // "client" or "server"
) {
    debug!(
        event = "quic_stream_opened",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        stream_id = stream_id,
        stream_type = stream_type,
        initiated_by = initiated_by,
        "QUIC stream opened"
    );
}

/// Log QUIC stream closed
pub fn log_stream_closed(
    ctx: &QuicConnectionContext,
    stream_id: u64,
    duration: Duration,
    bytes_sent: u64,
    bytes_received: u64,
) {
    trace!(
        event = "quic_stream_closed",
        connection_id = %ctx.connection_id,
        server_name = %ctx.server_name,
        stream_id = stream_id,
        duration_ms = duration.as_millis() as u64,
        bytes_sent = bytes_sent,
        bytes_received = bytes_received,
        "QUIC stream closed"
    );
}

/// Log packet loss event
pub fn log_packet_loss(
    ctx: &QuicConnectionContext,
    packets_lost: u64,
    total_packets: u64,
    loss_ratio: f64,
) {
    if loss_ratio > 0.05 {
        // More than 5% loss
        warn!(
            event = "quic_packet_loss",
            connection_id = %ctx.connection_id,
            client_addr = %ctx.client_addr,
            server_name = %ctx.server_name,
            packets_lost = packets_lost,
            total_packets = total_packets,
            loss_ratio = format!("{:.2}%", loss_ratio * 100.0),
            severity = "high",
            "High packet loss detected"
        );
    } else {
        debug!(
            event = "quic_packet_loss",
            connection_id = %ctx.connection_id,
            server_name = %ctx.server_name,
            packets_lost = packets_lost,
            total_packets = total_packets,
            loss_ratio = format!("{:.2}%", loss_ratio * 100.0),
            severity = "normal",
            "Packet loss detected"
        );
    }
}

/// Log RTT measurement
pub fn log_rtt_update(
    ctx: &QuicConnectionContext,
    latest_rtt: Duration,
    min_rtt: Duration,
    smoothed_rtt: Duration,
) {
    trace!(
        event = "quic_rtt_update",
        connection_id = %ctx.connection_id,
        server_name = %ctx.server_name,
        latest_rtt_ms = latest_rtt.as_millis() as u64,
        min_rtt_ms = min_rtt.as_millis() as u64,
        smoothed_rtt_ms = smoothed_rtt.as_millis() as u64,
        "QUIC RTT measurement updated"
    );
}

/// Log congestion event
pub fn log_congestion_event(
    ctx: &QuicConnectionContext,
    congestion_window: u64,
    bytes_in_flight: u64,
    event_type: &str, // "fast_retransmit", "timeout", "explicit_congestion"
) {
    warn!(
        event = "quic_congestion_event",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        congestion_window = congestion_window,
        bytes_in_flight = bytes_in_flight,
        event_type = event_type,
        "QUIC congestion event"
    );
}

/// Log flow control blocked
pub fn log_flow_control_blocked(
    ctx: &QuicConnectionContext,
    stream_id: Option<u64>,
    blocked_duration: Duration,
) {
    debug!(
        event = "quic_flow_control_blocked",
        connection_id = %ctx.connection_id,
        server_name = %ctx.server_name,
        stream_id = ?stream_id,
        blocked_ms = blocked_duration.as_millis() as u64,
        "QUIC flow control blocked"
    );
}

/// Log QUIC protocol error
pub fn log_protocol_error(
    ctx: &QuicConnectionContext,
    error_code: u64,
    error_type: &str,
    error_message: &str,
) {
    error!(
        event = "quic_protocol_error",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        error_code = error_code,
        error_type = error_type,
        error_message = error_message,
        "QUIC protocol error"
    );
}

/// Log QUIC timeout
pub fn log_connection_timeout(
    ctx: &QuicConnectionContext,
    idle_timeout: Duration,
) {
    warn!(
        event = "quic_connection_timeout",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        idle_timeout_ms = idle_timeout.as_millis() as u64,
        "QUIC connection idle timeout"
    );
}

/// Log HTTP/3 request
pub fn log_http3_request(
    ctx: &QuicConnectionContext,
    stream_id: u64,
    method: &str,
    path: &str,
    status: u16,
    duration: Duration,
) {
    info!(
        event = "http3_request",
        connection_id = %ctx.connection_id,
        client_addr = %ctx.client_addr,
        server_name = %ctx.server_name,
        stream_id = stream_id,
        method = method,
        path = path,
        status = status,
        duration_ms = duration.as_millis() as u64,
        "HTTP/3 request"
    );
}

/// Log HTTP/3 server push
pub fn log_http3_push_promise(
    ctx: &QuicConnectionContext,
    push_id: u64,
    request_stream_id: u64,
    pushed_path: &str,
) {
    debug!(
        event = "http3_push_promise",
        connection_id = %ctx.connection_id,
        server_name = %ctx.server_name,
        push_id = push_id,
        request_stream_id = request_stream_id,
        pushed_path = pushed_path,
        "HTTP/3 server push promise sent"
    );
}

/// Log version negotiation
pub fn log_version_negotiation(
    client_addr: SocketAddr,
    server_name: &str,
    client_version: u32,
    server_versions: &[u32],
    negotiated_version: u32,
) {
    debug!(
        event = "quic_version_negotiation",
        client_addr = %client_addr,
        server_name = server_name,
        client_version = client_version,
        server_versions = ?server_versions,
        negotiated_version = negotiated_version,
        "QUIC version negotiation completed"
    );
}

/// Log stateless reset
pub fn log_stateless_reset(
    client_addr: SocketAddr,
    server_name: &str,
) {
    warn!(
        event = "quic_stateless_reset",
        client_addr = %client_addr,
        server_name = server_name,
        "QUIC stateless reset sent"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_quic_connection_id() {
        let id1 = QuicConnectionId::new();
        let id2 = QuicConnectionId::new();

        assert!(id1.as_str().starts_with("quic-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_quic_logging() {
        let conn_id = QuicConnectionId::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 51234);
        let ctx = QuicConnectionContext::new(
            conn_id.clone(),
            client_addr,
            "h3.example.com".to_string(),
            "h3-29".to_string(),
        );

        // Test connection lifecycle
        log_connection_established(&ctx, true, Some(Duration::from_millis(50)));
        log_connection_closed(&ctx, Duration::from_secs(120), 1048576, 2097152, 10, None);

        // Test 0-RTT
        log_zero_rtt_attempt(&ctx, Duration::from_secs(3600));
        log_zero_rtt_result(&ctx, true, None);

        // Test migration
        let new_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 51235);
        log_connection_migration(&ctx, client_addr, new_addr, true);

        // Test streams
        log_stream_opened(&ctx, 0, "bidirectional", "client");
        log_stream_closed(&ctx, 0, Duration::from_secs(30), 65536, 131072);

        // Test packet loss
        log_packet_loss(&ctx, 10, 1000, 0.01);

        // Test RTT
        log_rtt_update(
            &ctx,
            Duration::from_millis(50),
            Duration::from_millis(40),
            Duration::from_millis(45),
        );

        // Test congestion
        log_congestion_event(&ctx, 65536, 32768, "fast_retransmit");

        // Test HTTP/3
        log_http3_request(&ctx, 0, "GET", "/api/users", 200, Duration::from_millis(100));

        assert!(true);
    }
}
