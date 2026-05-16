//! HTTP client for upstream connections

use crate::config::ConnectionPoolConfig;
use crate::proxy::pool_metrics::ConnectionPoolMetrics;
use crate::proxy::retry::{RetryConfig, RetryExecutor, RetryPolicy, RetryStrategy};
use crate::Result;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Empty, Full};
use hyper::body::Incoming;
use hyper::{Method, Request, Response, Uri};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

/// HTTP client for upstream connections
/// Supports both HTTP/1.1 and HTTP/2
#[derive(Clone)]
pub struct Client {
    inner: HyperClient<HttpConnector, BoxBody<Bytes, hyper::Error>>,
    retry_executor: Option<Arc<RetryExecutor>>,
    pool_metrics: Arc<ConnectionPoolMetrics>,
}

impl Client {
    /// Create a new client with HTTP/1.1 and HTTP/2 support
    pub fn new() -> Self {
        Self::with_config(None, None)
    }

    /// Create a new client with retry configuration
    pub fn with_retry(retry_config: Option<RetryConfig>) -> Self {
        Self::with_config(retry_config, None)
    }

    /// Create a new client with retry and pool configuration
    pub fn with_config(
        retry_config: Option<RetryConfig>,
        pool_config: Option<ConnectionPoolConfig>,
    ) -> Self {
        let pool_config = pool_config.unwrap_or_default();

        let mut connector = HttpConnector::new();

        // Connection timeouts
        connector.set_connect_timeout(Some(Duration::from_secs(5)));

        // TCP optimizations
        connector.set_nodelay(true); // Disable Nagle's algorithm for lower latency
        connector.set_keepalive(Some(Duration::from_secs(60))); // Keep connections alive

        // Connection reuse settings
        connector.set_reuse_address(true); // Enable SO_REUSEADDR
        connector.enforce_http(false); // Allow both HTTP and HTTPS

        // Enable HTTP/2 support with production-optimized settings
        let inner = HyperClient::builder(TokioExecutor::new())
            // Connection pool settings from configuration
            .pool_idle_timeout(pool_config.idle_timeout)
            .pool_max_idle_per_host(pool_config.max_idle_per_host)
            // Protocol support
            .http2_only(false) // Support both HTTP/1.1 and HTTP/2
            // HTTP/2 specific settings
            .http2_initial_stream_window_size(Some(65536)) // 64KB per stream
            .http2_initial_connection_window_size(Some(1048576)) // 1MB for connection
            .http2_adaptive_window(true) // Enable adaptive flow control
            .http2_max_frame_size(Some(16384)) // 16KB frames
            .http2_keep_alive_interval(Some(Duration::from_secs(10))) // Send pings every 10s
            .http2_keep_alive_timeout(Duration::from_secs(20)) // Timeout after 20s
            .http2_keep_alive_while_idle(true) // Keep alive even when idle
            .build(connector);

        let retry_executor = retry_config.map(|config| {
            Arc::new(RetryExecutor::new(
                config,
                RetryStrategy::Exponential,
                RetryPolicy::default(),
            ))
        });

        let pool_metrics = if pool_config.metrics_enabled {
            Arc::new(ConnectionPoolMetrics::new())
        } else {
            Arc::new(ConnectionPoolMetrics::new()) // Still create it but could be a no-op version
        };

        debug!(
            "HTTP client initialized: retry={}, pool_max_idle={}, pool_idle_timeout={:?}, metrics={}",
            retry_executor.is_some(),
            pool_config.max_idle_per_host,
            pool_config.idle_timeout,
            pool_config.metrics_enabled
        );

        // Log warning for pre-warming feature
        if pool_config.prewarm && pool_config.min_idle_per_host > 0 {
            warn!(
                "Connection pre-warming requested (min_idle={}), but not yet implemented. This will be added in a future update.",
                pool_config.min_idle_per_host
            );
        }

        // Log warning for max_connection_lifetime feature
        if let Some(lifetime) = pool_config.max_connection_lifetime {
            warn!(
                "Max connection lifetime ({:?}) configured, but not yet implemented. This will be added in a future update.",
                lifetime
            );
        }

        Self {
            inner,
            retry_executor,
            pool_metrics,
        }
    }

    /// Get connection pool metrics
    pub fn pool_metrics(&self) -> Arc<ConnectionPoolMetrics> {
        Arc::clone(&self.pool_metrics)
    }

    /// Forward a request to an upstream server
    pub async fn forward(
        &self,
        upstream_url: &str,
        method: Method,
        path: &str,
        headers: hyper::HeaderMap,
        body: Option<Bytes>,
    ) -> Result<Response<Incoming>> {
        // Build the upstream URI
        let uri = format!("{}{}", upstream_url.trim_end_matches('/'), path);

        // Track connection attempt
        // Note: Hyper's pool is internal, so we track at request level
        // A new connection is created if pool is exhausted

        let result = if let Some(retry_executor) = &self.retry_executor {
            // Use retry logic
            self.forward_with_retry(
                uri.clone(),
                method,
                headers.clone(),
                body.clone(),
                retry_executor,
            )
            .await
        } else {
            // Direct forwarding without retry
            self.forward_direct(uri.clone(), method, headers, body)
                .await
        };

        // Track connection status
        match &result {
            Ok(_) => {
                // Request succeeded - connection was either reused or created
                // We can't directly track Hyper's internal pool, but we can infer
                debug!("Request to {} succeeded", upstream_url);
            }
            Err(e) => {
                // Connection error
                self.pool_metrics.record_connection_error(upstream_url);
                debug!("Request to {} failed: {}", upstream_url, e);
            }
        }

        result
    }

    /// Forward request without retry
    async fn forward_direct(
        &self,
        uri: String,
        method: Method,
        headers: hyper::HeaderMap,
        body: Option<Bytes>,
    ) -> Result<Response<Incoming>> {
        debug!("Forwarding {} request to {}", method, uri);

        let uri: Uri = uri.parse()?;

        // Build request body
        use http_body_util::BodyExt;
        let body_boxed: BoxBody<Bytes, hyper::Error> = if let Some(body_bytes) = body {
            Full::new(body_bytes)
                .map_err(|never| match never {})
                .boxed()
        } else {
            Empty::<Bytes>::new()
                .map_err(|never| match never {})
                .boxed()
        };

        let mut req = Request::builder().method(method).uri(uri);

        // Copy headers
        for (key, value) in headers.iter() {
            req = req.header(key, value);
        }

        let req = req.body(body_boxed)?;

        match self.inner.request(req).await {
            Ok(response) => {
                debug!("Received response with status: {}", response.status());
                Ok(response)
            }
            Err(e) => {
                error!("Failed to forward request: {}", e);
                Err(e.into())
            }
        }
    }

    /// Forward request with retry logic
    async fn forward_with_retry(
        &self,
        uri: String,
        method: Method,
        headers: hyper::HeaderMap,
        body: Option<Bytes>,
        retry_executor: &Arc<RetryExecutor>,
    ) -> Result<Response<Incoming>> {
        debug!("Forwarding {} request to {} (with retry)", method, uri);

        let uri_parsed: Uri = uri.parse()?;
        let inner = self.inner.clone();

        retry_executor
            .execute(|| async {
                // Build request body
                use http_body_util::BodyExt;
                let body_boxed: BoxBody<Bytes, hyper::Error> =
                    if let Some(body_bytes) = body.clone() {
                        Full::new(body_bytes)
                            .map_err(|never| match never {})
                            .boxed()
                    } else {
                        Empty::<Bytes>::new()
                            .map_err(|never| match never {})
                            .boxed()
                    };

                let mut req_builder = Request::builder()
                    .method(method.clone())
                    .uri(uri_parsed.clone());

                // Copy headers
                for (key, value) in headers.iter() {
                    req_builder = req_builder.header(key, value);
                }

                let req = req_builder
                    .body(body_boxed)
                    .map_err(|e| format!("Failed to build request: {}", e))?;

                inner.request(req).await.map_err(|e| {
                    warn!("Request attempt failed: {}", e);
                    format!("Request failed: {}", e)
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_default_config() {
        let client = Client::new();
        let metrics = client.pool_metrics();
        assert_eq!(Arc::strong_count(&metrics), 2); // Client holds one, we hold one
    }

    #[test]
    fn test_client_with_custom_pool_config() {
        let pool_config = ConnectionPoolConfig {
            max_idle_per_host: 200,
            min_idle_per_host: 10,
            max_connection_lifetime: Some(Duration::from_secs(300)),
            idle_timeout: Duration::from_secs(120),
            prewarm: true,
            metrics_enabled: true,
        };

        let client = Client::with_config(None, Some(pool_config));
        let metrics = client.pool_metrics();
        assert_eq!(Arc::strong_count(&metrics), 2); // Client holds one, we hold one
    }

    #[test]
    fn test_client_with_metrics_disabled() {
        let pool_config = ConnectionPoolConfig {
            max_idle_per_host: 100,
            min_idle_per_host: 0,
            max_connection_lifetime: None,
            idle_timeout: Duration::from_secs(90),
            prewarm: false,
            metrics_enabled: false,
        };

        let client = Client::with_config(None, Some(pool_config));
        // Even with metrics disabled, we still create the pool_metrics object
        let metrics = client.pool_metrics();
        assert_eq!(Arc::strong_count(&metrics), 2); // Client holds one, we hold one
    }

    #[test]
    fn test_pool_config_defaults() {
        let config = ConnectionPoolConfig::default();
        assert_eq!(config.max_idle_per_host, 100);
        assert_eq!(config.min_idle_per_host, 0);
        assert_eq!(config.max_connection_lifetime, None);
        assert_eq!(config.idle_timeout, Duration::from_secs(90));
        assert_eq!(config.prewarm, false);
        assert_eq!(config.metrics_enabled, true);
    }
}
