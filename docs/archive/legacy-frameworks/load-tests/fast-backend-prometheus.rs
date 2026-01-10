// Fast HTTP backend server for load testing with Prometheus metrics
// Minimal overhead, designed to handle high RPS

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct Metrics {
    requests_total: Arc<AtomicU64>,
    requests_ok: Arc<AtomicU64>,
    requests_health: Arc<AtomicU64>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            requests_ok: Arc::new(AtomicU64::new(0)),
            requests_health: Arc::new(AtomicU64::new(0)),
        }
    }
}

async fn handle_request(req: Request<Body>, metrics: Metrics) -> Result<Response<Body>, Infallible> {
    metrics.requests_total.fetch_add(1, Ordering::Relaxed);

    match (req.method(), req.uri().path()) {
        // Health check endpoint
        (&Method::GET, "/health") => {
            metrics.requests_health.fetch_add(1, Ordering::Relaxed);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain")
                .body(Body::from("OK"))
                .unwrap())
        }

        // Prometheus metrics endpoint
        (&Method::GET, "/metrics") => {
            let total = metrics.requests_total.load(Ordering::Relaxed);
            let ok = metrics.requests_ok.load(Ordering::Relaxed);
            let health = metrics.requests_health.load(Ordering::Relaxed);

            let metrics_output = format!(
                "# HELP backend_requests_total Total number of requests\n\
                 # TYPE backend_requests_total counter\n\
                 backend_requests_total {}\n\
                 # HELP backend_requests_ok Successful requests\n\
                 # TYPE backend_requests_ok counter\n\
                 backend_requests_ok {}\n\
                 # HELP backend_requests_health Health check requests\n\
                 # TYPE backend_requests_health counter\n\
                 backend_requests_health {}\n",
                total, ok, health
            );

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain; version=0.0.4")
                .body(Body::from(metrics_output))
                .unwrap())
        }

        // Default response for load testing
        _ => {
            metrics.requests_ok.fetch_add(1, Ordering::Relaxed);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Backend-Server", &format!("{}", std::process::id()))
                .body(Body::from(r#"{"status":"ok","backend":"fast-backend"}"#))
                .unwrap())
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let metrics = Metrics::new();

    let make_svc = make_service_fn(move |_conn| {
        let metrics = metrics.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, metrics.clone())
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Fast backend with Prometheus metrics listening on http://{}", addr);
    println!("Endpoints:");
    println!("  /         - Load test endpoint");
    println!("  /health   - Health check");
    println!("  /metrics  - Prometheus metrics");
    println!("PID: {}", std::process::id());

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}
