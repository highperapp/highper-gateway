//! Ultra-simple Rust HTTP backend for load testing
//! Minimal overhead, maximum throughput

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Global request counter
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

/// Backend state
struct BackendState {
    hostname: String,
    port: u16,
    start_time: std::time::Instant,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse::<u16>()?;

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "rust-backend".to_string())
    });

    let state = Arc::new(BackendState {
        hostname: hostname.clone(),
        port,
        start_time: std::time::Instant::now(),
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    eprintln!("[{}] Rust HTTP backend listening on http://{}",
              chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
              addr);
    eprintln!("[{}] Hostname: {}",
              chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
              hostname);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| handle_request(req, state.clone())),
                )
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<BackendState>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let request_id = REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Build response based on path
    let response = match (&method, path.as_str()) {
        // Health check
        (_, "/health") => {
            let uptime = state.start_time.elapsed().as_secs();
            let body = json!({
                "status": "healthy",
                "service": "rust-http-backend",
                "hostname": state.hostname,
                "port": state.port,
                "uptime": uptime,
                "requests": request_id,
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        // Small response (~100 bytes)
        (_, "/small") | (_, "/api/ping") => {
            let body = json!({
                "message": "OK",
                "backend": state.hostname,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        // Medium response (~10KB)
        (_, "/medium") => {
            let data = "X".repeat(10000);
            let body = json!({
                "message": "Medium response",
                "backend": state.hostname,
                "size": data.len(),
                "data": data,
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        // Large response (~100KB)
        (_, "/large") => {
            let data = "X".repeat(100000);
            let body = json!({
                "message": "Large response",
                "backend": state.hostname,
                "size": data.len(),
                "data": data,
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        // API endpoints
        (&Method::GET, p) if p.starts_with("/api/") => {
            let body = json!({
                "method": "GET",
                "path": path,
                "backend": state.hostname,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        (&Method::POST, p) if p.starts_with("/api/") => {
            let body = json!({
                "method": "POST",
                "path": path,
                "backend": state.hostname,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::CREATED)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        (&Method::PUT, p) if p.starts_with("/api/") => {
            let body = json!({
                "method": "PUT",
                "path": path,
                "backend": state.hostname,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        (&Method::DELETE, p) if p.starts_with("/api/") => {
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::new()))
                .unwrap()
        }

        (&Method::PATCH, p) if p.starts_with("/api/") => {
            let body = json!({
                "method": "PATCH",
                "path": path,
                "backend": state.hostname,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        (&Method::HEAD, _) => {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::new()))
                .unwrap()
        }

        (&Method::OPTIONS, _) => {
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .header("Allow", "GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS")
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Requested-With, X-Forwarded-For, X-Forwarded-Proto, X-Forwarded-Host, X-Real-IP")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::new()))
                .unwrap()
        }

        // Default: root path
        (&Method::GET, "/") => {
            let body = json!({
                "message": "Highper Gateway Load Test Backend",
                "service": "rust-http-backend",
                "backend": state.hostname,
                "port": state.port,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }

        // 404 for everything else
        _ => {
            let body = json!({
                "error": "Not found",
                "path": path,
                "method": method.as_str(),
            });

            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .header("X-Backend-Host", &state.hostname)
                .header("X-Request-ID", format!("{}-{}", state.hostname, request_id))
                .body(Full::new(Bytes::from(body.to_string())))
                .unwrap()
        }
    };

    Ok(response)
}
