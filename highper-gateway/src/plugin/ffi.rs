//! FFI plugin loader for native Rust/C/C++ plugins
//!
//! This module provides dynamic library loading for high-performance
//! FFI plugins using the C ABI.

use super::config::PluginConfig;
use super::trait_def::BoxedPlugin;
use super::{PluginError, Result};
use std::os::raw::c_char;

#[cfg(feature = "plugin-ffi")]
use libloading::{Library, Symbol};

/// FFI plugin loader
pub struct FfiPluginLoader {
    // Loaded libraries need to be kept alive
    #[cfg(feature = "plugin-ffi")]
    libraries: Arc<parking_lot::RwLock<Vec<Arc<Library>>>>,
}

impl FfiPluginLoader {
    /// Create a new FFI plugin loader
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "plugin-ffi")]
            libraries: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    /// Load an FFI plugin from dynamic library
    pub async fn load(&self, config: &PluginConfig) -> Result<BoxedPlugin> {
        #[cfg(feature = "plugin-ffi")]
        {
            tracing::info!("Loading FFI plugin: {} from {:?}", config.name, config.path);

            // Load dynamic library
            let library = unsafe {
                Library::new(&config.path)
                    .map_err(|e| PluginError::LoadError(format!("Failed to load library: {}", e)))?
            };

            let library = Arc::new(library);

            // Look up plugin creation function
            let create_plugin: Symbol<unsafe extern "C" fn() -> *mut FfiPluginVTable> = unsafe {
                library.get(b"plugin_create\0")
                    .map_err(|e| PluginError::LoadError(format!("plugin_create not found: {}", e)))?
            };

            // Call creation function
            let vtable_ptr = unsafe { create_plugin() };
            if vtable_ptr.is_null() {
                return Err(PluginError::LoadError("plugin_create returned null".to_string()));
            }

            // Convert to reference (unsafe but necessary for FFI)
            let vtable = unsafe { &*vtable_ptr };

            // Get plugin name
            let name_ptr = (vtable.name)();
            let name = unsafe {
                if name_ptr.is_null() {
                    config.name.clone()
                } else {
                    CStr::from_ptr(name_ptr)
                        .to_str()
                        .unwrap_or(&config.name)
                        .to_string()
                }
            };

            // Initialize plugin
            let config_json = serde_json::to_string(&config.config)
                .map_err(|e| PluginError::Config(format!("Failed to serialize config: {}", e)))?;

            let config_cstring = CString::new(config_json)
                .map_err(|e| PluginError::Config(format!("Invalid config string: {}", e)))?;

            let init_result = (vtable.init)(config_cstring.as_ptr());
            if init_result != 0 {
                return Err(PluginError::InitFailed(format!("Plugin init failed with code: {}", init_result)));
            }

            // Create metadata
            let metadata = PluginMetadata {
                name: name.clone(),
                version: "1.0.0".to_string(), // TODO: Get from plugin
                author: None,
                description: None,
                plugin_type: PluginTypeInfo::Ffi,
                loaded_at: std::time::SystemTime::now(),
            };

            // Store library to keep it alive
            self.libraries.write().push(Arc::clone(&library));

            // Create plugin adapter
            let plugin = FfiPlugin {
                name,
                metadata,
                vtable: vtable_ptr,
                _library: library,
                stats: Arc::new(parking_lot::RwLock::new(PluginStats::default())),
            };

            tracing::info!("Successfully loaded FFI plugin: {}", plugin.name);

            Ok(Arc::new(plugin))
        }

        #[cfg(not(feature = "plugin-ffi"))]
        {
            let _ = config;
            Err(PluginError::Config("FFI support not compiled in".to_string()))
        }
    }
}

/// FFI plugin interface (C ABI)
///
/// This is the C interface that FFI plugins must implement.
/// Plugin authors will implement these functions in their .so/.dylib/.dll
#[repr(C)]
pub struct FfiPluginVTable {
    /// Get plugin name
    pub name: extern "C" fn() -> *const std::os::raw::c_char,

    /// Initialize plugin
    pub init: extern "C" fn(config: *const std::os::raw::c_char) -> i32,

    /// Handle request headers phase
    pub on_request_headers: extern "C" fn(
        ctx: *mut FfiPluginContext,
    ) -> i32,

    /// Handle request body phase
    pub on_request_body: extern "C" fn(
        ctx: *mut FfiPluginContext,
    ) -> i32,

    /// Handle response headers phase
    pub on_response_headers: extern "C" fn(
        ctx: *mut FfiPluginContext,
    ) -> i32,

    /// Handle response body phase
    pub on_response_body: extern "C" fn(
        ctx: *mut FfiPluginContext,
    ) -> i32,

    /// Destroy plugin
    pub destroy: extern "C" fn(),
}

/// FFI plugin context (passed across FFI boundary)
///
/// This is a simplified, C-compatible version of PluginExecutionContext
/// that can be safely passed across the FFI boundary.
#[repr(C)]
pub struct FfiPluginContext {
    /// Pointer to request JSON string
    pub request_ptr: *const c_char,
    /// Length of request JSON string
    pub request_len: usize,
    /// Pointer to response JSON string (null if no response yet)
    pub response_ptr: *const c_char,
    /// Length of response JSON string
    pub response_len: usize,
}

/// FFI plugin adapter that implements the Plugin trait
#[cfg(feature = "plugin-ffi")]
struct FfiPlugin {
    name: String,
    metadata: PluginMetadata,
    vtable: *mut FfiPluginVTable,
    _library: Arc<Library>, // Keep library alive
    stats: Arc<parking_lot::RwLock<PluginStats>>,
}

#[cfg(feature = "plugin-ffi")]
unsafe impl Send for FfiPlugin {}
#[cfg(feature = "plugin-ffi")]
unsafe impl Sync for FfiPlugin {}

#[cfg(feature = "plugin-ffi")]
impl FfiPlugin {
    /// Convert execution context to FFI context
    fn to_ffi_context(ctx: &mut PluginExecutionContext) -> FfiPluginContext {
        // Serialize context to JSON for simplicity
        // In production, this could be optimized with a more efficient format
        let request_json = serde_json::to_string(&FfiRequest {
            method: ctx.request.method.clone(),
            uri: ctx.request.uri.clone(),
            headers: ctx.request.headers.clone(),
        }).unwrap_or_default();

        let response_json = ctx.response.as_ref().map(|r| {
            serde_json::to_string(&FfiResponse {
                status: r.status,
                headers: r.headers.clone(),
            }).unwrap_or_default()
        }).unwrap_or_default();

        FfiPluginContext {
            request_ptr: request_json.as_ptr() as *const c_char,
            request_len: request_json.len(),
            response_ptr: if response_json.is_empty() {
                std::ptr::null()
            } else {
                response_json.as_ptr() as *const c_char
            },
            response_len: response_json.len(),
        }
    }

    /// Call FFI function and convert result
    async fn call_ffi_function(
        &self,
        func: unsafe extern "C" fn(*mut FfiPluginContext) -> i32,
        ctx: &mut PluginExecutionContext,
    ) -> Result<FilterResult> {
        let mut ffi_ctx = Self::to_ffi_context(ctx);

        let result = unsafe { func(&mut ffi_ctx as *mut FfiPluginContext) };

        // Convert result code to FilterResult
        match result {
            0 => Ok(FilterResult::Continue),
            1 => Ok(FilterResult::StopIteration),
            2 => Ok(FilterResult::Pause),
            _ => Ok(FilterResult::Error),
        }
    }
}

// Serializable types for FFI boundary
#[derive(serde::Serialize, serde::Deserialize)]
struct FfiRequest {
    method: String,
    uri: String,
    headers: std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FfiResponse {
    status: u16,
    headers: std::collections::HashMap<String, String>,
}

#[cfg(feature = "plugin-ffi")]
#[async_trait]
impl Plugin for FfiPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    async fn init(&mut self) -> Result<()> {
        // Init already called in load()
        tracing::debug!("FFI plugin initialized: {}", self.name);
        Ok(())
    }

    async fn on_request_headers(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        let vtable = unsafe { &*self.vtable };
        self.call_ffi_function(vtable.on_request_headers, ctx).await
    }

    async fn on_request_body(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        let vtable = unsafe { &*self.vtable };
        self.call_ffi_function(vtable.on_request_body, ctx).await
    }

    async fn on_response_headers(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        let vtable = unsafe { &*self.vtable };
        self.call_ffi_function(vtable.on_response_headers, ctx).await
    }

    async fn on_response_body(&self, ctx: &mut PluginExecutionContext) -> Result<FilterResult> {
        let vtable = unsafe { &*self.vtable };
        self.call_ffi_function(vtable.on_response_body, ctx).await
    }

    async fn destroy(&self) {
        tracing::debug!("Destroying FFI plugin: {}", self.name);
        let vtable = unsafe { &*self.vtable };
        (vtable.destroy)();
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
    fn test_ffi_loader_creation() {
        let _loader = FfiPluginLoader::new();
    }
}
