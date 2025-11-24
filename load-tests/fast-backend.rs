// Fast HTTP backend server for load testing
// Minimal overhead, designed to handle high RPS

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;

async fn handle_request(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"status":"ok"}"#))
        .unwrap())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 9000));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle_request)) });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Fast backend listening on http://{}", addr);
    println!("PID: {}", std::process::id());

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}
