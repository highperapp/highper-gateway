//! WASM plugin loader using wasmtime
//!
//! This module provides a WASM runtime integration for loading and executing
//! WebAssembly plugins with WASI support.

use super::config::PluginConfig;
use super::trait_def::BoxedPlugin;
use super::{PluginError, Result};

#[cfg(feature = "plugin-wasm")]
use wasmtime::*;

/// WASM plugin loader
pub struct WasmPluginLoader {
    #[cfg(feature = "plugin-wasm")]
    engine: Engine,
}

impl WasmPluginLoader {
    /// Create a new WASM plugin loader
    pub fn new() -> Result<Self> {
        #[cfg(feature = "plugin-wasm")]
        {
            let mut config = Config::new();
            config.async_support(true);
            config.consume_fuel(true); // Enable fuel for resource limiting
            config.cranelift_opt_level(OptLevel::Speed);
            config.epoch_interruption(true); // For timeout support

            let engine = Engine::new(&config)
                .map_err(|e| PluginError::LoadError(format!("Failed to create WASM engine: {}", e)))?;

            Ok(Self { engine })
        }

        #[cfg(not(feature = "plugin-wasm"))]
        {
            Err(PluginError::Config("WASM support not compiled in".to_string()))
        }
    }

    /// Load a WASM plugin from file
    pub async fn load(&self, config: &PluginConfig) -> Result<BoxedPlugin> {
        #[cfg(feature = "plugin-wasm")]
        {
            tracing::info!("Loading WASM plugin: {} from {:?}", config.name, config.path);

            // Read WASM file
            let wasm_bytes = std::fs::read(&config.path)
                .map_err(|e| PluginError::LoadError(format!("Failed to read WASM file: {}", e)))?;

            // Compile module
            let module = Module::new(&self.engine, &wasm_bytes)
                .map_err(|e| PluginError::LoadError(format!("Failed to compile WASM module: {}", e)))?;

            // Get limits
            let limits = config.limits.as_ref()
                .cloned()
                .unwrap_or_else(PluginLimits::default);

            // Create plugin instance
            let plugin = WasmPlugin::new(
                config.name.clone(),
                self.engine.clone(),
                module,
                limits,
                config.capabilities.clone(),
            )?;

            Ok(Arc::new(plugin))
        }

        #[cfg(not(feature = "plugin-wasm"))]
        {
            let _ = config;
            Err(PluginError::Config("WASM support not compiled in".to_string()))
        }
    }
}

/// WASM plugin adapter that implements the Plugin trait
#[cfg(feature = "plugin-wasm")]
struct WasmPlugin {
    name: String,
    metadata: PluginMetadata,
    engine: Engine,
    module: Module,
    limits: PluginLimits,
    capabilities: Option<super::config::PluginCapabilities>,
    stats: Arc<parking_lot::RwLock<PluginStats>>,
}

#[cfg(feature = "plugin-wasm")]
impl WasmPlugin {
    fn new(
        name: String,
        engine: Engine,
        module: Module,
        limits: PluginLimits,
        capabilities: Option<super::config::PluginCapabilities>,
    ) -> Result<Self> {
        let metadata = PluginMetadata {
            name: name.clone(),
            version: "1.0.0".to_string(), // TODO: Extract from WASM custom section
            author: None,
            description: None,
            plugin_type: PluginTypeInfo::Wasm,
            loaded_at: std::time::SystemTime::now(),
        };

        Ok(Self {
            name,
            metadata,
            engine,
            module,
            limits,
            capabilities,
            stats: Arc::new(parking_lot::RwLock::new(PluginStats::default())),
        })
    }


    /// Execute a plugin function
    async fn call_plugin_function(
        &self,
        func_name: &str,
        ctx: &mut PluginExecutionContext,
    ) -> Result<FilterResult> {
        // Create host state with cloned context
        let host_state = super::host_functions::HostState::new(ctx.clone());
        let mut store = Store::new(&self.engine, host_state);

        // Set resource limits
        store.set_fuel(self.limits.fuel)
            .map_err(|e| PluginError::Runtime(format!("Failed to set fuel: {}", e)))?;
        store.set_epoch_deadline(1);

        // Create linker
        let mut linker = Linker::new(&self.engine);

        // Add host functions (proxy API)
        super::host_functions::add_host_functions(&mut linker)?;

        // Instantiate module
        let instance = linker.instantiate_async(&mut store, &self.module).await
            .map_err(|e| PluginError::Runtime(format!("Failed to instantiate module: {}", e)))?;

        // Look up the function
        let func = instance
            .get_typed_func::<(), i32>(&mut store, func_name)
            .map_err(|e| PluginError::Runtime(format!("Function '{}' not found: {}", func_name, e)))?;

        // Call the function
        let result = func.call_async(&mut store, ()).await
            .map_err(|e| PluginError::Runtime(format!("Function call failed: {}", e)))?;

        // Copy modifications back to original context
        let final_state = store.data().context.read().clone();
        *ctx = final_state;

        // Convert result to FilterResult
        match result {
            0 => Ok(FilterResult::Continue),
            1 => Ok(FilterResult::StopIteration),
            2 => Ok(FilterResult::Pause),
            _ => Ok(FilterResult::Error),
        }
    }
}

#[cfg(feature = "plugin-wasm")]
#[async_trait]
impl Plugin for WasmPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    async fn init(&mut self) -> Result<()> {
        tracing::debug!("Initializing WASM plugin: {}", self.name);
        // Plugin initialization happens during instantiation
        Ok(())
    }

    async fn on_request_headers(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        self.call_plugin_function("on_request_headers", ctx).await
    }

    async fn on_request_body(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        self.call_plugin_function("on_request_body", ctx).await
    }

    async fn on_response_headers(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        self.call_plugin_function("on_response_headers", ctx).await
    }

    async fn on_response_body(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        self.call_plugin_function("on_response_body", ctx).await
    }

    async fn destroy(&self) {
        tracing::debug!("Destroying WASM plugin: {}", self.name);
        // Resources are automatically cleaned up when store is dropped
    }

    fn stats(&self) -> PluginStats {
        self.stats.read().clone()
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn active_requests(&self) -> u64 {
        self.stats.read().active_requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "plugin-wasm")]
    fn test_wasm_loader_creation() {
        let loader = WasmPluginLoader::new();
        assert!(loader.is_ok());
    }
}
