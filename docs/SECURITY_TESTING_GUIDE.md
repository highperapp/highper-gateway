# Security Testing Guide

**Highper Gateway** - Comprehensive Security Testing Procedures

---

## Table of Contents

1. [Overview](#overview)
2. [Automated Security Scanning](#automated-security-scanning)
3. [Manual Security Testing](#manual-security-testing)
4. [Integration Tests](#integration-tests)
5. [End-to-End Tests](#end-to-end-tests)
6. [Penetration Testing](#penetration-testing)
7. [Compliance Testing](#compliance-testing)
8. [CI/CD Integration](#cicd-integration)

---

## Overview

Security testing should be performed at multiple levels:

1. **Unit Tests**: Test individual security components in isolation
2. **Integration Tests**: Test security middleware interactions
3. **E2E Tests**: Test complete HTTP request/response flows
4. **Automated Scans**: OWASP ZAP, security header validators
5. **Manual Testing**: Penetration testing, code review
6. **Compliance Testing**: OWASP Top 10, PCI-DSS validation

**Testing Frequency**:
- Unit/Integration tests: Every commit (CI/CD)
- E2E tests: Every pull request
- Automated scans: Weekly
- Manual testing: Before major releases
- Compliance testing: Quarterly

---

## Automated Security Scanning

### OWASP ZAP Scanning

OWASP ZAP (Zed Attack Proxy) is an industry-standard security testing tool.

#### Installation

**Option 1: Download ZAP**
```bash
# Download from https://www.zaproxy.org/download/
# Or install via package manager:

# macOS
brew install --cask owasp-zap

# Ubuntu/Debian
sudo snap install zaproxy --classic

# Windows
# Download installer from website
```

**Option 2: Use Docker** (Recommended for CI/CD)
```bash
docker pull ghcr.io/zaproxy/zaproxy:stable
```

#### Running Scans

**1. Baseline Scan** (Recommended for regular testing)
```bash
# Using our script
./scripts/security-scan.sh http://localhost:8080

# Or directly
docker run -t ghcr.io/zaproxy/zaproxy:stable \
    zap-baseline.py -t http://localhost:8080 \
    -r report.html
```

**2. Full Scan** (Comprehensive, takes longer)
```bash
./scripts/security-scan.sh http://localhost:8080 --type full

# Expected duration: 30-60 minutes
```

**3. API Scan** (For API-only deployments)
```bash
./scripts/security-scan.sh http://localhost:8080/api --type api
```

#### Understanding ZAP Reports

**Risk Levels**:
- **High**: Critical vulnerabilities, fix immediately
- **Medium**: Significant issues, fix soon
- **Low**: Minor issues, address when possible
- **Informational**: Not vulnerabilities, but good to know

**Common Findings**:
- Missing security headers → Check middleware configuration
- XSS vulnerabilities → Verify CSP is enabled
- SQL injection → Validate backend protection
- Information disclosure → Review error messages

**Example Report Interpretation**:
```
Alert: X-Content-Type-Options Header Missing
Risk: Low
Confidence: Medium
Solution: Enable security headers middleware
```

### Security Header Validation

#### Using securityheaders.com

```bash
# Test public endpoints
curl https://securityheaders.com/?q=https://your-domain.com
```

Expected Grade: **A** or **A+**

#### Using Mozilla Observatory

```bash
# Comprehensive security scan
https://observatory.mozilla.org/analyze/your-domain.com
```

Expected Score: **90+**

#### Manual Header Check

```bash
# Test security headers
curl -I http://localhost:8080 | grep -E "(X-Frame|X-Content|Strict-Transport|Content-Security)"

# Expected output:
# X-Content-Type-Options: nosniff
# X-Frame-Options: DENY
# Strict-Transport-Security: max-age=31536000
# Content-Security-Policy: default-src 'self'
```

### SSL/TLS Testing

#### SSL Labs Test

```bash
# Test TLS configuration (for public domains)
https://www.ssllabs.com/ssltest/analyze.html?d=your-domain.com
```

Expected Grade: **A** or **A+**

#### Using testssl.sh

```bash
# Install
git clone https://github.com/drwetter/testssl.sh.git
cd testssl.sh

# Test TLS
./testssl.sh https://your-domain.com

# Expected results:
# ✓ TLS 1.2 and 1.3 only
# ✓ Strong cipher suites
# ✓ No vulnerabilities (POODLE, BEAST, Heartbleed, etc.)
# ✓ OCSP stapling enabled
```

---

## Manual Security Testing

### Security Headers Testing

**Test 1: Verify Default Headers**
```bash
curl -I http://localhost:8080

# Checklist:
# ✓ X-Content-Type-Options: nosniff
# ✓ X-Frame-Options: DENY or SAMEORIGIN
# ✓ X-XSS-Protection: 1; mode=block
# ✓ Strict-Transport-Security present
# ✓ Referrer-Policy present
```

**Test 2: Verify Strict Mode**
```bash
# Configure with preset = "strict"
curl -I http://localhost:8080

# Additional checklist:
# ✓ HSTS max-age >= 31536000 (1 year)
# ✓ HSTS includes includeSubDomains and preload
# ✓ Content-Security-Policy present
# ✓ Permissions-Policy present
# ✓ Referrer-Policy: no-referrer
```

**Test 3: Custom CSP**
```bash
# Configure custom CSP in config
curl -I http://localhost:8080

# Verify:
# ✓ CSP contains expected directives
# ✓ CSP allows only trusted domains
```

### Request Size Limit Testing

**Test 1: Within Limit**
```bash
# Send 5MB request (within 10MB default)
dd if=/dev/zero bs=1M count=5 | curl -X POST \
    -H "Content-Type: application/octet-stream" \
    --data-binary @- \
    http://localhost:8080/api/upload

# Expected: 200 OK
```

**Test 2: Exceeds Limit**
```bash
# Send 15MB request (exceeds 10MB default)
dd if=/dev/zero bs=1M count=15 | curl -X POST \
    -H "Content-Type: application/octet-stream" \
    --data-binary @- \
    http://localhost:8080/api/upload

# Expected: 413 Payload Too Large
```

**Test 3: Custom Error Message**
```bash
# Configure custom error message
# Send large request and verify error message
curl -X POST -d @large-file.dat http://localhost:8080/api/upload

# Expected: Custom error message in response
```

### Rate Limiting Testing

**Test 1: Normal Traffic**
```bash
# Send requests within limit
for i in {1..10}; do
    curl http://localhost:8080/api/test
    sleep 0.1
done

# Expected: All requests succeed (200 OK)
```

**Test 2: Burst Traffic**
```bash
# Send rapid requests to trigger rate limit
for i in {1..50}; do
    curl -w "%{http_code}\n" -o /dev/null -s http://localhost:8080/api/test &
done
wait

# Expected: Some requests return 429 Too Many Requests
```

**Test 3: Per-Route Limits**
```bash
# Test different routes with different limits
curl http://localhost:8080/api/public      # Lenient limit
curl http://localhost:8080/api/admin       # Strict limit

# Verify different rate limits apply
```

### XSS Testing

**Test 1: Reflected XSS**
```bash
# Try injecting script in query parameter
curl "http://localhost:8080/search?q=<script>alert('XSS')</script>"

# Expected: Script should NOT execute
# CSP should block or encode output
```

**Test 2: CSP Violation**
```javascript
// In browser console, try loading unauthorized script
var script = document.createElement('script');
script.src = 'https://evil.com/malicious.js';
document.head.appendChild(script);

// Expected: Browser blocks and logs CSP violation
```

### Injection Testing

**Test 1: SQL Injection** (Backend protection, but test end-to-end)
```bash
# Try SQL injection in input
curl -X POST http://localhost:8080/api/login \
    -d '{"username":"admin'--","password":"anything"}'

# Expected: Rejected or properly escaped by backend
```

**Test 2: Command Injection**
```bash
# Try command injection
curl "http://localhost:8080/api/file?name=test.txt;ls"

# Expected: Input validation rejects
```

---

## Integration Tests

### Running Integration Tests

```bash
# Run all integration tests
cargo test --tests

# Run security integration tests specifically
cargo test --test security_headers_integration
cargo test --test request_size_limit_integration

# Expected: All tests pass
# Security headers: 10/10 passing
# Request size limits: 17/17 passing
```

### Test Coverage

```bash
# Generate coverage report
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir ./coverage

# Expected: > 85% coverage
```

---

## End-to-End Tests

### Running E2E Tests

```bash
# Build release binary first
cargo build --release

# Run E2E security tests
cargo test --test e2e_security -- --ignored --test-threads=1

# Run E2E resilience tests
cargo test --test e2e_resilience -- --ignored --test-threads=1

# Expected: All tests pass
```

### E2E Test Coverage

**Security E2E Tests** (9 tests):
- Default security headers
- Strict security headers
- Custom CSP
- Security headers on errors
- Request size limit enforcement
- Custom error messages
- Per-route size limits
- Header preservation
- Disabled size limits

**Resilience E2E Tests** (8 tests):
- Rate limiting enforcement
- Circuit breaker behavior
- Health check failover
- Timeout handling
- Connection pooling
- Per-route rate limiting
- Concurrent requests
- Load balancing

---

## Penetration Testing

### Preparation

**1. Set Up Test Environment**
```bash
# Deploy to isolated test environment
# Do NOT test on production!

docker-compose -f deployment/docker/docker-compose.test.yml up
```

**2. Document Scope**
```
In Scope:
- Web interface security
- API security
- Authentication/authorization
- Input validation
- Security headers
- Rate limiting
- Error handling

Out of Scope:
- DoS attacks
- Physical security
- Social engineering
```

### Testing Checklist

#### Authentication & Authorization
- [ ] Test JWT validation
- [ ] Test token expiration
- [ ] Test invalid tokens
- [ ] Test authorization bypass
- [ ] Test privilege escalation
- [ ] Test session fixation

#### Input Validation
- [ ] Test SQL injection
- [ ] Test XSS (reflected, stored, DOM)
- [ ] Test command injection
- [ ] Test path traversal
- [ ] Test XXE (XML external entity)
- [ ] Test SSRF (server-side request forgery)

#### Business Logic
- [ ] Test rate limiting bypass
- [ ] Test race conditions
- [ ] Test integer overflow
- [ ] Test business logic flaws

#### Information Disclosure
- [ ] Test error messages
- [ ] Test debug information
- [ ] Test version disclosure
- [ ] Test source code disclosure
- [ ] Test backup file access

#### Configuration & Deployment
- [ ] Test default credentials
- [ ] Test insecure defaults
- [ ] Test missing patches
- [ ] Test insecure protocols
- [ ] Test open ports

### Tools for Pen Testing

**Web Application Testing**:
- Burp Suite Community/Pro
- OWASP ZAP
- Nikto
- DirBuster/GoBuster

**API Testing**:
- Postman
- curl
- httpie
- GraphQL Voyager

**TLS Testing**:
- testssl.sh
- SSL Labs
- nmap with ssl scripts

**Authentication Testing**:
- jwt_tool
- Auth Analyzer (Burp extension)

---

## Compliance Testing

### OWASP Top 10 2021 Testing

**A01: Broken Access Control**
```bash
# Test 1: Try accessing admin endpoints without auth
curl http://localhost:8080/admin/users

# Test 2: Try accessing other user's data
curl -H "Authorization: Bearer <user1-token>" \
    http://localhost:8080/api/users/user2/profile

# Test 3: Try privilege escalation
curl -X POST -H "Authorization: Bearer <user-token>" \
    http://localhost:8080/admin/create-admin
```

**A02: Cryptographic Failures**
```bash
# Test 1: Verify TLS 1.2+ only
nmap --script ssl-enum-ciphers -p 443 your-domain.com

# Test 2: Verify strong ciphers
testssl.sh --protocols https://your-domain.com

# Test 3: Check for hardcoded secrets
grep -r "password\|secret\|key" . --include=*.toml --include=*.yaml
```

**A03: Injection**
```bash
# Already covered in Manual Testing section
```

**A05: Security Misconfiguration**
```bash
# Test 1: Verify security headers
curl -I http://localhost:8080

# Test 2: Check for debug mode
curl http://localhost:8080/debug

# Test 3: Verify error messages don't leak info
curl http://localhost:8080/nonexistent
```

**A07: Identification and Authentication Failures**
```bash
# Test 1: Brute force protection
for i in {1..100}; do
    curl -X POST http://localhost:8080/api/login \
        -d '{"username":"admin","password":"wrong'$i'"}'
done

# Expected: Rate limiting or account lockout
```

### PCI-DSS Testing (if handling payment data)

**Requirement 4: Encrypt transmission of cardholder data**
```bash
# Verify TLS 1.2+ only
testssl.sh https://your-domain.com

# Expected: TLS 1.2 or 1.3 only
```

**Requirement 6: Develop and maintain secure systems**
```bash
# Run security scans
./scripts/security-scan.sh https://your-domain.com

# Expected: No high or medium vulnerabilities
```

**Requirement 8: Identify and authenticate access**
```bash
# Verify strong authentication
# Test MFA if implemented
# Test account lockout after failed attempts
```

### GDPR Compliance Testing

**Data Minimization**
```bash
# Review logs for PII
tail -n 100 /var/log/highper-gateway/access.log | grep -E "(email|phone|ssn)"

# Expected: No PII in logs
```

**Right to Erasure**
```bash
# Verify log deletion capability
# Test user data deletion endpoints
```

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Security Tests

on: [push, pull_request]

jobs:
  security-tests:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run integration tests
        run: |
          cargo test --test security_headers_integration
          cargo test --test request_size_limit_integration

      - name: Build release binary
        run: cargo build --release

      - name: Start test server
        run: |
          ./target/release/highper-gateway --config config-test.toml &
          sleep 5

      - name: Run OWASP ZAP scan
        run: |
          docker run -t ghcr.io/zaproxy/zaproxy:stable \
            zap-baseline.py -t http://localhost:8080 \
            -r zap-report.html

      - name: Upload security reports
        uses: actions/upload-artifact@v3
        with:
          name: security-reports
          path: zap-report.html
```

### GitLab CI Example

```yaml
security-tests:
  stage: test
  image: rust:latest
  script:
    - cargo test --test security_headers_integration
    - cargo test --test request_size_limit_integration
    - cargo build --release
    - ./target/release/highper-gateway --config config-test.toml &
    - sleep 5
    - docker run -t ghcr.io/zaproxy/zaproxy:stable zap-baseline.py -t http://localhost:8080
  artifacts:
    paths:
      - zap-report.html
```

### Pre-commit Hooks

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Run security tests before committing
echo "Running security tests..."

cargo test --test security_headers_integration --quiet
if [ $? -ne 0 ]; then
    echo "❌ Security header tests failed"
    exit 1
fi

cargo test --test request_size_limit_integration --quiet
if [ $? -ne 0 ]; then
    echo "❌ Request size limit tests failed"
    exit 1
fi

echo "✅ Security tests passed"
```

---

## Security Testing Schedule

### Continuous (Automated)

**Every Commit**:
- Unit tests
- Integration tests
- Static code analysis

**Every Pull Request**:
- E2E tests
- Security linting
- Dependency audit

### Regular (Weekly)

**Every Week**:
- OWASP ZAP baseline scan
- Security header validation
- SSL/TLS testing
- Dependency vulnerability scan

### Periodic (Monthly/Quarterly)

**Monthly**:
- Full OWASP ZAP scan
- Manual penetration testing
- Log review for security events

**Quarterly**:
- Comprehensive security audit
- Compliance testing (OWASP Top 10, PCI-DSS)
- Third-party security assessment
- Update security documentation

### Pre-Release

**Before Major Releases**:
- Full security testing suite
- External penetration testing
- Security code review
- Compliance validation
- Update security advisories

---

## Security Testing Metrics

### Key Metrics to Track

**Test Coverage**:
- Unit test coverage: > 85%
- Integration test coverage: 100% of security features
- E2E test coverage: All critical paths

**Vulnerability Metrics**:
- High severity vulnerabilities: 0
- Medium severity vulnerabilities: < 5
- Time to fix: < 7 days for high, < 30 days for medium

**Security Score**:
- OWASP ZAP: Risk score < 10
- SSL Labs: Grade A or A+
- Security Headers: Grade A or A+
- Mozilla Observatory: Score > 90

---

## Troubleshooting

### Common Issues

**Issue 1: ZAP scan finds missing security headers**
```
Solution:
1. Verify middleware is enabled in config
2. Check middleware order
3. Restart proxy
4. Re-run scan
```

**Issue 2: E2E tests fail**
```
Solution:
1. Ensure release binary is built: cargo build --release
2. Check no other process using ports
3. Run tests sequentially: --test-threads=1
4. Check test logs for specific errors
```

**Issue 3: False positives in security scan**
```
Solution:
1. Review finding details
2. Verify actual behavior
3. Add to ZAP ignore list if confirmed false positive
4. Document decision
```

---

## Additional Resources

### Documentation
- [Security Features Guide](./SECURITY_FEATURES.md)
- [Production Deployment Guide](./PRODUCTION_DEPLOYMENT_GUIDE.md)
- [Security Hardening Guide](./dev-notes/SECURITY_HARDENING_GUIDE.md)

### External Resources
- [OWASP ZAP Documentation](https://www.zaproxy.org/docs/)
- [OWASP Testing Guide](https://owasp.org/www-project-web-security-testing-guide/)
- [Mozilla Web Security Guidelines](https://infosec.mozilla.org/guidelines/web_security)
- [PortSwigger Web Security Academy](https://portswigger.net/web-security)

### Security Tools
- [Burp Suite](https://portswigger.net/burp)
- [OWASP ZAP](https://www.zaproxy.org/)
- [testssl.sh](https://testssl.sh/)
- [Nikto](https://github.com/sullo/nikto)

---

**Last Updated**: November 25, 2025
**Version**: 1.0
**Status**: Production-Ready

---
