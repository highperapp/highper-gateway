//! Web Application Firewall (WAF) - Adapter Pattern
//!
//! This module provides a flexible WAF system supporting multiple engines:
//! - Custom pattern-based engine
//! - Coraza (OWASP Core Rule Set)
//! - AWS WAF integration
//! - IP allowlist/blocklist
//!
//! ## Architecture
//!
//! The WAF uses an adapter pattern similar to the compression system:
//! - `WafEngine` trait defines the interface
//! - Multiple implementations (Custom, Coraza, AWS)
//! - `WafEngineRegistry` manages available engines
//! - `WafMiddleware` integrates with the middleware chain
//!
//! ## Usage
//!
//! ```rust
//! use highper_gateway::middleware::waf::{WafConfig, WafMode};
//!
//! let config = WafConfig {
//!     enabled: true,
//!     mode: WafMode::Custom,
//!     block_mode: true,
//!     ..Default::default()
//! };
//!
//! // Use config with WafMiddleware::new(config).await in async context
//! assert!(config.enabled);
//! ```

pub mod engine;
pub mod custom_engine;
pub mod coraza_engine;
pub mod modsecurity_engine;
pub mod aws_engine;

use crate::middleware::Middleware;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use engine::*;
pub use custom_engine::{CustomWafConfig, CustomWafEngine};
pub use coraza_engine::{CorazaConfig, CorazaWafEngine};
pub use modsecurity_engine::{ModSecurityConfig, ModSecurityEngine, DetectionMode};
pub use aws_engine::{AwsWafConfig, AwsWafEngine, ManagedRuleGroup, FallbackAction};

/// WAF mode selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WafMode {
    /// Custom pattern-based engine
    Custom,
    /// Coraza with OWASP CRS
    Coraza,
    /// ModSecurity v3 compatible engine
    ModSecurity,
    /// AWS WAF integration
    Aws,
}

/// WAF middleware configuration
#[derive(Debug, Clone)]
pub struct WafConfig {
    /// Enable WAF
    pub enabled: bool,

    /// WAF engine to use
    pub mode: WafMode,

    /// Block mode (true) or log-only mode (false)
    pub block_mode: bool,

    /// Custom engine configuration
    pub custom: Option<CustomWafConfig>,

    /// Coraza engine configuration
    pub coraza: Option<CorazaConfig>,

    /// ModSecurity engine configuration
    pub modsecurity: Option<ModSecurityConfig>,

    /// AWS WAF engine configuration
    pub aws: Option<AwsWafConfig>,

    /// Maximum request body size to inspect (bytes)
    pub max_body_size: usize,
}

impl Default for WafConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: WafMode::Custom,
            block_mode: true,
            custom: Some(CustomWafConfig::default()),
            coraza: None,
            modsecurity: None,
            aws: None,
            max_body_size: 1024 * 1024, // 1 MB
        }
    }
}

/// WAF Middleware implementation
pub struct WafMiddleware {
    config: WafConfig,
    engine: Arc<dyn WafEngine>,
}

impl WafMiddleware {
    /// Create a new WAF middleware
    pub fn new(config: WafConfig) -> anyhow::Result<Self> {
        // Select and initialize the appropriate engine
        let engine: Arc<dyn WafEngine> = match config.mode {
            WafMode::Custom => {
                let custom_config = config.custom.clone().unwrap_or_default();
                Arc::new(CustomWafEngine::new(custom_config))
            }
            WafMode::Coraza => {
                let coraza_config = config.coraza.clone().unwrap_or_default();
                Arc::new(CorazaWafEngine::new(coraza_config))
            }
            WafMode::ModSecurity => {
                let modsec_config = config.modsecurity.clone().unwrap_or_default();
                Arc::new(ModSecurityEngine::new(modsec_config)?)
            }
            WafMode::Aws => {
                let aws_config = config.aws.clone().unwrap_or_default();
                Arc::new(AwsWafEngine::new(aws_config)?)
            }
        };

        // Verify engine is available
        if !engine.is_available() {
            tracing::warn!(
                "WAF engine '{}' is not fully available, some features may be limited",
                engine.name()
            );
        }

        Ok(Self { config, engine })
    }

    /// Create middleware with custom engine
    pub fn with_custom(config: CustomWafConfig) -> Self {
        Self {
            config: WafConfig {
                enabled: true,
                mode: WafMode::Custom,
                block_mode: true,
                custom: Some(config.clone()),
                coraza: None,
                modsecurity: None,
                aws: None,
                max_body_size: 1024 * 1024,
            },
            engine: Arc::new(CustomWafEngine::new(config)),
        }
    }

    /// Create middleware with Coraza engine
    pub fn with_coraza(config: CorazaConfig) -> Self {
        Self {
            config: WafConfig {
                enabled: true,
                mode: WafMode::Coraza,
                block_mode: true,
                custom: None,
                coraza: Some(config.clone()),
                modsecurity: None,
                aws: None,
                max_body_size: 1024 * 1024,
            },
            engine: Arc::new(CorazaWafEngine::new(config)),
        }
    }

    /// Create middleware with ModSecurity engine
    pub fn with_modsecurity(config: ModSecurityConfig) -> anyhow::Result<Self> {
        Ok(Self {
            config: WafConfig {
                enabled: true,
                mode: WafMode::ModSecurity,
                block_mode: true,
                custom: None,
                coraza: None,
                modsecurity: Some(config.clone()),
                aws: None,
                max_body_size: 1024 * 1024,
            },
            engine: Arc::new(ModSecurityEngine::new(config)?),
        })
    }

    /// Create middleware with AWS WAF engine
    pub fn with_aws(config: AwsWafConfig) -> anyhow::Result<Self> {
        Ok(Self {
            config: WafConfig {
                enabled: true,
                mode: WafMode::Aws,
                block_mode: true,
                custom: None,
                coraza: None,
                modsecurity: None,
                aws: Some(config.clone()),
                max_body_size: 1024 * 1024,
            },
            engine: Arc::new(AwsWafEngine::new(config)?),
        })
    }

    /// Get middleware name
    pub fn name(&self) -> &'static str {
        "waf"
    }

    /// Get WAF statistics
    pub fn get_stats(&self) -> WafStats {
        self.engine.get_stats()
    }

    /// Get engine information
    pub fn get_engine_info(&self) -> WafEngineInfo {
        self.engine.get_info()
    }

    /// Create block response
    fn create_block_response(reason: &str, rule_id: Option<&str>, severity: WafSeverity) -> Response<Full<Bytes>> {
        let body = serde_json::json!({
            "error": "Forbidden",
            "reason": reason,
            "rule_id": rule_id,
            "severity": format!("{:?}", severity),
        });

        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap()
    }

    /// Create rate limit response
    fn create_rate_limit_response(retry_after: u64) -> Response<Full<Bytes>> {
        let body = serde_json::json!({
            "error": "Too Many Requests",
            "message": "Rate limit exceeded. Please try again later.",
            "retry_after": retry_after,
        });

        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "application/json")
            .header("Retry-After", retry_after.to_string())
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap()
    }

    /// Extract request context for WAF analysis
    async fn extract_context(
        req: &Request<hyper::body::Incoming>,
        _max_body_size: usize,
    ) -> WafContext {
        use std::collections::HashMap;

        let method = req.method().to_string();
        let uri = req.uri();
        let path = uri.path().to_string();
        let query = uri.query().map(|q| q.to_string());

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in req.headers() {
            if let Ok(val) = value.to_str() {
                headers.insert(name.to_string(), val.to_string());
            }
        }

        // Extract specific headers
        let content_type = headers.get("content-type").cloned();
        let user_agent = headers.get("user-agent").cloned();

        // Get client IP (from X-Forwarded-For or connection)
        let client_ip = headers
            .get("x-forwarded-for")
            .and_then(|h| h.split(',').next())
            .unwrap_or("unknown")
            .trim()
            .to_string();

        WafContext {
            method,
            path,
            query,
            headers,
            client_ip,
            body: None, // Body inspection requires consuming the request body
            content_type,
            user_agent,
        }
    }
}

impl Middleware for WafMiddleware {
    fn name(&self) -> &str {
        "waf"
    }

    fn process_request(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Pin<Box<dyn Future<Output = Result<Request<hyper::body::Incoming>, Response<Full<Bytes>>>> + Send>> {
        let config = self.config.clone();
        let engine = self.engine.clone();

        Box::pin(async move {
            if !config.enabled {
                return Ok(req);
            }

            // Extract context for WAF analysis
            let context = Self::extract_context(&req, config.max_body_size).await;

            // Check request against WAF engine
            let decision = engine.check_request(&context);

            match decision {
                WafDecision::Allow => Ok(req),
                WafDecision::Block { reason, rule_id, severity } => {
                    if config.block_mode {
                        tracing::warn!(
                            "WAF blocked request: {} (rule: {:?}, severity: {:?})",
                            reason,
                            rule_id,
                            severity
                        );
                        Err(Self::create_block_response(&reason, rule_id.as_deref(), severity))
                    } else {
                        tracing::info!(
                            "WAF detected threat (log-only): {} (rule: {:?})",
                            reason,
                            rule_id
                        );
                        Ok(req)
                    }
                }
                WafDecision::RateLimit { retry_after } => {
                    tracing::warn!("Rate limit exceeded for IP: {}", context.client_ip);
                    Err(Self::create_rate_limit_response(retry_after))
                }
                WafDecision::Log { reason, rule_id } => {
                    tracing::info!(
                        "WAF logged suspicious activity: {} (rule: {:?})",
                        reason,
                        rule_id
                    );
                    Ok(req)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waf_config_default() {
        let config = WafConfig::default();
        assert!(config.enabled);
        assert_eq!(config.mode, WafMode::Custom);
        assert!(config.block_mode);
        assert!(config.custom.is_some());
    }

    #[test]
    fn test_waf_middleware_creation() {
        let config = WafConfig::default();
        let waf = WafMiddleware::new(config);
        assert!(waf.is_ok());
    }

    #[test]
    fn test_waf_with_custom() {
        let custom_config = CustomWafConfig::default();
        let waf = WafMiddleware::with_custom(custom_config);
        assert_eq!(waf.name(), "waf");
    }

    #[test]
    fn test_waf_engine_info() {
        let waf = WafMiddleware::with_custom(CustomWafConfig::default());
        let info = waf.get_engine_info();
        assert_eq!(info.name, "custom");
        assert!(info.available);
    }
}
