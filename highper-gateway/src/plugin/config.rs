//! Plugin configuration structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Plugin type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    /// WebAssembly plugin (safe, sandboxed)
    Wasm,
    /// FFI plugin (high-performance, trusted)
    Ffi,
}

impl Default for PluginType {
    fn default() -> Self {
        PluginType::Wasm // Default to safe option
    }
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name (unique identifier)
    pub name: String,

    /// Plugin type (wasm or ffi)
    #[serde(rename = "type")]
    pub plugin_type: PluginType,

    /// Path to plugin file (.wasm, .so, .dylib, .dll)
    pub path: PathBuf,

    /// Whether plugin is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Plugin priority (higher = executed earlier)
    /// Default: 50
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Resource limits (for WASM plugins)
    #[serde(default)]
    pub limits: Option<PluginLimits>,

    /// Capabilities (for WASM plugins)
    #[serde(default)]
    pub capabilities: Option<PluginCapabilities>,

    /// Plugin-specific configuration (JSON)
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> u32 {
    50
}

/// Resource limits for WASM plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLimits {
    /// Maximum memory in megabytes
    #[serde(default = "default_memory_mb")]
    pub memory_mb: usize,

    /// Maximum fuel (instructions) per execution
    /// Prevents infinite loops
    #[serde(default = "default_fuel")]
    pub fuel: u64,

    /// Maximum execution time in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,
}

fn default_memory_mb() -> usize {
    64
}

fn default_fuel() -> u64 {
    1_000_000
}

fn default_timeout_ms() -> u64 {
    100
}

fn default_max_concurrent() -> usize {
    100
}

impl Default for PluginLimits {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            fuel: default_fuel(),
            timeout_ms: default_timeout_ms(),
            max_concurrent_requests: default_max_concurrent(),
        }
    }
}

/// Plugin capabilities (WASI permissions)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    /// Allow filesystem access
    #[serde(default)]
    pub filesystem: bool,

    /// Network access configuration
    #[serde(default)]
    pub network: NetworkCapabilities,

    /// Environment variable access
    #[serde(default)]
    pub environment: Vec<String>,

    /// Allow random number generation
    #[serde(default)]
    pub random: bool,

    /// Allow clock access
    #[serde(default = "default_true")]
    pub clock: bool,
}

/// Network capabilities for WASM plugins
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkCapabilities {
    /// Allow outbound HTTP calls
    #[serde(default)]
    pub allow_outbound: Vec<String>, // Domain patterns: ["*.api.internal", "example.com"]

    /// Deny private IP addresses
    #[serde(default = "default_true")]
    pub deny_private_ips: bool,

    /// Maximum outbound request timeout (ms)
    #[serde(default = "default_network_timeout")]
    pub timeout_ms: u64,
}

fn default_network_timeout() -> u64 {
    5000 // 5 seconds
}

/// Plugin discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDiscoveryConfig {
    /// Directory to scan for plugins
    pub plugin_dir: PathBuf,

    /// Watch for file changes and hot-reload
    #[serde(default = "default_true")]
    pub watch: bool,

    /// Auto-load plugins on startup
    #[serde(default = "default_true")]
    pub auto_load: bool,

    /// Allowed plugin file extensions
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
}

fn default_extensions() -> Vec<String> {
    vec![
        "wasm".to_string(),
        "so".to_string(),
        "dylib".to_string(),
        "dll".to_string(),
    ]
}

impl Default for PluginDiscoveryConfig {
    fn default() -> Self {
        Self {
            plugin_dir: PathBuf::from("plugins"),
            watch: true,
            auto_load: true,
            extensions: default_extensions(),
        }
    }
}

/// Main plugin system configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginSystemConfig {
    /// Plugin discovery configuration
    #[serde(default)]
    pub discovery: PluginDiscoveryConfig,

    /// Individual plugin configurations
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,

    /// Global default limits for WASM plugins
    #[serde(default)]
    pub default_limits: PluginLimits,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_defaults() {
        let config = PluginConfig {
            name: "test".to_string(),
            plugin_type: PluginType::Wasm,
            path: PathBuf::from("test.wasm"),
            enabled: default_true(),
            priority: default_priority(),
            limits: Some(PluginLimits::default()),
            capabilities: Some(PluginCapabilities::default()),
            config: serde_json::json!({}),
        };

        assert_eq!(config.name, "test");
        assert_eq!(config.plugin_type, PluginType::Wasm);
        assert_eq!(config.enabled, true);
        assert_eq!(config.priority, 50);
    }

    #[test]
    fn test_plugin_limits_defaults() {
        let limits = PluginLimits::default();

        assert_eq!(limits.memory_mb, 64);
        assert_eq!(limits.fuel, 1_000_000);
        assert_eq!(limits.timeout_ms, 100);
        assert_eq!(limits.max_concurrent_requests, 100);
    }

    #[test]
    fn test_plugin_config_serde() {
        let yaml = r#"
name: auth_plugin
type: wasm
path: plugins/auth.wasm
enabled: true
priority: 100
limits:
  memory_mb: 128
  fuel: 2000000
  timeout_ms: 200
  max_concurrent_requests: 50
capabilities:
  filesystem: false
  network:
    allow_outbound:
      - "*.api.internal"
    deny_private_ips: true
  environment: []
config:
  api_key_header: "X-API-Key"
"#;

        let config: PluginConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.name, "auth_plugin");
        assert_eq!(config.plugin_type, PluginType::Wasm);
        assert_eq!(config.priority, 100);
        assert!(config.limits.is_some());

        let limits = config.limits.unwrap();
        assert_eq!(limits.memory_mb, 128);
        assert_eq!(limits.fuel, 2_000_000);
    }

    #[test]
    fn test_plugin_system_config() {
        let config = PluginSystemConfig::default();

        assert_eq!(config.discovery.plugin_dir, PathBuf::from("plugins"));
        assert_eq!(config.discovery.watch, true);
        assert_eq!(config.discovery.auto_load, true);
        assert_eq!(config.default_limits.memory_mb, 64);
    }
}
