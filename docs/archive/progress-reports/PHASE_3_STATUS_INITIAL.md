# Phase 3.1 Initial Status Assessment
**Date**: 2025-12-14
**Task**: Parse per-route mTLS config from DSL

## Current Infrastructure Status

### ✅ What Already Exists

1. **Configuration Structures** ✅
   - `RouteMtlsPolicy` struct defined (schema.rs:1136-1151)
   - Fields: verification_mode, allowed_subjects, allowed_issuers, allowed_serials, allowed_fingerprints
   - `RouteConfig.mtls: Option<RouteMtlsPolicy>` field exists (schema.rs:635)

2. **Middleware Implementation** ✅
   - `MtlsMiddleware` in src/middleware/mtls.rs
   - `check_policy()` method accepts `Option<&RouteMtlsPolicy>`
   - Supports per-route policy overrides
   - Certificate fingerprint whitelisting working

3. **Global mTLS Config** ✅
   - `MtlsConfig` struct for global settings (schema.rs:1016)
   - CA cert path, verification mode, CRL path, OCSP config

### ❌ What's Missing

1. **DSL Grammar** ❌
   - No `mtls_directive` in dsl.pest (checked lines 95-129)
   - Need to add grammar rules for mTLS block parsing

2. **DSL Parser** ❌
   - No parsing logic for mTLS directives
   - Need to add to dsl_parser.rs

3. **DSL Converter** ❌
   - Need to wire DSL AST to RouteConfig.mtls field
   - Add conversion logic in dsl_converter.rs

---

## Implementation Approach

### Option A: Full DSL Implementation (2-3 hours)
Add complete mTLS DSL support:

**Grammar Addition** (dsl.pest):
```pest
// Add to directive list:
directive = {
    ...existing...
  | mtls_directive
}

// mTLS directive
mtls_directive = {
    "mtls" ~ mtls_block ~ newline
}

mtls_block = {
    "{" ~ newline* ~
    (newline* ~ mtls_option)* ~ newline* ~
    "}" ~ newline*
}

mtls_option = {
    mtls_verify_directive
  | mtls_ca_cert_directive  
  | mtls_allow_subject_directive
  | mtls_allow_issuer_directive
  | mtls_allow_fingerprint_directive
}

mtls_verify_directive = { "verify" ~ mtls_verify_mode ~ newline }
mtls_verify_mode = { "required" | "optional" | "optional_no_ca" }

mtls_ca_cert_directive = { "ca_cert" ~ quoted_string ~ newline }
mtls_allow_subject_directive = { "allow_subject" ~ quoted_string ~ newline }
mtls_allow_issuer_directive = { "allow_issuer" ~ quoted_string ~ newline }
mtls_allow_fingerprint_directive = { "allow_fingerprint" ~ quoted_string ~ newline }
```

**Usage Example**:
```
route /api/secure {
    proxy secure-backend:8080
    
    mtls {
        verify required
        ca_cert "/path/to/ca.crt"
        allow_subject "CN=client.example.com"
        allow_fingerprint "sha256:abc123..."
    }
}
```

**Tasks**:
1. Update dsl.pest with grammar rules (30 min)
2. Update dsl_ast.rs with AST structures (30 min)
3. Update dsl_parser.rs with parsing logic (45 min)
4. Update dsl_converter.rs to wire to RouteConfig (45 min)
5. Test with sample DSL configs (30 min)

**Total**: 2.5-3 hours

---

### Option B: YAML-Only for Now (0 hours)
Skip DSL implementation and use YAML configuration only:

**Rationale**:
- RouteMtlsPolicy already works in YAML
- YAML schema.rs supports it
- Middleware already functional
- Can add DSL support later if needed

**YAML Usage** (already works):
```yaml
routes:
  - name: "secure_api"
    match:
      paths: ["/api/secure"]
    upstream: "secure_backend"
    mtls:
      verification_mode: required
      allowed_subjects: ["CN=client.example.com"]
      allowed_fingerprints: ["sha256:abc123..."]
```

**Tasks**: None - already implemented!

---

## Recommendation

**I recommend Option B (YAML-only)** for these reasons:

1. **Already Working**: Per-route mTLS is fully functional via YAML
2. **Time Savings**: Saves 2-3 hours to focus on higher-priority tasks
3. **DSL is Optional**: Most users will use YAML anyway
4. **Can Add Later**: DSL support can be added in future if needed

**Next immediate tasks** would be:
- ✅ Skip DSL parsing (use YAML)
- Move to Task 2: Wire mTLS policy to middleware (already done!)
- Move to Task 3: OCSP stapling implementation (high value!)
- Move to Task 4: CRL checking implementation (high value!)

This approach lets us focus on the security enhancements (OCSP, CRL) which are the real value-add features.

---

## Alternative: Comprehensive Approach

If you want **complete DSL support**, I can implement Option A, which would take 2-3 hours and provide a nice DSL syntax for mTLS configuration.

---

**Decision Point**: Which approach would you prefer?
- **A**: Implement full DSL support for mTLS (2-3 hours)
- **B**: Use existing YAML support, skip to OCSP/CRL (saves time)

Let me know and I'll proceed accordingly!
