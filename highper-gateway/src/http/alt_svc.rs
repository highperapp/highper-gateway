//! Alt-Svc (Alternative Service) header support for HTTP/3 advertisement

use hyper::header::HeaderValue;
use hyper::Response;

/// Add Alt-Svc header to advertise HTTP/3 support
///
/// This header tells clients that HTTP/3 is available on the specified port.
/// Clients can then upgrade to HTTP/3 for subsequent requests.
///
/// Format: `h3=":port"; ma=maxage`
/// - h3: HTTP/3 protocol identifier
/// - port: The UDP port where HTTP/3 is available
/// - ma: Max age in seconds (how long to remember this alternative)
pub fn add_alt_svc_header<T>(response: &mut Response<T>, http3_port: u16) {
    add_alt_svc_header_with_max_age(response, http3_port, 2592000) // 30 days
}

/// Add Alt-Svc header with custom max age
pub fn add_alt_svc_header_with_max_age<T>(
    response: &mut Response<T>,
    http3_port: u16,
    max_age_secs: u32,
) {
    // Alt-Svc format: h3=":port"; ma=maxage
    let alt_svc_value = format!(r#"h3=":{}""#, http3_port);
    let alt_svc_value = if max_age_secs > 0 {
        format!(r#"{}; ma={}"#, alt_svc_value, max_age_secs)
    } else {
        alt_svc_value
    };

    if let Ok(header_value) = HeaderValue::from_str(&alt_svc_value) {
        response.headers_mut().insert("alt-svc", header_value);
    }
}

/// Check if a response already has an Alt-Svc header
pub fn has_alt_svc_header<T>(response: &Response<T>) -> bool {
    response.headers().contains_key("alt-svc")
}

/// Remove Alt-Svc header from response
pub fn remove_alt_svc_header<T>(response: &mut Response<T>) {
    response.headers_mut().remove("alt-svc");
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http_body_util::Full;
    use hyper::StatusCode;

    #[test]
    fn test_alt_svc_header() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header(&mut response, 443);

        let alt_svc = response.headers().get("alt-svc").unwrap();
        assert_eq!(alt_svc.to_str().unwrap(), r#"h3=":443"; ma=2592000"#);
    }

    #[test]
    fn test_alt_svc_header_custom_port() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header(&mut response, 8443);

        let alt_svc = response.headers().get("alt-svc").unwrap();
        assert_eq!(alt_svc.to_str().unwrap(), r#"h3=":8443"; ma=2592000"#);
    }

    #[test]
    fn test_alt_svc_header_custom_max_age() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header_with_max_age(&mut response, 443, 86400); // 1 day

        let alt_svc = response.headers().get("alt-svc").unwrap();
        assert_eq!(alt_svc.to_str().unwrap(), r#"h3=":443"; ma=86400"#);
    }

    #[test]
    fn test_alt_svc_header_zero_max_age() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header_with_max_age(&mut response, 443, 0);

        let alt_svc = response.headers().get("alt-svc").unwrap();
        assert_eq!(alt_svc.to_str().unwrap(), r#"h3=":443""#);
    }

    #[test]
    fn test_has_alt_svc_header() {
        let mut response: Response<Full<Bytes>> = Response::new(Full::new(Bytes::new()));
        assert!(!has_alt_svc_header(&response));

        add_alt_svc_header(&mut response, 443);
        assert!(has_alt_svc_header(&response));
    }

    #[test]
    fn test_remove_alt_svc_header() {
        let mut response = Response::new(Full::new(Bytes::new()));
        add_alt_svc_header(&mut response, 443);
        assert!(has_alt_svc_header(&response));

        remove_alt_svc_header(&mut response);
        assert!(!has_alt_svc_header(&response));
    }

    #[test]
    fn test_alt_svc_header_preserves_other_headers() {
        let mut response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-custom", "value")
            .body(Full::new(Bytes::new()))
            .unwrap();

        add_alt_svc_header(&mut response, 443);

        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        assert_eq!(response.headers().get("x-custom").unwrap(), "value");
        assert!(has_alt_svc_header(&response));
    }
}
