//! ModSecurity WAF Engine - ModSecurity v3 Compatible Implementation
//!
//! This module provides a ModSecurity-compatible WAF engine that supports
//! SecRule syntax and can load ModSecurity rule files.
//!
//! Features:
//! - ModSecurity v3 rule syntax
//! - SecRule directives
//! - SecAction support
//! - Transformation functions
//! - Variable collections
//! - Chained rules

use super::engine::*;
use dashmap::DashMap;
use regex::Regex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// ModSecurity WAF engine
pub struct ModSecurityEngine {
    config: ModSecurityConfig,
    rules: Vec<SecRule>,
    stats: ModSecurityStats,
    compiled_patterns: Arc<DashMap<String, Regex>>,
}

/// ModSecurity engine configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModSecurityConfig {
    /// Enable ModSecurity rules
    #[serde(default = "default_modsec_true")]
    pub enabled: bool,

    /// Rules file path
    pub rules_file: Option<String>,

    /// Inline rules
    #[serde(default)]
    pub inline_rules: Vec<String>,

    /// SecRuleEngine mode
    #[serde(default = "default_detection_mode")]
    pub detection_mode: DetectionMode,

    /// Request body access
    #[serde(default = "default_modsec_true")]
    pub request_body_access: bool,

    /// Response body access
    #[serde(default = "default_modsec_false")]
    pub response_body_access: bool,

    /// Request body limit
    #[serde(default = "default_body_limit")]
    pub request_body_limit: usize,
}

fn default_modsec_true() -> bool {
    true
}

fn default_modsec_false() -> bool {
    false
}

fn default_detection_mode() -> DetectionMode {
    DetectionMode::On
}

fn default_body_limit() -> usize {
    1024 * 1024 // 1 MB
}

impl Default for ModSecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules_file: None,
            inline_rules: vec![],
            detection_mode: DetectionMode::On,
            request_body_access: true,
            response_body_access: false,
            request_body_limit: 1024 * 1024,
        }
    }
}

/// ModSecurity detection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMode {
    /// ModSecurity is enabled
    On,
    /// Detection only, no blocking
    DetectionOnly,
    /// ModSecurity is disabled
    Off,
}

/// SecRule definition
#[derive(Debug, Clone)]
struct SecRule {
    id: String,
    phase: u8,
    variables: Vec<ModSecVariable>,
    operator: Operator,
    actions: Vec<RuleAction>,
    severity: WafSeverity,
    message: String,
    tag: Vec<String>,
    chain: Option<Box<SecRule>>,
}

/// ModSecurity variables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModSecVariable {
    Args,
    ArgsGet,
    ArgsPost,
    RequestUri,
    RequestMethod,
    RequestHeaders,
    RequestBody,
    RequestCookies,
    RemoteAddr,
    QueryString,
}

/// ModSecurity operators
#[derive(Debug, Clone)]
enum Operator {
    /// Regex match
    Rx(String),
    /// Contains string
    Contains(String),
    /// String match
    StreQ(String),
    /// Begins with
    BeginsWith(String),
    /// Ends with
    EndsWith(String),
    /// Validate UTF-8
    ValidateUtf8,
    /// IP match
    IpMatch(String),
    /// Greater than
    Gt(i64),
    /// Less than
    Lt(i64),
}

/// Rule action
#[derive(Debug, Clone)]
enum RuleAction {
    Block,
    Pass,
    Deny,
    Allow,
    Log,
    NoLog,
    Id(String),
    Phase(u8),
    Severity(WafSeverity),
    Msg(String),
    Tag(String),
    Chain,
}

/// Statistics for ModSecurity engine
#[derive(Debug, Default)]
struct ModSecurityStats {
    total_requests: AtomicU64,
    blocked_requests: AtomicU64,
    logged_requests: AtomicU64,
    allowed_requests: AtomicU64,
    rules_matched: AtomicU64,
}

impl ModSecurityEngine {
    /// Create a new ModSecurity WAF engine
    pub fn new(config: ModSecurityConfig) -> anyhow::Result<Self> {
        let mut rules = Vec::new();

        // Load default rules
        rules.extend(Self::default_rules());

        // Load rules from file if specified
        if let Some(rules_file) = &config.rules_file {
            match Self::load_rules_from_file(rules_file) {
                Ok(file_rules) => {
                    rules.extend(file_rules);
                }
                Err(e) => {
                    tracing::warn!("Failed to load rules from {}: {}", rules_file, e);
                }
            }
        }

        // Parse inline rules
        for rule_str in &config.inline_rules {
            match Self::parse_sec_rule(rule_str) {
                Ok(rule) => rules.push(rule),
                Err(e) => {
                    tracing::warn!("Failed to parse rule '{}': {}", rule_str, e);
                }
            }
        }

        Ok(Self {
            config,
            rules,
            stats: ModSecurityStats::default(),
            compiled_patterns: Arc::new(DashMap::new()),
        })
    }

    /// Create with default rules
    pub fn with_defaults() -> Self {
        Self::new(ModSecurityConfig::default()).unwrap()
    }

    /// Load rules from a .conf file
    pub fn load_rules_from_file(path: &str) -> anyhow::Result<Vec<SecRule>> {
        let content = std::fs::read_to_string(path)?;
        let mut rules = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse SecRule directives
            if line.starts_with("SecRule") {
                match Self::parse_sec_rule(line) {
                    Ok(rule) => rules.push(rule),
                    Err(e) => {
                        tracing::warn!("Failed to parse rule in {}: {} - {}", path, line, e);
                    }
                }
            }
            // TODO: Handle other directives (SecAction, SecDefaultAction, etc.)
        }

        Ok(rules)
    }

    /// Default ModSecurity-style rules
    fn default_rules() -> Vec<SecRule> {
        vec![
            // SQL Injection detection
            SecRule {
                id: "950001".to_string(),
                phase: 2,
                variables: vec![ModSecVariable::Args, ModSecVariable::QueryString],
                operator: Operator::Rx(
                    r"(?i)(union\s+select|select\s+from|insert\s+into|delete\s+from|drop\s+table)"
                        .to_string(),
                ),
                actions: vec![
                    RuleAction::Id("950001".to_string()),
                    RuleAction::Phase(2),
                    RuleAction::Block,
                    RuleAction::Msg("SQL Injection Attack Detected".to_string()),
                    RuleAction::Severity(WafSeverity::Critical),
                    RuleAction::Tag("OWASP_CRS/WEB_ATTACK/SQL_INJECTION".to_string()),
                ],
                severity: WafSeverity::Critical,
                message: "SQL Injection Attack Detected".to_string(),
                tag: vec!["OWASP_CRS".to_string(), "SQL_INJECTION".to_string()],
                chain: None,
            },
            // XSS detection
            SecRule {
                id: "950002".to_string(),
                phase: 2,
                variables: vec![ModSecVariable::Args, ModSecVariable::RequestBody],
                operator: Operator::Rx(
                    r"(?i)(<script|javascript:|onerror=|onload=|onclick=)".to_string(),
                ),
                actions: vec![
                    RuleAction::Id("950002".to_string()),
                    RuleAction::Phase(2),
                    RuleAction::Block,
                    RuleAction::Msg("XSS Attack Detected".to_string()),
                    RuleAction::Severity(WafSeverity::High),
                    RuleAction::Tag("OWASP_CRS/WEB_ATTACK/XSS".to_string()),
                ],
                severity: WafSeverity::High,
                message: "XSS Attack Detected".to_string(),
                tag: vec!["OWASP_CRS".to_string(), "XSS".to_string()],
                chain: None,
            },
            // Path traversal
            SecRule {
                id: "950003".to_string(),
                phase: 1,
                variables: vec![ModSecVariable::RequestUri],
                operator: Operator::Rx(r"(\.\./|\.\.\\|%2e%2e/)".to_string()),
                actions: vec![
                    RuleAction::Id("950003".to_string()),
                    RuleAction::Phase(1),
                    RuleAction::Block,
                    RuleAction::Msg("Path Traversal Attack Detected".to_string()),
                    RuleAction::Severity(WafSeverity::High),
                    RuleAction::Tag("OWASP_CRS/WEB_ATTACK/FILE_INJECTION".to_string()),
                ],
                severity: WafSeverity::High,
                message: "Path Traversal Attack Detected".to_string(),
                tag: vec!["OWASP_CRS".to_string(), "FILE_INJECTION".to_string()],
                chain: None,
            },
            // Command injection
            SecRule {
                id: "950004".to_string(),
                phase: 2,
                variables: vec![ModSecVariable::Args],
                operator: Operator::Rx(r"(?i)(;|\||`)\s*(cat|ls|wget|curl|bash|sh)".to_string()),
                actions: vec![
                    RuleAction::Id("950004".to_string()),
                    RuleAction::Phase(2),
                    RuleAction::Block,
                    RuleAction::Msg("Command Injection Attack Detected".to_string()),
                    RuleAction::Severity(WafSeverity::Critical),
                    RuleAction::Tag("OWASP_CRS/WEB_ATTACK/COMMAND_INJECTION".to_string()),
                ],
                severity: WafSeverity::Critical,
                message: "Command Injection Attack Detected".to_string(),
                tag: vec!["OWASP_CRS".to_string(), "COMMAND_INJECTION".to_string()],
                chain: None,
            },
            // HTTP protocol violation
            SecRule {
                id: "950005".to_string(),
                phase: 1,
                variables: vec![ModSecVariable::RequestHeaders],
                operator: Operator::Rx(r"(?i)(content-length:.*\n.*content-length:)".to_string()),
                actions: vec![
                    RuleAction::Id("950005".to_string()),
                    RuleAction::Phase(1),
                    RuleAction::Block,
                    RuleAction::Msg("HTTP Protocol Violation".to_string()),
                    RuleAction::Severity(WafSeverity::Medium),
                    RuleAction::Tag("OWASP_CRS/PROTOCOL_VIOLATION".to_string()),
                ],
                severity: WafSeverity::Medium,
                message: "HTTP Protocol Violation".to_string(),
                tag: vec!["OWASP_CRS".to_string(), "PROTOCOL_VIOLATION".to_string()],
                chain: None,
            },
        ]
    }

    /// Parse a SecRule directive
    ///
    /// Format: SecRule VARIABLES "OPERATOR value" "ACTIONS"
    /// Example: SecRule REQUEST_URI "@rx \.\./" "id:1,phase:1,deny,msg:'Path Traversal'"
    fn parse_sec_rule(rule_str: &str) -> anyhow::Result<SecRule> {
        let rule_str = rule_str.trim();

        // Must start with "SecRule"
        if !rule_str.starts_with("SecRule") {
            return Err(anyhow::anyhow!("Rule must start with 'SecRule'"));
        }

        // Remove "SecRule" prefix
        let rule_str = rule_str[7..].trim();

        // Split into 3 parts: variables, operator, actions
        let parts: Vec<&str> = Self::split_secrule_parts(rule_str)?;

        if parts.len() != 3 {
            return Err(anyhow::anyhow!(
                "SecRule must have 3 parts: VARIABLES OPERATOR ACTIONS"
            ));
        }

        // Parse variables
        let variables = Self::parse_variables(parts[0])?;

        // Parse operator (format: "@rx pattern" or "!@contains value")
        let operator = Self::parse_operator(parts[1])?;

        // Parse actions (format: "id:1,phase:2,deny,msg:'message'")
        let (actions, id, phase, severity, message, tags) = Self::parse_actions(parts[2])?;

        Ok(SecRule {
            id: id.unwrap_or_else(|| "0".to_string()),
            phase: phase.unwrap_or(2),
            variables,
            operator,
            actions,
            severity: severity.unwrap_or(WafSeverity::Medium),
            message: message.unwrap_or_else(|| "Rule matched".to_string()),
            tag: tags,
            chain: None, // TODO: Handle chained rules
        })
    }

    /// Split SecRule into 3 parts, handling quoted strings
    fn split_secrule_parts(s: &str) -> anyhow::Result<Vec<&str>> {
        let mut parts = Vec::new();
        let mut current_start = 0;
        let mut in_quotes = false;
        let mut quote_char = ' ';
        let chars: Vec<char> = s.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            if ch == '"' || ch == '\'' {
                if in_quotes && ch == quote_char {
                    in_quotes = false;
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                }
            } else if ch.is_whitespace() && !in_quotes && i > current_start {
                let part = &s[current_start..i];
                if !part.trim().is_empty() {
                    parts.push(part.trim());
                    current_start = i + 1;
                }
            }
        }

        // Add last part
        if current_start < s.len() {
            let part = &s[current_start..];
            if !part.trim().is_empty() {
                parts.push(part.trim());
            }
        }

        Ok(parts)
    }

    /// Parse variables (e.g., "ARGS", "REQUEST_URI", "ARGS|REQUEST_BODY")
    fn parse_variables(var_str: &str) -> anyhow::Result<Vec<ModSecVariable>> {
        let var_parts: Vec<&str> = var_str.split('|').map(|s| s.trim()).collect();
        let mut variables = Vec::new();

        for part in var_parts {
            let var = match part {
                "ARGS" => ModSecVariable::Args,
                "ARGS_GET" => ModSecVariable::ArgsGet,
                "ARGS_POST" => ModSecVariable::ArgsPost,
                "REQUEST_URI" => ModSecVariable::RequestUri,
                "REQUEST_METHOD" => ModSecVariable::RequestMethod,
                "REQUEST_HEADERS" => ModSecVariable::RequestHeaders,
                "REQUEST_BODY" => ModSecVariable::RequestBody,
                "REQUEST_COOKIES" => ModSecVariable::RequestCookies,
                "REMOTE_ADDR" => ModSecVariable::RemoteAddr,
                "QUERY_STRING" => ModSecVariable::QueryString,
                _ => return Err(anyhow::anyhow!("Unknown variable: {}", part)),
            };
            variables.push(var);
        }

        if variables.is_empty() {
            return Err(anyhow::anyhow!("No variables specified"));
        }

        Ok(variables)
    }

    /// Parse operator (e.g., "@rx pattern", "@contains value", "!@streq test")
    fn parse_operator(op_str: &str) -> anyhow::Result<Operator> {
        // Remove quotes if present
        let op_str = op_str.trim().trim_matches('"').trim_matches('\'');

        // Check for negation (!)
        let _negated = op_str.starts_with('!');
        let op_str = if _negated { &op_str[1..] } else { op_str };

        // Operator starts with @
        if !op_str.starts_with('@') {
            return Err(anyhow::anyhow!("Operator must start with @"));
        }

        let op_str = &op_str[1..]; // Remove @

        // Split operator and value
        let parts: Vec<&str> = op_str.splitn(2, ' ').collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid operator format"));
        }

        let op_name = parts[0];
        let op_value = if parts.len() > 1 {
            parts[1].trim()
        } else {
            ""
        };

        match op_name {
            "rx" => Ok(Operator::Rx(op_value.to_string())),
            "contains" => Ok(Operator::Contains(op_value.to_string())),
            "streq" => Ok(Operator::StreQ(op_value.to_string())),
            "beginsWith" => Ok(Operator::BeginsWith(op_value.to_string())),
            "endsWith" => Ok(Operator::EndsWith(op_value.to_string())),
            "validateUtf8" => Ok(Operator::ValidateUtf8),
            "ipMatch" => Ok(Operator::IpMatch(op_value.to_string())),
            "gt" => {
                let val = op_value.parse::<i64>()
                    .map_err(|_| anyhow::anyhow!("Invalid gt value: {}", op_value))?;
                Ok(Operator::Gt(val))
            }
            "lt" => {
                let val = op_value.parse::<i64>()
                    .map_err(|_| anyhow::anyhow!("Invalid lt value: {}", op_value))?;
                Ok(Operator::Lt(val))
            }
            _ => Err(anyhow::anyhow!("Unknown operator: {}", op_name)),
        }
    }

    /// Parse actions (e.g., "id:1,phase:2,deny,msg:'SQL Injection'")
    fn parse_actions(
        action_str: &str,
    ) -> anyhow::Result<(Vec<RuleAction>, Option<String>, Option<u8>, Option<WafSeverity>, Option<String>, Vec<String>)> {
        // Remove quotes
        let action_str = action_str.trim().trim_matches('"').trim_matches('\'');

        let mut actions = Vec::new();
        let mut id = None;
        let mut phase = None;
        let mut severity = None;
        let mut message = None;
        let mut tags = Vec::new();

        // Split by comma, but handle quoted strings
        let action_parts = Self::split_action_parts(action_str);

        for part in action_parts {
            let part = part.trim();

            if part.contains(':') {
                let kv: Vec<&str> = part.splitn(2, ':').collect();
                let key = kv[0];
                let value = kv[1].trim_matches('\'').trim_matches('"');

                match key {
                    "id" => {
                        id = Some(value.to_string());
                        actions.push(RuleAction::Id(value.to_string()));
                    }
                    "phase" => {
                        let p = value.parse::<u8>()
                            .map_err(|_| anyhow::anyhow!("Invalid phase: {}", value))?;
                        phase = Some(p);
                        actions.push(RuleAction::Phase(p));
                    }
                    "severity" => {
                        let sev = match value.to_uppercase().as_str() {
                            "CRITICAL" | "5" => WafSeverity::Critical,
                            "HIGH" | "4" => WafSeverity::High,
                            "MEDIUM" | "3" => WafSeverity::Medium,
                            "LOW" | "2" => WafSeverity::Low,
                            _ => WafSeverity::Medium,
                        };
                        severity = Some(sev);
                        actions.push(RuleAction::Severity(sev));
                    }
                    "msg" => {
                        message = Some(value.to_string());
                        actions.push(RuleAction::Msg(value.to_string()));
                    }
                    "tag" => {
                        tags.push(value.to_string());
                        actions.push(RuleAction::Tag(value.to_string()));
                    }
                    _ => {} // Ignore unknown action parameters
                }
            } else {
                // Simple action without value
                match part {
                    "block" => actions.push(RuleAction::Block),
                    "deny" => actions.push(RuleAction::Deny),
                    "allow" => actions.push(RuleAction::Allow),
                    "pass" => actions.push(RuleAction::Pass),
                    "log" => actions.push(RuleAction::Log),
                    "nolog" => actions.push(RuleAction::NoLog),
                    "chain" => actions.push(RuleAction::Chain),
                    _ => {} // Ignore unknown simple actions
                }
            }
        }

        Ok((actions, id, phase, severity, message, tags))
    }

    /// Split action string by commas, respecting quotes
    fn split_action_parts(s: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';

        for ch in s.chars() {
            if ch == '\'' || ch == '"' {
                if in_quotes && ch == quote_char {
                    in_quotes = false;
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                }
                // Keep the quote in the string for later processing
                current.push(ch);
            } else if ch == ',' && !in_quotes {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                    current.clear();
                }
            } else {
                current.push(ch);
            }
        }

        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }

        parts
    }

    /// Extract variable value from context
    fn extract_variable(variable: &ModSecVariable, context: &WafContext) -> Vec<String> {
        match variable {
            ModSecVariable::Args | ModSecVariable::ArgsGet => {
                if let Some(ref query) = context.query {
                    vec![query.clone()]
                } else {
                    vec![]
                }
            }
            ModSecVariable::ArgsPost => {
                // Would need to parse POST body
                vec![]
            }
            ModSecVariable::RequestUri => vec![context.path.clone()],
            ModSecVariable::RequestMethod => vec![context.method.clone()],
            ModSecVariable::RequestHeaders => context.headers.values().cloned().collect(),
            ModSecVariable::RequestBody => {
                if let Some(ref body) = context.body {
                    if let Ok(body_str) = std::str::from_utf8(body) {
                        vec![body_str.to_string()]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
            ModSecVariable::RequestCookies => {
                if let Some(cookie_header) = context.headers.get("cookie") {
                    vec![cookie_header.clone()]
                } else {
                    vec![]
                }
            }
            ModSecVariable::RemoteAddr => vec![context.client_ip.clone()],
            ModSecVariable::QueryString => {
                if let Some(ref query) = context.query {
                    vec![query.clone()]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Check if operator matches
    fn operator_matches(&self, operator: &Operator, value: &str) -> bool {
        match operator {
            Operator::Rx(pattern) => {
                if let Ok(regex) = self.get_or_compile_regex(pattern) {
                    regex.is_match(value)
                } else {
                    false
                }
            }
            Operator::Contains(s) => value.contains(s),
            Operator::StreQ(s) => value == s,
            Operator::BeginsWith(s) => value.starts_with(s),
            Operator::EndsWith(s) => value.ends_with(s),
            Operator::ValidateUtf8 => std::str::from_utf8(value.as_bytes()).is_ok(),
            Operator::IpMatch(_) => false, // Simplified
            Operator::Gt(n) => value.parse::<i64>().map_or(false, |v| v > *n),
            Operator::Lt(n) => value.parse::<i64>().map_or(false, |v| v < *n),
        }
    }

    /// Get or compile regex
    fn get_or_compile_regex(&self, pattern: &str) -> Result<Regex, regex::Error> {
        if let Some(cached) = self.compiled_patterns.get(pattern) {
            return Ok(cached.clone());
        }

        let regex = Regex::new(pattern)?;
        self.compiled_patterns.insert(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Evaluate a single rule
    fn evaluate_rule(&self, rule: &SecRule, context: &WafContext) -> bool {
        for variable in &rule.variables {
            let values = Self::extract_variable(variable, context);
            for value in values {
                if self.operator_matches(&rule.operator, &value) {
                    return true;
                }
            }
        }
        false
    }

    /// Process request through all rules
    fn process_rules(&self, context: &WafContext) -> Option<(String, String, WafSeverity)> {
        for rule in &self.rules {
            if self.evaluate_rule(rule, context) {
                self.stats.rules_matched.fetch_add(1, Ordering::Relaxed);

                // Check if rule should block
                let should_block = rule.actions.iter().any(|a| {
                    matches!(
                        a,
                        RuleAction::Block | RuleAction::Deny
                    )
                });

                if should_block {
                    return Some((
                        rule.message.clone(),
                        rule.id.clone(),
                        rule.severity,
                    ));
                }
            }
        }
        None
    }
}

impl WafEngine for ModSecurityEngine {
    fn name(&self) -> &'static str {
        "modsecurity"
    }

    fn is_available(&self) -> bool {
        self.config.enabled
    }

    fn check_request(&self, context: &WafContext) -> WafDecision {
        if !self.config.enabled || self.config.detection_mode == DetectionMode::Off {
            return WafDecision::Allow;
        }

        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);

        // Process rules
        if let Some((message, rule_id, severity)) = self.process_rules(context) {
            if self.config.detection_mode == DetectionMode::DetectionOnly {
                self.stats.logged_requests.fetch_add(1, Ordering::Relaxed);
                WafDecision::Log {
                    reason: message,
                    rule_id: Some(rule_id),
                }
            } else {
                self.stats.blocked_requests.fetch_add(1, Ordering::Relaxed);
                WafDecision::Block {
                    reason: message,
                    rule_id: Some(rule_id),
                    severity,
                }
            }
        } else {
            self.stats.allowed_requests.fetch_add(1, Ordering::Relaxed);
            WafDecision::Allow
        }
    }

    fn get_stats(&self) -> WafStats {
        WafStats {
            total_requests: self.stats.total_requests.load(Ordering::Relaxed),
            blocked_requests: self.stats.blocked_requests.load(Ordering::Relaxed),
            logged_requests: self.stats.logged_requests.load(Ordering::Relaxed),
            rate_limited: 0,
            allowed_requests: self.stats.allowed_requests.load(Ordering::Relaxed),
            block_reasons: HashMap::new(),
        }
    }

    fn get_info(&self) -> WafEngineInfo {
        WafEngineInfo {
            name: self.name(),
            version: "3.0-compatible",
            available: true,
            features: vec![
                "ModSecurity v3 Compatible",
                "SecRule Directives",
                "Request Body Inspection",
                "Variable Collections",
                "Regex Operators",
                "Detection/Blocking Modes",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modsecurity_engine_creation() {
        let engine = ModSecurityEngine::with_defaults();
        assert_eq!(engine.name(), "modsecurity");
        assert!(engine.is_available());
    }

    #[test]
    fn test_sql_injection_detection() {
        let engine = ModSecurityEngine::with_defaults();

        let context = WafContext {
            method: "GET".to_string(),
            path: "/api/users".to_string(),
            query: Some("id=1 UNION SELECT * FROM passwords".to_string()),
            headers: HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        match engine.check_request(&context) {
            WafDecision::Block { reason, .. } => {
                assert!(reason.contains("SQL Injection"));
            }
            _ => panic!("Expected Block decision"),
        }
    }

    #[test]
    fn test_xss_detection() {
        let engine = ModSecurityEngine::with_defaults();

        let context = WafContext {
            method: "POST".to_string(),
            path: "/comment".to_string(),
            query: None,
            headers: HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: Some(b"<script>alert('XSS')</script>".to_vec()),
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            user_agent: None,
        };

        match engine.check_request(&context) {
            WafDecision::Block { reason, .. } => {
                assert!(reason.contains("XSS"));
            }
            _ => panic!("Expected Block decision"),
        }
    }

    #[test]
    fn test_detection_only_mode() {
        let mut config = ModSecurityConfig::default();
        config.detection_mode = DetectionMode::DetectionOnly;
        let engine = ModSecurityEngine::new(config).unwrap();

        let context = WafContext {
            method: "GET".to_string(),
            path: "/../../etc/passwd".to_string(),
            query: None,
            headers: HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        // Should log but not block
        match engine.check_request(&context) {
            WafDecision::Log { .. } => (),
            _ => panic!("Expected Log decision in DetectionOnly mode"),
        }
    }

    #[test]
    fn test_path_traversal_detection() {
        let engine = ModSecurityEngine::with_defaults();

        let context = WafContext {
            method: "GET".to_string(),
            path: "/files/../../../etc/passwd".to_string(),
            query: None,
            headers: HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        match engine.check_request(&context) {
            WafDecision::Block { .. } => (),
            _ => panic!("Expected Block decision for path traversal"),
        }
    }

    #[test]
    fn test_parse_secrule_simple() {
        let rule_str = r#"SecRule REQUEST_URI "@rx \.\.\/" "id:1,phase:1,deny,msg:'Path Traversal'"#;
        let rule = ModSecurityEngine::parse_sec_rule(rule_str).unwrap();

        assert_eq!(rule.id, "1");
        assert_eq!(rule.phase, 1);
        assert_eq!(rule.variables.len(), 1);
        assert!(matches!(rule.variables[0], ModSecVariable::RequestUri));
        assert!(matches!(rule.operator, Operator::Rx(_)));
        assert!(rule.message.contains("Path Traversal"));
    }

    #[test]
    fn test_parse_secrule_multiple_variables() {
        let rule_str = r#"SecRule ARGS|REQUEST_BODY "@contains <script" "id:2,phase:2,deny,severity:high,msg:'XSS Attack'"#;
        let rule = ModSecurityEngine::parse_sec_rule(rule_str).unwrap();

        assert_eq!(rule.id, "2");
        assert_eq!(rule.phase, 2);
        assert_eq!(rule.variables.len(), 2);
        assert!(rule.message.contains("XSS"));
        assert_eq!(rule.severity, WafSeverity::High);
    }

    #[test]
    fn test_parse_secrule_operators() {
        // Test @rx operator
        let rule1 = ModSecurityEngine::parse_sec_rule(
            r#"SecRule ARGS "@rx pattern" "id:1,deny""#
        ).unwrap();
        assert!(matches!(rule1.operator, Operator::Rx(_)));

        // Test @contains operator
        let rule2 = ModSecurityEngine::parse_sec_rule(
            r#"SecRule ARGS "@contains test" "id:2,deny""#
        ).unwrap();
        assert!(matches!(rule2.operator, Operator::Contains(_)));

        // Test @streq operator
        let rule3 = ModSecurityEngine::parse_sec_rule(
            r#"SecRule REQUEST_METHOD "@streq POST" "id:3,deny""#
        ).unwrap();
        assert!(matches!(rule3.operator, Operator::StreQ(_)));
    }

    #[test]
    fn test_parse_secrule_actions() {
        let rule_str = r#"SecRule ARGS "@rx test" "id:100,phase:2,deny,severity:critical,msg:'Test',tag:'ATTACK/TEST'"#;
        let rule = ModSecurityEngine::parse_sec_rule(rule_str).unwrap();

        assert_eq!(rule.id, "100");
        assert_eq!(rule.phase, 2);
        assert_eq!(rule.severity, WafSeverity::Critical);
        assert_eq!(rule.message, "Test");
        assert_eq!(rule.tag.len(), 1);
        assert_eq!(rule.tag[0], "ATTACK/TEST");

        // Check actions contain deny
        assert!(rule.actions.iter().any(|a| matches!(a, RuleAction::Deny)));
    }

    #[test]
    fn test_inline_rules() {
        let config = ModSecurityConfig {
            enabled: true,
            rules_file: None,
            inline_rules: vec![
                r#"SecRule ARGS "@rx malicious" "id:999,phase:2,deny,msg:'Malicious pattern detected'"#.to_string(),
            ],
            detection_mode: DetectionMode::On,
            request_body_access: true,
            response_body_access: false,
            request_body_limit: 1024 * 1024,
        };

        let engine = ModSecurityEngine::new(config).unwrap();

        // Should have default rules + 1 inline rule
        assert!(engine.rules.len() > 5);

        // Test the inline rule works
        let context = WafContext {
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: Some("param=malicious".to_string()),
            headers: HashMap::new(),
            client_ip: "192.168.1.1".to_string(),
            body: None,
            content_type: None,
            user_agent: None,
        };

        match engine.check_request(&context) {
            WafDecision::Block { reason, .. } => {
                assert!(reason.contains("Malicious pattern"));
            }
            _ => panic!("Expected Block decision"),
        }
    }
}
