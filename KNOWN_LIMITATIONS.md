# Highper Gateway - Known Limitations

**Version:** 1.1.0 (superseded for release tracking)
**Last Updated:** January 24, 2026 (revised 2026-05-02)
**Repository:** https://github.com/highperapp/highper-gateway

---

> ## ⚠️ Reconciliation banner — refreshed 2026-05-16
>
> **This document's "only 1 remaining item" summary (line 42) is OUT OF DATE for release tracking.**
>
> The current authoritative source for v1.0 GA tracking is the v2 roadmap at
> [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md), organised per-UC
> (UC1–UC15) with cross-cutting workstreams in §3. As of 2026-05-16, **4 of the
> 14 prior B-blockers are closed** (rate-limit safety, GraphQL depth/complexity,
> PostgreSQL pool validation, bounded channels); the remaining 10 are folded
> into the per-UC §2 sections. Approximate remaining effort: ~180 engineer-days
> (~10 calendar weeks at two-engineer pace).
>
> The earlier `B1`–`B14` list and the 2026-05-02 audit that founded it are
> preserved in the archive at
> [`docs/planning/archive/2026-05-16-v1/`](docs/planning/archive/2026-05-16-v1/)
> with an `ARCHIVE_MANIFEST.md` cross-reference.
>
> The "Fixed in v1.1.0" tables below remain accurate for the items they describe;
> they do **not** describe everything blocking v1.0 GA.
>
> The list of items below is preserved as historical reference. **Do not** read this
> document as the current limitation set.

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
| **Web Server** | 0 | 0 | 0 | 0 | 0 | ✅ Directory Listing |
| **Authentication** | 0 | 0 | 0 | 0 | 0 | ✅ OAuth2 Complete |
| **TLS/Security** | 0 | 0 | 0 | 0 | 0 | ✅ OCSP, CRL, Cert Val |
| **Middleware** | 0 | 0 | 0 | 0 | 0 | ✅ Compression Docs |
| **Infrastructure** | 0 | 0 | 1 | 0 | 1 | ✅ Cloud Cleanup |
| **Documentation** | 0 | 0 | 0 | 0 | 0 | ✅ Centralized Docs |
| **Total** | **0** | **0** | **1** | **0** | **1** | **10 Fixed** |

**Only 1 remaining item:** Windows Native Testing (P3) - Use WSL2 as workaround

### Fixes in v1.1.0

**Phase 1 (Critical):**
- ✅ **Static Service Discovery** - Fully implemented with health checks
- ✅ **OAuth2 Implementation** - Token refresh, PKCE, revocation, 9 providers
- ✅ **OCSP Fetcher Hardened** - Exponential backoff, multiple responders, stale fallback
- ✅ **Cloud Test Cleanup** - Automatic trap handlers for all exit scenarios

**Phase 2 (Quality):**
- ✅ **Directory Listing** - HTML/JSON formats, hidden file filtering
- ✅ **Certificate Validation** - Partial chains, cross-signed certs, AIA fetching
- ✅ **CRL Checker Enhanced** - Delta CRL, LRU cache, fallback URLs

**Documentation:**
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

### 2. ~~Directory Listing Not Implemented~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/webserver/static_files.rs`

**Description:**
Directory listing is now fully implemented with:
- ✅ HTML format with modern styled layout and icons
- ✅ JSON format for machine consumption
- ✅ Hidden files filtering (`show_hidden_files` config)
- ✅ Parent directory navigation
- ✅ File size and modification time display
- ✅ Comprehensive unit tests (8 tests)

**Configuration Example:**
```toml
[webserver]
enable_static_files = true
document_root = "/var/www/html"
directory_listing = true
directory_listing_format = "html"  # or "json"
show_hidden_files = false
```

**Environment Variable:**
```bash
export HIGHPER_WEBSERVER_DIRECTORY_LISTING=true
export HIGHPER_WEBSERVER_DIRECTORY_LISTING_FORMAT=html
export HIGHPER_WEBSERVER_SHOW_HIDDEN_FILES=false
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

### 5. ~~Certificate Validation Edge Cases~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/tls/cert_validator.rs`

**Description:**
Certificate validator now handles all edge cases:
- ✅ Partial chain handling (auto-builds from AIA)
- ✅ Cross-signed certificate support
- ✅ Self-signed certificate option
- ✅ Configurable max chain depth
- ✅ Expiration warnings
- ✅ Comprehensive unit tests

**Configuration:**
```toml
[tls.cert_validation]
max_chain_depth = 10
allow_partial_chains = true
expiry_warning_days = 30
allow_self_signed = false
fetch_timeout_secs = 10
```

**Features:**
- Automatically fetches intermediate certificates from AIA extension
- Validates chain relationships (issuer matches subject)
- Warns when certificates expire soon
- Supports loop detection in certificate chains

---

### 6. ~~CRL Checker Enhancement Needed~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/tls/crl_checker.rs`

**Description:**
CRL checker is now production-ready with:
- ✅ Delta CRL support (merge with base CRL)
- ✅ LRU cache with configurable max entries
- ✅ Multiple CRL distribution points (fallback URLs)
- ✅ Exponential backoff retry
- ✅ Graceful degradation with stale fallback
- ✅ Hard/soft fail modes
- ✅ Per-issuer CRL caching
- ✅ Comprehensive unit tests

**Configuration:**
```toml
[tls.crl]
crl_url = "http://crl.example.com/ca.crl"
fallback_urls = ["http://crl2.example.com/ca.crl"]
refresh_interval_secs = 3600
timeout_secs = 10
max_retries = 3
initial_retry_delay_ms = 500
max_cache_entries = 100
enable_delta_crl = true
allow_stale_crl = true
max_stale_age_secs = 86400
hard_fail = false
```

---

### 7. ~~Compression Module Documentation Example~~ ✅ FIXED in v1.1.0

**Status:** ✅ **FIXED**
**Location:** `highper-gateway/src/middleware/compression/mod.rs`

**Description:**
Documentation example has been updated with a complete working example showing how to implement a custom compressor.

**Current Status:**
- ✅ Compression middleware works fully (gzip, brotli, zstd, deflate)
- ✅ Documentation examples are complete and functional
- ✅ Custom compressor example provided

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
- ❌ Windows native builds do **not** work. `io-uring` is an unconditional dependency and does
  not type-check on Windows at all (`cannot find function syscall`, `MSG_TRUNC` missing from
  `libc`), so `cargo check` fails there with any flag combination. Separately, the default
  `jemalloc` feature fails its autoconf step on Windows. Measured 2026-09-11,
  x86_64-pc-windows-msvc, rustc 1.94.1
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

### Completed in v1.0.1 (Patch Release)
- [x] OCSP fetcher production hardening
- [x] Compression module documentation update
- [x] Cloud load test automatic cleanup
- [x] CRL checker enhancements

### Completed in v1.1.0 (Minor Release)
- [x] Static service discovery implementation
- [x] Complete OAuth2 implementation (PKCE, refresh, revocation)
- [x] Enhanced certificate validation (cross-signed certs)
- [x] Improved CRL delta support
- [x] Directory listing for static files
- [x] Stick Tables (HAProxy-style session persistence)
- [x] BFF Pattern (Backend for Frontend routing)
- [x] Response Aggregation

### Planned for v1.2.0 (Minor Release)
- [ ] PowerShell scripts for Windows native support
- [ ] Enhanced observability dashboards
- [ ] Plugin marketplace integration
- [ ] WASM plugin SDK improvements

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
- **Feature Improvement Roadmap:** archived at [docs/planning/archive/2026-05-16-v1/FEATURE_IMPROVEMENT_ROADMAP.md](docs/planning/archive/2026-05-16-v1/FEATURE_IMPROVEMENT_ROADMAP.md); current planning lives in [docs/planning/ROADMAP.md](docs/planning/ROADMAP.md)

### Key Improvements (v1.1.0)

| Phase | Features | Status |
|-------|----------|--------|
| **Phase 1** | Automatic HTTPS (ACME), API-based Config, Least Response Time LB, Connection Draining, Zero-Config Mode, OpenTelemetry | ✅ Complete |
| **Phase 2** | Slow Start, Disk Cache, Status Dashboard, Request Validation, etcd Discovery, JSON Config, Stick Tables | ✅ Complete |
| **Phase 3** | Response Aggregation, BFF Pattern, Database Protocol Awareness | ✅ Complete |

**Recently Added Features (v1.1.0):**
- ✅ Stick Tables (HAProxy-style session persistence)
- ✅ BFF Pattern (Backend for Frontend routing)
- ✅ Response Aggregation (multi-backend request composition)

---

## Support & Contact

- **Documentation:** https://github.com/highperapp/highper-gateway/tree/main/docs
- **Issues:** https://github.com/highperapp/highper-gateway/issues
- **Discussions:** https://github.com/highperapp/highper-gateway/discussions

---

**Last Updated:** January 25, 2026
**Next Review:** February 25, 2026 (monthly)
