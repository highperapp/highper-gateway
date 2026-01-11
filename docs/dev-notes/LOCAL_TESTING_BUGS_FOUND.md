# Local Testing - Bugs Found

**Date Started**: 2025-12-04
**Purpose**: Track all bugs found during local scenario testing

---

## Bug #1: Configuration Format Mismatch - DSL vs YAML

**Scenario**: Scenario 01 - Layer 4 TCP
**Severity**: **Critical** (blocks all testing)
**Date Found**: 2025-12-04
**Status**: 🔴 Open

### Description

The 15 scenario configuration files in `configs/scenarios/` are written in DSL format (e.g., `:8080 tcp-service { ... }`), but the gateway binary expects YAML format when using `--config` or `-c` flag.

### Steps to Reproduce

1. Start backend servers on ports 8081-8083
2. Run gateway with DSL scenario file:
   ```bash
   ./target/release/highper-gateway start -c configs/scenarios/scenario-01-layer4-tcp.yaml
   ```
3. Observe error

### Expected Behavior

Gateway should:
- Accept DSL format configuration files
- Parse DSL syntax correctly
- Load the configuration and start proxy

### Actual Behavior

Gateway fails with error:
```
Error: Failed to load configuration

Caused by:
    Failed to parse YAML config: invalid type: string ":8080 tcp-service {",
    expected struct Config at line 5 column 1
```

### Error Messages / Logs

```
[INFO] Starting Highper Gateway v0.1.0
[INFO] Loading configuration from: configs/scenarios/scenario-01-layer4-tcp.yaml
Error: Failed to load configuration
Caused by:
    Failed to parse YAML config: invalid type: string ":8080 tcp-service {",
    expected struct Config at line 5 column 1
```

### Root Cause Analysis

**Current State**:
- Scenario files use DSL syntax (clean, readable)
- Gateway's `-c/--config` flag only parses YAML format
- DSL parser exists (`src/config/dsl_parser.rs`) but not integrated with main config loading

**Code Location**:
- Config loading: `highper-gateway/src/config/loader.rs`
- DSL parser: `highper-gateway/src/config/dsl_parser.rs`
- Main entry point: `highper-gateway/src/main.rs`

### Proposed Solutions

#### Option 1: Add DSL Detection (Recommended)
Add automatic format detection in config loader:
- Check file extension or first line
- If DSL format detected, use DSL parser
- If YAML format detected, use YAML parser

**Files to modify**:
- `highper-gateway/src/config/loader.rs`
- `highper-gateway/src/main.rs`

**Effort**: 2-4 hours
**Impact**: Enables all 15 scenarios to work

#### Option 2: Add `--dsl` Flag
Add separate flag for DSL files:
```bash
./highper-gateway start --dsl configs/scenarios/scenario-01-layer4-tcp.yaml
```

**Effort**: 1-2 hours
**Impact**: Requires updating test scripts

#### Option 3: Convert Scenarios to YAML
Convert all 15 scenario files from DSL to YAML format.

**Effort**: 4-8 hours (15 files × 20-30 min each)
**Impact**: Loses benefit of clean DSL syntax

### Recommended Fix: Option 1

Implement automatic format detection:

```rust
// In src/config/loader.rs

pub fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;

    // Detect format
    if is_dsl_format(&content) {
        // Use DSL parser
        let dsl_ast = parse_dsl(&content)?;
        convert_dsl_to_config(dsl_ast)
    } else {
        // Use YAML parser
        serde_yaml::from_str(&content)
            .context("Failed to parse YAML config")
    }
}

fn is_dsl_format(content: &str) -> bool {
    // Check for DSL syntax patterns
    content.contains(" {") ||
    content.trim_start().starts_with(":") ||
    content.contains("localhost:") && content.contains(" {")
}
```

### Workaround

Until fixed, convert one scenario to YAML format manually for testing:

```bash
# Create YAML version of scenario 01
cat > /tmp/scenario-01-tcp.yaml << 'EOF'
server:
  bind: ["0.0.0.0:8080"]

routes:
  - name: "tcp-service"
    listen: ":8080"
    protocol: "tcp"
    upstreams:
      - "127.0.0.1:8081"
      - "127.0.0.1:8082"
      - "127.0.0.1:8083"
    load_balancer:
      strategy: "round_robin"
    health_check:
      enabled: true
      interval: 10
      timeout: 5

logging:
  level: "info"

metrics:
  enabled: true
  prometheus:
    enabled: true
    port: 9090
EOF

# Test with YAML
./target/release/highper-gateway start -c /tmp/scenario-01-tcp.yaml
```

### Impact

- ❌ **Blocks all 15 scenario tests**
- ❌ **Cannot validate any functionality**
- ❌ **Cannot proceed with load testing**
- ✅ **Not a security issue**
- ✅ **Not a performance issue**

### Priority

**CRITICAL** - Must fix before any scenario testing can proceed.

### Next Steps

1. ✅ Document bug (this file)
2. ⏳ Implement Option 1 (automatic format detection)
3. ⏳ Test with Scenario 01
4. ⏳ Verify all 15 scenarios work
5. ⏳ Update documentation

---

---

## Bug #3: Incomplete DSL Parser Implementation

**Scenario**: All DSL-based scenarios
**Severity**: **Critical** (blocks all DSL testing)
**Date Found**: 2025-12-04
**Status**: 🔴 Open

### Description

The DSL parser (dsl.pest) only implements a subset of directives, but scenario files use many unsupported directives. Both Scenario 01 and 02 fail to parse.

### Unsupported Directives Found

Scenarios use these directives, but they're not in the DSL grammar:

**In scenarios but not in grammar**:
- `keepalive 90s`
- `max_conns 3000000`
- `connect_timeout 5s`
- `idle_timeout 300s`
- `buffer_pool enabled size=16384 pool_size=16777216`
- `backpressure enabled max_conns=3000000 memory_limit=49152mb`
- `metrics prometheus port=9090` (grammar only has: `metrics on/off`)

**Supported directives** (from dsl.pest):
- `proxy` ✅
- `lb` ✅
- `pool` ✅
- `health` ✅
- `tls` ✅
- `cors` ✅
- `websocket` ✅
- `grpc` ✅
- `compress` ✅
- `rate_limit` ✅
- `timeout` ✅
- `headers` ✅

### Impact

- ❌ **All 15 scenarios fail to parse**
- ❌ **Cannot test any scenario with current DSL**
- ❌ **Blocks entire local testing plan**

### Proposed Solutions

#### Option 1: Complete DSL Parser (Long-term)
Implement all missing directives in dsl.pest and dsl_converter.rs
- **Effort**: 2-4 weeks
- **Impact**: Full DSL support

#### Option 2: Convert Scenarios to YAML (Recommended)
Create YAML versions of all scenarios
- **Effort**: 4-8 hours (15 scenarios)
- **Impact**: Immediate testing capability

#### Option 3: Minimal DSL Scenarios
Create minimal DSL configs using only supported directives
- **Effort**: 2-4 hours
- **Impact**: Limited testing (basic features only)

### Recommendation

**Use Option 2**: Convert scenarios to YAML format to unblock testing. The YAML config loader works correctly.

---

## Bug #4: DSL Routing Not Working

**Scenario**: Minimal DSL config
**Severity**: **Critical**
**Date Found**: 2025-12-04
**Status**: 🔴 Open

### Description

Even with a minimal DSL config that parses successfully, routing doesn't work. Gateway returns "No matching route found" for all requests.

### Test Case

**Config** (`/tmp/test-minimal.proxy`):
```
localhost:8080 {
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
    health interval=10s timeout=5s
}

log info
```

**Result**:
- ✅ Gateway starts successfully
- ✅ Backends are running
- ❌ All requests return "No matching route found" (HTTP 404)

### Possible Causes

1. DSL-to-Config converter not creating routes correctly
2. Route matching logic expects different format
3. Host header matching issue (`localhost` vs `127.0.0.1`)

### Impact

- ❌ **Even minimal DSL configs don't work**
- ❌ **Cannot test gateway functionality with DSL**
- ✅ **YAML configs may still work**

---

## Bug Summary

| # | Scenario | Severity | Status | Description |
|---|----------|----------|--------|-------------|
| 1 | All scenarios | Critical | ✅ Fixed | Configuration format mismatch (DSL vs YAML) |
| 2 | Scenario 01 | High | ✅ Fixed | Invalid DSL token `tcp-service` (changed to `tcp`) |
| 3 | All scenarios | Critical | 🔴 Open | Incomplete DSL parser - many directives unsupported |
| 4 | DSL routing | Critical | 🔴 Open | DSL-to-Config conversion doesn't create working routes |

---

**Total Bugs**: 4
**Critical**: 2 (open) + 1 (fixed)
**High**: 1 (fixed)
**Medium**: 0
**Low**: 0

**Status**: DSL configuration path is blocked. Recommend switching to YAML.

---

*Last Updated: 2025-12-04 05:00*
