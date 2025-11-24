//! WebSocket upgrade and proxying handler
//!
//! Detects WebSocket upgrade requests and establishes bidirectional proxying

use hyper::{Request, Response, StatusCode, header::{self, HeaderValue}};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, error, info};
use anyhow::{Result, anyhow};

/// Check if a request is a WebSocket upgrade request
pub fn is_websocket_upgrade<B>(req: &Request<B>) -> bool {
    // Check for required WebSocket upgrade headers
    let has_upgrade = req.headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let has_connection_upgrade = req.headers()
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("upgrade"))
        .unwrap_or(false);

    let has_websocket_key = req.headers()
        .contains_key(header::SEC_WEBSOCKET_KEY);

    let has_websocket_version = req.headers()
        .get(header::SEC_WEBSOCKET_VERSION)
        .is_some();

    has_upgrade && has_connection_upgrade && has_websocket_key && has_websocket_version
}

/// Get the WebSocket accept key from the client key
pub fn get_websocket_accept_key(client_key: &str) -> String {
    use sha1::{Sha1, Digest};
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let hash = hasher.finalize();

    BASE64.encode(hash)
}

/// Create a WebSocket upgrade response
pub fn create_upgrade_response<B>(req: &Request<B>) -> Result<Response<crate::http::ResponseBody>> {
    let client_key = req.headers()
        .get(header::SEC_WEBSOCKET_KEY)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("Missing Sec-WebSocket-Key header"))?;

    let accept_key = get_websocket_accept_key(client_key);

    let mut response = Response::new(crate::http::ResponseBody::empty());
    *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;

    let headers = response.headers_mut();
    headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
    headers.insert(
        header::SEC_WEBSOCKET_ACCEPT,
        HeaderValue::from_str(&accept_key)?
    );

    // Copy Sec-WebSocket-Protocol if present
    if let Some(protocol) = req.headers().get(header::SEC_WEBSOCKET_PROTOCOL) {
        headers.insert(header::SEC_WEBSOCKET_PROTOCOL, protocol.clone());
    }

    Ok(response)
}

/// Proxy WebSocket frames bidirectionally
pub async fn proxy_websocket<C, S>(mut client: C, mut server: S) -> Result<()>
where
    C: AsyncRead + AsyncWrite + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    use tokio::io::copy_bidirectional;

    debug!("Starting WebSocket bidirectional proxy");

    // Use tokio's optimized bidirectional copy
    // This handles both directions simultaneously
    match copy_bidirectional(&mut client, &mut server).await {
        Ok((client_to_server, server_to_client)) => {
            info!(
                "WebSocket connection closed. Transferred: client→server: {} bytes, server→client: {} bytes",
                client_to_server,
                server_to_client
            );
            Ok(())
        }
        Err(e) => {
            error!("WebSocket proxy error: {}", e);
            Err(anyhow!("WebSocket proxy error: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Empty;
    use bytes::Bytes;

    #[test]
    fn test_is_websocket_upgrade() {
        let req = Request::builder()
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "Upgrade")
            .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .body(Empty::<Bytes>::new())
            .unwrap();

        assert!(is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_not_websocket_upgrade() {
        let req = Request::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(Empty::<Bytes>::new())
            .unwrap();

        assert!(!is_websocket_upgrade(&req));
    }

    #[test]
    fn test_websocket_accept_key() {
        // RFC 6455 example
        let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

        assert_eq!(get_websocket_accept_key(client_key), expected);
    }

    #[test]
    fn test_create_upgrade_response() {
        let req = Request::builder()
            .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = create_upgrade_response(&req).unwrap();

        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        assert_eq!(
            response.headers().get(header::UPGRADE).unwrap(),
            "websocket"
        );
        assert_eq!(
            response.headers().get(header::CONNECTION).unwrap(),
            "Upgrade"
        );
        assert_eq!(
            response.headers().get(header::SEC_WEBSOCKET_ACCEPT).unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
