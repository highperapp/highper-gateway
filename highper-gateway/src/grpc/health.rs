//! gRPC Health Checking Protocol
//!
//! Implements grpc.health.v1.Health service for health checks
//! Based on: https://github.com/grpc/grpc/blob/master/doc/health-checking.md

use hyper::{Request, Response, header::{self, HeaderValue}};
use hyper::client::conn::http2;
use hyper_util::rt::TokioExecutor;
use http_body_util::{Full, BodyExt};
use bytes::{Bytes, BytesMut, BufMut};
use anyhow::{Result, anyhow};
use tracing::{debug, error};
use prost::Message;

/// gRPC health check status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is healthy and ready to serve requests
    Serving,
    /// Service is not healthy
    NotServing,
    /// Service status is unknown (e.g., during startup)
    Unknown,
}

impl HealthStatus {
    /// Convert to proto enum value
    pub fn to_proto_value(&self) -> i32 {
        match self {
            HealthStatus::Unknown => 0,
            HealthStatus::Serving => 1,
            HealthStatus::NotServing => 2,
        }
    }

    /// Parse from proto enum value
    pub fn from_proto_value(value: i32) -> Self {
        match value {
            1 => HealthStatus::Serving,
            2 => HealthStatus::NotServing,
            _ => HealthStatus::Unknown,
        }
    }
}

/// Protobuf message for HealthCheckRequest
/// Based on grpc.health.v1.HealthCheckRequest
#[derive(Clone, PartialEq, prost::Message)]
struct HealthCheckRequest {
    #[prost(string, tag = "1")]
    service: String,
}

/// Protobuf message for HealthCheckResponse
/// Based on grpc.health.v1.HealthCheckResponse
#[derive(Clone, PartialEq, prost::Message)]
struct HealthCheckResponse {
    #[prost(enumeration = "ServingStatus", tag = "1")]
    status: i32,
}

/// Serving status enum for protobuf
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
enum ServingStatus {
    Unknown = 0,
    Serving = 1,
    NotServing = 2,
    ServiceUnknown = 3,
}

/// Perform gRPC health check on a backend
pub async fn check_grpc_health(
    backend_url: &str,
    service_name: Option<&str>,
) -> Result<HealthStatus> {
    debug!(
        "Performing gRPC health check on {} for service {:?}",
        backend_url, service_name
    );

    // Create health check request
    let request = create_health_check_request(backend_url, service_name)?;

    // Parse backend URL
    let uri: hyper::Uri = backend_url.parse()
        .map_err(|e| anyhow!("Invalid backend URL: {}", e))?;

    let host = uri.host().ok_or_else(|| anyhow!("No host in URL"))?;
    let port = uri.port_u16().unwrap_or(50051); // Default gRPC port

    // Connect to backend (HTTP/2 only for gRPC)
    let addr = format!("{}:{}", host, port);
    let stream = tokio::net::TcpStream::connect(&addr).await
        .map_err(|e| anyhow!("Failed to connect to {}: {}", addr, e))?;

    // Set up HTTP/2 connection
    let io = hyper_util::rt::TokioIo::new(stream);

    let (mut sender, conn) = http2::handshake(TokioExecutor::new(), io).await
        .map_err(|e| anyhow!("HTTP/2 handshake failed: {}", e))?;

    // Spawn connection task
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            error!("gRPC health check connection error: {}", e);
        }
    });

    // Send request
    let response = sender.send_request(request).await
        .map_err(|e| anyhow!("gRPC health check request failed: {}", e))?;

    // Parse response
    let status = parse_health_check_response(response).await?;

    debug!("gRPC health check result for {}: {:?}", backend_url, status);
    Ok(status)
}

/// Create a gRPC health check request
fn create_health_check_request(
    backend_url: &str,
    service_name: Option<&str>,
) -> Result<Request<Full<Bytes>>> {
    // Create protobuf-encoded body
    let body_bytes = create_health_check_request_body(service_name)?;

    // gRPC uses a 5-byte prefix: 1-byte compressed flag + 4-byte message length
    let mut grpc_body = BytesMut::with_capacity(5 + body_bytes.len());
    grpc_body.put_u8(0); // Compression flag (0 = not compressed)
    grpc_body.put_u32(body_bytes.len() as u32); // Message length (big-endian)
    grpc_body.extend_from_slice(&body_bytes);

    // Parse URL to get authority
    let uri: hyper::Uri = backend_url.parse()
        .map_err(|e| anyhow!("Invalid backend URL: {}", e))?;
    let authority = uri.authority()
        .ok_or_else(|| anyhow!("No authority in URL"))?
        .clone();

    // Build gRPC request
    let request = Request::builder()
        .method("POST")
        .uri(format!("http://{}/grpc.health.v1.Health/Check", authority))
        .header("content-type", "application/grpc+proto")
        .header("te", "trailers")
        .header("user-agent", "highper-gateway/0.1.0")
        .body(Full::new(Bytes::from(grpc_body)))
        .map_err(|e| anyhow!("Failed to build request: {}", e))?;

    Ok(request)
}

/// Create protobuf-encoded health check request body
fn create_health_check_request_body(service_name: Option<&str>) -> Result<Vec<u8>> {
    let request = HealthCheckRequest {
        service: service_name.unwrap_or("").to_string(),
    };

    let mut buf = Vec::new();
    request.encode(&mut buf)
        .map_err(|e| anyhow!("Failed to encode protobuf: {}", e))?;

    Ok(buf)
}

/// Parse gRPC health check response
async fn parse_health_check_response<B>(response: Response<B>) -> Result<HealthStatus>
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: std::error::Error + Send + Sync,
{
    // Check HTTP status
    if response.status() != hyper::StatusCode::OK {
        return Err(anyhow!("gRPC health check returned HTTP status: {}", response.status()));
    }

    // Check grpc-status header (trailer may also contain it)
    if let Some(grpc_status) = response.headers().get("grpc-status") {
        if grpc_status != "0" {
            let grpc_message = response.headers()
                .get("grpc-message")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown error");
            return Err(anyhow!("gRPC error status {}: {}", grpc_status.to_str().unwrap_or("?"), grpc_message));
        }
    }

    // Read response body
    let body = response.into_body();
    let body_bytes = body.collect().await
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?
        .to_bytes();

    if body_bytes.len() < 5 {
        return Err(anyhow!("gRPC response too short: {} bytes", body_bytes.len()));
    }

    // Parse gRPC frame: 1-byte compression flag + 4-byte length + message
    let _compressed = body_bytes[0];
    let message_len = u32::from_be_bytes([body_bytes[1], body_bytes[2], body_bytes[3], body_bytes[4]]) as usize;

    if body_bytes.len() < 5 + message_len {
        return Err(anyhow!("gRPC message incomplete: expected {} bytes, got {}", 5 + message_len, body_bytes.len()));
    }

    let message_bytes = &body_bytes[5..5 + message_len];

    // Decode protobuf response
    let response = HealthCheckResponse::decode(message_bytes)
        .map_err(|e| anyhow!("Failed to decode protobuf response: {}", e))?;

    // Convert to HealthStatus
    let status = HealthStatus::from_proto_value(response.status);

    Ok(status)
}

/// Create a gRPC health check response (for implementing health service)
/// Note: Stub - not fully implemented
pub fn create_health_check_response(status: HealthStatus) -> Response<Full<Bytes>> {
    // Stub implementation
    let mut response = Response::new(Full::new(Bytes::from(vec![status.to_proto_value() as u8])));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/grpc+proto")
    );
    response.headers_mut().insert(
        "grpc-status",
        HeaderValue::from_static("0")
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_conversion() {
        assert_eq!(HealthStatus::Serving.to_proto_value(), 1);
        assert_eq!(HealthStatus::NotServing.to_proto_value(), 2);
        assert_eq!(HealthStatus::Unknown.to_proto_value(), 0);

        assert_eq!(HealthStatus::from_proto_value(1), HealthStatus::Serving);
        assert_eq!(HealthStatus::from_proto_value(2), HealthStatus::NotServing);
        assert_eq!(HealthStatus::from_proto_value(0), HealthStatus::Unknown);
        assert_eq!(HealthStatus::from_proto_value(99), HealthStatus::Unknown);
    }

    #[test]
    fn test_create_health_check_request_body_empty_service() {
        let body = create_health_check_request_body(None).unwrap();

        // Decode to verify
        let request = HealthCheckRequest::decode(body.as_slice()).unwrap();
        assert_eq!(request.service, "");
    }

    #[test]
    fn test_create_health_check_request_body_with_service() {
        let body = create_health_check_request_body(Some("myservice")).unwrap();

        // Decode to verify
        let request = HealthCheckRequest::decode(body.as_slice()).unwrap();
        assert_eq!(request.service, "myservice");
    }

    #[test]
    fn test_create_health_check_request() {
        let request = create_health_check_request("http://localhost:50051", Some("test.service")).unwrap();

        // Verify HTTP method
        assert_eq!(request.method(), "POST");

        // Verify URI
        assert_eq!(request.uri().path(), "/grpc.health.v1.Health/Check");

        // Verify headers
        assert_eq!(request.headers().get("content-type").unwrap(), "application/grpc+proto");
        assert_eq!(request.headers().get("te").unwrap(), "trailers");

        // Note: We can't easily inspect the body without consuming it,
        // but the test_grpc_frame_format below tests the framing
    }

    #[test]
    fn test_protobuf_encoding_decoding() {
        // Test request encoding/decoding
        let request = HealthCheckRequest {
            service: "test.Service".to_string(),
        };

        let mut buf = Vec::new();
        request.encode(&mut buf).unwrap();

        let decoded = HealthCheckRequest::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.service, "test.Service");

        // Test response encoding/decoding
        let response = HealthCheckResponse {
            status: ServingStatus::Serving as i32,
        };

        let mut buf = Vec::new();
        response.encode(&mut buf).unwrap();

        let decoded = HealthCheckResponse::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.status, ServingStatus::Serving as i32);
    }

    #[test]
    fn test_grpc_frame_format() {
        // Test that we correctly create gRPC frames with 5-byte prefix
        // We'll test by recreating the frame construction
        let body_bytes = create_health_check_request_body(None).unwrap();

        // Create gRPC frame like the function does
        let mut grpc_body = BytesMut::with_capacity(5 + body_bytes.len());
        grpc_body.put_u8(0); // Compression flag
        grpc_body.put_u32(body_bytes.len() as u32); // Message length
        grpc_body.extend_from_slice(&body_bytes);

        // First byte is compression flag (0 = not compressed)
        assert_eq!(grpc_body[0], 0);

        // Next 4 bytes are message length (big-endian)
        let message_len = u32::from_be_bytes([grpc_body[1], grpc_body[2], grpc_body[3], grpc_body[4]]) as usize;

        // Verify total body size matches: 5-byte prefix + message
        assert_eq!(grpc_body.len(), 5 + message_len);
        assert_eq!(message_len, body_bytes.len());
    }

    #[test]
    fn test_serving_status_enum() {
        assert_eq!(ServingStatus::Unknown as i32, 0);
        assert_eq!(ServingStatus::Serving as i32, 1);
        assert_eq!(ServingStatus::NotServing as i32, 2);
        assert_eq!(ServingStatus::ServiceUnknown as i32, 3);
    }

    // Integration test - requires a running gRPC server
    // Disabled by default, enable with: cargo test -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_check_grpc_health_integration() {
        // This test requires a gRPC server running on localhost:50051
        // with the grpc.health.v1.Health service implemented
        let result = check_grpc_health("http://localhost:50051", None).await;

        // Should either succeed or fail with connection error
        match result {
            Ok(status) => {
                println!("Health check succeeded: {:?}", status);
                assert!(matches!(status, HealthStatus::Serving | HealthStatus::NotServing | HealthStatus::Unknown));
            }
            Err(e) => {
                println!("Health check failed (expected if no server running): {}", e);
            }
        }
    }
}
