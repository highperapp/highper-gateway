# DSL Parsing Findings
## Date: December 5, 2025

## Root Cause Identified

**Issue**: Scenarios 04-15 fail validation due to TLS syntax inconsistency

### Working TLS Syntax (Scenario 03)
```
tls "/path/to/cert" "/path/to/key"
```

### Non-Working TLS Syntax (Scenarios 04-15)
```
tls cert=/path/to/cert key=/path/to/key
```

### Verification
- Created minimal test config with `cert=` syntax: ❌ FAILED
- Created same config with quoted paths: ✅ PASSED

## Grammar Analysis

The Pest grammar in `highper-gateway/src/config/dsl.pest` shows:
```pest
tls_directive = {
    "tls" ~ (
        "internal"                             // Self-signed
      | email                                  // Auto ACME
      | (("cert=" ~ quoted_string | quoted_string) ~ ("key=" ~ quoted_string | quoted_string))       // Cert + key files
    )? ~ newline
}
```

**Grammar supports both syntaxes** but parsing fails with `cert=` format. This appears to be a grammar precedence or ordering issue in the Pest parser.

## Complete DSL Feature Status

### ✅ Fully Implemented in AST/Parser
- TLS configuration (basic syntax)
- TLS protocols
- CORS
- WebSocket directives
- gRPC directives
- HTTP/2 configuration
- HTTP/3 configuration
- QUIC configuration
- Header manipulation (add/remove/passthrough)
- Circuit breaker
- Compression with levels
- Rate limiting with per_ip
- Request timeout
- All timeout types

### ❌ Implementation Gaps
1. **TLS Syntax**: `cert=` and `key=` parameters don't parse correctly
2. **Protocol Support**: HTTP/3, WebSocket, gRPC protocols not functionally implemented (AST exists, runtime missing)
3. **Advanced Features**: CORS, circuit breaker, header manipulation have AST but no runtime implementation

## Fix Options

### Option A: Fix Grammar (Complex)
- Debug Pest grammar precedence
- Ensure `cert=` syntax parses correctly
- Estimated effort: 2-4 hours

### Option B: Update All Scenario Configs (Practical) ⭐ RECOMMENDED
- Replace `tls cert=X key=Y` with `tls "X" "Y"` in all scenarios
- Remove unimplemented protocol directives
- Keep only working features
- Estimated effort: 30 minutes

### Option C: Hybrid Approach
- Fix scenarios 04-08 with simplified configs
- Document scenarios 09-15 as future work
- Focus on core gateway testing
- Estimated effort: 1 hour

## Recommended Action Plan

1. **Immediate** (30 min):
   - Fix TLS syntax in scenarios 04-15
   - Remove unimplemented directives (HTTP/3, WebSocket, gRPC protocol features)
   - Keep implemented features (compression, rate limiting, health checks)

2. **Short Term** (2-4 hours):
   - Implement missing runtime features (CORS basic support, header manipulation)
   - Test scenarios 04-08 with real backends

3. **Long Term** (weeks):
   - HTTP/3 protocol support
   - WebSocket proxying
   - gRPC proxying
   - Full circuit breaker implementation

## Current Testing Status

- ✅ Scenario 01 (TCP): Validated and runtime tested
- ✅ Scenario 02 (HTTP): Validated and runtime tested
- ✅ Scenario 03 (HTTPS/TLS): Validated and runtime tested
- ❌ Scenarios 04-15: Blocked by TLS syntax issue

## Files Involved

- `highper-gateway/src/config/dsl.pest` - Grammar definitions
- `highper-gateway/src/config/dsl_parser.rs` - Parser implementation (904 lines)
- `highper-gateway/src/config/dsl_ast.rs` - AST definitions (483 lines)
- `highper-gateway/src/config/dsl_converter.rs` - DSL to internal config conversion
- `configs/scenarios/scenario-*.proxy` - All scenario configuration files

## Next Steps

**Recommendation**: Proceed with Option B - fix all scenario configs to use working syntax. This unblocks testing immediately and allows validation of scenarios 04-15 with implemented features.

**Command to fix**:
```bash
sed -i 's/tls cert=\([^ ]*\) key=\([^ ]*\)/tls "\1" "\2"/g' configs/scenarios/scenario-*.proxy
```

This will update all TLS directives to use the working syntax format.
