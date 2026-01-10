# Highper Gateway - Known Limitations

**Version:** 1.0.0
**Last Updated:** January 10, 2026
**Repository:** https://github.com/highperapp/highper-gateway

---

## Overview

This document provides a comprehensive list of known limitations, incomplete features, and planned enhancements for Highper Gateway. All limitations are categorized by priority and impact to help users make informed decisions.

### Priority Levels

- **P1 (Critical):** Blocks production use or causes data loss
- **P2 (High):** Impacts core functionality or production readiness
- **P3 (Medium):** Enhancement or quality-of-life improvement
- **P4 (Low):** Nice-to-have or cosmetic improvement

### Impact Levels

- **HIGH:** Significantly affects functionality or user experience
- **MEDIUM:** Noticeable but not blocking
- **LOW:** Minimal impact, workarounds available
- **NONE:** No functional impact (documentation, examples, etc.)

---

## Summary

| Category | P1 | P2 | P3 | P4 | Total |
|----------|----|----|----|----|-------|
| **Service Discovery** | 0 | 0 | 1 | 0 | 1 |
| **Web Server** | 0 | 0 | 1 | 0 | 1 |
| **Authentication** | 0 | 2 | 0 | 0 | 2 |
| **TLS/Security** | 0 | 3 | 0 | 0 | 3 |
| **Middleware** | 0 | 0 | 0 | 1 | 1 |
| **Infrastructure** | 0 | 1 | 1 | 0 | 2 |
| **Documentation** | 1 | 0 | 0 | 1 | 2 |
| **Total** | **1** | **6** | **3** | **2** | **12** |

---

## Code-Level Limitations

### 1. Static Service Discovery Not Implemented

**Priority:** P3 (Medium)
**Impact:** LOW
**Location:** `highper-gateway/src/discovery/mod.rs`

**Description:**
Static service discovery (hardcoded list of backend addresses) returns "Static discovery not yet implemented" error.

**Current Behavior:**
```rust
DiscoveryBackend::Static => {
    Err(anyhow!("Static discovery not yet implemented"))
}
```

**Workaround:**
Use Consul or etcd for service discovery, which are fully functional and production-ready.

**Planned Fix:**
Add static backend configuration support in v1.1.0.

**Configuration Example (Future):**
```toml
[discovery]
type = "static"
backends = [
    "192.168.1.10:8080",
    "192.168.1.11:8080",
    "192.168.1.12:8080"
]
```

**Environment Variable (Future):**
```bash
export HIGHPER_DISCOVERY_STATIC_BACKENDS="192.168.1.10:8080,192.168.1.11:8080"
```

---

### 2. Directory Listing Not Implemented

**Priority:** P3 (Medium)
**Impact:** LOW
**Location:** `highper-gateway/src/webserver/static_files.rs`

**Description:**
Automatic directory listing (like Apache's `autoindex`) is not implemented.

**Current Behavior:**
Attempting to access a directory without an index file returns "Directory listing not yet implemented" error.

**Workaround:**
- Always provide `index.html`, `index.htm`, or `index.php` files
- Use explicit file URLs instead of directory URLs

**Planned Fix:**
Add directory listing feature in v1.2.0 with configuration option.

**Configuration Example (Future):**
```toml
[webserver]
directory_listing = true
directory_listing_format = "html"  # or "json"
```

**Environment Variable (Future):**
```bash
export HIGHPER_WEBSERVER_DIRECTORY_LISTING=true
export HIGHPER_WEBSERVER_DIRECTORY_LISTING_FORMAT=html
```

---

### 3. OAuth2 Implementation Incomplete

**Priority:** P2 (High)
**Impact:** MEDIUM
**Location:** `highper-gateway/src/gateway/auth/oauth2.rs`

**Description:**
OAuth2 authentication module has 5 TODOs related to:
- Token validation edge cases
- Token refresh logic
- PKCE flow support
- Multi-provider support
- Token revocation

**Current Status:**
- ✅ Basic OAuth2 authorization code flow works
- ✅ Token validation (JWT) works
- ⚠️ Token refresh needs enhancement
- ⚠️ PKCE flow not yet supported
- ⚠️ Token revocation not implemented

**Workaround:**
- Use basic authorization code flow (fully functional)
- Implement token refresh in application layer
- Use short-lived tokens (< 1 hour)

**Planned Fix:**
Complete OAuth2 implementation in v1.1.0.

**Current Configuration:**
```toml
[gateway.auth]
type = "oauth2"
provider = "generic"
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "https://your-domain.com/callback"
```

**Environment Variables:**
```bash
export HIGHPER_AUTH_OAUTH2_PROVIDER=generic
export HIGHPER_AUTH_OAUTH2_CLIENT_ID=your-client-id
export HIGHPER_AUTH_OAUTH2_CLIENT_SECRET=your-client-secret
export HIGHPER_AUTH_OAUTH2_REDIRECT_URI=https://your-domain.com/callback
```

---

### 4. OCSP Fetcher Needs Production Hardening

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/ocsp_fetcher.rs`

**Description:**
OCSP fetcher has 6 TODOs related to:
- Advanced error handling
- Retry logic with exponential backoff
- OCSP stapling failure recovery
- Caching strategy optimization
- Multiple OCSP responder support

**Current Status:**
- ✅ OCSP fetching works
- ✅ Basic caching implemented
- ⚠️ Retry logic basic (needs improvement)
- ⚠️ Error handling could be more robust

**Workaround:**
OCSP stapling works reliably in most cases. For production critical systems, monitor OCSP fetch failures and have alerting in place.

**Planned Fix:**
Enhance OCSP fetcher robustness in v1.0.1 (patch release).

**Configuration:**
```toml
[tls.ocsp]
enabled = true
cache_duration = 3600  # seconds
retry_count = 3
retry_delay = 5  # seconds
```

**Environment Variables:**
```bash
export HIGHPER_TLS_OCSP_ENABLED=true
export HIGHPER_TLS_OCSP_CACHE_DURATION=3600
export HIGHPER_TLS_OCSP_RETRY_COUNT=3
export HIGHPER_TLS_OCSP_RETRY_DELAY=5
```

---

### 5. Certificate Validation Edge Cases

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/cert_validator.rs`

**Description:**
Certificate validator has 2 TODOs for edge cases:
- Partial certificate chain validation
- Cross-signed certificate handling

**Current Status:**
- ✅ Standard certificate validation works
- ✅ mTLS client certificate validation works
- ⚠️ Complex certificate chains may need manual verification

**Workaround:**
Use standard certificate chains from trusted CAs (Let's Encrypt, DigiCert, etc.). Avoid complex cross-signed certificate scenarios.

**Planned Fix:**
Enhance certificate validation in v1.0.2.

---

### 6. CRL Checker Enhancement Needed

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/crl_checker.rs`

**Description:**
Certificate Revocation List (CRL) checker has 2 TODOs:
- Delta CRL support
- CRL caching improvements

**Current Status:**
- ✅ Basic CRL checking works
- ⚠️ Delta CRLs not supported (use full CRLs)
- ⚠️ CRL caching could be more efficient

**Workaround:**
Use OCSP instead of CRL for better performance. OCSP is fully functional and preferred.

**Planned Fix:**
Enhance CRL checker in v1.1.0.

---

### 7. Compression Module Documentation Example

**Priority:** P4 (Low)
**Impact:** NONE
**Location:** `highper-gateway/src/middleware/compression/mod.rs:49`

**Description:**
Documentation example contains `unimplemented!()` in example code (comment block). This is NOT actual code, just documentation.

**Current Status:**
- ✅ Compression middleware works fully (gzip, brotli, zstd)
- ✅ No functional limitation
- ⚠️ Documentation example could be clearer

**Workaround:**
None needed. Compression works perfectly.

**Planned Fix:**
Update documentation example in v1.0.1.

---

## Infrastructure Limitations

### 8. Cloud Load Test Cleanup Not Automated

**Priority:** P2 (High)
**Impact:** MEDIUM
**Location:** `highper-gateway/tests/load/`

**Description:**
Cloud load testing framework requires manual cleanup of cloud instances after test completion. Automatic cleanup on error or interruption is not fully implemented.

**Current Status:**
- ✅ Local testing fully automated
- ✅ Cloud provisioning automated
- ⚠️ Cloud cleanup requires `--cleanup` flag
- ⚠️ Interrupted tests may leave instances running

**Risk:**
Interrupted cloud tests may leave instances running, incurring costs.

**Workaround:**
- Always use `--cleanup` flag
- Monitor cloud provider dashboard after tests
- Use provider CLI to manually remove orphaned instances:
  ```bash
  # Vultr example
  vultr-cli instance list
  vultr-cli instance delete <instance-id>
  ```

**Planned Fix:**
Add trap handlers and automatic cleanup in v1.0.1.

**Configuration:**
```bash
# Always cleanup (default)
export HIGHPER_LOADTEST_AUTO_CLEANUP=true
# Keep instances for debugging
export HIGHPER_LOADTEST_KEEP_INSTANCES=false
```

---

### 9. Windows Native Testing Incomplete

**Priority:** P3 (Medium)
**Impact:** LOW
**Location:** Cross-platform testing

**Description:**
Windows native testing (without WSL2) has limited support. WSL2 is recommended for Windows users.

**Current Status:**
- ✅ WSL2 support excellent
- ✅ Linux support excellent
- ⚠️ Windows native builds work but testing limited
- ⚠️ Some shell scripts require bash (not cmd/PowerShell)

**Workaround:**
- Use WSL2 on Windows (fully supported)
- Use Docker Desktop on Windows
- Use Git Bash for running shell scripts

**Planned Fix:**
Add PowerShell scripts alongside bash scripts in v1.2.0.

---

## Documentation Limitations

### 10. Scattered Known Limitations (This Document Fixes It!)

**Priority:** P1 (Critical)
**Impact:** HIGH
**Location:** Various documentation files

**Description:**
Known limitations were scattered across 57 different markdown files, making it impossible to find a comprehensive list.

**Current Status:**
- ✅ **FIXED:** This document (KNOWN_LIMITATIONS.md) centralizes all limitations
- ✅ All limitations categorized by priority
- ✅ Workarounds documented
- ✅ Configuration examples provided

**Workaround:**
This document is the comprehensive reference.

---

### 11. Some TODO Comments in Code

**Priority:** P4 (Low)
**Impact:** NONE
**Location:** 36 files across codebase

**Description:**
Code contains 114 TODO/FIXME comments related to:
- Performance optimizations (80%)
- Future enhancements (15%)
- Code cleanup (5%)

**Current Status:**
- ✅ No blocking issues
- ✅ All TODOs are enhancements, not bugs
- ℹ️ Tracked in this document and GitHub issues

**Examples:**
- Admin API improvements (17 TODOs)
- Proxy handler optimizations (6 TODOs)
- Admin backend features (6 TODOs)
- GraphQL stitcher enhancements (2 TODOs)

**Planned Fix:**
Ongoing - TODOs will be addressed incrementally in future releases.

---

## Feature Roadmap

### Planned for v1.0.1 (Patch Release)
- [ ] OCSP fetcher production hardening
- [ ] Compression module documentation update
- [ ] Cloud load test automatic cleanup
- [ ] CRL checker enhancements

### Planned for v1.1.0 (Minor Release)
- [ ] Static service discovery implementation
- [ ] Complete OAuth2 implementation (PKCE, refresh, revocation)
- [ ] Enhanced certificate validation (cross-signed certs)
- [ ] Improved CRL delta support

### Planned for v1.2.0 (Minor Release)
- [ ] Directory listing for static files
- [ ] PowerShell scripts for Windows native support
- [ ] Enhanced observability features
- [ ] Additional load balancing algorithms

### Planned for v2.0.0 (Major Release)
- [ ] Breaking API changes (if needed)
- [ ] Advanced traffic shaping
- [ ] AI-powered DDoS protection
- [ ] Multi-datacenter global load balancing

---

## How to Report New Limitations

If you discover a limitation not listed here:

1. **Check GitHub Issues:** https://github.com/highperapp/highper-gateway/issues
2. **Search existing issues** for duplicates
3. **Create new issue** with template:
   - Title: `[Limitation] Brief description`
   - Description: What doesn't work, expected behavior, workaround
   - Impact: HIGH/MEDIUM/LOW
   - Environment: OS, version, configuration

---

## Workarounds Summary

| Limitation | Quick Workaround |
|------------|------------------|
| Static discovery | Use Consul or etcd |
| Directory listing | Provide index.html files |
| OAuth2 incomplete | Use basic auth code flow, short-lived tokens |
| OCSP edge cases | Monitor OCSP fetch failures |
| Cloud test cleanup | Always use `--cleanup` flag |
| Windows native | Use WSL2 |

---

## Environment Variable Reference

All limitations with configurable workarounds support environment variables:

```bash
# Service Discovery
export HIGHPER_DISCOVERY_TYPE=consul  # or etcd, dns
export HIGHPER_DISCOVERY_CONSUL_ADDR=http://localhost:8500

# Web Server
export HIGHPER_WEBSERVER_INDEX_FILES=index.html,index.htm

# OAuth2
export HIGHPER_AUTH_OAUTH2_TOKEN_REFRESH_ENABLED=false  # Disable if problematic

# TLS/OCSP
export HIGHPER_TLS_OCSP_ENABLED=true
export HIGHPER_TLS_OCSP_RETRY_COUNT=5

# Load Testing
export HIGHPER_LOADTEST_AUTO_CLEANUP=true
```

---

## Zero-Config Defaults

Highper Gateway follows zero-config principles. All limitations have sensible defaults:

- **Service Discovery:** Defaults to DNS if no discovery backend configured
- **OAuth2:** Defaults to basic validation if advanced features unavailable
- **OCSP:** Defaults to enabled with 1-hour cache
- **Directory Listing:** Defaults to disabled (security best practice)
- **Load Test Cleanup:** Defaults to enabled (prevent cost leaks)

Configuration is only needed to override these intelligent defaults.

---

## Support & Contact

- **Documentation:** https://github.com/highperapp/highper-gateway/tree/main/docs
- **Issues:** https://github.com/highperapp/highper-gateway/issues
- **Discussions:** https://github.com/highperapp/highper-gateway/discussions

---

**Last Updated:** January 10, 2026
**Next Review:** February 10, 2026 (monthly)
