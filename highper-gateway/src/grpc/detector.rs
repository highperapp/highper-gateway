//! gRPC request detection and parsing
//!
//! Detects gRPC requests based on HTTP/2 headers and content-type

use super::{GrpcCallType, GrpcRequest};
use hyper::{header, Request};
use std::time::Duration;

/// Check if a request is a gRPC request
pub fn is_grpc_request<B>(req: &Request<B>) -> bool {
    // gRPC requires HTTP/2
    if req.version() != hyper::Version::HTTP_2 {
        return false;
    }

    // Check content-type header
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.starts_with("application/grpc"))
        .unwrap_or(false)
}

/// Parse gRPC request information from HTTP/2 request
pub fn parse_grpc_request<B>(req: &Request<B>) -> Option<GrpcRequest> {
    if !is_grpc_request(req) {
        return None;
    }

    let path = req.uri().path().to_string();

    // Determine call type from headers
    let call_type = determine_call_type(req);

    // Extract metadata (convert headers to gRPC metadata)
    let metadata = extract_metadata(req);

    // Extract timeout from grpc-timeout header
    let timeout = extract_timeout(req);

    // Get content type
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("application/grpc")
        .to_string();

    Some(GrpcRequest {
        path,
        call_type,
        metadata,
        timeout,
        content_type,
    })
}

/// Determine gRPC call type
fn determine_call_type<B>(req: &Request<B>) -> GrpcCallType {
    // In practice, call type is determined by the service definition
    // For now, we'll assume unary by default and detect streaming from headers
    // A more sophisticated implementation would parse the protobuf descriptor

    // Check for grpc-encoding header (indicates streaming)
    let has_encoding = req.headers().contains_key("grpc-encoding");

    // For now, default to unary
    // In a full implementation, we'd need to:
    // 1. Maintain a registry of gRPC services and their methods
    // 2. Look up the method type from the service definition
    // 3. Or detect streaming from the first few frames

    if has_encoding {
        // Heuristic: if encoding is present, might be streaming
        GrpcCallType::Unary // Still default to unary for now
    } else {
        GrpcCallType::Unary
    }
}

/// Extract gRPC metadata from HTTP/2 headers
fn extract_metadata<B>(req: &Request<B>) -> Vec<(String, String)> {
    let mut metadata = Vec::new();

    for (name, value) in req.headers() {
        let name_str = name.as_str();

        // gRPC metadata keys that start with "grpc-" are reserved
        // Custom metadata should not start with "grpc-"
        // Binary metadata keys end with "-bin"

        // Skip pseudo-headers (start with :)
        if name_str.starts_with(':') {
            continue;
        }

        // Skip standard HTTP headers that aren't gRPC metadata
        if name_str == "content-type" || name_str == "te" || name_str == "user-agent" {
            continue;
        }

        if let Ok(value_str) = value.to_str() {
            metadata.push((name_str.to_string(), value_str.to_string()));
        }
    }

    metadata
}

/// Extract timeout from grpc-timeout header
fn extract_timeout<B>(req: &Request<B>) -> Option<Duration> {
    req.headers()
        .get("grpc-timeout")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_grpc_timeout)
}

/// Parse gRPC timeout format (e.g., "1H", "2M", "30S", "1000m", "1000000u", "1000000000n")
fn parse_grpc_timeout(timeout_str: &str) -> Option<Duration> {
    if timeout_str.is_empty() {
        return None;
    }

    let len = timeout_str.len();
    let unit = timeout_str.chars().last()?;
    let value_str = &timeout_str[..len - 1];
    let value: u64 = value_str.parse().ok()?;

    match unit {
        'H' => Some(Duration::from_secs(value * 3600)), // Hours
        'M' => Some(Duration::from_secs(value * 60)),   // Minutes
        'S' => Some(Duration::from_secs(value)),        // Seconds
        'm' => Some(Duration::from_millis(value)),      // Milliseconds
        'u' => Some(Duration::from_micros(value)),      // Microseconds
        'n' => Some(Duration::from_nanos(value)),       // Nanoseconds
        _ => None,
    }
}

/// Validate gRPC service path
pub fn is_valid_grpc_path(path: &str) -> bool {
    // gRPC paths follow the format: /package.Service/Method
    // Examples:
    //   /grpc.health.v1.Health/Check
    //   /myapp.UserService/GetUser

    if !path.starts_with('/') {
        return false;
    }

    let parts: Vec<&str> = path[1..].split('/').collect();
    if parts.len() != 2 {
        return false;
    }

    let service = parts[0];
    let method = parts[1];

    // Service should contain at least one dot (package.Service)
    if !service.contains('.') {
        return false;
    }

    // Method should not be empty
    if method.is_empty() {
        return false;
    }

    true
}

/// Extract service name from gRPC path
pub fn extract_service_name(path: &str) -> Option<&str> {
    if !path.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = path[1..].split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    Some(parts[0])
}

/// Extract method name from gRPC path
pub fn extract_method_name(path: &str) -> Option<&str> {
    if !path.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = path[1..].split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    Some(parts[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::{header::HeaderValue, Request};

    #[test]
    fn test_is_grpc_request() {
        // Valid gRPC request
        let mut req = Request::builder()
            .version(hyper::Version::HTTP_2)
            .header(header::CONTENT_TYPE, "application/grpc")
            .uri("/grpc.health.v1.Health/Check")
            .body(())
            .unwrap();

        assert!(is_grpc_request(&req));

        // Not gRPC (HTTP/1.1)
        req = Request::builder()
            .version(hyper::Version::HTTP_11)
            .header(header::CONTENT_TYPE, "application/grpc")
            .body(())
            .unwrap();

        assert!(!is_grpc_request(&req));

        // Not gRPC (wrong content-type)
        req = Request::builder()
            .version(hyper::Version::HTTP_2)
            .header(header::CONTENT_TYPE, "application/json")
            .body(())
            .unwrap();

        assert!(!is_grpc_request(&req));
    }

    #[test]
    fn test_parse_grpc_timeout() {
        assert_eq!(parse_grpc_timeout("1H"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_grpc_timeout("2M"), Some(Duration::from_secs(120)));
        assert_eq!(parse_grpc_timeout("30S"), Some(Duration::from_secs(30)));
        assert_eq!(
            parse_grpc_timeout("1000m"),
            Some(Duration::from_millis(1000))
        );
        assert_eq!(
            parse_grpc_timeout("1000u"),
            Some(Duration::from_micros(1000))
        );
        assert_eq!(
            parse_grpc_timeout("1000n"),
            Some(Duration::from_nanos(1000))
        );
        assert_eq!(parse_grpc_timeout("invalid"), None);
    }

    #[test]
    fn test_is_valid_grpc_path() {
        assert!(is_valid_grpc_path("/grpc.health.v1.Health/Check"));
        assert!(is_valid_grpc_path("/myapp.UserService/GetUser"));
        assert!(is_valid_grpc_path("/a.b.c.Service/Method"));

        assert!(!is_valid_grpc_path("/invalid"));
        assert!(!is_valid_grpc_path("no-slash"));
        assert!(!is_valid_grpc_path("/NoPackage/Method"));
        assert!(!is_valid_grpc_path("/package.Service/"));
    }

    #[test]
    fn test_extract_service_name() {
        assert_eq!(
            extract_service_name("/grpc.health.v1.Health/Check"),
            Some("grpc.health.v1.Health")
        );
        assert_eq!(
            extract_service_name("/myapp.UserService/GetUser"),
            Some("myapp.UserService")
        );
        assert_eq!(extract_service_name("/invalid"), None);
    }

    #[test]
    fn test_extract_method_name() {
        assert_eq!(
            extract_method_name("/grpc.health.v1.Health/Check"),
            Some("Check")
        );
        assert_eq!(
            extract_method_name("/myapp.UserService/GetUser"),
            Some("GetUser")
        );
        assert_eq!(extract_method_name("/invalid"), None);
    }
}
