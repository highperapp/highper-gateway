# Changelog

All notable changes to Highper Gateway will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Reconciliation note (2026-05-02).** The `[1.0.0] - 2026-01-10` entry below described
> the codebase as "Complete Production Release". The 2026-05-02 audit
> (`docs/AUDIT_2026-05-02.md`) and consolidated roadmap (`docs/planning/ROADMAP.md` §4.1)
> identify **14 release blockers** (B1–B14) that must close before any v1.0 GA tag.
> The historical `[1.0.0]` entry below is **retained as authored** for changelog
> integrity but should be read as **v1.0-rc**, not GA. The next GA tag will land
> after Phase 0 of the roadmap completes.

## [Unreleased] — v1.0-rc trajectory

### Tracking

All in-flight work is tracked in [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md). Highlights:

- Phase 0 — close 14 release blockers B1–B14.
- Phase 1 — cloud-validation matrix, 7-/30-day soak, SBOM (Trivy + syft+Grype),
  DAST (Dastardly + OWASP ZAP), OTLP, Vault/Secrets integration.
- Phase 2 — UC16 AI/LLM Gateway MVP (conditional on owner-finalized design;
  see `docs/planning/USECASE_16_AI_LLM_GATEWAY.md`).
- Phases 3 & 4 — UC16 Beta/GA + competitor-feature catch-up + ecosystem
  (xDS, K8s operator, kTLS, post-quantum TLS).

### Planned (legacy entries, retained for reference)
- Static service discovery implementation
- Directory listing for static file server
- Complete OAuth2 implementation (PKCE, token refresh, revocation)
- PowerShell scripts for Windows native support
- Enhanced OCSP fetcher with better retry logic
- Automated cloud load test cleanup

---

## [1.0.0] - 2026-01-10 *(re-classified as v1.0-rc on 2026-05-02 — see banner above)*

### Added
- **Complete Production Release:** All 15 use case scenarios fully implemented and production-ready
- **Core Features:**
  - Layer 4 TCP proxying with connection pooling and health checks
  - Layer 7 HTTP/1.1 load balancing with advanced routing
  - HTTPS/TLS termination with ACME, mTLS, and OCSP stapling
  - API Gateway with rate limiting (Token Bucket, Sliding Window, Fixed Window)
  - HTTP/3 QUIC support using Cloudflare quiche library
  - WebSocket load balancing with sticky sessions and keepalive
  - gRPC Gateway with HTTP/2 and streaming support
  - Database load balancing for MySQL, PostgreSQL, Redis
  - WAF with 4 engines (ModSecurity, Coraza, AWS WAF, Custom)
  - Multi-protocol hybrid routing (TCP, HTTP, WebSocket, gRPC)
  - CDN edge caching (InMemory, Redis, Multi-tier)
  - Service discovery (Consul, etcd) with circuit breaker
  - GraphQL Gateway with schema stitching and federation
  - Static file server with PHP-FPM FastCGI support
  - Geographic load balancing with MaxMind and IP2Location
- **Configuration:**
  - TOML configuration format with comprehensive schema
  - HCL/DSL configuration (Caddy-inspired, simple and powerful)
  - Environment variable support for all configuration options
  - Zero-config defaults with intelligent fallbacks
  - 12-Factor methodology compliance (100%)
- **Observability:**
  - Prometheus metrics integration
  - Distributed tracing support
  - Structured logging (JSON, logfmt)
  - Health check endpoints
  - Admin API for runtime management
- **Security:**
  - TLS 1.2/1.3 support
  - mTLS client certificate validation
  - ACME/Let's Encrypt automatic certificate management
  - OCSP stapling with automatic refresh
  - 4 WAF engines for comprehensive protection
  - Rate limiting at multiple levels
- **Plugin System:**
  - WASM plugin support
  - FFI plugin support (Rust native)
  - Hot reload capability
  - Host functions for plugin integration
- **Load Balancing Algorithms:**
  - Round-robin
  - Least connections
  - IP hash (sticky sessions)
  - Weighted round-robin
  - Geographic routing
- **Performance:**
  - Async/await architecture with Tokio
  - Zero-copy where possible
  - Connection pooling
  - HTTP keep-alive
  - Efficient memory management
- **Testing:**
  - Comprehensive load testing framework
  - Support for local (docker-compose) and cloud testing
  - Cloud provider support: Vultr, Hetzner, PhoenixNAP, DigitalOcean
  - 24 test scripts covering all 15 scenarios
  - Integration tests for all major features
- **Documentation:**
  - Comprehensive README with quick start guide
  - Architecture documentation
  - Deployment scenarios and best practices
  - Security hardening guide
  - Plugin development guide
  - API documentation
  - Known limitations document
  - Load testing strategy guide
- **Infrastructure:**
  - Docker support
  - Docker Compose configurations
  - Kubernetes manifests
  - Systemd service units
  - Prometheus and Grafana dashboards
- **License:**
  - Apache 2.0 license for core functionality
  - Includes plugin SDK and plugin architecture
  - Free and open source

### Changed
- **Project Organization:** Major reorganization for professional GitHub repository
  - Consolidated duplicate directories (deploy + deployment → infrastructure/)
  - Organized 71 root markdown files into docs/archive/progress-reports/
  - Archived legacy load test frameworks
  - Standardized directory structure
  - Comprehensive .gitignore configuration
- **Documentation Structure:**
  - Created organized docs/ hierarchy
  - Separated documentation by category (architecture, development, operations, validation, testing)
  - Moved historical progress reports to archive
  - Updated all internal documentation links

### Fixed
- Removed HTTP/4 reference from documentation (non-existent protocol)
- Fixed HTTP/3 duplicate server spawn issue
- Fixed gRPC HTTP/2 prior knowledge handling
- Fixed GraphQL request body forwarding
- Cleaned up build artifacts and logs from repository

### Removed
- Legacy load test framework in root `./load-tests/`
- Legacy load test scripts in `./scripts/loadtest/`
- Historical test results from November 2025
- Duplicate configuration directories

### Known Limitations
See [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) for comprehensive list. Key limitations:
- Static service discovery not yet implemented (use Consul or etcd)
- Directory listing not implemented (provide index files)
- OAuth2 implementation incomplete (basic flow works, advanced features pending)
- OCSP fetcher needs production hardening
- Cloud load test cleanup not fully automated
- Windows native testing limited (use WSL2)

### Deprecated
None for v1.0.0 initial release.

### Security
- No security vulnerabilities known at release
- All dependencies up to date
- Security audit passed (see docs/architecture/SECURITY.md)
- WAF protection enabled by default
- Rate limiting enabled by default
- TLS 1.2+ required for HTTPS

---

## Version History

### v1.0.0 (2026-01-10) - Initial Production Release
- Complete implementation of all 15 use case scenarios
- Production-ready with comprehensive testing
- Full documentation and examples
- Apache 2.0 license

### Pre-releases (Development)
- v0.9.x (Dec 2025) - PHP-FPM and geographic load balancing completion
- v0.8.x (Dec 2025) - GraphQL and service discovery implementation
- v0.7.x (Dec 2025) - WAF and security features
- v0.6.x (Dec 2025) - HTTP/3 QUIC implementation
- v0.5.x (Dec 2025) - gRPC gateway implementation
- v0.4.x (Nov 2025) - WebSocket support
- v0.3.x (Nov 2025) - TLS and ACME implementation
- v0.2.x (Nov 2025) - HTTP load balancing
- v0.1.x (Oct 2025) - Initial TCP proxying

---

## Upgrade Guide

### From Development Versions (v0.x) to v1.0.0

#### Configuration Changes
No breaking changes. All v0.x configurations are compatible with v1.0.0.

#### Environment Variables
New environment variables added (all optional):
```bash
# Service Discovery
export HIGHPER_DISCOVERY_TYPE=consul
export HIGHPER_DISCOVERY_CONSUL_ADDR=http://localhost:8500

# OAuth2
export HIGHPER_AUTH_OAUTH2_PROVIDER=generic
export HIGHPER_AUTH_OAUTH2_CLIENT_ID=your-client-id

# TLS/OCSP
export HIGHPER_TLS_OCSP_ENABLED=true
export HIGHPER_TLS_OCSP_CACHE_DURATION=3600

# Load Testing
export HIGHPER_LOADTEST_AUTO_CLEANUP=true
```

#### API Changes
No breaking API changes.

#### Database Migrations
Not applicable (Highper Gateway is stateless).

---

## Contribution Guidelines

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to this changelog.

### Changelog Entry Format

When contributing, add entries in this format:

```markdown
### Added
- New feature description with reference to PR/issue #123

### Changed
- Modified behavior description with reference to PR/issue #456

### Fixed
- Bug fix description with reference to PR/issue #789

### Deprecated
- Deprecated feature description with migration guide

### Removed
- Removed feature description with alternative solution

### Security
- Security fix description with CVE reference (if applicable)
```

---

## Support

- **Documentation:** https://github.com/highperapp/highper-gateway/tree/main/docs
- **Issues:** https://github.com/highperapp/highper-gateway/issues
- **Discussions:** https://github.com/highperapp/highper-gateway/discussions
- **Releases:** https://github.com/highperapp/highper-gateway/releases

---

[Unreleased]: https://github.com/highperapp/highper-gateway/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/highperapp/highper-gateway/releases/tag/v1.0.0
