//! Integration tests for the plugin system
//!
//! These tests verify that plugins can be loaded, executed, and unloaded correctly.

use bytes::Bytes;
use highper_gateway::plugin::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::test]
async fn test_plugin_manager_creation() {
    let config = config::PluginSystemConfig::default();
    let manager = PluginManager::new(config);

    assert_eq!(manager.registry().count(), 0);
}

#[tokio::test]
async fn test_plugin_registry_operations() {
    let registry = PluginRegistry::new();

    // Create a test plugin
    let plugin: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("test-plugin"));

    // Register plugin
    assert!(registry.register(plugin.clone(), 50).is_ok());
    assert_eq!(registry.count(), 1);
    assert_eq!(registry.enabled_count(), 1);
    assert!(registry.contains("test-plugin"));

    // Get plugin
    let retrieved = registry.get("test-plugin");
    assert!(retrieved.is_some());

    // Disable plugin
    assert!(registry.disable("test-plugin").is_ok());
    assert_eq!(registry.enabled_count(), 0);

    // Enable plugin
    assert!(registry.enable("test-plugin").is_ok());
    assert_eq!(registry.enabled_count(), 1);

    // Unregister plugin
    let removed = registry.unregister("test-plugin");
    assert!(removed.is_ok());
    assert_eq!(registry.count(), 0);
}

#[tokio::test]
async fn test_plugin_priority_ordering() {
    let registry = PluginRegistry::new();

    // Register plugins with different priorities
    let plugin1: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("low-priority"));
    let plugin2: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("high-priority"));
    let plugin3: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("medium-priority"));

    registry.register(plugin1, 10).unwrap();
    registry.register(plugin2, 100).unwrap();
    registry.register(plugin3, 50).unwrap();

    // Get enabled plugins (should be sorted by priority)
    let plugins = registry.get_enabled_plugins();
    assert_eq!(plugins.len(), 3);

    // Verify order: high (100), medium (50), low (10)
    assert_eq!(plugins[0].name(), "high-priority");
    assert_eq!(plugins[1].name(), "medium-priority");
    assert_eq!(plugins[2].name(), "low-priority");
}

#[tokio::test]
async fn test_plugin_execution_context() {
    let request = PluginRequest {
        method: "GET".to_string(),
        uri: "/api/test".to_string(),
        headers: HashMap::new(),
        body: None,
        metadata: HashMap::new(),
    };

    let mut ctx = PluginExecutionContext::new(request);

    // Test state operations
    ctx.set_state("key1".to_string(), Bytes::from("value1"));
    let value = ctx.get_state("key1");
    assert_eq!(value, Some(Bytes::from("value1")));

    // Test response setting
    let response = PluginResponse {
        status: 200,
        headers: HashMap::new(),
        body: None,
    };
    ctx.set_response(response);
    assert!(ctx.response.is_some());
    assert_eq!(ctx.response.as_ref().unwrap().status, 200);
}

#[tokio::test]
async fn test_plugin_stats_tracking() {
    let mut stats = PluginStats::default();

    // Record successful executions
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

    // Record failure
    stats.record_failure();
    assert_eq!(stats.requests_total, 3);
    assert_eq!(stats.requests_failed, 1);

    // Active request tracking
    stats.increment_active();
    stats.increment_active();
    assert_eq!(stats.active_requests, 2);

    stats.decrement_active();
    assert_eq!(stats.active_requests, 1);
}

#[tokio::test]
async fn test_noop_plugin_execution() {
    let mut plugin = trait_def::NoOpPlugin::new("test");

    // Test initialization
    assert!(plugin.init().await.is_ok());

    // Test metadata
    let metadata = plugin.metadata();
    assert_eq!(metadata.name, "test");
    assert_eq!(metadata.version, "1.0.0");

    // Create execution context
    let request = PluginRequest {
        method: "GET".to_string(),
        uri: "/test".to_string(),
        headers: HashMap::new(),
        body: None,
        metadata: HashMap::new(),
    };

    let mut ctx = PluginExecutionContext::new(request);

    // Test request headers phase
    let result = plugin.on_request_headers(&mut ctx).await.unwrap();
    assert_eq!(result, trait_def::FilterResult::Continue);

    // Test request body phase
    let result = plugin.on_request_body(&mut ctx).await.unwrap();
    assert_eq!(result, trait_def::FilterResult::Continue);

    // Test response headers phase
    let result = plugin.on_response_headers(&mut ctx).await.unwrap();
    assert_eq!(result, trait_def::FilterResult::Continue);

    // Test response body phase
    let result = plugin.on_response_body(&mut ctx).await.unwrap();
    assert_eq!(result, trait_def::FilterResult::Continue);

    // Test destroy
    plugin.destroy().await;
}

#[tokio::test]
async fn test_filter_result_variants() {
    use FilterResult;

    assert_eq!(FilterResult::Continue, FilterResult::Continue);
    assert_ne!(FilterResult::Continue, FilterResult::StopIteration);
    assert_ne!(FilterResult::Continue, FilterResult::Pause);
    assert_ne!(FilterResult::Continue, FilterResult::Error);
}

#[tokio::test]
async fn test_plugin_phase_display() {
    use PluginPhase;

    assert_eq!(PluginPhase::RequestHeaders.to_string(), "request_headers");
    assert_eq!(PluginPhase::RequestBody.to_string(), "request_body");
    assert_eq!(PluginPhase::ResponseHeaders.to_string(), "response_headers");
    assert_eq!(PluginPhase::ResponseBody.to_string(), "response_body");
    assert_eq!(PluginPhase::Init.to_string(), "init");
    assert_eq!(PluginPhase::Destroy.to_string(), "destroy");
}

#[tokio::test]
async fn test_plugin_config_defaults() {
    let limits = config::PluginLimits::default();

    assert_eq!(limits.memory_mb, 64);
    assert_eq!(limits.fuel, 1_000_000);
    assert_eq!(limits.timeout_ms, 100);
    assert_eq!(limits.max_concurrent_requests, 100);
}

#[tokio::test]
async fn test_plugin_capabilities_defaults() {
    let caps = PluginCapabilities::default();

    assert_eq!(caps.filesystem, false);
    assert_eq!(caps.network.allow_outbound.len(), 0);
    // Note: deny_private_ips defaults to false with Default trait, true only with serde default
    assert_eq!(caps.environment.len(), 0);
    assert_eq!(caps.random, false);
    // Note: clock defaults to false with Default trait, true only with serde default
}

#[tokio::test]
async fn test_plugin_metadata_creation() {
    let metadata = PluginMetadata {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        author: Some("Test Author".to_string()),
        description: Some("A test plugin".to_string()),
        plugin_type: PluginTypeInfo::Wasm,
        loaded_at: std::time::SystemTime::now(),
    };

    assert_eq!(metadata.name, "test-plugin");
    assert_eq!(metadata.version, "1.0.0");
    assert_eq!(metadata.plugin_type, PluginTypeInfo::Wasm);
}

#[cfg(feature = "plugin-wasm")]
#[tokio::test]
async fn test_wasm_loader_creation() {
    let loader = wasm::WasmPluginLoader::new();
    assert!(loader.is_ok());
}

#[cfg(feature = "plugin-ffi")]
#[tokio::test]
async fn test_ffi_loader_creation() {
    let _loader = ffi::FfiPluginLoader::new();
    // Just verify it creates without panic
}

#[tokio::test]
async fn test_hot_reload_monitor_creation() {
    let plugin_dir = PathBuf::from("./test-plugins");
    let registry = std::sync::Arc::new(PluginRegistry::new());

    let monitor = hot_reload::HotReloadMonitor::new(plugin_dir, registry);
    assert!(monitor.is_ok());
}

#[tokio::test]
async fn test_plugin_request_serialization() {
    let request = PluginRequest {
        method: "POST".to_string(),
        uri: "/api/users".to_string(),
        headers: {
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), "application/json".to_string());
            h
        },
        body: Some(Bytes::from(r#"{"name":"test"}"#)),
        metadata: HashMap::new(),
    };

    assert_eq!(request.method, "POST");
    assert_eq!(request.uri, "/api/users");
    assert_eq!(
        request.headers.get("content-type").unwrap(),
        "application/json"
    );
    assert!(request.body.is_some());
}

#[tokio::test]
async fn test_plugin_response_creation() {
    let response = PluginResponse {
        status: 201,
        headers: {
            let mut h = HashMap::new();
            h.insert("location".to_string(), "/api/users/123".to_string());
            h
        },
        body: Some(Bytes::from(r#"{"id":123}"#)),
    };

    assert_eq!(response.status, 201);
    assert_eq!(response.headers.get("location").unwrap(), "/api/users/123");
    assert!(response.body.is_some());
}

#[tokio::test]
async fn test_multiple_plugins_execution() {
    let registry = PluginRegistry::new();

    // Register multiple plugins
    for i in 1..=5 {
        let plugin: BoxedPlugin =
            std::sync::Arc::new(trait_def::NoOpPlugin::new(&format!("plugin-{}", i)));
        registry.register(plugin, i * 10).unwrap();
    }

    assert_eq!(registry.count(), 5);
    assert_eq!(registry.enabled_count(), 5);

    // Get sorted plugins
    let plugins = registry.get_enabled_plugins();
    assert_eq!(plugins.len(), 5);

    // Verify descending priority order
    assert_eq!(plugins[0].name(), "plugin-5"); // Priority 50
    assert_eq!(plugins[1].name(), "plugin-4"); // Priority 40
    assert_eq!(plugins[2].name(), "plugin-3"); // Priority 30
    assert_eq!(plugins[3].name(), "plugin-2"); // Priority 20
    assert_eq!(plugins[4].name(), "plugin-1"); // Priority 10
}

#[tokio::test]
async fn test_plugin_system_config_yaml() {
    let yaml = r#"
discovery:
  plugin_dir: ./plugins
  watch: true
  auto_load: true
  extensions:
    - wasm
    - so

default_limits:
  memory_mb: 128
  fuel: 2000000
  timeout_ms: 200
  max_concurrent_requests: 50

plugins:
  - name: test-plugin
    type: wasm
    path: ./plugins/test.wasm
    enabled: true
    priority: 100
"#;

    let config: PluginSystemConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.discovery.plugin_dir, PathBuf::from("./plugins"));
    assert_eq!(config.discovery.watch, true);
    assert_eq!(config.default_limits.memory_mb, 128);
    assert_eq!(config.plugins.len(), 1);
    assert_eq!(config.plugins[0].name, "test-plugin");
}

#[tokio::test]
async fn test_error_types() {
    use PluginError;

    let err = PluginError::NotFound("test".to_string());
    assert_eq!(err.to_string(), "Plugin not found: test");

    let err = PluginError::AlreadyLoaded("test".to_string());
    assert_eq!(err.to_string(), "Plugin already loaded: test");

    let err = PluginError::Timeout("test".to_string());
    assert_eq!(err.to_string(), "Plugin timeout: test");
}

#[tokio::test]
async fn test_registry_metadata_listing() {
    let registry = PluginRegistry::new();

    // Register plugins
    let plugin1: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("plugin1"));
    let plugin2: BoxedPlugin = std::sync::Arc::new(trait_def::NoOpPlugin::new("plugin2"));

    registry.register(plugin1, 50).unwrap();
    registry.register(plugin2, 60).unwrap();

    // List metadata
    let metadata_list = registry.list_metadata();
    assert_eq!(metadata_list.len(), 2);

    // List plugin names
    let names = registry.list_plugins();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"plugin1".to_string()));
    assert!(names.contains(&"plugin2".to_string()));
}

#[tokio::test]
async fn test_registry_clear() {
    let registry = PluginRegistry::new();

    // Register multiple plugins
    for i in 1..=3 {
        let plugin: BoxedPlugin =
            std::sync::Arc::new(trait_def::NoOpPlugin::new(&format!("plugin-{}", i)));
        registry.register(plugin, i * 10).unwrap();
    }

    assert_eq!(registry.count(), 3);

    // Clear all plugins
    registry.clear();
    assert_eq!(registry.count(), 0);
    assert_eq!(registry.enabled_count(), 0);
}
