# Highper Gateway - Known Limitations

**Version:** 1.1.0
**Last Updated:** January 24, 2026
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

| Category | P1 | P2 | P3 | P4 | Total | Fixed in v1.1.0 |
|----------|----|----|----|----|-------|-----------------|
| **Service Discovery** | 0 | 0 | 0 | 0 | 0 | ✅ Static Discovery |
| **Web Server** | 0 | 0 | 1 | 0 | 1 | |
| **Authentication** | 0 | 0 | 0 | 0 | 0 | ✅ OAuth2 Complete |
| **TLS/Security** | 0 | 2 | 0 | 0 | 2 | ✅ OCSP Hardened |
| **Middleware** | 0 | 0 | 0 | 1 | 1 | |
| **Infrastructure** | 0 | 0 | 1 | 0 | 1 | ✅ Cloud Cleanup |
| **Documentation** | 0 | 0 | 0 | 1 | 1 | ✅ Centralized Docs |
| **Total** | **0** | **2** | **2** | **2** | **6** | **5 Fixed** |

### Fixes in v1.1.0

- ✅ **Static Service Discovery** - Now fully implemented with health checks
- ✅ **OAuth2 Implementation** - Token refresh, PKCE, revocation, 9 providers
- ✅ **OCSP Fetcher Hardened** - Exponential backoff, multiple responders, stale fallback
- ✅ **Cloud Test Cleanup** - Automatic trap handlers for all exit scenarios
- ✅ **Documentation Centralized** - This document now the single source of truth

---

## Code-Level Limitations

### 1. ~~Static Service Discovery Not Implemented~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/discovery/static.rs`

**Description:**
Static service discovery is now fully implemented with:
- Hardcoded list of backend addresses
- Automatic TCP health checking with configurable intervals
- Dynamic backend registration/deregistration
- Comprehensive unit tests (12 tests)

**Configuration Example:**
```toml
[discovery]
type = "static"
health_check_enabled = true
static_health_check_interval = 10
static_health_check_timeout = 3

[[discovery.static_backends]]
id = "backend-1"
service_name = "my-service"
address = "192.168.1.10"
port = 8080
metadata = { zone = "us-east-1a", version = "v1.2.0" }
```

**Environment Variable:**
```bash
export HIGHPER_DISCOVERY_TYPE=static
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

### 3. ~~OAuth2 Implementation Incomplete~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/gateway/auth/oauth2.rs`, `oauth2_providers.rs`

**Description:**
OAuth2 authentication is now fully implemented with:
- ✅ Basic OAuth2 authorization code flow
- ✅ Token validation (JWT)
- ✅ Token refresh with automatic expiry checking
- ✅ PKCE flow support (SHA-256)
- ✅ Token revocation (RFC 7009)
- ✅ Multi-provider support (9 providers)

**Supported Providers:**
| Provider | Auth URL | Token URL | Revocation | Default Scopes |
|----------|----------|-----------|------------|----------------|
| Google | ✅ | ✅ | ✅ | openid, email, profile |
| GitHub | ✅ | ✅ | ✅ | user, user:email |
| Microsoft | ✅ | ✅ | ✅ | openid, profile, email |
| GitLab | ✅ | ✅ | ✅ | read_user, email |
| Discord | ✅ | ✅ | ✅ | identify, email |
| Facebook | ✅ | ✅ | ❌ | public_profile, email |
| Twitter | ✅ | ✅ | ✅ | tweet.read, users.read |
| Okta | ✅ | ✅ | ✅ | openid, profile, email |
| Auth0 | ✅ | ✅ | ✅ | openid, profile, email |

**Configuration Example:**
```toml
[gateway.auth]
type = "oauth2"
provider = "google"  # Auto-configures URLs and scopes
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "https://your-domain.com/callback"
revocation_url = "https://oauth2.googleapis.com/revoke"

# Token refresh settings
auto_refresh_enabled = true
refresh_threshold_secs = 300  # Refresh 5 minutes before expiry
```

---

### 4. ~~OCSP Fetcher Needs Production Hardening~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/tls/ocsp_fetcher.rs`

**Description:**
OCSP fetcher is now production-hardened with:
- ✅ Exponential backoff retry (configurable initial delay, max delay, multiplier)
- ✅ Multiple OCSP responder support (fallback URLs)
- ✅ Graceful degradation with stale response fallback
- ✅ Configurable timeouts and retry limits
- ✅ Comprehensive unit tests (9 tests)

**Configuration:**
```toml
[tls.ocsp]
timeout_secs = 10
max_retries = 3
initial_retry_delay_ms = 500
max_retry_delay_ms = 30000
retry_multiplier = 2.0
fallback_responder_urls = [
    "http://ocsp.example.com",
    "http://ocsp2.example.com"
]
allow_stale_responses = true
max_stale_age_secs = 86400  # 24 hours
```

**Behavior:**
1. Tries primary OCSP responder (from cert AIA) with exponential backoff
2. Falls back to configured `fallback_responder_urls` if primary fails
3. If all responders fail and `allow_stale_responses` is true, returns cached stale response
4. Stale responses are used only if within `max_stale_age_secs`

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

### 8. ~~Cloud Load Test Cleanup Not Automated~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/tests/load/lib/cloud-cleanup.sh`

**Description:**
Cloud load testing framework now has automatic cleanup:
- ✅ Trap handlers for EXIT, INT, TERM, ERR signals
- ✅ Automatic instance tracking and cleanup
- ✅ Multi-provider support (Vultr, AWS, DigitalOcean)
- ✅ Resource cleanup (volumes, IPs, etc.)
- ✅ SKIP_CLEANUP option for debugging

**Behavior:**
- Instances are automatically tracked when created
- Cleanup runs on ANY exit (success, error, Ctrl+C, kill signal)
- Failed cleanups are logged but don't prevent other cleanups

**Usage:**
```bash
# Run cloud test (cleanup happens automatically)
./cloud-test-runner.sh vultr 02

# Keep instances for debugging
SKIP_CLEANUP=true ./cloud-test-runner.sh vultr 02
```

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

## Feature Improvement Roadmap

For a comprehensive comparison with industry leaders and planned improvements, see:

- **Feature Comparison Matrix:** [deploy/docs/FEATURE_COMPARISON_MATRIX.md](deploy/docs/FEATURE_COMPARISON_MATRIX.md)
- **Feature Improvement Roadmap:** [docs/FEATURE_IMPROVEMENT_ROADMAP.md](docs/FEATURE_IMPROVEMENT_ROADMAP.md)

### Key Planned Improvements

| Phase | Features | Timeline |
|-------|----------|----------|
| **Phase 1** | Automatic HTTPS (ACME), API-based Config, Least Response Time LB, Connection Draining, Zero-Config Mode, OpenTelemetry | 6-8 weeks |
| **Phase 2** | Slow Start, Disk Cache, Status Dashboard, Request Validation, etcd Discovery, JSON Config, Stick Tables | 8-12 weeks |
| **Phase 3** | Response Aggregation, BFF Pattern, Database Protocol Awareness | 6-8 weeks |

---

## Support & Contact

- **Documentation:** https://github.com/highperapp/highper-gateway/tree/main/docs
- **Issues:** https://github.com/highperapp/highper-gateway/issues
- **Discussions:** https://github.com/highperapp/highper-gateway/discussions

---

**Last Updated:** January 24, 2026
**Next Review:** February 24, 2026 (monthly)
