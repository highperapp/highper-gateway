# Plugin System Integration Guide

This guide walks you through integrating the plugin system into your highper-gateway deployment.

## Quick Start

### 1. Basic Setup

Create a plugin directory:
```bash
mkdir -p /etc/highper-gateway/plugins
chmod 700 /etc/highper-gateway/plugins
```

### 2. Configuration

Add to your `config.yaml`:

```yaml
plugins:
  discovery:
    plugin_dir: /etc/highper-gateway/plugins
    watch: true
    auto_load: true
    extensions: [wasm, so, dylib, dll]

  default_limits:
    memory_mb: 64
    fuel: 1000000
    timeout_ms: 100
    max_concurrent_requests: 100
```

### 3. Deploy a Plugin

Copy your plugin to the directory:
```bash
# WASM plugin
cp auth.wasm /etc/highper-gateway/plugins/

# FFI plugin
cp librate_limit.so /etc/highper-gateway/plugins/
```

### 4. Verify Plugin Loaded

Check logs:
```bash
tail -f /var/log/highper-gateway/proxy.log | grep -i plugin
```

Expected output:
```
[INFO] Initializing plugin manager
[INFO] Loading plugin: auth (wasm)
[INFO] Successfully loaded plugin: auth
[INFO] Hot reload monitor started successfully
```

## Common Use Cases

### Use Case 1: Authentication

**Goal**: Add JWT authentication to all requests

**Plugin Type**: WASM (safe for third-party auth providers)

**Configuration**:
```yaml
plugins:
  plugins:
    - name: jwt-auth
      type: wasm
      path: /etc/highper-gateway/plugins/jwt_auth.wasm
      enabled: true
      priority: 100  # Execute first
      limits:
        memory_mb: 64
        timeout_ms: 50  # Fast auth check
      capabilities:
        network:
          allow_outbound: ["auth.internal:443"]  # Allow auth service calls
      config:
        jwt_secret: "${JWT_SECRET}"
        issuer: "https://auth.example.com"
        audience: "api.example.com"
```

**Plugin Code** (Rust → WASM):
```rust
#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    // Get Authorization header
    let mut auth_buf = [0u8; 512];
    let len = unsafe {
        get_request_header(
            "authorization".as_ptr() as i32,
            "authorization".len() as i32,
            auth_buf.as_mut_ptr() as i32,
            auth_buf.len() as i32,
        )
    };

    if len <= 0 {
        // No auth header, return 401
        unsafe {
            set_response_status(401);
            set_response_header(
                "content-type".as_ptr() as i32,
                "content-type".len() as i32,
                "application/json".as_ptr() as i32,
                "application/json".len() as i32,
            );
        }
        return STOP;  // Stop processing
    }

    // Validate JWT (simplified)
    let token = &auth_buf[..len as usize];
    if !validate_jwt(token) {
        unsafe { set_response_status(403); }
        return STOP;
    }

    // Auth successful, continue
    CONTINUE
}
```

### Use Case 2: Rate Limiting

**Goal**: Limit requests to 100 req/s per IP

**Plugin Type**: FFI (high performance, in-process)

**Configuration**:
```yaml
plugins:
  plugins:
    - name: rate-limiter
      type: ffi
      path: /etc/highper-gateway/plugins/librate_limit.so
      enabled: true
      priority: 90
      config:
        limit: 100
        window_seconds: 1
        storage: "memory"  # or "redis"
```

**Plugin Code** (Rust FFI):
```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static RATE_LIMITS: Mutex<HashMap<String, (u64, u32)>> = Mutex::new(HashMap::new());

extern "C" fn on_request_headers_impl(ctx: *mut PluginContext) -> i32 {
    // Extract client IP from context
    let client_ip = extract_client_ip(ctx);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut limits = RATE_LIMITS.lock().unwrap();
    let (last_reset, count) = limits
        .entry(client_ip.clone())
        .or_insert((now, 0));

    // Reset counter if window expired
    if now > *last_reset {
        *last_reset = now;
        *count = 0;
    }

    *count += 1;

    if *count > 100 {
        // Rate limit exceeded
        unsafe {
            set_response_status(429);
            set_response_header(
                "retry-after".as_ptr() as i32,
                "retry-after".len() as i32,
                "1".as_ptr() as i32,
                1,
            );
        }
        return STOP;
    }

    CONTINUE
}
```

### Use Case 3: Custom Headers

**Goal**: Add custom headers to all responses

**Plugin Type**: WASM (simple, safe)

**Configuration**:
```yaml
plugins:
  plugins:
    - name: custom-headers
      type: wasm
      path: /etc/highper-gateway/plugins/headers.wasm
      enabled: true
      priority: 10  # Execute late
      config:
        headers:
          X-Served-By: "highper-gateway"
          X-Version: "1.0.0"
          X-Request-ID: "${REQUEST_ID}"
```

**Plugin Code**:
```rust
#[no_mangle]
pub extern "C" fn on_response_headers() -> i32 {
    // Add custom headers
    add_header("X-Served-By", "highper-gateway");
    add_header("X-Version", "1.0.0");

    // Generate request ID
    let request_id = generate_uuid();
    add_header("X-Request-ID", &request_id);

    CONTINUE
}

fn add_header(key: &str, value: &str) {
    unsafe {
        set_response_header(
            key.as_ptr() as i32, key.len() as i32,
            value.as_ptr() as i32, value.len() as i32,
        );
    }
}
```

### Use Case 4: Request Transformation

**Goal**: Transform JSON to Protocol Buffers

**Plugin Type**: FFI (zero-copy, high performance)

**Configuration**:
```yaml
plugins:
  plugins:
    - name: json-to-protobuf
      type: ffi
      path: /etc/highper-gateway/plugins/libtransform.so
      enabled: true
      priority: 50
      config:
        input_format: "json"
        output_format: "protobuf"
        schema_file: "/etc/schemas/api.proto"
```

### Use Case 5: Logging & Analytics

**Goal**: Log all requests to analytics service

**Plugin Type**: WASM (network isolated)

**Configuration**:
```yaml
plugins:
  plugins:
    - name: analytics
      type: wasm
      path: /etc/highper-gateway/plugins/analytics.wasm
      enabled: true
      priority: 1  # Execute last
      capabilities:
        network:
          allow_outbound: ["analytics.internal:443"]
      config:
        endpoint: "https://analytics.internal/events"
        batch_size: 100
```

## Plugin Chaining

Execute multiple plugins in order:

```yaml
plugins:
  plugins:
    # 1. Authentication (priority 100)
    - name: jwt-auth
      type: wasm
      priority: 100

    # 2. Rate Limiting (priority 90)
    - name: rate-limiter
      type: ffi
      priority: 90

    # 3. Request Validation (priority 80)
    - name: validator
      type: wasm
      priority: 80

    # 4. Custom Transform (priority 50)
    - name: transformer
      type: ffi
      priority: 50

    # 5. Add Headers (priority 10)
    - name: headers
      type: wasm
      priority: 10
```

**Execution Flow**:
```
Request → jwt-auth → rate-limiter → validator → transformer → headers → Proxy → Backend
                ↓           ↓            ↓            ↓            ↓
              (401)      (429)        (400)       (transform)   (headers)
```

## Hot Reload

### Deploying Updates

```bash
# Build new version
cargo build --release --target wasm32-wasi

# Copy to staging
cp target/wasm32-wasi/release/auth.wasm /tmp/auth.wasm.new

# Test the new version (optional)
wasm-validate /tmp/auth.wasm.new

# Atomic replacement (hot reload triggers automatically)
mv /tmp/auth.wasm.new /etc/highper-gateway/plugins/auth.wasm
```

**What happens:**
1. File watcher detects change
2. 100ms debounce (ensure write complete)
3. Old plugin continues serving active requests
4. New plugin loads and validates
5. New requests use new plugin
6. Old plugin unloads when idle

**Logs**:
```
[INFO] Plugin file changed: auth.wasm
[INFO] Hot reload detected for plugin: auth
[INFO] Unloading plugin: auth (waiting for 3 active requests)
[INFO] Loading plugin: auth (wasm)
[INFO] Successfully loaded plugin: auth
[INFO] Hot reload complete: auth
```

### Rollback

If the new plugin fails to load:
```bash
# Restore previous version
cp /etc/highper-gateway/plugins/auth.wasm.backup /etc/highper-gateway/plugins/auth.wasm
```

The old version continues running if new version fails to load.

## Monitoring & Debugging

### Enable Debug Logging

```yaml
logging:
  level: debug
  filters:
    - "highper_gateway::plugin=debug"
```

### View Plugin Statistics

Use the Admin API:
```bash
curl http://localhost:9090/admin/plugins

# Response:
{
  "plugins": [
    {
      "name": "jwt-auth",
      "type": "wasm",
      "enabled": true,
      "priority": 100,
      "stats": {
        "requests_total": 10000,
        "requests_success": 9500,
        "requests_failed": 500,
        "avg_execution_time_us": 15,
        "peak_execution_time_us": 120,
        "active_requests": 5,
        "memory_allocated": 67108864
      }
    }
  ]
}
```

### Disable Plugin Runtime

```bash
curl -X POST http://localhost:9090/admin/plugins/jwt-auth/disable

# Response:
{"status": "disabled", "plugin": "jwt-auth"}
```

### Enable Plugin Runtime

```bash
curl -X POST http://localhost:9090/admin/plugins/jwt-auth/enable

# Response:
{"status": "enabled", "plugin": "jwt-auth"}
```

### View Plugin Logs

Plugins use the host's logging:
```bash
tail -f /var/log/highper-gateway/proxy.log | grep "\[WASM Plugin\]\|\[FFI Plugin\]"
```

Output:
```
[INFO] [WASM Plugin] Processing request
[DEBUG] [WASM Plugin] JWT validation successful
[INFO] [FFI Plugin] Rate limit check: 45/100
```

## Performance Tuning

### 1. Choose the Right Plugin Type

**Use WASM when:**
- Plugin is from third party
- Safety is critical
- Multi-language support needed
- Moderate performance acceptable (10-50μs)

**Use FFI when:**
- Plugin is trusted (your code)
- Maximum performance required (1-5μs)
- Direct memory access needed
- Large data transformations

### 2. Optimize Resource Limits

**For Fast Operations** (auth, headers):
```yaml
limits:
  memory_mb: 32        # Reduce memory
  fuel: 500000         # Reduce fuel
  timeout_ms: 25       # Tight timeout
```

**For Heavy Operations** (body transform):
```yaml
limits:
  memory_mb: 128       # More memory
  fuel: 5000000        # More fuel
  timeout_ms: 500      # Longer timeout
```

### 3. Plugin Priority Optimization

**Best Practice**: Order by execution speed
```yaml
# Fast plugins first (fail fast)
- name: rate-limiter (FFI)
  priority: 100

- name: auth (WASM, cached)
  priority: 90

- name: validator (WASM)
  priority: 80

# Slow plugins last
- name: transformer (FFI, heavy)
  priority: 50
```

### 4. Reduce Plugin Count

Instead of:
```yaml
# 3 separate plugins
- name: add-header-1
- name: add-header-2
- name: add-header-3
```

Use:
```yaml
# 1 combined plugin
- name: add-all-headers
```

### 5. Cache Plugin State

```rust
// Bad: Calculate every request
#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    let config = load_config();  // Slow!
    let value = compute_value();  // Slow!
    // ...
}

// Good: Cache in plugin state
static mut CACHED_CONFIG: Option<Config> = None;
static mut CACHED_VALUE: Option<String> = None;

#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    unsafe {
        if CACHED_CONFIG.is_none() {
            CACHED_CONFIG = Some(load_config());
        }
        let config = CACHED_CONFIG.as_ref().unwrap();
        // Use cached config
    }
}
```

## Security Best Practices

### 1. WASM Plugin Sandboxing

```yaml
capabilities:
  filesystem: false              # Disable unless needed
  network:
    allow_outbound:
      - "specific.service.internal"  # Whitelist only
    deny_private_ips: true       # Block RFC1918
  environment: []                # No env vars unless needed
  random: false                  # Disable unless needed
  clock: true                    # Usually safe
```

### 2. FFI Plugin Security

⚠️ **Critical**: FFI plugins are NOT sandboxed

**Checklist**:
- [ ] Code review by security team
- [ ] Sign plugin binaries
- [ ] Restrict plugin directory (chmod 700)
- [ ] Run with least privilege
- [ ] Monitor system calls
- [ ] Use AppArmor/SELinux profiles

**Example AppArmor profile**:
```
/etc/apparmor.d/highper-gateway-plugins
#include <tunables/global>

/etc/highper-gateway/plugins/* {
  #include <abstractions/base>

  # Allow reading plugin files
  /etc/highper-gateway/plugins/*.so r,

  # Deny everything else
  deny /etc/** w,
  deny /home/** rw,
  deny /root/** rw,
  deny network,
}
```

### 3. Input Validation

Always validate plugin inputs:
```rust
#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    let mut header_buf = [0u8; 8192];  // Reasonable limit
    let len = unsafe {
        get_request_header(
            "content-length".as_ptr() as i32,
            "content-length".len() as i32,
            header_buf.as_mut_ptr() as i32,
            header_buf.len() as i32,
        )
    };

    if len < 0 || len > 8192 {
        log_error("Invalid header length");
        return ERROR;
    }

    // Parse and validate
    let value = match std::str::from_utf8(&header_buf[..len as usize]) {
        Ok(v) => v,
        Err(_) => {
            log_error("Invalid UTF-8");
            return ERROR;
        }
    };

    // Continue processing
    CONTINUE
}
```

### 4. Rate Limit Plugin Operations

Prevent abuse:
```rust
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

#[no_mangle]
pub extern "C" fn on_request_headers() -> i32 {
    let count = REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

    if count % 1000 == 0 {
        // Every 1000 requests, log stats
        log_info(&format!("Processed {} requests", count));
    }

    CONTINUE
}
```

## Troubleshooting

### Plugin Won't Load

**Error**: `Failed to load plugin: Permission denied`

**Solution**:
```bash
chmod +r /etc/highper-gateway/plugins/myplugin.wasm
```

---

**Error**: `WASM loader not initialized`

**Solution**: Enable WASM feature:
```bash
cargo build --features plugin-wasm
```

---

**Error**: `Function 'on_request_headers' not found`

**Solution**: Ensure function is exported:
```rust
#[no_mangle]  // Required!
pub extern "C" fn on_request_headers() -> i32 {
    CONTINUE
}
```

### Plugin Timeouts

**Error**: `Plugin timeout: auth`

**Solutions**:
1. Increase timeout:
   ```yaml
   limits:
     timeout_ms: 200  # Increase from 100ms
   ```

2. Increase fuel (WASM):
   ```yaml
   limits:
     fuel: 2000000  # Increase from 1M
   ```

3. Optimize plugin code
4. Move to FFI for performance

### Memory Issues

**Error**: `Resource limit exceeded: memory`

**Solutions**:
1. Increase limit:
   ```yaml
   limits:
     memory_mb: 128  # Increase from 64MB
   ```

2. Reduce plugin memory usage
3. Free unused memory in plugin

### Hot Reload Not Working

**Problem**: File changes not detected

**Solutions**:
1. Check watch is enabled:
   ```yaml
   discovery:
     watch: true
   ```

2. Verify file watcher running:
   ```bash
   ps aux | grep highper-gateway
   # Should show process with inotify/kqueue
   ```

3. Check file permissions:
   ```bash
   ls -la /etc/highper-gateway/plugins/
   ```

4. Test manually:
   ```bash
   touch /etc/highper-gateway/plugins/test.wasm
   # Check logs for file change event
   ```

## Production Deployment Checklist

- [ ] **Plugin Audit**
  - [ ] Security review completed
  - [ ] Performance tested
  - [ ] Error handling verified
  - [ ] Resource limits configured

- [ ] **Infrastructure**
  - [ ] Plugin directory created (chmod 700)
  - [ ] Backups configured
  - [ ] Monitoring set up
  - [ ] Alerts configured

- [ ] **Configuration**
  - [ ] Resource limits tuned
  - [ ] Priorities optimized
  - [ ] Hot-reload enabled
  - [ ] Logging configured

- [ ] **Testing**
  - [ ] Load tested
  - [ ] Failure scenarios tested
  - [ ] Hot-reload tested
  - [ ] Rollback procedure tested

- [ ] **Documentation**
  - [ ] Plugin purpose documented
  - [ ] Dependencies listed
  - [ ] Configuration documented
  - [ ] Runbook created

## Summary

The highper-gateway plugin system provides powerful extensibility while maintaining performance and safety. Key takeaways:

1. **Choose the right type**: WASM for safety, FFI for performance
2. **Configure limits**: Tune memory, fuel, timeout for your use case
3. **Use priorities**: Order plugins by execution speed
4. **Monitor closely**: Track stats, errors, performance
5. **Secure FFI plugins**: They have full system access
6. **Test hot-reload**: Ensure zero-downtime updates work

For more information:
- [Host Functions Reference](./PLUGIN_HOST_FUNCTIONS.md)
- [Plugin System Design](../PLUGIN_SYSTEM_DESIGN.md)
- [Examples](../examples/plugins/)
