//! Example WASM plugin that adds a custom header
//!
//! This plugin demonstrates the basic structure of a WASM plugin for highper-gateway.
//! It adds an "X-Hello-From" header to all requests and logs messages.

/// Plugin return codes
const CONTINUE: i32 = 0;
const STOP: i32 = 1;
const PAUSE: i32 = 2;
const _ERROR: i32 = 3;

/// Log levels
const _LOG_TRACE: i32 = 0;
const _LOG_DEBUG: i32 = 1;
const LOG_INFO: i32 = 2;
const _LOG_WARN: i32 = 3;
const _LOG_ERROR: i32 = 4;

// Host functions imported from highper-gateway
#[link(wasm_import_module = "proxy")]
extern "C" {
    /// Get request header value
    fn get_request_header(
        key_ptr: i32,
        key_len: i32,
        value_ptr: i32,
        value_cap: i32,
    ) -> i32;

    /// Set request header
    fn set_request_header(
        key_ptr: i32,
        key_len: i32,
        value_ptr: i32,
        value_len: i32,
    ) -> i32;

    /// Set response header
    fn set_response_header(
        key_ptr: i32,
        key_len: i32,
        value_ptr: i32,
        value_len: i32,
    ) -> i32;

    /// Log a message
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;
}

/// Helper to log a message
fn log_info(msg: &str) {
    unsafe {
        log(LOG_INFO, msg.as_ptr() as i32, msg.len() as i32);
    }
}

/// Helper to set request header
fn set_req_header(key: &str, value: &str) {
    unsafe {
        set_request_header(
            key.as_ptr() as i32,
            key.len() as i32,
            value.as_ptr() as i32,
            value.len() as i32,
        );
    }
}

/// Helper to set response header
fn set_resp_header(key: &str, value: &str) {
    unsafe {
        set_response_header(
            key.as_ptr() as i32,
            key.len() as i32,
            value.as_ptr() as i32,
            value.len() as i32,
        );
    }
}

/// Handle request headers phase
///
/// Called when request headers are received but before body.
/// Can modify headers, reject requests, etc.
#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    log_info("Hello from WASM plugin - request headers phase");

    // Add custom header to request
    set_req_header("X-Hello-From", "WASM Plugin");
    set_req_header("X-Plugin-Version", "1.0.0");

    CONTINUE
}

/// Handle request body phase
///
/// Called when the full request body is available.
#[no_mangle]
pub extern "C" fn on_request_body() -> i32 {
    log_info("Hello from WASM plugin - request body phase");
    CONTINUE
}

/// Handle response headers phase
///
/// Called before sending response headers to client.
#[no_mangle]
pub extern "C" fn on_response_headers() -> i32 {
    log_info("Hello from WASM plugin - response headers phase");

    // Add custom response header
    set_resp_header("X-Processed-By", "highper-gateway WASM");
    set_resp_header("X-Plugin-Name", "wasm-hello-plugin");

    CONTINUE
}

/// Handle response body phase
///
/// Called before sending response body to client.
#[no_mangle]
pub extern "C" fn on_response_body() -> i32 {
    log_info("Hello from WASM plugin - response body phase");
    CONTINUE
}
