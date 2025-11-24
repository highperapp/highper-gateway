# Plugin Host Functions Reference

This document describes all host functions available to WASM plugins in highper-gateway.

## Overview

Host functions are provided by the highper-gateway host and can be called from WASM plugins to interact with HTTP requests, responses, logging, metrics, and plugin state.

All host functions are available under the `proxy` module:

```rust
#[link(wasm_import_module = "proxy")]
extern "C" {
    fn function_name(...) -> i32;
}
```

## Return Values

Most functions return `i32`:
- **Positive value or 0**: Success (often the number of bytes written)
- **-1**: Error

## Request Header Functions

### `get_request_header`

Get a request header value.

```c
int32_t get_request_header(
    int32_t key_ptr,      // Pointer to header name
    int32_t key_len,      // Length of header name
    int32_t value_ptr,    // Pointer to output buffer
    int32_t value_cap     // Capacity of output buffer
);
```

**Returns**:
- Number of bytes written to `value_ptr` on success
- `0` if header not found
- `-1` on error

**Example (Rust)**:
```rust
let mut value_buf = [0u8; 256];
let len = unsafe {
    get_request_header(
        "user-agent".as_ptr() as i32,
        "user-agent".len() as i32,
        value_buf.as_mut_ptr() as i32,
        value_buf.len() as i32,
    )
};
if len > 0 {
    let user_agent = std::str::from_utf8(&value_buf[..len as usize]).unwrap();
}
```

### `set_request_header`

Set or update a request header.

```c
int32_t set_request_header(
    int32_t key_ptr,      // Pointer to header name
    int32_t key_len,      // Length of header name
    int32_t value_ptr,    // Pointer to header value
    int32_t value_len     // Length of header value
);
```

**Returns**: `0` on success, `-1` on error

**Example (Rust)**:
```rust
unsafe {
    set_request_header(
        "X-Custom-Header".as_ptr() as i32,
        "X-Custom-Header".len() as i32,
        "my-value".as_ptr() as i32,
        "my-value".len() as i32,
    );
}
```

### `delete_request_header`

Delete a request header.

```c
int32_t delete_request_header(
    int32_t key_ptr,      // Pointer to header name
    int32_t key_len       // Length of header name
);
```

**Returns**: `0` on success, `-1` on error

## Response Header Functions

### `get_response_header`

Get a response header value (only available in response phases).

```c
int32_t get_response_header(
    int32_t key_ptr,      // Pointer to header name
    int32_t key_len,      // Length of header name
    int32_t value_ptr,    // Pointer to output buffer
    int32_t value_cap     // Capacity of output buffer
);
```

**Returns**:
- Number of bytes written on success
- `0` if header not found or no response yet
- `-1` on error

### `set_response_header`

Set or update a response header.

```c
int32_t set_response_header(
    int32_t key_ptr,      // Pointer to header name
    int32_t key_len,      // Length of header name
    int32_t value_ptr,    // Pointer to header value
    int32_t value_len     // Length of header value
);
```

**Returns**: `0` on success, `-1` on error or if no response yet

### `set_response_status`

Set the HTTP response status code.

```c
int32_t set_response_status(
    int32_t status        // HTTP status code (100-599)
);
```

**Returns**: `0` on success, `-1` on error or invalid status

**Example (Rust)**:
```rust
// Return 403 Forbidden
unsafe {
    set_response_status(403);
}
```

## Request Body Functions

### `get_request_body_size`

Get the size of the request body in bytes.

```c
int32_t get_request_body_size();
```

**Returns**: Body size in bytes, or `0` if no body

### `get_request_body`

Read the request body into a buffer.

```c
int32_t get_request_body(
    int32_t buf_ptr,      // Pointer to output buffer
    int32_t buf_len       // Capacity of output buffer
);
```

**Returns**:
- Number of bytes written on success
- `0` if no body
- `-1` on error

**Example (Rust)**:
```rust
let size = unsafe { get_request_body_size() };
if size > 0 {
    let mut body = vec![0u8; size as usize];
    let read = unsafe {
        get_request_body(body.as_mut_ptr() as i32, body.len() as i32)
    };
    // Process body...
}
```

### `set_request_body`

Replace the request body with new data.

```c
int32_t set_request_body(
    int32_t data_ptr,     // Pointer to new body data
    int32_t data_len      // Length of new body
);
```

**Returns**: `0` on success, `-1` on error

**Example (Rust)**:
```rust
let new_body = b"transformed data";
unsafe {
    set_request_body(new_body.as_ptr() as i32, new_body.len() as i32);
}
```

## State Functions

Plugin state is shared across all invocations of the same plugin for a single request.

### `get_state`

Get a value from plugin state.

```c
int32_t get_state(
    int32_t key_ptr,      // Pointer to state key
    int32_t key_len,      // Length of state key
    int32_t value_ptr,    // Pointer to output buffer
    int32_t value_cap     // Capacity of output buffer
);
```

**Returns**:
- Number of bytes written on success
- `0` if key not found
- `-1` on error

### `set_state`

Set a value in plugin state.

```c
int32_t set_state(
    int32_t key_ptr,      // Pointer to state key
    int32_t key_len,      // Length of state key
    int32_t value_ptr,    // Pointer to value data
    int32_t value_len     // Length of value
);
```

**Returns**: `0` on success, `-1` on error

**Example (Rust)**:
```rust
// In request_headers phase
unsafe {
    set_state(
        "request_time".as_ptr() as i32,
        "request_time".len() as i32,
        timestamp_bytes.as_ptr() as i32,
        timestamp_bytes.len() as i32,
    );
}

// In response_headers phase
let mut buf = [0u8; 16];
let len = unsafe {
    get_state(
        "request_time".as_ptr() as i32,
        "request_time".len() as i32,
        buf.as_mut_ptr() as i32,
        buf.len() as i32,
    )
};
```

## Logging Functions

### `log`

Log a message at a specified level.

```c
int32_t log(
    int32_t level,        // Log level (0-4)
    int32_t msg_ptr,      // Pointer to message string
    int32_t msg_len       // Length of message
);
```

**Log Levels**:
- `0` - TRACE
- `1` - DEBUG
- `2` - INFO
- `3` - WARN
- `4` - ERROR

**Returns**: `0` on success, `-1` on error

**Example (Rust)**:
```rust
const LOG_INFO: i32 = 2;

fn log_info(msg: &str) {
    unsafe {
        log(LOG_INFO, msg.as_ptr() as i32, msg.len() as i32);
    }
}

log_info("Processing request");
```

## Metrics Functions

### `emit_metric`

Emit a metric value.

```c
int32_t emit_metric(
    int32_t name_ptr,     // Pointer to metric name
    int32_t name_len,     // Length of metric name
    double value,         // Metric value
    int32_t metric_type   // Metric type (0-2)
);
```

**Metric Types**:
- `0` - Counter (monotonically increasing)
- `1` - Gauge (can go up or down)
- `2` - Histogram (distribution of values)

**Returns**: `0` on success, `-1` on error

**Example (Rust)**:
```rust
const METRIC_COUNTER: i32 = 0;

unsafe {
    emit_metric(
        "plugin.requests".as_ptr() as i32,
        "plugin.requests".len() as i32,
        1.0,
        METRIC_COUNTER,
    );
}
```

## Error Handling

All host functions use `i32` return codes:

```rust
let result = unsafe { some_host_function(...) };
match result {
    r if r >= 0 => {
        // Success - r may contain bytes written or other data
    },
    -1 => {
        // Error occurred
        log_error("Host function failed");
    },
    _ => {
        // Unexpected return value
    }
}
```

## Memory Management

### String and Buffer Passing

When passing strings or buffers to/from host functions:

1. **Input strings**: Pass pointer and length
   ```rust
   let s = "hello";
   func(s.as_ptr() as i32, s.len() as i32)
   ```

2. **Output buffers**: Pre-allocate and pass pointer and capacity
   ```rust
   let mut buf = vec![0u8; 256];
   let len = func(buf.as_mut_ptr() as i32, buf.len() as i32);
   let data = &buf[..len as usize];
   ```

### Pointer Safety

- All pointers must be valid WASM linear memory addresses
- Buffers must not exceed WASM memory limits
- The host validates all pointer accesses

## Resource Limits

WASM plugins are subject to resource limits:

- **Memory**: Maximum heap size (default: 64MB)
- **Fuel**: Instruction count limit (default: 1M instructions)
- **Timeout**: Maximum execution time (default: 100ms)
- **Concurrency**: Maximum concurrent requests (default: 100)

If limits are exceeded, plugin execution is terminated.

## Best Practices

1. **Check return values**: Always check host function return values
2. **Buffer sizing**: Use appropriately sized buffers for header values
3. **State cleanup**: Clean up plugin state when no longer needed
4. **Logging**: Use appropriate log levels (avoid excessive DEBUG/TRACE in production)
5. **Error handling**: Log errors but don't crash on host function failures
6. **Performance**: Minimize host function calls in hot paths

## Example: Complete Plugin

```rust
#[link(wasm_import_module = "proxy")]
extern "C" {
    fn set_request_header(k_ptr: i32, k_len: i32, v_ptr: i32, v_len: i32) -> i32;
    fn set_response_header(k_ptr: i32, k_len: i32, v_ptr: i32, v_len: i32) -> i32;
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;
    fn emit_metric(name_ptr: i32, name_len: i32, value: f64, mtype: i32) -> i32;
}

const CONTINUE: i32 = 0;
const LOG_INFO: i32 = 2;
const METRIC_COUNTER: i32 = 0;

fn log_info(msg: &str) {
    unsafe { log(LOG_INFO, msg.as_ptr() as i32, msg.len() as i32); }
}

fn set_header(key: &str, value: &str) {
    unsafe {
        set_request_header(
            key.as_ptr() as i32, key.len() as i32,
            value.as_ptr() as i32, value.len() as i32,
        );
    }
}

fn increment_metric(name: &str) {
    unsafe {
        emit_metric(
            name.as_ptr() as i32, name.len() as i32,
            1.0, METRIC_COUNTER,
        );
    }
}

#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    log_info("Processing request");
    set_header("X-Plugin-Version", "1.0.0");
    increment_metric("plugin.requests");
    CONTINUE
}

#[no_mangle]
pub extern "C" fn on_response_headers() -> i32 {
    log_info("Processing response");
    unsafe {
        set_response_header(
            "X-Processed-By".as_ptr() as i32,
            "X-Processed-By".len() as i32,
            "highper-gateway".as_ptr() as i32,
            "highper-gateway".len() as i32,
        );
    }
    CONTINUE
}
```

## See Also

- [Plugin System Design](../PLUGIN_SYSTEM_DESIGN.md)
- [Plugin Implementation Status](../PLUGIN_SYSTEM_IMPLEMENTATION_STATUS.md)
- [WASM Plugin Example](../examples/plugins/wasm-hello-rust/)
