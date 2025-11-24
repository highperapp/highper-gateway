// Ultra-fast Rust backend for load testing
// Minimal overhead, maximum throughput

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};

static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

async fn handle_request(
    _req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .header("X-Backend-Server", "rust-backend")
        .body(Full::new(Bytes::from("OK\n")))
        .unwrap();

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 80));
    let listener = TcpListener::bind(addr).await?;

    println!("Backend server listening on {}", addr);
    println!("Endpoints:");
    println!("  GET  /       - Fast response (minimal latency)");
    println!("  GET  /stats  - Request statistics");

    // Stats reporter task
    tokio::spawn(async {
        let mut ticker = interval(Duration::from_secs(10));
        let mut last_count = 0u64;

        loop {
            ticker.tick().await;
            let current = REQUEST_COUNT.load(Ordering::Relaxed);
            let delta = current - last_count;
            let rps = delta as f64 / 10.0;
            println!("Backend RPS: {:.0}, Total: {}", rps, current);
            last_count = current;
        }
    });

    // Accept connections
    loop {
        let (stream, _) = listener.accept().await?;
        let io = hyper_util::rt::TokioIo::new(stream);

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_request))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
