//! Example FFI plugin for highper-gateway
//!
//! This demonstrates how to write a high-performance native plugin using the FFI/C ABI.
//! FFI plugins have zero-copy access and maximum performance but require trust.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use serde::{Deserialize, Serialize};

/// Plugin return codes
const CONTINUE: i32 = 0;
const STOP: i32 = 1;
const PAUSE: i32 = 2;
const _ERROR: i32 = 3;

/// FFI plugin context (matches highper-gateway's FfiPluginContext)
#[repr(C)]
pub struct PluginContext {
    request_ptr: *const c_char,
    request_len: usize,
    response_ptr: *const c_char,
    response_len: usize,
}

/// Request structure (matches FfiRequest in highper-gateway)
#[derive(Debug, Serialize, Deserialize)]
struct Request {
    method: String,
    uri: String,
    headers: std::collections::HashMap<String, String>,
}

/// Response structure (matches FfiResponse in highper-gateway)
#[derive(Debug, Serialize, Deserialize)]
struct Response {
    status: u16,
    headers: std::collections::HashMap<String, String>,
}

/// Plugin state (stored in thread-local for this example)
struct PluginState {
    request_count: usize,
    config: serde_json::Value,
}

static mut PLUGIN_STATE: Option<PluginState> = None;

/// Plugin VTable structure (must match highper-gateway's FfiPluginVTable)
#[repr(C)]
pub struct PluginVTable {
    name: extern "C" fn() -> *const c_char,
    init: extern "C" fn(*const c_char) -> i32,
    on_request_headers: extern "C" fn(*mut PluginContext) -> i32,
    on_request_body: extern "C" fn(*mut PluginContext) -> i32,
    on_response_headers: extern "C" fn(*mut PluginContext) -> i32,
    on_response_body: extern "C" fn(*mut PluginContext) -> i32,
    destroy: extern "C" fn(),
}

/// Static VTable instance
static VTABLE: PluginVTable = PluginVTable {
    name: plugin_name,
    init: plugin_init,
    on_request_headers: on_request_headers_impl,
    on_request_body: on_request_body_impl,
    on_response_headers: on_response_headers_impl,
    on_response_body: on_response_body_impl,
    destroy: plugin_destroy,
};

/// Plugin creation function - called by highper-gateway to get the VTable
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut PluginVTable {
    &VTABLE as *const PluginVTable as *mut PluginVTable
}

/// Get plugin name
extern "C" fn plugin_name() -> *const c_char {
    static NAME: &[u8] = b"ffi-hello-plugin\0";
    NAME.as_ptr() as *const c_char
}

/// Initialize plugin with configuration
extern "C" fn plugin_init(config_ptr: *const c_char) -> i32 {
    if config_ptr.is_null() {
        return -1;
    }

    unsafe {
        let config_str = match CStr::from_ptr(config_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let config: serde_json::Value = match serde_json::from_str(config_str) {
            Ok(c) => c,
            Err(_) => serde_json::json!({}),
        };

        PLUGIN_STATE = Some(PluginState {
            request_count: 0,
            config,
        });
    }

    eprintln!("[FFI Plugin] Initialized");
    0 // Success
}

/// Helper to parse request from context
fn parse_request(ctx: &PluginContext) -> Option<Request> {
    if ctx.request_ptr.is_null() || ctx.request_len == 0 {
        return None;
    }

    unsafe {
        let slice = std::slice::from_raw_parts(ctx.request_ptr as *const u8, ctx.request_len);
        let json_str = std::str::from_utf8(slice).ok()?;
        serde_json::from_str(json_str).ok()
    }
}

/// Helper to parse response from context
fn parse_response(ctx: &PluginContext) -> Option<Response> {
    if ctx.response_ptr.is_null() || ctx.response_len == 0 {
        return None;
    }

    unsafe {
        let slice = std::slice::from_raw_parts(ctx.response_ptr as *const u8, ctx.response_len);
        let json_str = std::str::from_utf8(slice).ok()?;
        serde_json::from_str(json_str).ok()
    }
}

/// Handle request headers
extern "C" fn on_request_headers_impl(ctx: *mut PluginContext) -> i32 {
    if ctx.is_null() {
        return -1;
    }

    unsafe {
        let context = &*ctx;

        if let Some(request) = parse_request(context) {
            eprintln!("[FFI Plugin] Request headers: {} {}", request.method, request.uri);

            // Increment request count
            if let Some(state) = &mut PLUGIN_STATE {
                state.request_count += 1;
                eprintln!("[FFI Plugin] Total requests: {}", state.request_count);
            }

            // In a real plugin, you could modify headers here
            // For FFI plugins, modifications would need to be communicated back
            // through a shared memory structure or callback mechanism
        }
    }

    CONTINUE
}

/// Handle request body
extern "C" fn on_request_body_impl(_ctx: *mut PluginContext) -> i32 {
    eprintln!("[FFI Plugin] Request body phase");
    CONTINUE
}

/// Handle response headers
extern "C" fn on_response_headers_impl(ctx: *mut PluginContext) -> i32 {
    if ctx.is_null() {
        return -1;
    }

    unsafe {
        let context = &*ctx;

        if let Some(response) = parse_response(context) {
            eprintln!("[FFI Plugin] Response status: {}", response.status);
        }
    }

    CONTINUE
}

/// Handle response body
extern "C" fn on_response_body_impl(_ctx: *mut PluginContext) -> i32 {
    eprintln!("[FFI Plugin] Response body phase");
    CONTINUE
}

/// Cleanup plugin
extern "C" fn plugin_destroy() {
    unsafe {
        if let Some(state) = &PLUGIN_STATE {
            eprintln!("[FFI Plugin] Shutting down - processed {} requests", state.request_count);
        }
        PLUGIN_STATE = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let vtable = plugin_create();
        assert!(!vtable.is_null());
    }

    #[test]
    fn test_plugin_name() {
        let name_ptr = plugin_name();
        assert!(!name_ptr.is_null());

        unsafe {
            let name = CStr::from_ptr(name_ptr).to_str().unwrap();
            assert_eq!(name, "ffi-hello-plugin");
        }
    }

    #[test]
    fn test_plugin_init() {
        let config = CString::new(r#"{"enabled": true}"#).unwrap();
        let result = plugin_init(config.as_ptr());
        assert_eq!(result, 0);
    }
}
