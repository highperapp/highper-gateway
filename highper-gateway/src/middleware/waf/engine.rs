//! WAF Engine Trait - Adapter Pattern
//!
//! Defines the common interface for all WAF engines, allowing pluggable
//! WAF implementations (Custom, Coraza, AWS WAF, ModSecurity, etc.)

use std::collections::HashMap;
use std::sync::Arc;

/// WAF engine trait - adapter pattern for different WAF implementations
pub trait WafEngine: Send + Sync {
    /// Get engine name
    fn name(&self) -> &'static str;

    /// Check if engine is available and properly configured
    fn is_available(&self) -> bool;

    /// Analyze a request and return a decision
    fn check_request(&self, context: &WafContext) -> WafDecision;

    /// Update WAF rules (if supported)
    fn update_rules(&self, rules: &[WafRule]) -> Result<(), WafError> {
        Err(WafError::Unsupported(format!(
            "{} does not support dynamic rule updates",
            self.name()
        )))
    }

    /// Get WAF statistics
    fn get_stats(&self) -> WafStats;

    /// Get engine configuration info
    fn get_info(&self) -> WafEngineInfo {
        WafEngineInfo {
            name: self.name(),
            version: "unknown",
            available: self.is_available(),
            features: vec![],
        }
    }
}

/// Request context for WAF analysis
#[derive(Debug, Clone)]
pub struct WafContext {
    /// Request method
    pub method: String,

    /// Request URI path
    pub path: String,

    /// Query string (if any)
    pub query: Option<String>,

    /// Request headers
    pub headers: HashMap<String, String>,

    /// Client IP address
    pub client_ip: String,

    /// Request body (if available and within size limit)
    pub body: Option<Vec<u8>>,

    /// Content-Type header value
    pub content_type: Option<String>,

    /// User-Agent header value
    pub user_agent: Option<String>,
}

/// WAF decision after analyzing a request
#[derive(Debug, Clone, PartialEq)]
pub enum WafDecision {
    /// Allow the request to proceed
    Allow,

    /// Block the request with a reason
    Block {
        reason: String,
        rule_id: Option<String>,
        severity: WafSeverity,
    },

    /// Rate limit exceeded
    RateLimit {
        retry_after: u64,
    },

    /// Log suspicious activity but allow
    Log {
        reason: String,
        rule_id: Option<String>,
    },
}

/// Severity level for WAF violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// WAF rule definition
#[derive(Debug, Clone)]
pub struct WafRule {
    /// Rule ID
    pub id: String,

    /// Rule description
    pub description: String,

    /// Rule pattern (regex or string match)
    pub pattern: String,

    /// Where to apply the rule
    pub target: WafRuleTarget,

    /// Action to take
    pub action: WafRuleAction,

    /// Severity level
    pub severity: WafSeverity,

    /// Enabled flag
    pub enabled: bool,
}

/// Target for WAF rule application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafRuleTarget {
    Path,
    Query,
    Headers,
    Body,
    UserAgent,
    All,
}

/// Action to take when rule matches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WafRuleAction {
    Block,
    Log,
    Allow,
}

/// WAF statistics
#[derive(Debug, Clone, Default)]
pub struct WafStats {
    /// Total requests analyzed
    pub total_requests: u64,

    /// Requests blocked
    pub blocked_requests: u64,

    /// Requests logged (suspicious but allowed)
    pub logged_requests: u64,

    /// Rate limited requests
    pub rate_limited: u64,

    /// Allowed requests
    pub allowed_requests: u64,

    /// Breakdown by rule/reason
    pub block_reasons: HashMap<String, u64>,
}

/// WAF engine information
#[derive(Debug, Clone)]
pub struct WafEngineInfo {
    /// Engine name
    pub name: &'static str,

    /// Engine version
    pub version: &'static str,

    /// Is the engine available?
    pub available: bool,

    /// Supported features
    pub features: Vec<&'static str>,
}

/// WAF error types
#[derive(Debug, thiserror::Error)]
pub enum WafError {
    #[error("Engine not available: {0}")]
    NotAvailable(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Rule error: {0}")]
    RuleError(String),

    #[error("Engine error: {0}")]
    EngineError(String),

    #[error("Feature not supported: {0}")]
    Unsupported(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// WAF engine registry - manages available WAF engines
pub struct WafEngineRegistry {
    engines: HashMap<String, Arc<dyn WafEngine>>,
}

impl WafEngineRegistry {
    /// Create a new WAF engine registry
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    /// Register a WAF engine
    pub fn register(&mut self, engine: Arc<dyn WafEngine>) {
        self.engines.insert(engine.name().to_string(), engine);
    }

    /// Get a WAF engine by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn WafEngine>> {
        self.engines.get(name).cloned()
    }

    /// List all registered engines
    pub fn list(&self) -> Vec<String> {
        self.engines.keys().cloned().collect()
    }

    /// List engine information
    pub fn list_info(&self) -> Vec<WafEngineInfo> {
        self.engines
            .values()
            .map(|engine| engine.get_info())
            .collect()
    }
}

impl Default for WafEngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}
