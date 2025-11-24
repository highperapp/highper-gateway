# WASM Plugin Example (Rust)

This is an example WASM plugin written in Rust that demonstrates how to create plugins for highper-gateway.

## Building

To build this plugin as a WASM module:

```bash
# Install wasm32-wasi target
rustup target add wasm32-wasi

# Build for WASM
cargo build --target wasm32-wasi --release

# The compiled plugin will be at:
# target/wasm32-wasi/release/wasm_hello_plugin.wasm
```

## Configuration

Add this to your highper-gateway configuration:

```yaml
plugins:
  discovery:
    plugin_dir: ./plugins
    auto_load: true
    watch: true

  plugins:
    - name: hello-wasm
      type: wasm
      path: ./plugins/wasm_hello_plugin.wasm
      enabled: true
      priority: 50
      limits:
        memory_mb: 64
        fuel: 1000000
        timeout_ms: 100
        max_concurrent_requests: 100
      capabilities:
        filesystem: false
        network:
          allow_outbound: []
          deny_private_ips: true
        environment: []
        random: false
        clock: true
```

## Plugin Interface

WASM plugins must export these functions:

- `on_request_headers() -> i32` - Called when request headers are received
- `on_request_body() -> i32` - Called when request body is available
- `on_response_headers() -> i32` - Called before sending response headers
- `on_response_body() -> i32` - Called before sending response body

Return values:
- `0` - Continue to next plugin
- `1` - Stop iteration and return response
- `2` - Pause for async operation
- `3` - Error occurred

## Host Functions

Plugins can call these host functions (provided by highper-gateway):

- `proxy_get_header(key, value_buf) -> i32`
- `proxy_set_header(key, value) -> i32`
- `proxy_delete_header(key) -> i32`
- `proxy_get_body(buf) -> i32`
- `proxy_set_body(data) -> i32`
- `proxy_log(level, message)`
- `proxy_emit_metric(name, value, type)`

## Limitations

WASM plugins run in a sandboxed environment with:
- Limited memory (configurable, default 64MB)
- Fuel/instruction limits to prevent infinite loops
- Timeout enforcement
- No direct filesystem access (unless explicitly granted)
- No network access (unless explicitly granted)
- Limited CPU and memory usage

For high-performance use cases, consider using FFI plugins instead.
