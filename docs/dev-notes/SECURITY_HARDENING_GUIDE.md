# Security Hardening Guide
## Rust Reverse Proxy - Production Deployment

**Date:** November 17, 2025
**Version:** 1.0
**Audience:** DevOps, Security Engineers, System Administrators

---

## Quick Start Checklist

Before deploying to production, complete this checklist:

### Critical (Must Do)

- [ ] **Enable TLS/HTTPS** with valid certificates
- [ ] **Enable security headers** (`preset = "strict"`)
- [ ] **Configure request size limits** (default: 10 MB)
- [ ] **Enable rate limiting** per IP/route
- [ ] **Enable circuit breaker** for backend protection
- [ ] **Configure health checks** for all upstreams
- [ ] **Bind admin API to localhost** or add authentication
- [ ] **Enable observability** (logs, metrics)
- [ ] **Run security validation** (`./load-tests/security-validation.sh`)
- [ ] **Review OWASP audit** (`SECURITY_AUDIT_OWASP.md`)

### Recommended (Should Do)

- [ ] **Enable WAF** (Coraza with OWASP CRS)
- [ ] **Configure CORS** (if serving API)
- [ ] **Enable mTLS** for backend connections (if needed)
- [ ] **Set up monitoring** and alerting
- [ ] **Document incident response** procedures
- [ ] **Schedule security audits** (quarterly)

### Optional (Nice to Have)

- [ ] **Enable OAuth2/JWT** authentication
- [ ] **Configure IP allowlist/blocklist**
- [ ] **Enable compression** for bandwidth savings
- [ ] **Set up distributed tracing** (Jaeger)

---

##1. TLS/HTTPS Configuration

### Minimum Requirements

```toml
[server.tls]
cert = "/etc/ssl/certs/server.crt"
key = "/etc/ssl/private/server.key"
min_version = "1.2"  # TLS 1.2+only (no SSL, TLS 1.0, TLS 1.1)
```

### Recommended (Strong Security)

```toml
[server.tls]
cert = "/etc/ssl/certs/server.crt"
key = "/etc/ssl/private/server.key"
min_version = "1.2"

# Strong cipher suites only
cipher_suites = [
    "TLS_AES_256_GCM_SHA384",
    "TLS_AES_128_GCM_SHA256",
    "TLS_CHACHA20_POLY1305_SHA256",
]

# OCSP stapling for certificate validation
ocsp_stapling = true
```

### Testing TLS Configuration

```bash
# Test TLS 1.2
openssl s_client -connect yourdomain.com:443 -tls1_2

# Test TLS 1.3
openssl s_client -connect yourdomain.com:443 -tls1_3

# Test for weak ciphers (should fail)
openssl s_client -connect yourdomain.com:443 -cipher 'DES-CBC3-SHA'

# Use testssl.sh for comprehensive testing
./testssl.sh yourdomain.com
```

---

## 2. Security Headers

### Recommended Configuration

```toml
[middleware.security_headers]
enabled = true
preset = "strict"
```

**"strict" preset includes:**
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `X-XSS-Protection: 1; mode=block`
- `Strict-Transport-Security: max-age=63072000; includeSubDomains; preload`
- `Content-Security-Policy: default-src 'self'`
- `Referrer-Policy: no-referrer`
- `Permissions-Policy: geolocation=(), microphone=(), camera=()`

### Custom Configuration

```toml
[middleware.security_headers.custom]
hsts = "max-age=31536000; includeSubDomains; preload"
csp = "default-src 'self'; script-src 'self' https://cdn.trusted.com; style-src 'self' 'unsafe-inline'"
```

### Testing Security Headers

```bash
curl -I https://yourdomain.com | grep -i "strict-transport"
curl -I https://yourdomain.com | grep -i "x-frame-options"
curl -I https://yourdomain.com | grep -i "content-security-policy"
```

---

## 3. Request Size Limits

### Configuration

```toml
[middleware.request_size_limit]
enabled = true
max_body_size = 10485760  # 10 MB (adjust based on needs)
error_message = "Request too large"
```

### Recommendations by Use Case

- **API Backend**: 1-5 MB
- **File Upload Service**: 50-100 MB (or higher)
- **General Web App**: 10 MB (default)
- **Microservices**: 1 MB

### Testing

```bash
# Should pass (1 MB)
dd if=/dev/zero of=/tmp/1mb.bin bs=1M count=1
curl -X POST -H "Content-Type: application/octet-stream" \
  --data-binary "@/tmp/1mb.bin" https://yourdomain.com/upload

# Should fail with 413 (15 MB)
dd if=/dev/zero of=/tmp/15mb.bin bs=1M count=15
curl -X POST -H "Content-Type: application/octet-stream" \
  --data-binary "@/tmp/15mb.bin" https://yourdomain.com/upload
```

---

## 4. Rate Limiting

### Basic Configuration

```toml
[middleware.rate_limit]
enabled = true
requests_per_second = 100
burst = 20
scope = "ip"  # or "global"
```

### Per-Route Rate Limiting

```toml
[[routes]]
name = "api"
upstream = "backend"

[routes.rate_limit]
enabled = true
requests_per_second = 50  # Stricter for API
burst = 10
```

### Tuning Guidelines

| Use Case | req/s | Burst | Notes |
|----------|-------|-------|-------|
| Public Website | 100-500 | 20-50 | Balance UX and protection |
| API (Public) | 10-100 | 5-20 | Prevent abuse |
| API (Internal) | 500-1000 | 50-100 | Higher limits |
| File Upload | 5-20 | 5 | Lower for heavy operations |

### Testing

```bash
# Run 100 requests rapidly
for i in {1..100}; do
  curl -s -o /dev/null -w "%{http_code}\n" https://yourdomain.com/
done | grep "429" | wc -l
```

---

## 5. Web Application Firewall (WAF)

### Enabling WAF with OWASP CRS

```toml
[middleware.waf]
enabled = true
mode = "coraza"  # OWASP Core Rule Set
block_mode = true
max_body_size = 1048576  # 1 MB

[middleware.waf.coraza]
rules_path = "/etc/highper-gateway/waf/coreruleset"
paranoia_level = 2  # 1=basic, 2=moderate, 3=strict, 4=paranoid
```

### Paranoia Levels

- **Level 1**: Basic protection, low false positives
- **Level 2**: Moderate protection (recommended for production)
- **Level 3**: Strict protection, may have false positives
- **Level 4**: Paranoid, requires tuning for your application

### IP Allowlist/Blocklist

```toml
[middleware.waf.coraza]
# Trusted IPs (bypass WAF)
allowlist = [
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
]

# Blocked IPs
blocklist = [
    "203.0.113.0/24",  # Example malicious network
]
```

### Testing WAF

```bash
# SQL injection attempt (should be blocked)
curl "https://yourdomain.com/search?q=' OR '1'='1"

# XSS attempt (should be blocked)
curl "https://yourdomain.com/search?q=<script>alert('XSS')</script>"

# Path traversal (should be blocked)
curl "https://yourdomain.com/../../../etc/passwd"
```

---

## 6. Circuit Breaker & Resilience

### Configuration

```toml
[upstreams.connection]
circuit_breaker_enabled = true
circuit_breaker_threshold = 5        # Open after 5 failures
circuit_breaker_timeout = "30s"      # Try to close after 30s

# Connection pool (from Week 1 optimization)
max_connections_per_upstream = 500
min_idle_connections = 50

[upstreams.connection.connection_pool]
max_idle_per_host = 200
min_idle_per_host = 50
prewarm = true
```

### Health Checks

```toml
[upstreams.health_check]
enabled = true
interval = "5s"
timeout = "2s"
path = "/health"
healthy_threshold = 2
unhealthy_threshold = 3
```

---

## 7. Admin API Security

### Secure Configuration

```toml
[admin]
# Option 1: Localhost only (recommended)
bind = "127.0.0.1:9090"

# Option 2: Specific internal IP
# bind = "10.0.1.5:9090"

# Option 3: Use firewall rules to restrict access
# bind = "0.0.0.0:9090"  # + firewall rules
```

### Accessing Admin API Securely

```bash
# SSH tunnel (if localhost-only)
ssh -L 9090:localhost:9090 user@proxy-server

# Then access locally
curl http://localhost:9090/health
curl http://localhost:9090/metrics
```

### Adding Authentication (Future Enhancement)

```toml
[admin.auth]
enabled = true
api_key = "your-secret-api-key"
# or use JWT, OAuth2, etc.
```

---

## 8. Monitoring & Observability

### Logging Configuration

```toml
[observability]
log_level = "info"  # error, warn, info, debug, trace
access_log = true
access_log_format = "json"  # or "combined", "custom"
```

### Metrics

```toml
[observability]
metrics_enabled = true
metrics_port = 9090
metrics_path = "/metrics"
```

### Recommended Metrics to Monitor

**Security Metrics:**
- `http_requests_total{status="429"}` - Rate limit hits
- `http_requests_total{status="413"}` - Request size limit hits
- `waf_requests_blocked_total` - WAF blocks
- `circuit_breaker_state` - Circuit breaker status
- `tls_handshake_errors_total` - TLS errors

**Performance Metrics:**
- `http_request_duration_seconds` - Latency
- `http_requests_total` - Request rate
- `upstream_connection_pool_size` - Pool utilization
- `upstream_health_status` - Backend health

### Alerting Rules (Prometheus)

```yaml
groups:
  - name: security
    rules:
      - alert: HighRateLimitHits
        expr: rate(http_requests_total{status="429"}[5m]) > 10
        for: 5m
        annotations:
          summary: "High rate limiting activity"

      - alert: WAFUnderAttack
        expr: rate(waf_requests_blocked_total[1m]) > 10
        for: 1m
        annotations:
          summary: "Possible DDoS/attack detected"

      - alert: CircuitBreakerOpen
        expr: circuit_breaker_state{state="open"} == 1
        for: 1m
        annotations:
          summary: "Circuit breaker opened - backend issues"
```

---

## 9. Common Security Pitfalls

### ❌ Don't Do This

1. **Exposing Admin API to Internet**
   ```toml
   # BAD
   [admin]
   bind = "0.0.0.0:9090"  # No authentication!
   ```

2. **Weak TLS Configuration**
   ```toml
   # BAD
   [server.tls]
   min_version = "1.0"  # Allows TLS 1.0/1.1 (insecure)
   ```

3. **Disabling Security Headers**
   ```toml
   # BAD
   [middleware.security_headers]
   enabled = false
   ```

4. **No Request Size Limits**
   ```toml
   # BAD
   [middleware.request_size_limit]
   enabled = false  # Allows unlimited request sizes
   ```

5. **No Rate Limiting**
   ```toml
   # BAD - no rate limiting configuration
   ```

### ✅ Do This Instead

1. **Secure Admin API**
   ```toml
   [admin]
   bind = "127.0.0.1:9090"  # Localhost only
   ```

2. **Strong TLS**
   ```toml
   [server.tls]
   min_version = "1.2"  # TLS 1.2+ only
   ```

3. **Enable Security Features**
   ```toml
   [middleware.security_headers]
   enabled = true
   preset = "strict"
   ```

---

## 10. Security Validation

### Running Security Tests

```bash
# Run security validation suite
cd /home/infy/reverse_proxy/load-tests
./security-validation.sh

# Expected output:
# Total Tests: 40+
# Passed: 40+
# Failed: 0
# Security Grade: A
```

### Manual Security Checks

```bash
# 1. Check security headers
curl -I https://yourdomain.com | grep -i "strict-transport"

# 2. Test rate limiting
for i in {1..100}; do curl -s https://yourdomain.com/; done

# 3. Test request size limit
curl -X POST -H "Content-Length: 15728640" https://yourdomain.com/

# 4. Test TLS version
openssl s_client -connect yourdomain.com:443 -tls1

# 5. Run SSL Labs test
# Visit: https://www.ssllabs.com/ssltest/analyze.html?d=yourdomain.com
```

---

## 11. Incident Response

### Security Incident Checklist

1. **Detection**
   - Monitor alerts (rate limiting, WAF, circuit breaker)
   - Review logs for anomalies
   - Check metrics dashboards

2. **Investigation**
   ```bash
   # Check recent logs
   journalctl -u highper-gateway -n 1000 | grep -i "error\|warn"

   # Check WAF blocks
   grep "waf_blocked" /var/log/highper-gateway/waf.log

   # Check rate limit hits
   grep "429" /var/log/highper-gateway/access.log | wc -l
   ```

3. **Response**
   - If DDoS: Increase rate limits temporarily or add IP to blocklist
   - If vulnerability: Apply WAF rule or patch immediately
   - If backend compromise: Open circuit breaker manually

4. **Recovery**
   - Restore service once threat mitigated
   - Document incident and lessons learned
   - Update security policies

---

## 12. Compliance & Auditing

### OWASP Top 10 Compliance

See `SECURITY_AUDIT_OWASP.md` for full compliance report.

**Current Status:** A- (Strong Security)

### Regular Security Audits

**Monthly:**
- Review access logs for anomalies
- Check certificate expiration (< 30 days warning)
- Review rate limiting effectiveness

**Quarterly:**
- Run full security scan (OWASP ZAP, Burp Suite)
- Update WAF rules
- Review and update security policies

**Annually:**
- Third-party security audit
- Penetration testing
- Update threat model

---

## 13. Production Deployment

### Pre-Deployment Checklist

```bash
# 1. Review configuration
cat config-production-secure.toml

# 2. Build release binary
cd highper-gateway
cargo build --release

# 3. Run security validation
cd ../load-tests
./security-validation.sh

# 4. Run chaos testing
./chaos-testing.sh

# 5. Test configuration
../target/release/highper-gateway check --config ../config-production-secure.toml

# 6. Start proxy
../target/release/highper-gateway start --config ../config-production-secure.toml
```

### Post-Deployment

```bash
# 1. Verify security headers
curl -I https://yourdomain.com

# 2. Check metrics
curl http://localhost:9090/metrics

# 3. Monitor logs
tail -f /var/log/highper-gateway/access.log

# 4. Run SSL Labs test
# https://www.ssllabs.com/ssltest/

# 5. Verify rate limiting
for i in {1..100}; do curl -s https://yourdomain.com/; done | grep "429"
```

---

## Conclusion

**Security Status:** ✅ **PRODUCTION READY**

The Rust proxy has comprehensive security features:
- 19 security controls implemented
- OWASP Top 10 2021 compliant (A- rating)
- Zero critical vulnerabilities
- Enterprise-grade resilience

**Use this guide to:**
1. Configure security features correctly
2. Validate security posture
3. Monitor and respond to incidents
4. Maintain security over time

**For more information:**
- OWASP Audit: `SECURITY_AUDIT_OWASP.md`
- Security Features Summary: `WEEK2_SECURITY_FEATURES_SUMMARY.md`
- Production Config: `config-production-secure.toml`

---

**Last Updated:** November 17, 2025
**Version:** 1.0
**Maintained by:** Highper Gateway Security Team
