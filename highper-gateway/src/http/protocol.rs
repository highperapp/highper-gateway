//! Protocol detection and negotiation

/// HTTP/2 connection preface (magic string)
const HTTP2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Detected HTTP protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    Http1,
    Http2,
}

/// Detect protocol from the initial bytes of a connection
///
/// HTTP/2 connections start with a fixed preface (magic string)
/// HTTP/1.x connections start with method (GET, POST, etc.)
pub fn detect_protocol(buf: &[u8]) -> Option<DetectedProtocol> {
    if buf.is_empty() {
        return None;
    }

    // Check for HTTP/2 preface
    if buf.len() >= 24 && buf.starts_with(HTTP2_PREFACE) {
        return Some(DetectedProtocol::Http2);
    }

    // Check for common HTTP/1.x methods
    let methods: &[&[u8]] = &[
        b"GET ",
        b"POST ",
        b"PUT ",
        b"DELETE ",
        b"HEAD ",
        b"OPTIONS ",
        b"PATCH ",
        b"TRACE ",
        b"CONNECT ",
    ];

    for method in methods {
        if buf.starts_with(method) {
            return Some(DetectedProtocol::Http1);
        }
    }

    // Not enough data or unknown protocol
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http2_preface() {
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        assert_eq!(detect_protocol(preface), Some(DetectedProtocol::Http2));
    }

    #[test]
    fn test_detect_http1_get() {
        let req = b"GET / HTTP/1.1\r\n";
        assert_eq!(detect_protocol(req), Some(DetectedProtocol::Http1));
    }

    #[test]
    fn test_detect_http1_post() {
        let req = b"POST /api HTTP/1.1\r\n";
        assert_eq!(detect_protocol(req), Some(DetectedProtocol::Http1));
    }

    #[test]
    fn test_detect_empty() {
        assert_eq!(detect_protocol(b""), None);
    }

    #[test]
    fn test_detect_insufficient_data() {
        assert_eq!(detect_protocol(b"GE"), None);
    }
}
