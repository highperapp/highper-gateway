//! mTLS middleware for client certificate validation and header injection
//!
//! This middleware enforces per-route mTLS policies and injects client
//! certificate information as HTTP headers for backend services.

use crate::config::{CertVerificationMode, RouteMtlsPolicy};
use crate::tls::ClientCertInfo;
use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use http_body_util::Full;
use bytes::Bytes;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{debug, warn};

/// mTLS middleware that validates client certificates and injects headers
pub struct MtlsMiddleware {
    /// Client certificate info (if available)
    client_cert: Option<ClientCertInfo>,
}

impl MtlsMiddleware {
    /// Create new mTLS middleware
    ///
    /// # Arguments
    /// * `client_cert` - Client certificate info extracted from TLS connection
    ///
    /// # Returns
    /// * `Self` - New middleware instance
    pub fn new(client_cert: Option<ClientCertInfo>) -> Self {
        Self { client_cert }
    }

    /// Check if request satisfies route mTLS policy
    ///
    /// # Arguments
    /// * `policy` - Route-specific mTLS policy
    /// * `global_mode` - Global verification mode (fallback)
    ///
    /// # Returns
    /// * `Result<(), Response<Full<Bytes>>>` - Ok if policy satisfied, Err with 403 response otherwise
    pub fn check_policy(
        &self,
        policy: Option<&RouteMtlsPolicy>,
        global_mode: Option<CertVerificationMode>,
    ) -> Result<(), Response<Full<Bytes>>> {
        // Determine effective verification mode
        let verification_mode = policy
            .and_then(|p| p.verification_mode)
            .or(global_mode)
            .unwrap_or(CertVerificationMode::Optional);

        debug!(
            "Checking mTLS policy: mode={:?}, has_cert={}",
            verification_mode,
            self.client_cert.is_some()
        );

        // Check if client certificate is required
        match verification_mode {
            CertVerificationMode::Required => {
                if self.client_cert.is_none() {
                    warn!("Client certificate required but not provided");
                    return Err(Self::forbidden_response(
                        "Client certificate required but not provided",
                    ));
                }
            }
            CertVerificationMode::Optional | CertVerificationMode::OptionalNoCA => {
                // Certificate not required, continue
                if self.client_cert.is_none() {
                    debug!("No client certificate provided (optional mode)");
                    return Ok(());
                }
            }
        }

        // If we have a certificate and a policy with whitelist, check it
        if let Some(cert_info) = &self.client_cert {
            if let Some(policy) = policy {
                if !policy.allowed_fingerprints.is_empty() {
                    if !cert_info.matches_whitelist(&policy.allowed_fingerprints) {
                        warn!(
                            "Client certificate fingerprint not in whitelist: {}",
                            cert_info.fingerprint
                        );
                        return Err(Self::forbidden_response(
                            "Client certificate not authorized for this route",
                        ));
                    }
                    debug!("Client certificate fingerprint matches whitelist");
                }
            }
        }

        Ok(())
    }

    /// Inject client certificate headers into request
    ///
    /// # Arguments
    /// * `request` - HTTP request to modify
    ///
    /// # Returns
    /// * `Request<Incoming>` - Modified request with certificate headers
    pub fn inject_headers(&self, mut request: Request<Incoming>) -> Request<Incoming> {
        if let Some(cert_info) = &self.client_cert {
            debug!(
                "Injecting client certificate headers: subject={}",
                cert_info.subject_dn
            );

            // Add all certificate headers
            let headers = request.headers_mut();
            for (name, value) in cert_info.as_headers() {
                if let Ok(header_name) = hyper::header::HeaderName::try_from(name.as_str()) {
                    if let Ok(header_value) = hyper::header::HeaderValue::from_str(&value) {
                        headers.insert(header_name, header_value);
                    } else {
                        warn!("Invalid header value for {}: {}", name, value);
                    }
                } else {
                    warn!("Invalid header name: {}", name);
                }
            }

            debug!("Injected {} client certificate headers", cert_info.as_headers().len());
        } else {
            debug!("No client certificate to inject");
        }

        request
    }

    /// Create a 403 Forbidden response
    ///
    /// # Arguments
    /// * `message` - Error message
    ///
    /// # Returns
    /// * `Response<Full<Bytes>>` - 403 response
    fn forbidden_response(message: &str) -> Response<Full<Bytes>> {
        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "text/plain")
            .body(Full::new(Bytes::from(format!(
                "403 Forbidden: {}\n",
                message
            ))))
            .unwrap()
    }
}

/// Service wrapper that applies mTLS middleware
pub struct MtlsService<S> {
    inner: S,
    middleware: MtlsMiddleware,
    policy: Option<RouteMtlsPolicy>,
    global_mode: Option<CertVerificationMode>,
}

impl<S> MtlsService<S> {
    /// Create new mTLS service wrapper
    ///
    /// # Arguments
    /// * `inner` - Inner service to wrap
    /// * `middleware` - mTLS middleware
    /// * `policy` - Route-specific mTLS policy
    /// * `global_mode` - Global verification mode
    ///
    /// # Returns
    /// * `Self` - New service wrapper
    pub fn new(
        inner: S,
        middleware: MtlsMiddleware,
        policy: Option<RouteMtlsPolicy>,
        global_mode: Option<CertVerificationMode>,
    ) -> Self {
        Self {
            inner,
            middleware,
            policy,
            global_mode,
        }
    }
}

impl<S> tower::Service<Request<Incoming>> for MtlsService<S>
where
    S: tower::Service<Request<Incoming>, Response = Response<Full<Bytes>>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<Full<Bytes>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<Incoming>) -> Self::Future {
        // Check mTLS policy
        if let Err(response) = self.middleware.check_policy(self.policy.as_ref(), self.global_mode) {
            return Box::pin(async move { Ok(response) });
        }

        // Inject certificate headers
        let request = self.middleware.inject_headers(request);

        // Call inner service
        let mut inner = self.inner.clone();
        Box::pin(async move {
            inner.call(request).await.map_err(Into::into)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::ClientCertInfo;

    #[test]
    fn test_mtls_middleware_no_cert_optional() {
        let middleware = MtlsMiddleware::new(None);

        // Optional mode should allow no certificate
        let result = middleware.check_policy(None, Some(CertVerificationMode::Optional));
        assert!(result.is_ok());
    }

    #[test]
    fn test_mtls_middleware_no_cert_required() {
        let middleware = MtlsMiddleware::new(None);

        // Required mode should reject no certificate
        let result = middleware.check_policy(None, Some(CertVerificationMode::Required));
        assert!(result.is_err());

        if let Err(response) = result {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }

    #[test]
    fn test_mtls_middleware_with_cert_whitelist_match() {
        let cert_info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "abc123".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        let middleware = MtlsMiddleware::new(Some(cert_info));

        let policy = RouteMtlsPolicy {
            verification_mode: Some(CertVerificationMode::Required),
            allowed_subjects: vec![],
            allowed_issuers: vec![],
            allowed_serials: vec![],
            allowed_fingerprints: vec!["ABC123".to_string()], // Case-insensitive
        };

        let result = middleware.check_policy(Some(&policy), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mtls_middleware_with_cert_whitelist_no_match() {
        let cert_info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "xyz789".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        let middleware = MtlsMiddleware::new(Some(cert_info));

        let policy = RouteMtlsPolicy {
            verification_mode: Some(CertVerificationMode::Required),
            allowed_subjects: vec![],
            allowed_issuers: vec![],
            allowed_serials: vec![],
            allowed_fingerprints: vec!["abc123".to_string()], // Different fingerprint
        };

        let result = middleware.check_policy(Some(&policy), None);
        assert!(result.is_err());

        if let Err(response) = result {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }

    #[test]
    fn test_mtls_middleware_empty_whitelist() {
        let cert_info = ClientCertInfo {
            subject_dn: "CN=test".to_string(),
            issuer_dn: "CN=CA".to_string(),
            serial: "123".to_string(),
            fingerprint: "any".to_string(),
            not_before: None,
            not_after: None,
            version: 3,
        };

        let middleware = MtlsMiddleware::new(Some(cert_info));

        let policy = RouteMtlsPolicy {
            verification_mode: Some(CertVerificationMode::Optional),
            allowed_subjects: vec![],
            allowed_issuers: vec![],
            allowed_serials: vec![],
            allowed_fingerprints: vec![], // Empty whitelist = allow all
        };

        let result = middleware.check_policy(Some(&policy), None);
        assert!(result.is_ok());
    }

    // Note: inject_headers test requires a real Incoming body which is hard to construct in tests
    // This functionality is tested via the integration tests with real TLS connections
}
