# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.1.x   | :white_check_mark: |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**DO NOT** create a public GitHub issue for security vulnerabilities.

Instead, please report security vulnerabilities by emailing:

**security@highper.dev**

Or use GitHub's private vulnerability reporting:
1. Go to the Security tab of this repository
2. Click "Report a vulnerability"
3. Fill out the form with details

### What to Include

Please include the following in your report:

- **Description**: A clear description of the vulnerability
- **Impact**: What an attacker could achieve
- **Steps to Reproduce**: Detailed steps to reproduce the issue
- **Affected Versions**: Which versions are affected
- **Possible Fix**: If you have suggestions for fixing the issue
- **Your Contact**: How we can reach you for follow-up

### Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Fix Timeline**: Depends on severity
  - Critical: 24-72 hours
  - High: 1-2 weeks
  - Medium: 2-4 weeks
  - Low: Next release cycle

### Disclosure Policy

- We will acknowledge your report within 48 hours
- We will keep you informed of our progress
- We will credit you in the security advisory (unless you prefer anonymity)
- We ask that you give us reasonable time to fix the issue before public disclosure
- We follow coordinated disclosure practices

## Security Features

Highper Gateway includes comprehensive security features:

### Authentication
- JWT token validation (RS256, HS256, ES256)
- OAuth2/OIDC with PKCE support
- API key authentication
- mTLS client certificate verification

### Authorization
- Role-based access control
- Scope-based permissions
- Route-level authorization

### Web Application Firewall (WAF)
- SQL injection protection (17 patterns)
- XSS protection (13 patterns)
- Path traversal protection
- OWASP Core Rule Set (Coraza engine)
- ModSecurity v3 compatible
- AWS WAF integration

### Transport Security
- TLS 1.2 and 1.3 support
- Automatic HTTPS with ACME
- OCSP stapling
- CRL checking
- Certificate pinning support

### Security Headers
- Content-Security-Policy (CSP)
- Strict-Transport-Security (HSTS)
- X-Content-Type-Options
- X-Frame-Options
- X-XSS-Protection
- Referrer-Policy

### Rate Limiting & DDoS Protection
- Token bucket algorithm
- Sliding window rate limiting
- Per-IP connection limits
- Slowloris attack prevention
- Automatic IP banning

### Secure Configuration
- Secure-by-default settings
- Environment variable support for secrets
- No hardcoded credentials
- Configuration validation

## Security Best Practices

When deploying Highper Gateway:

1. **Enable TLS**: Always use HTTPS in production
2. **Enable WAF**: Use Coraza engine with OWASP CRS
3. **Use Strict Headers**: Apply "strict" security header preset
4. **Rotate Secrets**: Regularly rotate JWT secrets and API keys
5. **Monitor Logs**: Enable security audit logging
6. **Update Regularly**: Keep Highper Gateway updated
7. **Principle of Least Privilege**: Only enable needed features

## Security Audits

- Internal security review: January 2026
- OWASP Top 10 compliance: Verified
- No known critical vulnerabilities

## Acknowledgments

We thank the following security researchers for responsibly disclosing vulnerabilities:

*No vulnerabilities reported yet.*

---

Thank you for helping keep Highper Gateway and its users safe!
