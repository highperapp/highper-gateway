//! gRPC request handler and proxying
//!
//! Handles gRPC proxying including streaming support

use super::{GrpcRequest, GrpcStatusCode};
use hyper::{Request, Response, StatusCode, header::{self, HeaderValue}};
use hyper::body::Incoming;
use http_body_util::Full;
use tracing::debug;
use anyhow::{Result, anyhow};
use bytes::Bytes;

/// Create a gRPC error response
pub fn create_grpc_error_response(
    status_code: GrpcStatusCode,
    message: &str,
) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = StatusCode::OK; // gRPC always uses 200 OK

    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/grpc")
    );

    // Set grpc-status trailer
    headers.insert(
        "grpc-status",
        HeaderValue::from_str(&(status_code as i32).to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("13")) // Internal error
    );

    // Set grpc-message trailer
    if let Ok(msg) = HeaderValue::from_str(message) {
        headers.insert("grpc-message", msg);
    }

    response
}

/// Proxy a gRPC request to backend with full streaming support
///
/// Supports all gRPC call types:
/// - Unary: single request → single response
/// - Client streaming: stream of requests → single response
/// - Server streaming: single request → stream of responses
/// - Bidirectional: stream of requests ↔ stream of responses
pub async fn proxy_grpc_request(
    grpc_req: &GrpcRequest,
    backend_url: &str,
    client_request: Request<Incoming>,
) -> Result<Response<Incoming>> {
    use hyper_util::client::legacy::Client as HyperClient;
    use hyper_util::client::legacy::connect::HttpConnector;
    use hyper_util::rt::TokioExecutor;

    debug!("Proxying gRPC request: {} (type: {:?})", grpc_req.path, grpc_req.call_type);

    // Build backend URI
    let backend_uri = format!("{}{}", backend_url.trim_end_matches('/'), grpc_req.path);
    let uri = backend_uri.parse::<hyper::Uri>()
        .map_err(|e| anyhow!("Invalid backend URI: {}", e))?;

    // Create request builder
    let mut backend_req = Request::builder()
        .method(hyper::Method::POST) // gRPC always uses POST
        .uri(uri)
        .version(hyper::Version::HTTP_2); // gRPC requires HTTP/2

    // Forward all headers from client request
    let client_headers = client_request.headers();
    for (name, value) in client_headers {
        backend_req = backend_req.header(name, value);
    }

    // Ensure gRPC content-type is set
    if !client_headers.contains_key(header::CONTENT_TYPE) {
        backend_req = backend_req.header(
            header::CONTENT_TYPE,
            grpc_req.content_type.as_str()
        );
    }

    // Add grpc-timeout if specified
    if let Some(timeout) = grpc_req.timeout {
        backend_req = backend_req.header(
            "grpc-timeout",
            timeout_to_header(timeout)
        );
    }

    // Stream the body directly without buffering (zero-copy)
    let body = client_request.into_body();
    let backend_request = backend_req.body(body)
        .map_err(|e| anyhow!("Failed to build backend request: {}", e))?;

    // Create HTTP/2-only client for gRPC
    let connector = HttpConnector::new();
    let client = HyperClient::builder(TokioExecutor::new())
        .http2_only(true) // gRPC requires HTTP/2
        .build(connector);

    // Send request and stream response (supports all streaming types)
    let response = client.request(backend_request).await
        .map_err(|e| anyhow!("Backend request failed: {}", e))?;

    debug!("gRPC backend response: status={}, headers={:?}",
        response.status(), response.headers());

    // Return streaming response directly (preserves trailers)
    Ok(response)
}

/// Extract gRPC status from response headers
pub fn extract_grpc_status<B>(response: &Response<B>) -> Option<GrpcStatusCode> {
    response.headers()
        .get("grpc-status")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i32>().ok())
        .and_then(|code| match code {
            0 => Some(GrpcStatusCode::Ok),
            1 => Some(GrpcStatusCode::Cancelled),
            2 => Some(GrpcStatusCode::Unknown),
            3 => Some(GrpcStatusCode::InvalidArgument),
            4 => Some(GrpcStatusCode::DeadlineExceeded),
            5 => Some(GrpcStatusCode::NotFound),
            6 => Some(GrpcStatusCode::AlreadyExists),
            7 => Some(GrpcStatusCode::PermissionDenied),
            8 => Some(GrpcStatusCode::ResourceExhausted),
            9 => Some(GrpcStatusCode::FailedPrecondition),
            10 => Some(GrpcStatusCode::Aborted),
            11 => Some(GrpcStatusCode::OutOfRange),
            12 => Some(GrpcStatusCode::Unimplemented),
            13 => Some(GrpcStatusCode::Internal),
            14 => Some(GrpcStatusCode::Unavailable),
            15 => Some(GrpcStatusCode::DataLoss),
            16 => Some(GrpcStatusCode::Unauthenticated),
            _ => None,
        })
}

/// Extract gRPC message from response headers
pub fn extract_grpc_message<B>(response: &Response<B>) -> Option<String> {
    response.headers()
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Check if gRPC response is successful
pub fn is_grpc_success<B>(response: &Response<B>) -> bool {
    extract_grpc_status(response)
        .map(|status| status == GrpcStatusCode::Ok)
        .unwrap_or(false)
}

/// Add gRPC metadata to request headers
pub fn add_grpc_metadata<B>(
    request: &mut Request<B>,
    metadata: &[(String, String)],
) -> Result<()> {
    use hyper::header::HeaderName;
    let headers = request.headers_mut();

    for (key, value) in metadata {
        let header_name: HeaderName = key.parse()?;
        let header_value = HeaderValue::from_str(value)?;
        headers.insert(header_name, header_value);
    }

    Ok(())
}

/// Convert gRPC timeout to header value
pub fn timeout_to_header(timeout: std::time::Duration) -> String {
    let total_ms = timeout.as_millis();

    if total_ms >= 3600_000 {
        // Use hours if >= 1 hour
        format!("{}H", total_ms / 3600_000)
    } else if total_ms >= 60_000 {
        // Use minutes if >= 1 minute
        format!("{}M", total_ms / 60_000)
    } else if total_ms >= 1000 {
        // Use seconds if >= 1 second
        format!("{}S", total_ms / 1000)
    } else {
        // Use milliseconds
        format!("{}m", total_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Empty;
    use bytes::Bytes;

    #[test]
    fn test_create_grpc_error_response() {
        let response = create_grpc_error_response(
            GrpcStatusCode::NotFound,
            "Service not found"
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/grpc"
        );
        assert_eq!(
            response.headers().get("grpc-status").unwrap(),
            "5" // NotFound = 5
        );
    }

    #[test]
    fn test_timeout_to_header() {
        use std::time::Duration;

        assert_eq!(timeout_to_header(Duration::from_secs(3600)), "1H");
        assert_eq!(timeout_to_header(Duration::from_secs(120)), "2M");
        assert_eq!(timeout_to_header(Duration::from_secs(30)), "30S");
        assert_eq!(timeout_to_header(Duration::from_millis(500)), "500m");
    }

    #[test]
    fn test_extract_grpc_status() {
        let mut response = Response::new(Empty::<Bytes>::new());
        response.headers_mut().insert(
            "grpc-status",
            HeaderValue::from_static("0")
        );

        assert_eq!(extract_grpc_status(&response), Some(GrpcStatusCode::Ok));

        response.headers_mut().insert(
            "grpc-status",
            HeaderValue::from_static("5")
        );

        assert_eq!(extract_grpc_status(&response), Some(GrpcStatusCode::NotFound));
    }

    #[test]
    fn test_is_grpc_success() {
        let mut response = Response::new(Empty::<Bytes>::new());
        response.headers_mut().insert(
            "grpc-status",
            HeaderValue::from_static("0")
        );

        assert!(is_grpc_success(&response));

        response.headers_mut().insert(
            "grpc-status",
            HeaderValue::from_static("13")
        );

        assert!(!is_grpc_success(&response));
    }
}
