//! Core types for the plugin system

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Plugin metadata
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier)
    pub name: String,

    /// Plugin version
    pub version: String,

    /// Plugin author
    pub author: Option<String>,

    /// Plugin description
    pub description: Option<String>,

    /// Plugin type (WASM or FFI)
    pub plugin_type: PluginTypeInfo,

    /// When the plugin was loaded
    pub loaded_at: std::time::SystemTime,
}

/// Plugin type information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginTypeInfo {
    /// WebAssembly plugin
    Wasm,
    /// FFI (native) plugin
    Ffi,
}

/// Plugin statistics
#[derive(Debug, Clone, Default)]
pub struct PluginStats {
    /// Total number of requests processed
    pub requests_total: u64,

    /// Number of successful executions
    pub requests_success: u64,

    /// Number of failed executions
    pub requests_failed: u64,

    /// Number of timeouts
    pub requests_timeout: u64,

    /// Total execution time (microseconds)
    pub total_execution_time_us: u64,

    /// Average execution time (microseconds)
    pub avg_execution_time_us: u64,

    /// Peak execution time (microseconds)
    pub peak_execution_time_us: u64,

    /// Current active requests
    pub active_requests: u64,

    /// Total memory allocated (bytes)
    pub memory_allocated: usize,

    /// Peak memory usage (bytes)
    pub peak_memory_usage: usize,
}

impl PluginStats {
    /// Record a successful request execution
    pub fn record_success(&mut self, execution_time_us: u64) {
        self.requests_total += 1;
        self.requests_success += 1;
        self.total_execution_time_us += execution_time_us;

        // Update average
        if self.requests_total > 0 {
            self.avg_execution_time_us = self.total_execution_time_us / self.requests_total;
        }

        // Update peak
        if execution_time_us > self.peak_execution_time_us {
            self.peak_execution_time_us = execution_time_us;
        }
    }

    /// Record a failed request execution
    pub fn record_failure(&mut self) {
        self.requests_total += 1;
        self.requests_failed += 1;
    }

    /// Record a timeout
    pub fn record_timeout(&mut self) {
        self.requests_total += 1;
        self.requests_timeout += 1;
    }

    /// Increment active requests
    pub fn increment_active(&mut self) {
        self.active_requests += 1;
    }

    /// Decrement active requests
    pub fn decrement_active(&mut self) {
        if self.active_requests > 0 {
            self.active_requests -= 1;
        }
    }
}

/// HTTP request data passed to plugins
#[derive(Debug, Clone)]
pub struct PluginRequest {
    /// HTTP method
    pub method: String,

    /// Request URI
    pub uri: String,

    /// Request headers
    pub headers: HashMap<String, String>,

    /// Request body
    pub body: Option<Bytes>,

    /// Request metadata (route, upstream, etc.)
    pub metadata: HashMap<String, String>,
}

/// HTTP response data from plugins
#[derive(Debug, Clone)]
pub struct PluginResponse {
    /// HTTP status code
    pub status: u16,

    /// Response headers
    pub headers: HashMap<String, String>,

    /// Response body
    pub body: Option<Bytes>,
}

/// Plugin state storage
pub type PluginState = Arc<dashmap::DashMap<String, Bytes>>;

/// Plugin execution context
#[derive(Clone)]
pub struct PluginExecutionContext {
    /// Request data
    pub request: PluginRequest,

    /// Response data (for response phase hooks)
    pub response: Option<PluginResponse>,

    /// Plugin-specific state storage
    pub state: PluginState,

    /// Execution timeout
    pub timeout: Duration,

    /// Maximum memory limit
    pub memory_limit: usize,

    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
}

impl PluginExecutionContext {
    /// Create a new execution context
    pub fn new(request: PluginRequest) -> Self {
        Self {
            request,
            response: None,
            state: Arc::new(dashmap::DashMap::new()),
            timeout: Duration::from_millis(100),
            memory_limit: 64 * 1024 * 1024, // 64 MB default
            trace_id: None,
        }
    }

    /// Set response data
    pub fn set_response(&mut self, response: PluginResponse) {
        self.response = Some(response);
    }

    /// Get plugin state value
    pub fn get_state(&self, key: &str) -> Option<Bytes> {
        self.state.get(key).map(|v| v.clone())
    }

    /// Set plugin state value
    pub fn set_state(&self, key: String, value: Bytes) {
        self.state.insert(key, value);
    }
}

/// Plugin hook phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginPhase {
    /// Called when request headers are received
    RequestHeaders,

    /// Called when request body is available
    RequestBody,

    /// Called before sending response headers
    ResponseHeaders,

    /// Called before sending response body
    ResponseBody,

    /// Called on plugin initialization
    Init,

    /// Called on plugin shutdown
    Destroy,
}

impl std::fmt::Display for PluginPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginPhase::RequestHeaders => write!(f, "request_headers"),
            PluginPhase::RequestBody => write!(f, "request_body"),
            PluginPhase::ResponseHeaders => write!(f, "response_headers"),
            PluginPhase::ResponseBody => write!(f, "response_body"),
            PluginPhase::Init => write!(f, "init"),
            PluginPhase::Destroy => write!(f, "destroy"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_stats_record_success() {
        let mut stats = PluginStats::default();

        stats.record_success(100);
        assert_eq!(stats.requests_total, 1);
        assert_eq!(stats.requests_success, 1);
        assert_eq!(stats.avg_execution_time_us, 100);
        assert_eq!(stats.peak_execution_time_us, 100);

        stats.record_success(200);
        assert_eq!(stats.requests_total, 2);
        assert_eq!(stats.requests_success, 2);
        assert_eq!(stats.avg_execution_time_us, 150);
        assert_eq!(stats.peak_execution_time_us, 200);
    }

    #[test]
    fn test_plugin_stats_record_failure() {
        let mut stats = PluginStats::default();

        stats.record_failure();
        assert_eq!(stats.requests_total, 1);
        assert_eq!(stats.requests_failed, 1);
    }

    #[test]
    fn test_plugin_stats_active_requests() {
        let mut stats = PluginStats::default();

        stats.increment_active();
        assert_eq!(stats.active_requests, 1);

        stats.increment_active();
        assert_eq!(stats.active_requests, 2);

        stats.decrement_active();
        assert_eq!(stats.active_requests, 1);
    }

    #[test]
    fn test_execution_context_state() {
        let request = PluginRequest {
            method: "GET".to_string(),
            uri: "/test".to_string(),
            headers: HashMap::new(),
            body: None,
            metadata: HashMap::new(),
        };

        let ctx = PluginExecutionContext::new(request);

        // Set and get state
        ctx.set_state("key1".to_string(), Bytes::from("value1"));
        let value = ctx.get_state("key1");
        assert_eq!(value, Some(Bytes::from("value1")));

        // Non-existent key
        let value = ctx.get_state("nonexistent");
        assert_eq!(value, None);
    }
}
