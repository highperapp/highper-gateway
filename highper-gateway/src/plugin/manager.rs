//! Plugin manager - coordinates plugin loading, execution, and lifecycle

use super::config::*;
use super::registry::PluginRegistry;
use super::trait_def::{BoxedPlugin, FilterResult};
use super::types::*;
use super::{PluginError, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Plugin manager handles all plugin operations
pub struct PluginManager {
    /// Plugin registry
    registry: Arc<PluginRegistry>,

    /// Configuration
    config: PluginSystemConfig,

    /// WASM loader (initialized when needed)
    #[cfg(feature = "plugin-wasm")]
    wasm_loader: Option<Arc<super::wasm::WasmPluginLoader>>,

    /// FFI loader (initialized when needed)
    #[cfg(feature = "plugin-ffi")]
    ffi_loader: Option<Arc<super::ffi::FfiPluginLoader>>,

    /// Hot reload monitor
    #[cfg(feature = "plugin-hot-reload")]
    hot_reload: Option<Arc<super::hot_reload::HotReloadMonitor>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(config: PluginSystemConfig) -> Self {
        Self {
            registry: Arc::new(PluginRegistry::new()),
            config,
            #[cfg(feature = "plugin-wasm")]
            wasm_loader: None,
            #[cfg(feature = "plugin-ffi")]
            ffi_loader: None,
            #[cfg(feature = "plugin-hot-reload")]
            hot_reload: None,
        }
    }

    /// Initialize the plugin manager
    pub async fn init(&mut self) -> Result<()> {
        tracing::info!("Initializing plugin manager");

        // Initialize WASM loader
        #[cfg(feature = "plugin-wasm")]
        {
            self.wasm_loader = Some(Arc::new(super::wasm::WasmPluginLoader::new()?));
            tracing::info!("WASM plugin loader initialized");
        }

        // Initialize FFI loader
        #[cfg(feature = "plugin-ffi")]
        {
            self.ffi_loader = Some(Arc::new(super::ffi::FfiPluginLoader::new()));
            tracing::info!("FFI plugin loader initialized");
        }

        // Auto-load plugins if configured
        if self.config.discovery.auto_load {
            self.discover_and_load_plugins().await?;
        }

        // Start hot reload monitor if configured
        #[cfg(feature = "plugin-hot-reload")]
        if self.config.discovery.watch {
            self.start_hot_reload().await?;
        }

        Ok(())
    }

    /// Discover and load plugins from configured directory
    async fn discover_and_load_plugins(&mut self) -> Result<()> {
        let plugin_dir = &self.config.discovery.plugin_dir;

        if !plugin_dir.exists() {
            tracing::warn!("Plugin directory does not exist: {:?}", plugin_dir);
            std::fs::create_dir_all(plugin_dir).map_err(PluginError::Io)?;
            return Ok(());
        }

        tracing::info!("Discovering plugins in: {:?}", plugin_dir);

        for entry in std::fs::read_dir(plugin_dir).map_err(PluginError::Io)? {
            let entry = entry.map_err(PluginError::Io)?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Check if extension is allowed
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !self.config.discovery.extensions.contains(&ext.to_string()) {
                    continue;
                }

                // Try to load the plugin
                if let Err(e) = self.load_plugin_file(&path).await {
                    tracing::warn!("Failed to load plugin {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Load a plugin from a file
    async fn load_plugin_file(&mut self, path: &Path) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| PluginError::InvalidFormat("No file extension".to_string()))?;

        let plugin_type = match ext {
            "wasm" => PluginType::Wasm,
            "so" | "dylib" | "dll" => PluginType::Ffi,
            _ => {
                return Err(PluginError::InvalidFormat(format!(
                    "Unknown extension: {}",
                    ext
                )))
            }
        };

        // Extract plugin name from filename
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::InvalidFormat("Invalid filename".to_string()))?
            .to_string();

        // Create default config
        let config = PluginConfig {
            name: name.clone(),
            plugin_type,
            path: path.to_path_buf(),
            enabled: true,
            priority: 50,
            limits: Some(self.config.default_limits.clone()),
            capabilities: Some(PluginCapabilities::default()),
            config: serde_json::json!({}),
        };

        self.load_plugin(config).await
    }

    /// Load a plugin with configuration
    pub async fn load_plugin(&mut self, config: PluginConfig) -> Result<()> {
        if !config.enabled {
            tracing::debug!("Skipping disabled plugin: {}", config.name);
            return Ok(());
        }

        tracing::info!("Loading plugin: {} ({:?})", config.name, config.plugin_type);

        let plugin: BoxedPlugin = match config.plugin_type {
            #[cfg(feature = "plugin-wasm")]
            PluginType::Wasm => {
                let loader = self.wasm_loader.as_ref().ok_or_else(|| {
                    PluginError::Config("WASM loader not initialized".to_string())
                })?;
                loader.load(&config).await?
            }
            #[cfg(feature = "plugin-ffi")]
            PluginType::Ffi => {
                let loader = self
                    .ffi_loader
                    .as_ref()
                    .ok_or_else(|| PluginError::Config("FFI loader not initialized".to_string()))?;
                loader.load(&config).await?
            }
            #[cfg(not(feature = "plugin-wasm"))]
            PluginType::Wasm => {
                return Err(PluginError::Config(
                    "WASM plugin support not compiled in".to_string(),
                ))
            }
            #[cfg(not(feature = "plugin-ffi"))]
            PluginType::Ffi => {
                return Err(PluginError::Config(
                    "FFI plugin support not compiled in".to_string(),
                ))
            }
        };

        // Initialize the plugin
        // Note: We need a mutable reference, but plugin is Arc<dyn Plugin>
        // The init method is async and takes &mut self, which won't work with Arc
        // We'll need to handle this differently in the actual implementation

        self.registry.register(plugin, config.priority)?;

        tracing::info!("Successfully loaded plugin: {}", config.name);

        Ok(())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&mut self, name: &str) -> Result<()> {
        tracing::info!("Unloading plugin: {}", name);

        let plugin = self.registry.unregister(name)?;

        // Wait for active requests to complete
        self.wait_for_plugin_idle(&plugin).await;

        // Destroy the plugin
        plugin.destroy().await;

        tracing::info!("Successfully unloaded plugin: {}", name);

        Ok(())
    }

    /// Reload a plugin
    pub async fn reload_plugin(&mut self, name: &str) -> Result<()> {
        tracing::info!("Reloading plugin: {}", name);

        // Find the plugin config
        let config = self
            .config
            .plugins
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        // Unload existing plugin
        if self.registry.contains(name) {
            self.unload_plugin(name).await?;
        }

        // Load new version
        self.load_plugin(config).await?;

        tracing::info!("Successfully reloaded plugin: {}", name);

        Ok(())
    }

    /// Wait for a plugin to become idle (no active requests).
    ///
    /// Drain timeout and poll interval are loaded from `RuntimeConfig` per
    /// ROADMAP §0.1 env-var-only rule.
    async fn wait_for_plugin_idle(&self, plugin: &BoxedPlugin) {
        let cfg = crate::runtime_config::current();
        let timeout = *cfg.plugin.drain.get();
        let poll_interval = *cfg.plugin.idle_poll.get();
        let start = Instant::now();

        while plugin.active_requests() > 0 {
            if start.elapsed() > timeout {
                tracing::warn!(
                    "Plugin {} still has {} active requests after {:?}, forcing unload",
                    plugin.name(),
                    plugin.active_requests(),
                    timeout,
                );
                break;
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Execute a plugin phase for all enabled plugins
    pub async fn execute_phase(
        &self,
        phase: PluginPhase,
        ctx: &mut PluginExecutionContext,
    ) -> Result<FilterResult> {
        let plugins = self.registry.get_enabled_plugins();

        for plugin in plugins {
            let start = Instant::now();
            let plugin_name = plugin.name().to_string();

            // Increment active requests
            self.registry.update_stats(&plugin_name, |stats| {
                stats.increment_active();
            });

            let result = plugin.execute(phase, ctx).await;

            // Record execution time
            let execution_time_us = start.elapsed().as_micros() as u64;

            // Decrement active requests and update stats
            self.registry.update_stats(&plugin_name, |stats| {
                stats.decrement_active();
                match result {
                    Ok(FilterResult::Continue)
                    | Ok(FilterResult::Pause)
                    | Ok(FilterResult::StopIteration) => {
                        stats.record_success(execution_time_us);
                    }
                    Ok(FilterResult::Error) | Err(_) => {
                        stats.record_failure();
                    }
                }
            });

            match result {
                Ok(FilterResult::Continue) => continue,
                Ok(FilterResult::StopIteration) => return Ok(FilterResult::StopIteration),
                Ok(FilterResult::Pause) => return Ok(FilterResult::Pause),
                Ok(FilterResult::Error) | Err(_) => {
                    tracing::error!("Plugin {} failed during phase {}", plugin_name, phase);
                    // Continue to next plugin on error (or could stop here depending on policy)
                    continue;
                }
            }
        }

        Ok(FilterResult::Continue)
    }

    /// Get plugin registry
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Start hot reload monitoring
    #[cfg(feature = "plugin-hot-reload")]
    async fn start_hot_reload(&mut self) -> Result<()> {
        let mut monitor = super::hot_reload::HotReloadMonitor::new(
            self.config.discovery.plugin_dir.clone(),
            Arc::clone(&self.registry),
        )?;

        monitor.start().await?;

        self.hot_reload = Some(Arc::new(monitor));

        tracing::info!("Hot reload monitor started");

        Ok(())
    }

    /// Get plugin statistics
    pub fn get_stats(&self, name: &str) -> Option<PluginStats> {
        self.registry.get_stats(name)
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.registry.list_metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_creation() {
        let config = PluginSystemConfig::default();
        let _manager = PluginManager::new(config);
    }
}
