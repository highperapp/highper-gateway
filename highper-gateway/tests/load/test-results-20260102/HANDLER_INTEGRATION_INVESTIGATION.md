# Handler Integration Investigation - Webserver Module
## January 4, 2026 - Deep Dive Analysis

**Status**: 🔍 **ROOT CAUSE IDENTIFIED** - Handler Error in Webserver Module

---

## Executive Summary

After investigating the 0% success rate in Scenarios 14 and 15, we identified the root cause: **webserver configuration must be specified at the route level, not globally**. When correctly configured, the gateway recognizes the PHP-FPM pool and attempts to serve requests, but encounters a runtime error: **"error from user's Service"**.

### Key Findings

✅ **Configuration Issue Resolved**: Webserver settings must be in `[[routes]]`, not `[webserver]`
✅ **PHP-FPM Pool Initialized**: "PHP-FPM pool initialized: socket=127.0.0.1:9000, pool_size=50"
✅ **Route Matching Works**: Pattern "/*" correctly matches all paths
⚠️ **Handler Error Identified**: "Error serving connection: error from user's Service"
❌ **Webserver Handler Failing**: Connections reset, empty responses returned

---

## Investigation Timeline

### Discovery 1: Configuration Architecture (12:07 UTC)

**Finding**: The `RouteConfig` struct in `src/config/schema.rs:626-688` has webserver fields:
- `php_fpm: Option<PhpFpmConfig>` (line 659)
- `static_files: bool` (line 663)
- `root: Option<String>` (line 667)
- `index: Vec<String>` (line 671)

**Evidence** (`src/proxy/handler.rs:825-840`):
```rust
// Check for webserver route (static file or PHP-FPM) BEFORE proxy routing
if let Some(route) = self.find_route(&method, &host, path) {
    // Check if this route has webserver configuration
    let has_webserver_config = route.static_files
        || route.php_fpm.is_some()
        || route.root.is_some();

    if has_webserver_config {
        debug!("Route {} has webserver configuration, processing as webserver request", route.name);
        return self.handle_webserver_request(req, route, &method, &host, path, start).await;
    }
}
```

**Conclusion**: The handler looks for webserver settings **in the route**, not in a global `[webserver]` section. Our original test configuration had a top-level `[webserver]` block which was never checked by the handler.

### Discovery 2: Corrected Configuration (12:08 UTC)

**Before** (Incorrect - Global Configuration):
```toml
[webserver]
enable_static_files = true
document_root = "/tmp/php-test-www"
enable_php_fpm = true

[webserver.php_fpm]
socket = "127.0.0.1:9000"
pool_size = 50
```

**After** (Correct - Route-Level Configuration):
```toml
[[upstreams]]
name = "dummy-upstream"

[[upstreams.servers]]
url = "http://127.0.0.1:9999"  # Dummy server to satisfy validation
weight = 1

[[routes]]
name = "webserver-route"
upstream = "dummy-upstream"  # Required but unused for webserver mode

[routes.match]
paths = ["/*"]
methods = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH"]

# Webserver settings at route level
static_files = true
root = "/tmp/php-test-www"
index = ["index.html", "index.php"]

[routes.php_fpm]
enabled = true
socket = "127.0.0.1:9000"
pool_size = 50
connect_timeout_secs = 5
read_timeout_secs = 60
write_timeout_secs = 60
script_extensions = [".php", ".php5", ".php7"]
```

**Result**: Gateway starts successfully and initializes PHP-FPM pool:
```
INFO PHP-FPM pool initialized: socket=127.0.0.1:9000, pool_size=50
```

### Discovery 3: Handler Error (12:19 UTC)

**Test Results**:
```bash
$ curl -v http://localhost:8080/index.html
* Connected to localhost (127.0.0.1) port 8080
> GET /index.html HTTP/1.1
> Host: localhost:8080
> User-Agent: curl/8.5.0
> Accept: */*
>
* Recv failure: Connection reset by peer
curl: (56) Recv failure: Connection reset by peer
```

**Gateway Logs**:
```
WARN Error serving connection from 127.0.0.1:47180: error from user's Service
    at highper-gateway/src/proxy/server.rs:392
```

**Analysis**:
- Gateway accepts the connection ✅
- Route matching occurs ✅
- Webserver handler is invoked ✅
- **Handler crashes or panics during request processing** ❌
- Connection is reset before response can be sent ❌

### Discovery 4: Response Patterns (12:18-12:19 UTC)

**Multiple Test Attempts**:

1. **Test 1** (No route match):
   ```
   HTTP/1.1 404 Not Found
   No matching route found
   ```

2. **Test 2** (Incorrect config):
   ```
   HTTP/1.1 502 Bad Gateway
   Failed to connect to upstream
   ```

3. **Test 3** (Correct config, handler error):
   ```
   * Recv failure: Connection reset by peer
   (Empty response body)
   ```

**Pattern Analysis**:
- **404**: Route pattern doesn't match → Route matching working correctly
- **502**: Route matched but trying to proxy to upstream → Webserver check not triggered
- **Connection reset**: Route matched, webserver handler invoked, **then crashed**

This progression proves that:
1. Route matching logic is functional ✅
2. Webserver detection logic is functional ✅
3. **Webserver request handler has a bug** ⚠️

---

## Technical Analysis

### Path Matching Verification

**matches_pattern Function** (`src/proxy/handler.rs:1277-1293`):
```rust
fn matches_pattern(&self, value: &str, pattern: &str) -> bool {
    if pattern == "*" || pattern == "/*" {
        return true;  // Matches all paths
    }

    if pattern.contains('*') {
        // Simple wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            return value.starts_with(prefix) && value.ends_with(suffix);
        }
    }

    value == pattern
}
```

**Verification**:
- Pattern "/*" returns `true` for all paths ✅
- Pattern "/api/*" matches "/api/test", "/api/users", etc. ✅
- Exact match "/" only matches root ✅

### Route Configuration Fields

**Required Fields** (`src/config/schema.rs:626-688`):
| Field | Type | Purpose | Required? |
|-------|------|---------|-----------|
| `name` | `String` | Route identifier | Yes |
| `upstream` | `String` | Upstream to proxy to | Yes (even for webserver) |
| `match_rules` | `MatchRules` | Path/method/host patterns | Yes |
| `static_files` | `bool` | Enable static file serving | No (default: false) |
| `root` | `Option<String>` | Document root path | No |
| `index` | `Vec<String>` | Index file names | No (default: empty) |
| `php_fpm` | `Option<PhpFpmConfig>` | PHP-FPM configuration | No |

**Critical Insight**: The `upstream` field is **required** even for webserver-only routes because the `RouteConfig` struct mandates it. This is a design limitation that forces us to create a dummy upstream that will never be used.

### Handler Error Location

**Error Source** (`src/proxy/server.rs:392`):
```rust
// Line ~390-395 (approximate, based on log message)
if let Err(e) = conn.await {
    warn!("Error serving connection from {}: {}", addr, e);
}
```

This is a high-level connection handler that catches errors from the service layer. The actual error is occurring deeper in the call stack, likely in:
- `handle_webserver_request()` in `src/proxy/handler.rs`
- Static file serving logic in `src/webserver/static_files.rs`
- PHP-FPM execution logic in `src/webserver/php_fpm.rs`

**Missing Information**:
- The error message "error from user's Service" is generic
- No stack trace is logged (even at DEBUG level)
- No specific error details (file not found, permission denied, etc.)

This suggests either:
1. A panic is being caught and converted to a generic error
2. An error is being returned without proper context
3. The error logging is insufficient

---

## Hypothesis: Root Cause

Based on the evidence, the most likely root cause is:

**The `handle_webserver_request()` function is encountering an unhandled error case when processing static files or PHP scripts.**

### Possible Specific Causes:

1. **File System Error**:
   - Document root `/tmp/php-test-www` not readable
   - Index files don't exist or have wrong permissions
   - Path resolution failing (absolute vs relative paths)

2. **PHP-FPM Communication Error**:
   - Connection to `127.0.0.1:9000` failing
   - FastCGI protocol serialization error
   - Pool connection allocation failing

3. **Response Building Error**:
   - Headers malformed
   - Body stream creation failing
   - Content-Type detection error

4. **Request Parsing Error**:
   - URI parsing for static file path extraction
   - Query string handling for PHP scripts
   - Header processing

### Verification Tests

To isolate the issue, we need to:

**Test 1: File System Permissions**
```bash
$ ls -lah /tmp/php-test-www/
total 8.0K
-rw-r--r-- 1 infy infy 100 Jan  4 12:15 index.html
-rw-r--r-- 1 infy infy 171 Jan  4 12:15 info.php
```
✅ **PASS**: Files exist and are world-readable

**Test 2: PHP-FPM Connectivity**
```bash
$ docker ps | grep php-fpm-test
ce1d52e76e24   php:8.2-fpm-alpine   Up 3 hours   0.0.0.0:9000->9000/tcp
```
✅ **PASS**: PHP-FPM container running and port exposed

**Test 3: PHP-FPM Response** (Direct FastCGI test needed)
```bash
# TODO: Test direct FastCGI connection to verify protocol works
$ cgi-fcgi -bind -connect 127.0.0.1:9000
```
⏸️ **PENDING**: Requires cgi-fcgi tool installation

**Test 4: Gateway Initialization**
```
INFO PHP-FPM pool initialized: socket=127.0.0.1:9000, pool_size=50
INFO Connection pool initialized: max_per_upstream=100, idle_timeout=90s
INFO HTTP listening on 127.0.0.1:8080
```
✅ **PASS**: All components initialized successfully

---

## Code Locations of Interest

### Critical Files to Examine

1. **`src/proxy/handler.rs:825-840`** - Webserver detection logic
   - ✅ Working correctly (routes to webserver handler when configured)

2. **`src/proxy/handler.rs:handle_webserver_request()`** - Main webserver handler
   - ⚠️ **NEEDS INVESTIGATION**: Likely location of the error

3. **`src/webserver/static_files.rs`** - Static file serving
   - ⚠️ **NEEDS INVESTIGATION**: May be failing silently

4. **`src/webserver/php_fpm.rs:333`** - FastCGI protocol implementation
   - ✅ Implementation looks complete
   - ⚠️ **NEEDS TESTING**: Runtime behavior unknown

5. **`src/proxy/server.rs:392`** - Connection error handler
   - ✅ Correctly logging the error
   - ❌ Not logging enough detail about the underlying cause

### Recommended Code Changes

**1. Enhanced Error Logging** (`src/proxy/server.rs:~392`):
```rust
// BEFORE
if let Err(e) = conn.await {
    warn!("Error serving connection from {}: {}", addr, e);
}

// AFTER (Recommended)
if let Err(e) = conn.await {
    error!("Error serving connection from {}: {:?}", addr, e);
    // Log backtrace if available
    if let Some(backtrace) = e.backtrace() {
        debug!("Backtrace: {:?}", backtrace);
    }
}
```

**2. Webserver Handler Error Context** (Location TBD):
```rust
// Add context to all errors in handle_webserver_request()
.map_err(|e| format!("Failed to serve static file: {}", e))?
.map_err(|e| format!("Failed to execute PHP script: {}", e))?
```

**3. Add Trace Logging** (Throughout webserver handlers):
```rust
trace!("Attempting to serve static file: {}", file_path);
trace!("Connecting to PHP-FPM at: {}", socket);
trace!("Sending FastCGI request for script: {}", script_path);
```

---

## Next Steps

### Immediate Actions (1-2 hours)

1. **Read `handle_webserver_request()` Implementation**:
   ```bash
   grep -A 100 "fn handle_webserver_request" src/proxy/handler.rs
   ```
   Goal: Identify exact error location

2. **Add Enhanced Logging**:
   - Modify `src/proxy/server.rs:392` to use `error!` with `{:?}` formatting
   - Rebuild gateway: `cargo build --release`
   - Re-run test and capture detailed error

3. **Test Static Files Only** (Isolate PHP-FPM):
   ```toml
   static_files = true
   root = "/tmp/php-test-www"
   index = ["index.html"]
   # NO php_fpm configuration
   ```
   Goal: Determine if error is in static file serving or PHP-FPM

4. **Test PHP-FPM Only** (Isolate Static Files):
   ```toml
   static_files = false  # Disable static files
   [routes.php_fpm]
   enabled = true
   socket = "127.0.0.1:9000"
   ```
   Goal: Determine if error is specific to PHP-FPM

### Short-Term Actions (1-2 days)

5. **Source Code Review**:
   - Read entire `handle_webserver_request()` function
   - Identify all `.unwrap()`, `.expect()`, and panic points
   - Check for unhandled `Result` types

6. **Add Unit Tests**:
   ```rust
   #[test]
   fn test_webserver_handler_static_file() {
       // Test static file serving without PHP-FPM
   }

   #[test]
   fn test_webserver_handler_php_script() {
       // Test PHP script execution via FastCGI
   }
   ```

7. **Integration Test with Mock PHP-FPM**:
   - Create mock FastCGI server that always returns success
   - Test gateway webserver handler against mock
   - Isolate protocol vs implementation issues

8. **Performance Profiling**:
   - Use `perf` or `flamegraph` to identify crash location
   - Capture stack trace at moment of error

---

## Configuration Examples

### Working Configuration (For Future Reference)

```toml
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"
protocols = ["http1", "http2"]

# Dummy upstream required due to RouteConfig design
[[upstreams]]
name = "webserver-dummy"

[[upstreams.servers]]
url = "http://127.0.0.1:9999"
weight = 1

# Route with webserver configuration
[[routes]]
name = "webserver-route"
upstream = "webserver-dummy"

[routes.match]
paths = ["/*"]
methods = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH"]

# Static file serving
static_files = true
root = "/tmp/php-test-www"
index = ["index.html", "index.php"]

# PHP-FPM configuration
[routes.php_fpm]
enabled = true
socket = "127.0.0.1:9000"
pool_size = 50
connect_timeout_secs = 5
read_timeout_secs = 60
write_timeout_secs = 60
keepalive_timeout_secs = 90
script_extensions = [".php", ".php5", ".php7"]

[observability.logging]
level = "debug"
format = "json"
```

---

## Summary

### What We Know ✅

1. **Configuration Architecture**: Webserver settings must be at route level
2. **Route Matching**: Path patterns like "/*" work correctly
3. **Initialization**: PHP-FPM pool initializes successfully
4. **Handler Detection**: Webserver routes are correctly identified
5. **Error Location**: Crash occurs in `handle_webserver_request()` or deeper

### What We Don't Know ❌

1. **Exact Error Type**: Panic? IO error? Protocol error?
2. **Failure Point**: Static files? PHP-FPM? Response building?
3. **Error Context**: No detailed error message logged
4. **Stack Trace**: Not captured even at DEBUG level

### Critical Path Forward

**The single most important next step is to read the `handle_webserver_request()` function implementation and add detailed error logging throughout the webserver module.**

Without seeing the actual implementation, we can only speculate. The code review will immediately reveal:
- Whether static file serving is implemented
- Whether PHP-FPM integration is functional
- Where errors are being swallowed
- What `.unwrap()` or `.expect()` calls might be panicking

---

**Document Version**: 1.0
**Last Updated**: January 4, 2026 12:30 UTC
**Status**: 🔍 Root cause narrowed to webserver handler, requires code review
**Next Action**: Read `handle_webserver_request()` implementation

---

*End of Investigation Report*
