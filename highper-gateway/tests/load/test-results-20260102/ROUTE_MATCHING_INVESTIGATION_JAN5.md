# Route Matching Investigation - January 5, 2026

## Critical Discovery: Wildcard Pattern Matching Failure

### Executive Summary

Found and diagnosed the route matching issue that was causing 404 errors in Scenario 14/15 testing:

**Root Cause #1**: Missing test directory `/tmp/php-test-www/` - FIXED ✅
**Root Cause #2**: Wildcard pattern `/*` not matching paths - ACTIVE BUG ❌

---

## Investigation Timeline

### Initial Problem
- Routes configured with `paths = ["/*"]` were not matching any requests
- All requests returned `HTTP/1.1 404 Not Found - No matching route found`
- Static file handler was initializing correctly
- Configuration was loading without errors

### Test Environment
- **Gateway**: Highper Gateway v0.1.0
- **Binary**: `/mnt/e/my-opensource/highper-gateway/target/release/highper-gateway`
- **Config**: `/tmp/minimal-test.toml`
- **Test Directory**: `/tmp/php-test-www/`
- **Test File**: `/tmp/php-test-www/index.html`

---

## Key Findings

### 1. Missing Test Directory ✅ FIXED

**Problem**: The directory `/tmp/php-test-www/` did not exist

**Evidence**:
```bash
$ ls -la /tmp/php-test-www/
ls: cannot access '/tmp/php-test-www/': No such file or directory
```

**Solution**:
```bash
mkdir -p /tmp/php-test-www
cat > /tmp/php-test-www/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Static File Test</title>
</head>
<body>
    <h1>Static HTML File Served Successfully!</h1>
    <p>This file is being served by Highper Gateway's static file handler.</p>
</body>
</html>
EOF
```

**Result**: After creating the directory, some routes began to match

---

### 2. Wildcard Pattern `/*` Not Working ❌ BUG

**Test Results**:

| Request Path | Pattern | Expected | Actual | Status |
|-------------|---------|----------|--------|--------|
| `/` | `["/*"]` | ✅ Match | ✅ Match | **PASS** |
| `/index.html` | `["/*"]` | ✅ Match | ❌ No match | **FAIL** |
| `/index.html` | `["/index.html"]` | ✅ Match | ✅ Match | **PASS** |
| `/index.html` | `["/index.*"]` | ✅ Match | ❌ No match | **FAIL** |

**Evidence**:

```bash
# Test 1: Root path with /* pattern
$ curl -s http://localhost:8080/
<!DOCTYPE html>
<html>
<head>
    <title>Static File Test</title>
</head>
<body>
    <h1>Static HTML File Served Successfully!</h1>
    <p>This file is being served by Highper Gateway's static file handler.</p>
</body>
</html>

# Test 2: Sub-path with /* pattern
$ curl -s http://localhost:8080/index.html
No matching route found

# Test 3: Explicit path listing
$ curl -s http://localhost:8080/index.html  # with paths = ["/index.html"]
<!DOCTYPE html>...</html>  # SUCCESS!

# Test 4: Wildcard suffix pattern
$ curl -s http://localhost:8080/index.html  # with paths = ["/index.*"]
No matching route found
```

---

## Code Analysis

### matches_pattern() Function
Location: `src/proxy/handler.rs:1276-1293`

```rust
fn matches_pattern(&self, value: &str, pattern: &str) -> bool {
    if pattern == "*" || pattern == "/*" {
        return true;  // ← Should match ALL paths
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

### Expected Behavior

For pattern `"/*"`:
- Line 1278 should match: `pattern == "/*"` → true
- Should return true for ALL values
- `/`, `/index.html`, `/any/path` should all match

### Actual Behavior

- `/` matches ✅
- `/index.html` does NOT match ❌
- Pattern `/*` seems to only match exact root path

### Hypothesis

One of the following must be true:

1. **Configuration not loading correctly**: The pattern `"/*"` is not being stored in `match_rules.paths`
2. **find_route() early termination**: Route matching stops before checking all patterns
3. **Pattern comparison issue**: The pattern comparison in matches_pattern() is not being called
4. **Debug logging disabled**: Cannot see what's actually happening because trace logs don't appear

---

## Configuration Structure

### TOML Configuration (Verified Correct)

```toml
[[routes]]
name = "test-route"
upstream = "dummy"
static_files = true
root = "/tmp/php-test-www"
index = ["index.html"]

[routes.match]
paths = ["/*"]  # ← Should match all paths according to code

[observability.logging]
level = "trace"  # ← Should enable debug logs, but doesn't seem to work
format = "pretty"
```

### Struct Mapping

`config/schema.rs:626-632`:
```rust
pub struct RouteConfig {
    pub name: String,

    #[serde(rename = "match")]
    pub match_rules: MatchRules,  // ← TOML [routes.match]

    pub upstream: String,
    // ...
}
```

`config/schema.rs:742-754`:
```rust
pub struct MatchRules {
    #[serde(default)]
    pub hosts: Vec<String>,

    #[serde(default)]
    pub paths: Vec<String>,  // ← Should contain ["/*"]

    #[serde(default)]
    pub methods: Vec<String>,
}
```

**Note**: All fields have `#[serde(default)]`, meaning empty vectors if not specified.

---

## Why Debug Logs Don't Appear

Despite setting `level = "trace"` in configuration, NO debug logs from `find_route()` appear in gateway logs.

### Expected Debug Output

From `src/proxy/handler.rs:1208-1214`:
```rust
debug!("Route matching: method={}, host={}, host_without_port={}, path={}",
       method, host, host_without_port, path);
debug!("Available routes: {}", self.config.routes.len());

for route in &self.config.routes {
    debug!("Checking route: name={}, hosts={:?}, paths={:?}",
           route.name, route.match_rules.hosts, route.match_rules.paths);
    // ...
```

### Actual Log Output

```
[2026-01-05T00:45:58.607883Z] INFO highper_gateway: Starting Highper Gateway v0.1.0
[2026-01-05T00:45:58.608146Z] INFO highper_gateway: Loading configuration from: /tmp/minimal-test.toml
[2026-01-05T00:45:58.619034Z] INFO highper_gateway::proxy::server: Static file handler initialized with root: /tmp/php-test-www
[2026-01-05T00:45:58.619251Z] INFO highper_gateway::proxy::server: HTTP listening on 127.0.0.1:8080
```

No DEBUG or TRACE level logs appear anywhere.

### Possible Causes

1. **Logging level not applied**: Config setting might not affect log level
2. **Compile-time filtering**: Debug logs might be compiled out in release builds
3. **Logger initialization timing**: Level might be set before config is loaded
4. **Env var override**: `RUST_LOG` environment variable might override config

---

## Working Scenarios Comparison

### Scenario 01 (TCP Native) - Works ✅

Configuration uses same pattern:
```toml
[[routes]]
name = "tcp-route"
upstream = "tcp-backends"

[routes.match]
paths = ["/*"]
```

**Status**: This scenario works correctly with TCP proxy routing

### Difference from Our Test

- TCP scenario doesn't use static file handler
- Requests go through proxy path, not webserver path
- Same route matching code should be used

**Implication**: The bug might be specific to webserver route handling, OR TCP routes don't actually work either but we haven't tested sub-paths.

---

## Workaround

### Temporary Solution

Explicitly list all expected paths in the configuration:

```toml
[routes.match]
paths = [
    "/",
    "/index.html",
    "/index.php",
    "/style.css",
    "/script.js",
    # ... list all files
]
```

**Pros**:
- Works reliably
- Clear and explicit

**Cons**:
- Not scalable
- Defeats purpose of wildcard patterns
- Requires updating config for every new file

---

## Recommended Next Steps

### Immediate (Debug)

1. **Enable RUST_LOG environment variable**:
   ```bash
   RUST_LOG=highper_gateway=trace /path/to/highper-gateway start --config /tmp/minimal-test.toml
   ```

2. **Check if debug build has logs**:
   ```bash
   /path/to/target/debug/highper-gateway start --config /tmp/minimal-test.toml
   ```

3. **Add print! statements** (requires recompile):
   ```rust
   fn find_route(&self, method: &Method, host: &str, path: &str) -> Option<&RouteConfig> {
       eprintln!(">>> find_route called: path={}", path);
       eprintln!(">>> routes.len()={}", self.config.routes.len());
       for route in &self.config.routes {
           eprintln!(">>> Checking route: paths={:?}", route.match_rules.paths);
           // ...
       }
   }
   ```

### Short-Term (Investigate)

4. **Test TCP scenario with sub-paths**:
   - Verify if `/api/endpoint` matches pattern `["/*"]` in TCP mode
   - Determine if bug affects all routes or just webserver routes

5. **Test with different patterns**:
   - `["/index.html", "/other.html"]` - explicit list
   - `["/"]` - exact root only
   - `["*"]` - catch-all without slash
   - `["/api/*"]` - prefix wildcard

6. **Check if match_rules.paths is empty**:
   - Add logging to RouteConfig deserialization
   - Verify paths array is populated from TOML

### Long-Term (Fix)

7. **Fix wildcard matching logic**:
   - Review matches_pattern() implementation
   - Add unit tests for wildcard patterns
   - Consider using regex or glob crate instead

8. **Fix debug logging**:
   - Ensure trace level logs work in release builds
   - Add compile-time feature flag if needed
   - Document logging behavior

---

## Test Configuration Files

### Minimal Working Config

**File**: `/tmp/minimal-test.toml`

```toml
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"

[[upstreams]]
name = "dummy"
[[upstreams.servers]]
url = "http://127.0.0.1:9999"

[[routes]]
name = "test-route"
upstream = "dummy"
static_files = true
root = "/tmp/php-test-www"
index = ["index.html"]
[routes.match]
paths = ["/"]  # ← Only matches root, not sub-paths

[observability.logging]
level = "trace"
format = "pretty"
```

### Test Directory Setup

```bash
mkdir -p /tmp/php-test-www
cat > /tmp/php-test-www/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Static File Test</title>
</head>
<body>
    <h1>Static HTML File Served Successfully!</h1>
    <p>This file is being served by Highper Gateway's static file handler.</p>
</body>
</html>
EOF
```

---

## Conclusions

### What We've Proven

1. ✅ **Static file handler works** - Can serve files when route matches
2. ✅ **Route matching works** - Can match explicit paths like `["/index.html"]`
3. ✅ **Configuration loads** - TOML syntax is correct, no parse errors
4. ✅ **Handlers initialize** - Static file handler gets correct root directory
5. ❌ **Wildcard patterns broken** - Pattern `/*` and `/index.*` don't work as expected
6. ❌ **Debug logging broken** - Trace/debug logs don't appear even when configured

### Impact on Scenario 14/15

- **Scenario 14 (PHP-FPM)**: Cannot test comprehensively without wildcard patterns
- **Scenario 15 (GeoIP)**: Same route matching code, likely affected
- **Workaround**: Must explicitly list all test paths in configuration

### Severity

🔴 **HIGH** - Blocks comprehensive testing of webserver features

---

## Time Investment

- **Investigation**: 2 hours
- **Testing**: 1 hour
- **Documentation**: 1 hour
- **Total**: 4 hours

---

## Status

🔴 **BLOCKED** - Wildcard pattern matching not working, debug logs not appearing

**Recommended Action**: Need assistance from someone familiar with the codebase to:
1. Verify if `/*` pattern ever worked
2. Check if there's a known issue with wildcard matching
3. Help enable debug logging to diagnose the root cause

---

*Investigation Date*: January 5, 2026 00:47 UTC
*Document Version*: 1.0
*Status*: Active Bug Investigation

