//! Plugin trait definition and core interfaces

use super::types::*;
use super::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Result of a plugin filter execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterResult {
    /// Continue to the next plugin in the chain
    Continue,

    /// Pause execution and wait for async operation
    /// (Used for async HTTP calls, database queries, etc.)
    Pause,

    /// Stop the filter chain and return the current response
    /// (Used when plugin wants to short-circuit, e.g., auth failure)
    StopIteration,

    /// Error occurred during execution
    Error,
}

/// Plugin context with request/response data
pub type PluginContext = PluginExecutionContext;

/// Core plugin trait that all plugins must implement
///
/// This trait is implemented by both WASM and FFI plugin adapters.
/// Each plugin type (WASM/FFI) wraps the actual plugin implementation
/// and provides this unified interface.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin name
    fn name(&self) -> &str;

    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin
    ///
    /// Called once when the plugin is loaded.
    /// Use this to set up any required state, connections, etc.
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    /// Handle request headers phase
    ///
    /// Called when request headers are received but before body.
    /// Can modify headers, reject requests, etc.
    async fn on_request_headers(&self, ctx: &mut PluginContext) -> Result<FilterResult> {
        Ok(FilterResult::Continue)
    }

    /// Handle request body phase
    ///
    /// Called when the full request body is available.
    /// Can transform, validate, or reject the body.
    async fn on_request_body(&self, ctx: &mut PluginContext) -> Result<FilterResult> {
        Ok(FilterResult::Continue)
    }

    /// Handle response headers phase
    ///
    /// Called before sending response headers to client.
    /// Can add/modify headers, change status code, etc.
    async fn on_response_headers(&self, ctx: &mut PluginContext) -> Result<FilterResult> {
        Ok(FilterResult::Continue)
    }

    /// Handle response body phase
    ///
    /// Called before sending response body to client.
    /// Can transform or filter the response body.
    async fn on_response_body(&self, ctx: &mut PluginContext) -> Result<FilterResult> {
        Ok(FilterResult::Continue)
    }

    /// Execute plugin for a specific phase
    ///
    /// This is the main entry point that dispatches to the appropriate hook.
    async fn execute(&self, phase: PluginPhase, ctx: &mut PluginContext) -> Result<FilterResult> {
        match phase {
            PluginPhase::RequestHeaders => self.on_request_headers(ctx).await,
            PluginPhase::RequestBody => self.on_request_body(ctx).await,
            PluginPhase::ResponseHeaders => self.on_response_headers(ctx).await,
            PluginPhase::ResponseBody => self.on_response_body(ctx).await,
            PluginPhase::Init => {
                // Init is handled separately
                Ok(FilterResult::Continue)
            }
            PluginPhase::Destroy => {
                self.destroy().await;
                Ok(FilterResult::Continue)
            }
        }
    }

    /// Destroy the plugin
    ///
    /// Called when the plugin is being unloaded.
    /// Use this to clean up resources, close connections, etc.
    async fn destroy(&self) {
        // Default implementation does nothing
    }

    /// Get plugin statistics
    fn stats(&self) -> PluginStats {
        PluginStats::default()
    }

    /// Check if plugin is healthy
    fn is_healthy(&self) -> bool {
        true
    }

    /// Get number of active requests
    fn active_requests(&self) -> u64 {
        self.stats().active_requests
    }
}

/// Type alias for boxed plugin
pub type BoxedPlugin = Arc<dyn Plugin>;

/// Plugin factory trait for creating plugin instances
///
/// This is used by the plugin loader to instantiate plugins.
#[async_trait]
pub trait PluginFactory: Send + Sync {
    /// Create a new plugin instance
    async fn create(&self, config: &serde_json::Value) -> Result<BoxedPlugin>;

    /// Get factory name
    fn name(&self) -> &str;

    /// Get supported plugin type
    fn plugin_type(&self) -> PluginTypeInfo;
}

/// Host functions interface for WASM plugins
///
/// These functions are provided by the proxy host and can be called
/// from WASM plugins to interact with the proxy.
pub trait HostFunctions {
    /// Get request header value
    fn get_request_header(&self, key: &str) -> Option<String>;

    /// Set request header
    fn set_request_header(&mut self, key: String, value: String);

    /// Delete request header
    fn delete_request_header(&mut self, key: &str);

    /// Get request body
    fn get_request_body(&self) -> Option<Vec<u8>>;

    /// Set request body
    fn set_request_body(&mut self, body: Vec<u8>);

    /// Get response status code
    fn get_response_status(&self) -> u16;

    /// Set response status code
    fn set_response_status(&mut self, status: u16);

    /// Get response header value
    fn get_response_header(&self, key: &str) -> Option<String>;

    /// Set response header
    fn set_response_header(&mut self, key: String, value: String);

    /// Get plugin state
    fn get_state(&self, key: &str) -> Option<Vec<u8>>;

    /// Set plugin state
    fn set_state(&mut self, key: String, value: Vec<u8>);

    /// Log message
    fn log(&self, level: LogLevel, message: &str);

    /// Emit metric
    fn emit_metric(&self, name: &str, value: f64, metric_type: MetricType);

    /// Make HTTP call (for WASM plugins that need to call external services)
    fn http_call(&self, request: HttpCallRequest) -> Result<HttpCallResponse>;
}

/// Log level for plugin logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// HTTP call request (for plugins that need to call external services)
#[derive(Debug, Clone)]
pub struct HttpCallRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_ms: u64,
}

/// HTTP call response
#[derive(Debug, Clone)]
pub struct HttpCallResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Example plugin implementation for testing
#[derive(Debug, Clone)]
pub struct NoOpPlugin {
    name: String,
}

impl NoOpPlugin {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl Plugin for NoOpPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            author: Some("highper-gateway".to_string()),
            description: Some("No-op plugin for testing".to_string()),
            plugin_type: PluginTypeInfo::Ffi,
            loaded_at: std::time::SystemTime::now(),
        }
    }

    async fn init(&mut self) -> Result<()> {
        tracing::debug!("Initializing NoOpPlugin: {}", self.name);
        Ok(())
    }

    async fn on_request_headers(&self, _ctx: &mut PluginContext) -> Result<FilterResult> {
        Ok(FilterResult::Continue)
    }

    async fn destroy(&self) {
        tracing::debug!("Destroying NoOpPlugin: {}", self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_noop_plugin() {
        let mut plugin = NoOpPlugin::new("test");

        // Test init
        assert!(plugin.init().await.is_ok());

        // Test metadata
        let metadata = plugin.metadata();
        assert_eq!(metadata.name, "test");
        assert_eq!(metadata.version, "1.0.0");

        // Test execution
        let request = PluginRequest {
            method: "GET".to_string(),
            uri: "/test".to_string(),
            headers: HashMap::new(),
            body: None,
            metadata: HashMap::new(),
        };

        let mut ctx = PluginContext::new(request);
        let result = plugin.on_request_headers(&mut ctx).await.unwrap();
        assert_eq!(result, FilterResult::Continue);
    }

    #[test]
    fn test_filter_result() {
        assert_eq!(FilterResult::Continue, FilterResult::Continue);
        assert_ne!(FilterResult::Continue, FilterResult::StopIteration);
    }

    #[test]
    fn test_plugin_phase_display() {
        assert_eq!(PluginPhase::RequestHeaders.to_string(), "request_headers");
        assert_eq!(PluginPhase::RequestBody.to_string(), "request_body");
        assert_eq!(PluginPhase::ResponseHeaders.to_string(), "response_headers");
        assert_eq!(PluginPhase::ResponseBody.to_string(), "response_body");
    }
}
