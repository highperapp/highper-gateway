# Security Best Practices

This document provides comprehensive security guidance for deploying and operating Highper Gateway in production environments.

## Table of Contents

- [Overview](#overview)
- [Security Middlewares](#security-middlewares)
- [TLS/mTLS Configuration](#tlsmtls-configuration)
- [Authentication & Authorization](#authentication--authorization)
- [Rate Limiting & DDoS Protection](#rate-limiting--ddos-protection)
- [Input Validation](#input-validation)
- [Security Headers](#security-headers)
- [WAF Integration](#waf-integration)
- [Security Audit Logging](#security-audit-logging)
- [Network Security](#network-security)
- [Secrets Management](#secrets-management)
- [Monitoring & Alerting](#monitoring--alerting)
- [Incident Response](#incident-response)
- [Compliance](#compliance)

## Overview

Highper Gateway is designed with security as a top priority. This document outlines the security features available and best practices for securing your deployment.

### Security Architecture Principles

1. **Defense in Depth**: Multiple layers of security controls
2. **Least Privilege**: Minimal permissions and access
3. **Fail Secure**: Default to denying access on errors
4. **Security by Default**: Secure configurations out-of-the-box
5. **Audit Everything**: Comprehensive logging of security events

## Security Middlewares

Highper Gateway provides several security middlewares that can be enabled:

### Request Validation Middleware

Protects against common attacks by validating all incoming requests.

```rust
use highper_gateway::middleware::request_validation::{
    RequestValidationMiddleware, RequestValidationConfig
};

// Strict validation (recommended for production)
let validation = RequestValidationMiddleware::strict();

// API-optimized validation
let validation = RequestValidationMiddleware::api();

// Custom configuration
let config = RequestValidationConfig {
    sql_injection_detection: true,
    xss_detection: true,
    path_traversal_detection: true,
    max_body_size: 1048576,  // 1 MB
    max_header_size: 4096,
    max_url_length: 1024,
    block_suspicious_user_agents: true,
    null_byte_detection: true,
    command_injection_detection: true,
};
let validation = RequestValidationMiddleware::new(config);
```

**Protects Against**:
- SQL injection
- Cross-site scripting (XSS)
- Path traversal attacks
- Command injection
- Oversized requests
- Null byte attacks
- Malicious user agents

### DDoS Protection Middleware

Protects against Distributed Denial of Service attacks.

```rust
use highper_gateway::middleware::ddos_protection::{
    DdosProtectionMiddleware, DdosProtectionConfig
};

// Strict protection (tight limits)
let ddos = DdosProtectionMiddleware::strict();

// API protection (balanced)
let ddos = DdosProtectionMiddleware::api();

// Custom configuration
let config = DdosProtectionConfig {
    max_connections_per_ip: 100,
    max_requests_per_second: 50,
    burst_size: 100,
    slowloris_timeout: Duration::from_secs(30),
    ban_duration: Duration::from_secs(300),
    ban_threshold: 10,
    whitelist: vec![],
    blacklist: vec![],
    ..Default::default()
};
let ddos = DdosProtectionMiddleware::new(config);
```

**Protects Against**:
- Connection flooding
- Request flooding
- Slowloris attacks
- Application-layer DDoS

### Security Audit Middleware

Comprehensive security event logging.

```rust
use highper_gateway::middleware::security_audit::{
    SecurityAuditMiddleware, SecurityAuditConfig
};

// Compliance-level auditing
let audit = SecurityAuditMiddleware::compliance();

// Strict auditing (log everything)
let audit = SecurityAuditMiddleware::strict();
```

**Logs**:
- Authentication attempts
- Authorization decisions
- Input validation failures
- Rate limit violations
- Suspicious request patterns
- TLS/mTLS events

### Security Headers Middleware

Adds modern security headers to all responses.

```rust
use highper_gateway::middleware::headers::{
    SecurityHeadersMiddleware, SecurityHeadersConfig
};

// OWASP-recommended strict headers
let headers = SecurityHeadersMiddleware::strict();

// API-optimized headers
let headers = SecurityHeadersMiddleware::api();
```

**Headers Configured**:
- `Strict-Transport-Security` (HSTS)
- `Content-Security-Policy` (CSP)
- `X-Content-Type-Options`
- `X-Frame-Options`
- `X-XSS-Protection`
- `Referrer-Policy`
- `Permissions-Policy`
- Cross-Origin policies (COEP, COOP, CORP)

## TLS/mTLS Configuration

### TLS Best Practices

**Minimum Configuration**:

```yaml
tls:
  auto: false  # Set to true for auto-ACME in production
  certificates:
    - cert: /path/to/cert.pem
      key: /path/to/key.pem
  min_version: "1.2"  # Minimum TLS 1.2, prefer 1.3
  session_cache:
    enabled: true
    size: 1024
  ocsp_stapling:
    enabled: true
```

**Recommended Cipher Suites** (TLS 1.3):
- `TLS_AES_128_GCM_SHA256`
- `TLS_AES_256_GCM_SHA384`
- `TLS_CHACHA20_POLY1305_SHA256`

**TLS 1.2 Fallback Ciphers**:
- `TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256`
- `TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384`

### Mutual TLS (mTLS)

For zero-trust service-to-service authentication:

```yaml
tls:
  certificates:
    - cert: /path/to/server-cert.pem
      key: /path/to/server-key.pem
  min_version: "1.2"
  mtls:
    enabled: true
    ca_cert: /path/to/ca-cert.pem
    verify_client: true
    verify_depth: 3
    crl_file: /path/to/crl.pem  # Optional: Certificate Revocation List
```

**mTLS Best Practices**:
1. Use short-lived certificates (90 days max)
2. Implement certificate rotation
3. Use certificate pinning where possible
4. Monitor certificate expiry
5. Maintain CRL or use OCSP

### Auto-ACME (Let's Encrypt)

For automatic TLS certificate management:

```yaml
tls:
  auto: true
  acme:
    provider: "letsencrypt"
    email: "admin@example.com"
    directory_url: "https://acme-v02.api.letsencrypt.org/directory"
    challenge_type: "http-01"
    storage:
      type: "file"
      path: "/var/lib/highper-gateway/acme"
    renewal_days: 30
    renew_check_interval: 3600s
  min_version: "1.3"  # ACME certs work with TLS 1.3
```

## Authentication & Authorization

### Admin API Security

**Always enable authentication**:

```yaml
admin:
  enabled: true
  bind: "127.0.0.1:9000"  # Bind to localhost only
  auth_enabled: true
  api_keys:
    - "${ADMIN_API_KEY}"  # Use environment variables
  jwt_secret: "${JWT_SECRET}"
  jwt_expiration: "24h"
  cors_enabled: false  # Disable CORS for admin API
  read_only: false
```

**Best Practices**:
1. **Never expose admin API to the internet**
2. Use strong API keys (32+ characters, random)
3. Rotate API keys regularly (90 days)
4. Use JWT for session management
5. Enable audit logging for all admin operations
6. Consider IP whitelisting

### Backend Authentication

For authenticating to upstream services:

```yaml
upstreams:
  - name: secure-backend
    servers:
      - address: "backend.internal:8080"
    auth:
      type: "bearer"
      token: "${BACKEND_TOKEN}"
    # Or use mTLS
    tls:
      client_cert: "/path/to/client-cert.pem"
      client_key: "/path/to/client-key.pem"
```

## Rate Limiting & DDoS Protection

### Rate Limiting Configuration

**Per-IP Rate Limiting**:

```yaml
rate_limit:
  enabled: true
  algorithm: token_bucket
  capacity: 1000  # Maximum burst
  refill_rate: 100  # Requests per second
  window: 60s
  key_type: ip
  cleanup_interval: 60s
```

**Per-User Rate Limiting**:

```yaml
rate_limit:
  enabled: true
  algorithm: token_bucket
  capacity: 5000
  refill_rate: 500
  window: 60s
  key_type: header
  key_header: "X-User-ID"
```

### DDoS Protection Layers

**Layer 1: Infrastructure**
- Use cloud-based DDoS protection (Cloudflare, AWS Shield)
- Implement anycast routing
- Use CDN for static content

**Layer 2: Gateway**
- Enable DDoS protection middleware
- Configure connection limits
- Set request timeouts
- Use geographic filtering if applicable

**Layer 3: Application**
- Implement circuit breakers
- Use rate limiting per endpoint
- Enable request validation
- Monitor and alert on anomalies

### Recommended Limits by Scenario

**Public API**:
- Max connections per IP: 100
- Requests per second: 50
- Burst size: 100
- Ban duration: 5 minutes

**Internal API**:
- Max connections per IP: 500
- Requests per second: 200
- Burst size: 500
- Ban duration: 1 minute

**Static Files**:
- Max connections per IP: 200
- Requests per second: 100
- Burst size: 200
- Ban duration: 2 minutes

## Input Validation

### Request Size Limits

```yaml
server:
  performance:
    read_buffer_size: 8192
    write_buffer_size: 8192
    max_connections: 10000
    request_timeout: 30s
```

**Recommended Limits**:
- **API endpoints**: 512 KB max body size
- **File uploads**: 10 MB max body size
- **WebSocket messages**: 16 MB max frame size
- **Headers**: 8 KB max total size
- **URL length**: 2 KB maximum

### Validation Strategies

1. **Whitelist over Blacklist**: Define what is allowed, not what is blocked
2. **Sanitize Input**: Remove potentially harmful characters
3. **Validate Data Types**: Ensure correct types (string, number, etc.)
4. **Check Bounds**: Validate ranges and lengths
5. **Use Regex Carefully**: Avoid ReDoS vulnerabilities

### Example: API Input Validation

```rust
// Validate JSON payload
fn validate_user_input(data: &serde_json::Value) -> Result<(), ValidationError> {
    // Check required fields
    let username = data.get("username")
        .and_then(|v| v.as_str())
        .ok_or(ValidationError::MissingField("username"))?;

    // Validate format
    if username.len() < 3 || username.len() > 32 {
        return Err(ValidationError::InvalidLength("username"));
    }

    // Validate pattern
    let username_regex = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    if !username_regex.is_match(username) {
        return Err(ValidationError::InvalidFormat("username"));
    }

    Ok(())
}
```

## Security Headers

### Content Security Policy (CSP)

**Strict CSP** (recommended for web applications):

```
Content-Security-Policy: default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  connect-src 'self';
  frame-ancestors 'none';
  base-uri 'self';
  form-action 'self'
```

**API CSP** (minimal):

```
Content-Security-Policy: default-src 'none'; frame-ancestors 'none'
```

### HSTS Configuration

**Production**:
```
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
```

**Development**:
```
Strict-Transport-Security: max-age=300
```

### Cross-Origin Policies

```yaml
# Enable modern cross-origin isolation
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
```

## WAF Integration

### ModSecurity Configuration

```yaml
waf:
  enabled: true
  mode: "blocking"  # or "detection" for monitoring
  provider: "modsecurity"
  modsecurity:
    rules_path: "/etc/modsecurity/rules"
    audit_log: "/var/log/modsecurity/audit.log"
    request_body_limit: 1048576  # 1 MB
    anomaly_scoring:
      threshold: 5
      inbound_threshold: 5
      outbound_threshold: 4
    paranoia_level: 1  # 1-4, higher = stricter
    crs_version: "3.3"
```

### OWASP Core Rule Set (CRS)

**Paranoia Levels**:
- **Level 1** (default): Basic protection, low false positives
- **Level 2**: Enhanced protection, some false positives
- **Level 3**: Strict protection, moderate false positives
- **Level 4**: Maximum protection, high false positives

**Recommended**: Start with Level 1, tune rules, then increase.

### Custom WAF Rules

```yaml
waf:
  modsecurity:
    custom_rules:
      - id: 90001
        phase: 1
        action: "deny"
        msg: "Block suspicious User-Agents"
        rule: |
          SecRule REQUEST_HEADERS:User-Agent "@rx (?:bot|crawler|spider)" \
            "id:90001,phase:1,deny,status:403,msg:'Suspicious User-Agent'"

      - id: 90002
        phase: 2
        action: "deny"
        msg: "Block SQL injection attempts"
        rule: |
          SecRule ARGS "@detectSQLi" \
            "id:90002,phase:2,deny,status:403,msg:'SQL Injection Detected'"
```

## Security Audit Logging

### What to Log

**Authentication Events**:
- Login attempts (success/failure)
- Logout events
- Token generation/revocation
- Password changes
- MFA events

**Authorization Events**:
- Access granted/denied
- Permission changes
- Role assignments

**Security Events**:
- Input validation failures
- Rate limit violations
- WAF rule triggers
- DDoS protection activations
- Certificate errors
- Suspicious patterns

### Log Format

Use structured logging (JSON) for easy parsing:

```json
{
  "timestamp": "2025-12-25T12:00:00.000Z",
  "event_type": "auth_attempt",
  "severity": "warning",
  "client_ip": "192.168.1.100",
  "method": "POST",
  "path": "/api/login",
  "user_agent": "Mozilla/5.0...",
  "user_id": null,
  "message": "Authentication failed",
  "context": {
    "failure_reason": "invalid_credentials",
    "username": "admin"
  }
}
```

### Log Storage & Retention

**Best Practices**:
1. Store logs in a separate, secure system (SIEM)
2. Encrypt logs at rest and in transit
3. Implement log rotation
4. Retain logs for compliance requirements (typically 90-365 days)
5. Monitor log integrity (use checksums)
6. Alert on suspicious patterns

### SIEM Integration

Send logs to SIEM systems:

```yaml
observability:
  logging:
    level: info
    format: json
    output: stdout  # Pipe to log shipper
  tracing:
    enabled: true
    endpoint: "http://jaeger:14268/api/traces"
    sample_rate: 0.1
```

**Popular SIEM Solutions**:
- Splunk
- ELK Stack (Elasticsearch, Logstash, Kibana)
- Datadog
- Sumo Logic
- Azure Sentinel

## Network Security

### Network Segmentation

```
┌─────────────────┐
│   Internet      │
└────────┬────────┘
         │
┌────────▼────────┐
│  DMZ (Gateway)  │  ← Highper Gateway
└────────┬────────┘
         │
┌────────▼────────┐
│  Private Net    │  ← Backend services
│  (Backends)     │
└─────────────────┘
```

**Firewall Rules**:
- Allow inbound: 80 (HTTP), 443 (HTTPS), 8443 (TLS)
- Allow outbound: Backend services only
- Block all other traffic
- Admin API accessible from bastion host only

### IP Whitelisting

```yaml
# For admin API
admin:
  bind: "0.0.0.0:9000"
  auth_enabled: true
  ip_whitelist:
    - "10.0.0.0/8"      # Internal network
    - "172.16.0.0/12"   # Docker network
    - "192.168.1.100"   # Specific admin IP
```

### VPC/Private Networks

**AWS Example**:
```
VPC: 10.0.0.0/16
├── Public Subnet: 10.0.1.0/24  (Gateway)
└── Private Subnet: 10.0.2.0/24 (Backends)
```

**Security Groups**:
- Gateway SG: Allow 80/443 from 0.0.0.0/0
- Backend SG: Allow 8080 from Gateway SG only

## Secrets Management

### Never Commit Secrets

**Bad**:
```yaml
admin:
  api_keys:
    - "hardcoded-secret-key-12345"  # ❌ NEVER DO THIS
```

**Good**:
```yaml
admin:
  api_keys:
    - "${ADMIN_API_KEY}"  # ✅ Use environment variables
```

### Environment Variables

```bash
# .env file (DO NOT COMMIT)
ADMIN_API_KEY=randomly-generated-32-character-key-here
TLS_CERT_PATH=/secrets/tls-cert.pem
TLS_KEY_PATH=/secrets/tls-key.pem
DATABASE_PASSWORD=secure-password-here
```

### External Secrets Management

**AWS Secrets Manager**:
```bash
export ADMIN_API_KEY=$(aws secretsmanager get-secret-value \
  --secret-id prod/gateway/admin-key \
  --query SecretString \
  --output text)
```

**HashiCorp Vault**:
```bash
export ADMIN_API_KEY=$(vault kv get -field=admin_key secret/gateway/prod)
```

**Kubernetes Secrets**:
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: gateway-secrets
type: Opaque
data:
  admin-api-key: <base64-encoded-key>
```

### Certificate Management

1. Store private keys in encrypted form
2. Use HSM for production certificate keys
3. Implement certificate rotation
4. Monitor certificate expiry (alert 30 days before)
5. Use certificate pinning for high-security scenarios

## Monitoring & Alerting

### Security Metrics

Monitor these metrics:

**Authentication**:
- Failed login attempts (rate)
- Successful logins (rate)
- Token generation rate

**Rate Limiting**:
- Rate limit violations per IP
- Total requests blocked
- Top violating IPs

**Input Validation**:
- Validation failures by type
- Top attack patterns detected

**DDoS**:
- Requests per second
- Connections per IP
- Banned IPs count

### Alert Thresholds

**Critical Alerts** (immediate response):
- > 100 failed auth attempts in 1 minute
- > 1000 rate limit violations in 1 minute
- > 50 validation failures from single IP in 1 minute
- Certificate expiring in < 7 days
- mTLS verification failures > 10 in 1 minute

**Warning Alerts** (monitor):
- > 10 failed auth attempts in 1 minute
- > 100 rate limit violations in 5 minutes
- CPU > 80% for 5 minutes
- Memory > 85% for 5 minutes

### Monitoring Stack

**Recommended Setup**:
```yaml
observability:
  metrics:
    enabled: true
    bind: "0.0.0.0:9090"
    endpoint: "/metrics"

  # Prometheus scraping
  # Alert Manager for notifications
  # Grafana for visualization
```

**Prometheus Alert Rules**:
```yaml
groups:
  - name: security
    rules:
      - alert: HighAuthFailureRate
        expr: rate(auth_failures_total[1m]) > 100
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "High authentication failure rate detected"
```

## Incident Response

### Incident Response Plan

1. **Detection**: Monitor alerts, logs, metrics
2. **Containment**: Block malicious IPs, isolate affected systems
3. **Eradication**: Remove threat, patch vulnerabilities
4. **Recovery**: Restore services, verify integrity
5. **Lessons Learned**: Document incident, improve defenses

### Emergency Procedures

**Under Attack**:
```bash
# 1. Enable strict DDoS protection
# Update configuration
ddos_protection:
  mode: strict
  ban_duration: 3600s  # 1 hour
  ban_threshold: 3

# 2. Block specific IPs
curl -X POST http://localhost:9000/admin/blacklist \
  -H "X-API-Key: ${ADMIN_API_KEY}" \
  -d '{"ip": "1.2.3.4"}'

# 3. Enable WAF blocking mode
waf:
  mode: blocking
  paranoia_level: 2

# 4. Reduce rate limits
rate_limit:
  capacity: 100
  refill_rate: 10
```

**Compromise Recovery**:
```bash
# 1. Rotate all secrets
# 2. Review access logs
# 3. Check for backdoors
# 4. Update dependencies
# 5. Restore from known-good backup
```

### Logging & Evidence Collection

Always preserve evidence:
```bash
# Capture current state
journalctl -u highper-gateway > /tmp/gateway-logs.txt
cp /var/log/highper-gateway/audit.log /tmp/audit-backup.log
netstat -an > /tmp/network-state.txt

# Create forensic image (if needed)
tar czf /tmp/forensics-$(date +%Y%m%d-%H%M%S).tar.gz \
  /var/log/highper-gateway/ \
  /etc/highper-gateway/ \
  /tmp/gateway-logs.txt
```

## Compliance

### GDPR Compliance

**Data Protection**:
- Encrypt PII in transit (TLS 1.2+)
- Encrypt PII at rest
- Implement data retention policies
- Provide data deletion mechanisms
- Log data access for audit trails

**Configuration**:
```yaml
observability:
  logging:
    # Mask PII in logs
    mask_fields:
      - "password"
      - "credit_card"
      - "ssn"
      - "email"
    retention_days: 90
```

### PCI DSS Compliance

**Requirements**:
- Use TLS 1.2 or higher ✅
- Implement strong access control ✅
- Maintain audit logs ✅
- Use WAF for web applications ✅
- Encrypt cardholder data ✅
- Regular security testing ✅

**Additional Controls**:
```yaml
# Disable weak ciphers
tls:
  min_version: "1.2"
  ciphers:
    - TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
    - TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256

# Enable comprehensive audit logging
security_audit:
  mode: compliance
  log_all_access: true
  retention_days: 365
```

### HIPAA Compliance

**Technical Safeguards**:
- Access controls (authentication/authorization) ✅
- Audit controls (logging) ✅
- Integrity controls (checksums) ✅
- Transmission security (TLS/mTLS) ✅

**Configuration**:
```yaml
# Enable mTLS for PHI transmission
tls:
  mtls:
    enabled: true
    verify_client: true

# Comprehensive audit logging
security_audit:
  log_auth_attempts: true
  log_data_access: true
  retention_days: 2555  # 7 years
```

### SOC 2 Compliance

**Control Categories**:
- Security ✅
- Availability ✅
- Processing Integrity ✅
- Confidentiality ✅
- Privacy ✅

**Evidence Collection**:
```yaml
observability:
  # Comprehensive logging for audit evidence
  logging:
    level: info
    format: json
    output: /var/log/highper-gateway/security.log

  # Metrics for availability monitoring
  metrics:
    enabled: true
    retention_days: 365
```

## Security Checklist

Use this checklist before going to production:

### Infrastructure
- [ ] TLS 1.2+ configured
- [ ] Strong cipher suites only
- [ ] Auto-ACME configured for cert renewal
- [ ] Network segmentation implemented
- [ ] Firewall rules configured
- [ ] DDoS protection enabled (cloud + gateway)

### Authentication & Authorization
- [ ] Admin API auth enabled
- [ ] Strong API keys (32+ chars)
- [ ] API keys stored in secrets manager
- [ ] mTLS configured (if required)
- [ ] Role-based access control implemented

### Input Validation
- [ ] Request validation middleware enabled
- [ ] Request size limits configured
- [ ] SQL injection protection enabled
- [ ] XSS protection enabled
- [ ] Path traversal protection enabled

### Rate Limiting
- [ ] Global rate limiting configured
- [ ] Per-endpoint rate limits set
- [ ] DDoS protection middleware enabled
- [ ] IP whitelisting/blacklisting configured

### Security Headers
- [ ] HSTS enabled (max-age >= 1 year)
- [ ] CSP configured
- [ ] X-Content-Type-Options: nosniff
- [ ] X-Frame-Options: DENY
- [ ] Cross-origin policies configured

### WAF (if applicable)
- [ ] ModSecurity/Coraza configured
- [ ] OWASP CRS rules enabled
- [ ] Custom rules for application
- [ ] Audit logging enabled
- [ ] Anomaly scoring tuned

### Logging & Monitoring
- [ ] Security audit logging enabled
- [ ] Logs sent to SIEM
- [ ] Critical alerts configured
- [ ] Metrics monitoring active
- [ ] Incident response plan documented

### Secrets Management
- [ ] No secrets in configuration files
- [ ] Environment variables used
- [ ] Secrets manager integrated
- [ ] Certificate rotation automated
- [ ] API key rotation schedule established

### Compliance (if applicable)
- [ ] GDPR requirements met
- [ ] PCI DSS controls implemented
- [ ] HIPAA safeguards configured
- [ ] SOC 2 evidence collection active
- [ ] Audit trail complete

## Support & Resources

- **Security Issues**: Report to security@highper-gateway.io
- **Documentation**: https://docs.highper-gateway.io/security
- **CVE Database**: https://github.com/highper/highper-gateway/security/advisories
- **Community**: https://discord.gg/highper-gateway

## Security Updates

Subscribe to security announcements:
- GitHub Security Advisories
- Mailing list: security-announce@highper-gateway.io
- RSS feed: https://highper-gateway.io/security.rss

---

**Last Updated**: 2025-12-25
**Document Version**: 1.0.0
