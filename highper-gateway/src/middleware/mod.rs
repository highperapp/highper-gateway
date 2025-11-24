//! Middleware system for request/response processing

pub mod body_access;
pub mod compression;
pub mod compression_middleware;
pub mod cors;
pub mod headers;
pub mod logging;
pub mod transform;
pub mod mtls;
pub mod rate_limit;
pub mod request_size_limit;
pub mod streaming_validator;

// WAF module with adapter pattern
pub mod waf;

use crate::http::ResponseBody;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response};
use std::future::Future;
use std::pin::Pin;

/// Result type for middleware operations
pub type MiddlewareResult = crate::Result<Response<ResponseBody>>;

/// Middleware trait for processing requests and responses
pub trait Middleware: Send + Sync {
    /// Process a request before forwarding to upstream
    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>> {
        Box::pin(async move { Ok(req) })
    }

    /// Process a response before returning to client
    fn process_response(
        &self,
        response: Response<ResponseBody>,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send>> {
        Box::pin(async move { Ok(response) })
    }

    /// Get middleware name
    fn name(&self) -> &str;
}

/// Middleware chain for processing requests/responses
pub struct MiddlewareChain {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Create a new middleware chain
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the chain
    pub fn add<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Box::new(middleware));
    }

    /// Process request through all middlewares
    pub async fn process_request(
        &self,
        mut req: Request<hyper::body::Incoming>,
    ) -> Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>> {
        for middleware in &self.middlewares {
            req = middleware.process_request(req).await?;
        }
        Ok(req)
    }

    /// Process response through all middlewares (in reverse order)
    pub async fn process_response(&self, mut response: Response<ResponseBody>) -> MiddlewareResult {
        for middleware in self.middlewares.iter().rev() {
            response = middleware.process_response(response).await?;
        }
        Ok(response)
    }

    /// Get number of middlewares in the chain
    pub fn len(&self) -> usize {
        self.middlewares.len()
    }

    /// Check if chain is empty
    pub fn is_empty(&self) -> bool {
        self.middlewares.is_empty()
    }

    /// Get middleware names
    pub fn middleware_names(&self) -> Vec<&str> {
        self.middlewares.iter().map(|m| m.name()).collect()
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::StatusCode;

    struct TestMiddleware {
        name: String,
    }

    impl Middleware for TestMiddleware {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_middleware_chain() {
        let mut chain = MiddlewareChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);

        chain.add(TestMiddleware {
            name: "test1".to_string(),
        });
        chain.add(TestMiddleware {
            name: "test2".to_string(),
        });

        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 2);

        let names = chain.middleware_names();
        assert_eq!(names, vec!["test1", "test2"]);
    }
}
