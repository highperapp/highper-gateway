//! Custom WAF Engine - Simple pattern-based implementation
//!
//! Provides basic security features:
//! - SQL injection detection
//! - XSS detection
//! - Path traversal detection
//! - User-Agent filtering

use super::engine::*;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Custom pattern-based WAF engine
pub struct CustomWafEngine {
    config: CustomWafConfig,
    stats: CustomWafStats,
    rate_limits: Arc<DashMap<String, RateLimitEntry>>,
}

/// Configuration for custom WAF engine
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomWafConfig {
    #[serde(default = "default_true")]
    pub sql_injection_protection: bool,
    #[serde(default = "default_true")]
    pub xss_protection: bool,
    #[serde(default = "default_true")]
    pub path_traversal_protection: bool,
    #[serde(default = "default_true")]
    pub user_agent_filtering: bool,
    #[serde(default = "default_true")]
    pub rate_limit_enabled: bool,
    #[serde(default = "default_rate_limit_requests")]
    pub rate_limit_requests: usize,
    #[serde(default = "default_rate_limit_window")]
    pub rate_limit_window_secs: u64,
}

fn default_true() -> bool {
    true
}

fn default_rate_limit_requests() -> usize {
    100
}

fn default_rate_limit_window() -> u64 {
    60
}

impl Default for CustomWafConfig {
    fn default() -> Self {
        Self {
            sql_injection_protection: true,
            xss_protection: true,
            path_traversal_protection: true,
            user_agent_filtering: true,
            rate_limit_enabled: true,
            rate_limit_requests: 100,
            rate_limit_window_secs: 60,
        }
    }
}

/// Statistics for custom WAF engine
#[derive(Debug, Default)]
struct CustomWafStats {
    total_requests: AtomicU64,
    blocked_sql_injection: AtomicU64,
    blocked_xss: AtomicU64,
    blocked_path_traversal: AtomicU64,
    blocked_user_agent: AtomicU64,
    rate_limited: AtomicU64,
    allowed: AtomicU64,
}

/// Rate limit entry
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: usize,
    window_start: Instant,
}

impl CustomWafEngine {
    /// Create a new custom WAF engine
    pub fn new(config: CustomWafConfig) -> Self {
        Self {
            config,
            stats: CustomWafStats::default(),
            rate_limits: Arc::new(DashMap::new()),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CustomWafConfig::default())
    }

    /// Check rate limit for an IP
    fn check_rate_limit(&self, client_ip: &str) -> bool {
        if !self.config.rate_limit_enabled {
            return false;
        }

        let now = Instant::now();
        let window = Duration::from_secs(self.config.rate_limit_window_secs);

        let mut entry = self.rate_limits
            .entry(client_ip.to_string())
            .or_insert(RateLimitEntry {
                count: 0,
                window_start: now,
            });

        // Reset if window expired
        if now.duration_since(entry.window_start) > window {
            entry.count = 0;
            entry.window_start = now;
        }

        entry.count += 1;

        entry.count > self.config.rate_limit_requests
    }

    /// Detect SQL injection patterns
    fn detect_sql_injection(input: &str) -> bool {
        let input_lower = input.to_lowercase();

        let patterns = [
            "' or '1'='1",
            "' or 1=1",
            "admin'--",
            "' union select",
            "' drop table",
            "' delete from",
            "' insert into",
            "' update ",
            "; drop ",
            "; delete ",
            "exec(",
            "execute(",
            "xp_cmdshell",
            "sp_executesql",
            "union all select",
            "union select",
            "information_schema",
        ];

        patterns.iter().any(|pattern| input_lower.contains(pattern))
    }

    /// Detect XSS patterns
    fn detect_xss(input: &str) -> bool {
        let input_lower = input.to_lowercase();

        let patterns = [
            "<script",
            "javascript:",
            "onerror=",
            "onload=",
            "onclick=",
            "onmouseover=",
            "<iframe",
            "eval(",
            "alert(",
            "document.cookie",
            "document.write",
            "<img src",
            "<svg onload",
        ];

        patterns.iter().any(|pattern| input_lower.contains(pattern))
    }

    /// Detect path traversal
    fn detect_path_traversal(path: &str) -> bool {
        let patterns = ["../", "..\\", "%2e%2e/", "%2e%2e\\", "....//", "..%2f"];

        patterns.iter().any(|pattern| path.contains(pattern))
    }

    /// Check suspicious User-Agent
    fn check_user_agent(user_agent: &str) -> bool {
        let ua_lower = user_agent.to_lowercase();

        let suspicious = [
            "sqlmap",
            "nikto",
            "nmap",
            "masscan",
            "nessus",
            "acunetix",
            "burp",
            "owasp",
            "zap",
        ];

        suspicious.iter().any(|tool| ua_lower.contains(tool))
    }
}

impl WafEngine for CustomWafEngine {
    fn name(&self) -> &'static str {
        "custom"
    }

    fn is_available(&self) -> bool {
        true // Always available
    }

    fn check_request(&self, context: &WafContext) -> WafDecision {
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);

        // Check rate limit
        if self.check_rate_limit(&context.client_ip) {
            self.stats.rate_limited.fetch_add(1, Ordering::Relaxed);
            return WafDecision::RateLimit {
                retry_after: self.config.rate_limit_window_secs,
            };
        }

        // Check path traversal
        if self.config.path_traversal_protection && Self::detect_path_traversal(&context.path) {
            self.stats.blocked_path_traversal.fetch_add(1, Ordering::Relaxed);
            return WafDecision::Block {
                reason: "Path traversal attempt detected".to_string(),
                rule_id: Some("CUSTOM-001".to_string()),
                severity: WafSeverity::High,
            };
        }

        // Check query string
        if let Some(ref query) = context.query {
            if self.config.sql_injection_protection && Self::detect_sql_injection(query) {
                self.stats.blocked_sql_injection.fetch_add(1, Ordering::Relaxed);
                return WafDecision::Block {
                    reason: "SQL injection attempt detected".to_string(),
                    rule_id: Some("CUSTOM-002".to_string()),
                    severity: WafSeverity::Critical,
                };
            }

            if self.config.xss_protection && Self::detect_xss(query) {
                self.stats.blocked_xss.fetch_add(1, Ordering::Relaxed);
                return WafDecision::Block {
                    reason: "XSS attempt detected".to_string(),
                    rule_id: Some("CUSTOM-003".to_string()),
                    severity: WafSeverity::High,
                };
            }
        }

        // Check User-Agent
        if self.config.user_agent_filtering {
            if let Some(ref ua) = context.user_agent {
                if Self::check_user_agent(ua) {
                    self.stats.blocked_user_agent.fetch_add(1, Ordering::Relaxed);
                    return WafDecision::Block {
                        reason: "Suspicious User-Agent detected".to_string(),
                        rule_id: Some("CUSTOM-004".to_string()),
                        severity: WafSeverity::Medium,
                    };
                }
            }
        }

        self.stats.allowed.fetch_add(1, Ordering::Relaxed);
        WafDecision::Allow
    }

    fn get_stats(&self) -> WafStats {
        let mut block_reasons = std::collections::HashMap::new();
        block_reasons.insert(
            "SQL Injection".to_string(),
            self.stats.blocked_sql_injection.load(Ordering::Relaxed),
        );
        block_reasons.insert(
            "XSS".to_string(),
            self.stats.blocked_xss.load(Ordering::Relaxed),
        );
        block_reasons.insert(
            "Path Traversal".to_string(),
            self.stats.blocked_path_traversal.load(Ordering::Relaxed),
        );
        block_reasons.insert(
            "Suspicious User-Agent".to_string(),
            self.stats.blocked_user_agent.load(Ordering::Relaxed),
        );

        let total = self.stats.total_requests.load(Ordering::Relaxed);
        let blocked = self.stats.blocked_sql_injection.load(Ordering::Relaxed)
            + self.stats.blocked_xss.load(Ordering::Relaxed)
            + self.stats.blocked_path_traversal.load(Ordering::Relaxed)
            + self.stats.blocked_user_agent.load(Ordering::Relaxed);

        WafStats {
            total_requests: total,
            blocked_requests: blocked,
            logged_requests: 0,
            rate_limited: self.stats.rate_limited.load(Ordering::Relaxed),
            allowed_requests: self.stats.allowed.load(Ordering::Relaxed),
            block_reasons,
        }
    }

    fn get_info(&self) -> WafEngineInfo {
        WafEngineInfo {
            name: self.name(),
            version: "1.0.0",
            available: true,
            features: vec![
                "SQL Injection Detection",
                "XSS Detection",
                "Path Traversal Detection",
                "User-Agent Filtering",
                "IP Rate Limiting",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        assert!(CustomWafEngine::detect_sql_injection("test' OR '1'='1"));
        assert!(CustomWafEngine::detect_sql_injection("admin'--"));
        assert!(CustomWafEngine::detect_sql_injection("'; DROP TABLE users--"));
        assert!(!CustomWafEngine::detect_sql_injection("normal query"));
    }

    #[test]
    fn test_xss_detection() {
        assert!(CustomWafEngine::detect_xss("<script>alert(1)</script>"));
        assert!(CustomWafEngine::detect_xss("javascript:alert('xss')"));
        assert!(CustomWafEngine::detect_xss("<img onerror=alert(1)>"));
        assert!(!CustomWafEngine::detect_xss("normal text"));
    }

    #[test]
    fn test_path_traversal_detection() {
        assert!(CustomWafEngine::detect_path_traversal("../../etc/passwd"));
        assert!(CustomWafEngine::detect_path_traversal("..\\..\\windows"));
        assert!(CustomWafEngine::detect_path_traversal("%2e%2e/"));
        assert!(!CustomWafEngine::detect_path_traversal("/normal/path"));
    }

    #[test]
    fn test_user_agent_check() {
        assert!(CustomWafEngine::check_user_agent("sqlmap/1.0"));
        assert!(CustomWafEngine::check_user_agent("Nikto scanner"));
        assert!(!CustomWafEngine::check_user_agent("Mozilla/5.0"));
    }

    #[test]
    fn test_waf_engine_trait() {
        let engine = CustomWafEngine::with_defaults();

        assert_eq!(engine.name(), "custom");
        assert!(engine.is_available());

        let context = WafContext {
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: Some("id=1".to_string()),
            headers: std::collections::HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        let decision = engine.check_request(&context);
        assert_eq!(decision, WafDecision::Allow);
    }

    #[test]
    fn test_sql_injection_blocking() {
        let engine = CustomWafEngine::with_defaults();

        let context = WafContext {
            method: "GET".to_string(),
            path: "/search".to_string(),
            query: Some("q=' OR '1'='1".to_string()),
            headers: std::collections::HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        match engine.check_request(&context) {
            WafDecision::Block { reason, .. } => {
                assert!(reason.contains("SQL injection"));
            }
            _ => panic!("Expected Block decision"),
        }
    }
}
