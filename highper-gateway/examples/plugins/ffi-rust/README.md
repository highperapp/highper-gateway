# FFI Plugin Example (Rust)

This is an example FFI plugin written in Rust for highper-gateway. FFI plugins offer maximum performance with zero-copy access but require trust.

## When to Use FFI Plugins

**Use FFI plugins when:**
- You need maximum performance (sub-microsecond overhead)
- You need zero-copy access to request/response data
- The plugin is written in Rust, C, or C++
- The code is trusted (part of your infrastructure)
- You need access to native libraries

**Use WASM plugins instead when:**
- The plugin is from a third party
- You need strong isolation/sandboxing
- You want multi-language support (JS, Python, Go, etc.)
- Performance overhead of ~10-50μs is acceptable

## Performance Comparison

| Feature | WASM Plugin | FFI Plugin |
|---------|-------------|------------|
| Overhead | ~10-50μs | ~1-5μs |
| Memory | Isolated (64MB limit) | Shared (no limit) |
| Safety | Sandboxed | No sandbox |
| Languages | Any (Rust, JS, Python, Go, etc.) | Rust, C, C++ only |
| Use case | Untrusted code | Trusted code |

## Building

```bash
# Build for release (optimized)
cargo build --release

# The compiled plugin will be at:
# target/release/libffi_hello_plugin.so     (Linux)
# target/release/libffi_hello_plugin.dylib  (macOS)
# target/release/ffi_hello_plugin.dll       (Windows)
```

## Configuration

Add this to your highper-gateway configuration:

```yaml
plugins:
  discovery:
    plugin_dir: ./plugins
    auto_load: true

  plugins:
    - name: ffi-hello
      type: ffi
      path: ./plugins/libffi_hello_plugin.so
      enabled: true
      priority: 90  # Higher priority = executes earlier
      config:
        custom_setting: "value"
```

## Plugin Interface

FFI plugins must export a `plugin_create` function that returns a VTable:

```rust
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut PluginVTable;
```

The VTable contains function pointers for all plugin hooks:

```rust
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
```

## Context Structure

The plugin context is passed to each hook:

```rust
#[repr(C)]
pub struct PluginContext {
    request_ptr: *const c_char,
    request_len: usize,
    response_ptr: *const c_char,
    response_len: usize,
}
```

Request and response data is serialized as JSON:

```json
// Request
{
  "method": "GET",
  "uri": "/api/users",
  "headers": {
    "user-agent": "curl/7.68.0",
    "accept": "*/*"
  }
}

// Response
{
  "status": 200,
  "headers": {
    "content-type": "application/json",
    "x-custom-header": "value"
  }
}
```

## Return Values

All hook functions return `i32`:
- `0` - Continue to next plugin
- `1` - Stop iteration and return response
- `2` - Pause for async operation
- `3` - Error occurred

## Safety Considerations

FFI plugins run **without sandboxing**:

⚠️ **WARNING**: FFI plugins have full access to:
- All process memory
- File system
- Network
- System calls

**Security recommendations:**
1. Only load FFI plugins from trusted sources
2. Review all FFI plugin code before deployment
3. Use code signing for plugin binaries
4. Isolate plugin directory with file permissions
5. Monitor plugin behavior in production
6. Consider WASM for untrusted code

## Performance Optimization

For maximum performance:

1. **Avoid allocations** in hot paths
2. **Use `#[inline]`** for small functions
3. **Pre-allocate** buffers when possible
4. **Cache** expensive computations
5. **Profile** with tools like `perf` or `flamegraph`

Example optimizations:

```rust
// Instead of allocating every time:
fn process_header(key: &str) -> String {
    format!("X-{}", key)  // Allocates!
}

// Pre-allocate and reuse:
thread_local! {
    static BUFFER: RefCell<String> = RefCell::new(String::with_capacity(128));
}

fn process_header(key: &str) -> String {
    BUFFER.with(|buf| {
        let mut b = buf.borrow_mut();
        b.clear();
        b.push_str("X-");
        b.push_str(key);
        b.clone()  // Only clone the result
    })
}
```

## Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo run --features plugin-ffi
```

The example plugin logs to stderr:
```
[FFI Plugin] Initialized
[FFI Plugin] Request headers: GET /api/users
[FFI Plugin] Total requests: 1
[FFI Plugin] Response status: 200
```

## Cross-Language FFI

You can write FFI plugins in C/C++ as well:

**C Example:**

```c
#include <stdint.h>
#include <string.h>

typedef struct {
    const char* request_ptr;
    size_t request_len;
    const char* response_ptr;
    size_t response_len;
} PluginContext;

typedef struct {
    const char* (*name)(void);
    int32_t (*init)(const char*);
    int32_t (*on_request_headers)(PluginContext*);
    int32_t (*on_request_body)(PluginContext*);
    int32_t (*on_response_headers)(PluginContext*);
    int32_t (*on_response_body)(PluginContext*);
    void (*destroy)(void);
} PluginVTable;

const char* plugin_name(void) {
    return "my-c-plugin";
}

int32_t plugin_init(const char* config) {
    // Initialize plugin
    return 0;
}

int32_t on_request_headers(PluginContext* ctx) {
    // Process request
    return 0; // Continue
}

// ... implement other hooks ...

static PluginVTable vtable = {
    .name = plugin_name,
    .init = plugin_init,
    .on_request_headers = on_request_headers,
    // ... other hooks ...
};

__attribute__((visibility("default")))
PluginVTable* plugin_create(void) {
    return &vtable;
}
```

Compile with:
```bash
gcc -shared -fPIC -o libmyplugin.so plugin.c
```

## Testing

```bash
# Run tests
cargo test

# Run with address sanitizer (detect memory issues)
RUSTFLAGS="-Z sanitizer=address" cargo test

# Benchmark
cargo bench
```

## See Also

- [Plugin System Design](../../../PLUGIN_SYSTEM_DESIGN.md)
- [WASM Plugin Example](../wasm-hello-rust/)
- [Host Functions Reference](../../../docs/PLUGIN_HOST_FUNCTIONS.md)
