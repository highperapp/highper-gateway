//! Host functions that WASM plugins can call
//!
//! This module defines the functions that the proxy host provides to WASM plugins,
//! allowing them to interact with requests, responses, and the proxy environment.

use super::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

#[cfg(feature = "plugin-wasm")]
use wasmtime::*;

/// Host function state shared with WASM instance
#[derive(Clone)]
pub struct HostState {
    /// Plugin execution context (shared with WASM)
    pub context: Arc<RwLock<PluginExecutionContext>>,

    /// Shared memory for string/data passing
    pub shared_memory: Arc<RwLock<Vec<u8>>>,
}

impl HostState {
    pub fn new(context: PluginExecutionContext) -> Self {
        Self {
            context: Arc::new(RwLock::new(context)),
            shared_memory: Arc::new(RwLock::new(Vec::with_capacity(4096))),
        }
    }
}

/// Add host functions to the linker
#[cfg(feature = "plugin-wasm")]
pub fn add_host_functions(linker: &mut Linker<HostState>) -> Result<()> {
    // Request header functions
    linker
        .func_wrap(
            "proxy",
            "get_request_header",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_cap: i32|
             -> i32 {
                get_request_header_impl(&mut caller, key_ptr, key_len, value_ptr, value_cap)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add get_request_header: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "set_request_header",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_len: i32|
             -> i32 {
                set_request_header_impl(&mut caller, key_ptr, key_len, value_ptr, value_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add set_request_header: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "delete_request_header",
            |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
                delete_request_header_impl(&mut caller, key_ptr, key_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add delete_request_header: {}", e)))?;

    // Response header functions
    linker
        .func_wrap(
            "proxy",
            "get_response_header",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_cap: i32|
             -> i32 {
                get_response_header_impl(&mut caller, key_ptr, key_len, value_ptr, value_cap)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add get_response_header: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "set_response_header",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_len: i32|
             -> i32 {
                set_response_header_impl(&mut caller, key_ptr, key_len, value_ptr, value_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add set_response_header: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "set_response_status",
            |mut caller: Caller<'_, HostState>, status: i32| -> i32 {
                set_response_status_impl(&mut caller, status)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add set_response_status: {}", e)))?;

    // Body functions
    linker
        .func_wrap(
            "proxy",
            "get_request_body_size",
            |caller: Caller<'_, HostState>| -> i32 { get_request_body_size_impl(&caller) },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add get_request_body_size: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "get_request_body",
            |mut caller: Caller<'_, HostState>, buf_ptr: i32, buf_len: i32| -> i32 {
                get_request_body_impl(&mut caller, buf_ptr, buf_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add get_request_body: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "set_request_body",
            |mut caller: Caller<'_, HostState>, data_ptr: i32, data_len: i32| -> i32 {
                set_request_body_impl(&mut caller, data_ptr, data_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add set_request_body: {}", e)))?;

    // State functions
    linker
        .func_wrap(
            "proxy",
            "get_state",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_cap: i32|
             -> i32 {
                get_state_impl(&mut caller, key_ptr, key_len, value_ptr, value_cap)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add get_state: {}", e)))?;

    linker
        .func_wrap(
            "proxy",
            "set_state",
            |mut caller: Caller<'_, HostState>,
             key_ptr: i32,
             key_len: i32,
             value_ptr: i32,
             value_len: i32|
             -> i32 {
                set_state_impl(&mut caller, key_ptr, key_len, value_ptr, value_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add set_state: {}", e)))?;

    // Logging
    linker
        .func_wrap(
            "proxy",
            "log",
            |mut caller: Caller<'_, HostState>, level: i32, msg_ptr: i32, msg_len: i32| -> i32 {
                log_impl(&mut caller, level, msg_ptr, msg_len)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add log: {}", e)))?;

    // Metrics
    linker
        .func_wrap(
            "proxy",
            "emit_metric",
            |mut caller: Caller<'_, HostState>,
             name_ptr: i32,
             name_len: i32,
             value: f64,
             metric_type: i32|
             -> i32 {
                emit_metric_impl(&mut caller, name_ptr, name_len, value, metric_type)
            },
        )
        .map_err(|e| PluginError::Runtime(format!("Failed to add emit_metric: {}", e)))?;

    Ok(())
}

// Helper function to read string from WASM memory
#[cfg(feature = "plugin-wasm")]
fn read_string(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Result<String> {
    if ptr < 0 || len < 0 {
        return Err(PluginError::Runtime(
            "Invalid pointer or length".to_string(),
        ));
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| PluginError::Runtime("Failed to get memory export".to_string()))?;

    let mut buf = vec![0u8; len as usize];
    memory
        .read(&caller, ptr as usize, &mut buf)
        .map_err(|e| PluginError::Runtime(format!("Failed to read memory: {}", e)))?;

    String::from_utf8(buf).map_err(|e| PluginError::Runtime(format!("Invalid UTF-8: {}", e)))
}

// Helper function to write string to WASM memory
#[cfg(feature = "plugin-wasm")]
fn write_string(caller: &mut Caller<'_, HostState>, data: &str, ptr: i32, cap: i32) -> Result<i32> {
    if ptr < 0 || cap < 0 {
        return Err(PluginError::Runtime(
            "Invalid pointer or capacity".to_string(),
        ));
    }

    let bytes = data.as_bytes();
    let write_len = bytes.len().min(cap as usize);

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| PluginError::Runtime("Failed to get memory export".to_string()))?;

    memory
        .write(&mut *caller, ptr as usize, &bytes[..write_len])
        .map_err(|e| PluginError::Runtime(format!("Failed to write memory: {}", e)))?;

    Ok(write_len as i32)
}

// Helper function to write bytes to WASM memory
#[cfg(feature = "plugin-wasm")]
fn write_bytes(caller: &mut Caller<'_, HostState>, data: &[u8], ptr: i32, cap: i32) -> Result<i32> {
    if ptr < 0 || cap < 0 {
        return Err(PluginError::Runtime(
            "Invalid pointer or capacity".to_string(),
        ));
    }

    let write_len = data.len().min(cap as usize);

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| PluginError::Runtime("Failed to get memory export".to_string()))?;

    memory
        .write(&mut *caller, ptr as usize, &data[..write_len])
        .map_err(|e| PluginError::Runtime(format!("Failed to write memory: {}", e)))?;

    Ok(write_len as i32)
}

// Host function implementations

#[cfg(feature = "plugin-wasm")]
fn get_request_header_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_cap: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    // Extract value before calling write_string to avoid borrow conflicts
    let value_opt = {
        let state = caller.data();
        let ctx = state.context.read();
        ctx.request.headers.get(&key).cloned()
    };

    if let Some(value) = value_opt {
        match write_string(caller, &value, value_ptr, value_cap) {
            Ok(len) => len,
            Err(_) => -1,
        }
    } else {
        0 // Header not found
    }
}

#[cfg(feature = "plugin-wasm")]
fn set_request_header_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_len: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    let value = match read_string(caller, value_ptr, value_len) {
        Ok(v) => v,
        Err(_) => return -1,
    };

    let state = caller.data();
    let mut ctx = state.context.write();
    ctx.request.headers.insert(key, value);

    0 // Success
}

#[cfg(feature = "plugin-wasm")]
fn delete_request_header_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    let state = caller.data();
    let mut ctx = state.context.write();
    ctx.request.headers.remove(&key);

    0 // Success
}

#[cfg(feature = "plugin-wasm")]
fn get_response_header_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_cap: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    // Extract value before calling write_string to avoid borrow conflicts
    let value_opt = {
        let state = caller.data();
        let ctx = state.context.read();
        ctx.response
            .as_ref()
            .and_then(|r| r.headers.get(&key))
            .cloned()
    };

    if let Some(value) = value_opt {
        match write_string(caller, &value, value_ptr, value_cap) {
            Ok(len) => len,
            Err(_) => -1,
        }
    } else {
        0 // Header not found or no response yet
    }
}

#[cfg(feature = "plugin-wasm")]
fn set_response_header_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_len: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    let value = match read_string(caller, value_ptr, value_len) {
        Ok(v) => v,
        Err(_) => return -1,
    };

    let state = caller.data();
    let mut ctx = state.context.write();

    if let Some(response) = &mut ctx.response {
        response.headers.insert(key, value);
        0 // Success
    } else {
        -1 // No response yet
    }
}

#[cfg(feature = "plugin-wasm")]
fn set_response_status_impl(caller: &mut Caller<'_, HostState>, status: i32) -> i32 {
    if status < 100 || status > 599 {
        return -1; // Invalid status code
    }

    let state = caller.data();
    let mut ctx = state.context.write();

    if let Some(response) = &mut ctx.response {
        response.status = status as u16;
        0 // Success
    } else {
        -1 // No response yet
    }
}

#[cfg(feature = "plugin-wasm")]
fn get_request_body_size_impl(caller: &Caller<'_, HostState>) -> i32 {
    let state = caller.data();
    let ctx = state.context.read();

    ctx.request
        .body
        .as_ref()
        .map(|b| b.len() as i32)
        .unwrap_or(0)
}

#[cfg(feature = "plugin-wasm")]
fn get_request_body_impl(caller: &mut Caller<'_, HostState>, buf_ptr: i32, buf_len: i32) -> i32 {
    // Extract body before calling write_bytes to avoid borrow conflicts
    let body_opt = {
        let state = caller.data();
        let ctx = state.context.read();
        ctx.request.body.clone()
    };

    if let Some(body) = body_opt {
        match write_bytes(caller, &body, buf_ptr, buf_len) {
            Ok(len) => len,
            Err(_) => -1,
        }
    } else {
        0 // No body
    }
}

#[cfg(feature = "plugin-wasm")]
fn set_request_body_impl(caller: &mut Caller<'_, HostState>, data_ptr: i32, data_len: i32) -> i32 {
    if data_ptr < 0 || data_len < 0 {
        return -1;
    }

    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return -1,
    };

    let mut buf = vec![0u8; data_len as usize];
    if memory.read(&caller, data_ptr as usize, &mut buf).is_err() {
        return -1;
    }

    let state = caller.data();
    let mut ctx = state.context.write();
    ctx.request.body = Some(Bytes::from(buf));

    0 // Success
}

#[cfg(feature = "plugin-wasm")]
fn get_state_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_cap: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    // Extract value before calling write_bytes to avoid borrow conflicts
    let value_opt = {
        let state = caller.data();
        let ctx = state.context.read();
        ctx.get_state(&key)
    };

    if let Some(value) = value_opt {
        match write_bytes(caller, &value, value_ptr, value_cap) {
            Ok(len) => len,
            Err(_) => -1,
        }
    } else {
        0 // State not found
    }
}

#[cfg(feature = "plugin-wasm")]
fn set_state_impl(
    caller: &mut Caller<'_, HostState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_len: i32,
) -> i32 {
    let key = match read_string(caller, key_ptr, key_len) {
        Ok(k) => k,
        Err(_) => return -1,
    };

    if value_ptr < 0 || value_len < 0 {
        return -1;
    }

    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return -1,
    };

    let mut buf = vec![0u8; value_len as usize];
    if memory.read(&caller, value_ptr as usize, &mut buf).is_err() {
        return -1;
    }

    let state = caller.data();
    let mut ctx = state.context.write();
    ctx.set_state(key, Bytes::from(buf));

    0 // Success
}

#[cfg(feature = "plugin-wasm")]
fn log_impl(caller: &mut Caller<'_, HostState>, level: i32, msg_ptr: i32, msg_len: i32) -> i32 {
    let msg = match read_string(caller, msg_ptr, msg_len) {
        Ok(m) => m,
        Err(_) => return -1,
    };

    match level {
        0 => tracing::trace!("[WASM Plugin] {}", msg),
        1 => tracing::debug!("[WASM Plugin] {}", msg),
        2 => tracing::info!("[WASM Plugin] {}", msg),
        3 => tracing::warn!("[WASM Plugin] {}", msg),
        4 => tracing::error!("[WASM Plugin] {}", msg),
        _ => tracing::info!("[WASM Plugin] {}", msg),
    }

    0 // Success
}

#[cfg(feature = "plugin-wasm")]
fn emit_metric_impl(
    caller: &mut Caller<'_, HostState>,
    name_ptr: i32,
    name_len: i32,
    value: f64,
    metric_type: i32,
) -> i32 {
    let name = match read_string(caller, name_ptr, name_len) {
        Ok(n) => n,
        Err(_) => return -1,
    };

    // TODO: Integrate with metrics system
    tracing::debug!(
        "[WASM Plugin] Metric: {} = {} (type: {})",
        name,
        value,
        metric_type
    );

    0 // Success
}
