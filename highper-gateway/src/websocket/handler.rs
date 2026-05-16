//! WebSocket upgrade and proxying handler
//!
//! Detects WebSocket upgrade requests and establishes bidirectional proxying
//! Supports sticky sessions via cookies for load balancing

use crate::websocket::SessionId;
use anyhow::{anyhow, Result};
use hyper::{
    header::{self, HeaderValue},
    Request, Response, StatusCode,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, error, info, warn};

/// Check if a request is a WebSocket upgrade request
pub fn is_websocket_upgrade<B>(req: &Request<B>) -> bool {
    // Check for required WebSocket upgrade headers
    let has_upgrade = req
        .headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let has_connection_upgrade = req
        .headers()
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("upgrade"))
        .unwrap_or(false);

    let has_websocket_key = req.headers().contains_key(header::SEC_WEBSOCKET_KEY);

    let has_websocket_version = req.headers().get(header::SEC_WEBSOCKET_VERSION).is_some();

    has_upgrade && has_connection_upgrade && has_websocket_key && has_websocket_version
}

/// Get the WebSocket accept key from the client key
pub fn get_websocket_accept_key(client_key: &str) -> String {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use sha1::{Digest, Sha1};

    const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let hash = hasher.finalize();

    BASE64.encode(hash)
}

/// Extract session ID from Cookie header
pub fn extract_session_id_from_cookie<B>(req: &Request<B>, cookie_name: &str) -> Option<SessionId> {
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;

    // Parse cookies (format: "name1=value1; name2=value2")
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some((name, value)) = cookie.split_once('=') {
            if name == cookie_name {
                // Try to parse the session ID as UUID
                if let Ok(session_id) = value.parse::<SessionId>() {
                    debug!("Found existing session ID in cookie: {}", session_id);
                    return Some(session_id);
                } else {
                    warn!("Invalid session ID format in cookie: {}", value);
                }
            }
        }
    }

    None
}

/// Create Set-Cookie header value for session ID
pub fn create_session_cookie(
    session_id: &SessionId,
    cookie_name: &str,
    max_age_secs: u64,
) -> String {
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        cookie_name, session_id, max_age_secs
    )
}

/// Create a WebSocket upgrade response
pub fn create_upgrade_response<B>(req: &Request<B>) -> Result<Response<crate::http::ResponseBody>> {
    create_upgrade_response_with_session(req, None, None, 0)
}

/// Create a WebSocket upgrade response with optional session cookie
pub fn create_upgrade_response_with_session<B>(
    req: &Request<B>,
    session_id: Option<&SessionId>,
    cookie_name: Option<&str>,
    max_age_secs: u64,
) -> Result<Response<crate::http::ResponseBody>> {
    let client_key = req
        .headers()
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
        HeaderValue::from_str(&accept_key)?,
    );

    // Copy Sec-WebSocket-Protocol if present
    if let Some(protocol) = req.headers().get(header::SEC_WEBSOCKET_PROTOCOL) {
        headers.insert(header::SEC_WEBSOCKET_PROTOCOL, protocol.clone());
    }

    // Inject session cookie if provided
    if let (Some(sid), Some(cookie_name)) = (session_id, cookie_name) {
        let cookie_value = create_session_cookie(sid, cookie_name, max_age_secs);
        headers.insert(header::SET_COOKIE, HeaderValue::from_str(&cookie_value)?);
        debug!("Injected session cookie: {} = {}", cookie_name, sid);
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
    use bytes::Bytes;
    use http_body_util::Empty;

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
            response
                .headers()
                .get(header::SEC_WEBSOCKET_ACCEPT)
                .unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn test_extract_session_id_from_cookie() {
        use uuid::Uuid;

        let session_id = Uuid::now_v7();
        let req = Request::builder()
            .header(
                header::COOKIE,
                format!("other=value; HPGW_WS_SESSION={}; another=data", session_id),
            )
            .body(Empty::<Bytes>::new())
            .unwrap();

        let extracted = extract_session_id_from_cookie(&req, "HPGW_WS_SESSION");
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap(), session_id);
    }

    #[test]
    fn test_extract_session_id_no_cookie() {
        let req = Request::builder().body(Empty::<Bytes>::new()).unwrap();

        let extracted = extract_session_id_from_cookie(&req, "HPGW_WS_SESSION");
        assert!(extracted.is_none());
    }

    #[test]
    fn test_extract_session_id_invalid_format() {
        let req = Request::builder()
            .header(header::COOKIE, "HPGW_WS_SESSION=invalid-uuid-format")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let extracted = extract_session_id_from_cookie(&req, "HPGW_WS_SESSION");
        assert!(extracted.is_none());
    }

    #[test]
    fn test_create_session_cookie() {
        use uuid::Uuid;

        let session_id = Uuid::now_v7();
        let cookie = create_session_cookie(&session_id, "HPGW_WS_SESSION", 3600);

        assert!(cookie.contains("HPGW_WS_SESSION="));
        assert!(cookie.contains(&session_id.to_string()));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=3600"));
    }

    #[test]
    fn test_create_upgrade_response_with_session_cookie() {
        use uuid::Uuid;

        let session_id = Uuid::now_v7();
        let req = Request::builder()
            .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = create_upgrade_response_with_session(
            &req,
            Some(&session_id),
            Some("HPGW_WS_SESSION"),
            3600,
        )
        .unwrap();

        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

        // Check that Set-Cookie header is present
        let set_cookie = response.headers().get(header::SET_COOKIE).unwrap();
        let set_cookie_str = set_cookie.to_str().unwrap();

        assert!(set_cookie_str.contains("HPGW_WS_SESSION="));
        assert!(set_cookie_str.contains(&session_id.to_string()));
        assert!(set_cookie_str.contains("HttpOnly"));
    }
}
