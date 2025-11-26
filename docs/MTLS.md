# Mutual TLS (mTLS) Support

**Status:** ✅ Implemented in Phase 1.3
**Version:** 0.1.0+

## Overview

Rust Proxy supports mutual TLS (mTLS) authentication, allowing you to verify client certificates and implement certificate-based authentication and authorization. This enables secure machine-to-machine communication and fine-grained access control.

## Features

- ✅ **Client Certificate Verification** - Verify clients using CA certificates
- ✅ **Multiple Verification Modes** - Required, Optional, or OptionalNoCA
- ✅ **Certificate Information Extraction** - Parse and extract cert details
- ✅ **HTTP Header Injection** - Forward cert info to backend services
- ✅ **Per-Route Policies** - Different mTLS requirements per route
- ✅ **Fingerprint Whitelisting** - Allow specific certificates by fingerprint
- ✅ **Certificate Attributes** - DN, serial, issuer, validity dates
- ✅ **Zero Overhead** - Only active when mTLS is configured

## Quick Start

### 1. Generate Certificates

First, generate a CA certificate and client certificate:

```bash
# Generate CA private key and certificate
openssl genrsa -out ca-key.pem 4096
openssl req -new -x509 -key ca-key.pem -out ca-cert.pem -days 3650 \
  -subj "/C=US/ST=State/L=City/O=MyOrg/CN=My Root CA"

# Generate client private key
openssl genrsa -out client-key.pem 4096

# Generate client certificate signing request
openssl req -new -key client-key.pem -out client-csr.pem \
  -subj "/C=US/ST=State/L=City/O=MyOrg/CN=client.example.com"

# Sign client certificate with CA
openssl x509 -req -in client-csr.pem -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out client-cert.pem -days 365
```

### 2. Configure mTLS

Add mTLS configuration to your `config.yaml`:

```yaml
server:
  bind: ["0.0.0.0:8443"]

# TLS configuration with mTLS
tls:
  enabled: true
  cert_file: "/path/to/server-cert.pem"
  key_file: "/path/to/server-key.pem"

  # mTLS configuration
  mtls:
    enabled: true
    ca_cert_path: "/path/to/ca-cert.pem"

    # Global verification mode
    verification_mode: optional  # required | optional | optional_no_ca

    # Optional: Additional CA certificates
    additional_cas:
      - "/path/to/another-ca.pem"

# Routes
routes:
  - path: "/api"
    upstream: backend

    # Optional: Per-route mTLS policy
    mtls:
      verification_mode: required  # Override global setting
```

### 3. Test with curl

```bash
# Request with client certificate
curl --cert client-cert.pem --key client-key.pem \
  --cacert ca-cert.pem \
  https://localhost:8443/api

# Request without certificate (will fail if required mode)
curl --cacert ca-cert.pem https://localhost:8443/api
```

## Verification Modes

### Required Mode

Client certificate is **mandatory**. Connections without a valid client certificate are rejected during TLS handshake.

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/path/to/ca-cert.pem"
    verification_mode: required
```

**Use cases:**
- Internal microservices communication
- High-security APIs
- Machine-to-machine authentication

**Behavior:**
- ✅ Valid client cert: Connection accepted
- ❌ No client cert: TLS handshake fails
- ❌ Invalid client cert: TLS handshake fails

### Optional Mode

Client certificate is **requested but not required**. Connections are allowed with or without a client certificate.

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/path/to/ca-cert.pem"
    verification_mode: optional
```

**Use cases:**
- Mixed authentication (cert + API key)
- Gradual mTLS rollout
- Optional identity verification

**Behavior:**
- ✅ Valid client cert: Connection accepted, cert info available
- ✅ No client cert: Connection accepted, no cert info
- ❌ Invalid client cert: TLS handshake fails

### Optional No CA Mode

Client certificate is requested, verified if provided, but connection allowed without certificate.

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/path/to/ca-cert.pem"
    verification_mode: optional_no_ca
```

**Use cases:**
- Self-signed client certificates
- Permissive development environments
- Certificate fingerprint whitelisting

**Behavior:**
- ✅ Valid client cert: Connection accepted, cert info available
- ✅ No client cert: Connection accepted, no cert info
- ⚠️ Invalid client cert: Connection accepted, cert marked as unverified

## Per-Route Policies

You can override the global mTLS policy on a per-route basis:

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/path/to/ca-cert.pem"
    verification_mode: optional  # Global default

routes:
  # Public API - no client cert required
  - path: "/public"
    upstream: public_backend
    # No mtls override = use global (optional)

  # Authenticated API - client cert required
  - path: "/admin"
    upstream: admin_backend
    mtls:
      verification_mode: required

  # Internal API - specific clients only
  - path: "/internal"
    upstream: internal_backend
    mtls:
      verification_mode: required
      allowed_fingerprints:
        - "A1B2C3D4E5F6..."  # SHA-256 fingerprint
        - "F6E5D4C3B2A1..."
```

### Policy Fields

```yaml
mtls:
  # Verification mode (overrides global)
  verification_mode: required  # optional | optional_no_ca

  # Allowed client certificate subjects (DN patterns)
  allowed_subjects:
    - "CN=service-a.example.com"
    - "CN=*.internal.example.com"

  # Allowed client certificate issuers (DN patterns)
  allowed_issuers:
    - "CN=Internal CA"

  # Allowed certificate serial numbers
  allowed_serials:
    - "01234567890ABCDEF"

  # Allowed certificate fingerprints (SHA-256, hex)
  allowed_fingerprints:
    - "A1B2C3D4E5F6..."
```

## Certificate Information Headers

When a client certificate is provided, Rust Proxy extracts information and forwards it to backend services as HTTP headers:

| Header | Description | Example |
|--------|-------------|---------|
| `X-Client-Cert-Subject` | Certificate subject DN | `CN=client.example.com,O=MyOrg,C=US` |
| `X-Client-Cert-Issuer` | Certificate issuer DN | `CN=My Root CA,O=MyOrg,C=US` |
| `X-Client-Cert-Serial` | Serial number (hex) | `01234567890ABCDEF` |
| `X-Client-Cert-Fingerprint` | SHA-256 fingerprint (hex) | `A1B2C3D4E5F6...` |
| `X-Client-Cert-CN` | Common name from subject | `client.example.com` |
| `X-Client-Cert-Not-Before` | Validity start (UNIX timestamp) | `1609459200` |
| `X-Client-Cert-Not-After` | Validity end (UNIX timestamp) | `1672531200` |

### Backend Usage

Your backend services can read these headers to:

- Identify the client
- Make authorization decisions
- Audit access
- Implement custom logic based on certificate attributes

**Example (Node.js/Express):**

```javascript
app.get('/api/data', (req, res) => {
  const clientCN = req.headers['x-client-cert-cn'];
  const clientFingerprint = req.headers['x-client-cert-fingerprint'];

  console.log(`Request from: ${clientCN} (${clientFingerprint})`);

  // Your authorization logic
  if (isAuthorized(clientCN)) {
    res.json({ data: "secret" });
  } else {
    res.status(403).json({ error: "Forbidden" });
  }
});
```

## Certificate Fingerprint Whitelisting

Use fingerprint whitelisting when you want to allow specific certificates rather than all certificates signed by a CA:

### 1. Get Certificate Fingerprint

```bash
# Get fingerprint of a certificate
openssl x509 -in client-cert.pem -noout -fingerprint -sha256 | \
  cut -d '=' -f 2 | tr -d ':' | tr '[:upper:]' '[:lower:]'
```

Example output: `a1b2c3d4e5f6789012345678901234567890abcdef0123456789abcdef012345`

### 2. Configure Whitelist

```yaml
routes:
  - path: "/internal"
    upstream: internal_backend
    mtls:
      verification_mode: required
      allowed_fingerprints:
        - "a1b2c3d4e5f6789012345678901234567890abcdef0123456789abcdef012345"
        - "b2c3d4e5f6a1789012345678901234567890abcdef0123456789abcdef012345"
```

### 3. Behavior

- ✅ Certificate with matching fingerprint: Request allowed
- ❌ Certificate with non-matching fingerprint: 403 Forbidden
- ❌ No certificate: Depends on verification_mode

**Use cases:**
- Specific service-to-service communication
- Certificate rotation without CA re-signing
- Temporary certificate access grants

## Configuration Examples

### Example 1: Internal Microservices

All services must authenticate with client certificates:

```yaml
tls:
  enabled: true
  cert_file: "/etc/certs/server-cert.pem"
  key_file: "/etc/certs/server-key.pem"

  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/internal-ca.pem"
    verification_mode: required

routes:
  - path: "/api"
    upstream: backend_service
    # All requests require valid client cert
```

### Example 2: Mixed Public/Private API

Public endpoints don't require certs, admin endpoints do:

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/ca.pem"
    verification_mode: optional  # Global: optional

routes:
  # Public API - no cert needed
  - path: "/public"
    upstream: public_api
    # Uses global: optional

  # Admin API - cert required
  - path: "/admin"
    upstream: admin_api
    mtls:
      verification_mode: required
```

### Example 3: Certificate Fingerprint Whitelist

Only specific known clients can access:

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/ca.pem"
    verification_mode: required

routes:
  - path: "/partner-api"
    upstream: partner_backend
    mtls:
      verification_mode: required
      allowed_fingerprints:
        # Partner A's certificate
        - "a1b2c3d4e5f6789012345678901234567890abcdef0123456789abcdef012345"
        # Partner B's certificate
        - "b2c3d4e5f6a1789012345678901234567890abcdef0123456789abcdef012345"
```

### Example 4: Multiple CAs

Trust certificates from multiple certificate authorities:

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/primary-ca.pem"
    additional_cas:
      - "/etc/certs/partner-ca.pem"
      - "/etc/certs/legacy-ca.pem"
    verification_mode: required
```

## Error Responses

### 403 Forbidden - No Certificate

When `verification_mode: required` and no client certificate is provided:

```
HTTP/1.1 403 Forbidden
Content-Type: text/plain

403 Forbidden: Client certificate required but not provided
```

### 403 Forbidden - Fingerprint Mismatch

When certificate doesn't match whitelist:

```
HTTP/1.1 403 Forbidden
Content-Type: text/plain

403 Forbidden: Client certificate not authorized for this route
```

### TLS Handshake Failure

When client provides an invalid or untrusted certificate:

```
curl: (35) error:14094410:SSL routines:ssl3_read_bytes:sslv3 alert handshake failure
```

## Monitoring and Logging

### Log Messages

Rust Proxy logs mTLS events:

```
INFO  TLS handshake completed successfully
DEBUG Checking mTLS policy: mode=Required, has_cert=true
DEBUG Client certificate fingerprint matches whitelist
INFO  Injecting client certificate headers: subject=CN=client.example.com
```

### Failed Authentication

```
WARN  Client certificate required but not provided
WARN  Client certificate fingerprint not in whitelist: a1b2c3...
```

### Metrics

mTLS metrics are exposed at `/metrics`:

```
# Client certificate verification attempts
mtls_verification_total{status="success"} 1234
mtls_verification_total{status="no_cert"} 56
mtls_verification_total{status="invalid_cert"} 12
mtls_verification_total{status="whitelist_mismatch"} 3

# Per-route policy enforcement
mtls_policy_check_total{route="/admin",result="pass"} 1000
mtls_policy_check_total{route="/admin",result="fail"} 5
```

## Troubleshooting

### Problem: "Client certificate required but not provided"

**Cause:** Server requires client cert but client didn't provide one.

**Solutions:**

1. Provide client certificate:
   ```bash
   curl --cert client-cert.pem --key client-key.pem https://...
   ```

2. Change to optional mode:
   ```yaml
   verification_mode: optional
   ```

### Problem: TLS Handshake Failure

**Cause:** Client certificate is not trusted (not signed by configured CA).

**Solutions:**

1. Check CA certificate is correct:
   ```bash
   openssl verify -CAfile ca-cert.pem client-cert.pem
   ```

2. Add client cert's CA to `additional_cas`:
   ```yaml
   additional_cas:
     - "/path/to/client-ca.pem"
   ```

### Problem: "Certificate not authorized for this route"

**Cause:** Certificate fingerprint doesn't match whitelist.

**Solutions:**

1. Get certificate fingerprint:
   ```bash
   openssl x509 -in client-cert.pem -noout -fingerprint -sha256
   ```

2. Add to whitelist or remove whitelist restriction.

### Problem: Headers not appearing in backend

**Cause:** mTLS headers not being injected.

**Solutions:**

1. Verify certificate is provided:
   ```bash
   curl -v --cert client-cert.pem --key client-key.pem https://...
   ```

2. Check logs for "Injecting client certificate headers"

3. Verify middleware is enabled (automatic with mTLS)

## Security Best Practices

### 1. Use Strong Certificates

```bash
# Generate 4096-bit RSA key
openssl genrsa -out key.pem 4096

# Or use Ed25519 (faster, more secure)
openssl genpkey -algorithm Ed25519 -out key.pem
```

### 2. Set Appropriate Certificate Lifetimes

- **CA certificates:** 10+ years
- **Server certificates:** 1-2 years
- **Client certificates:** 90-365 days (shorter for automated rotation)

### 3. Rotate Certificates Regularly

```bash
# Automate certificate renewal
0 0 * * 0 /usr/local/bin/renew-client-certs.sh
```

### 4. Use Certificate Revocation

Configure CRL (Certificate Revocation List) checking:

```yaml
tls:
  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/ca.pem"
    crl_path: "/etc/certs/ca.crl"  # Optional
```

### 5. Monitor Certificate Expiration

```bash
# Check certificate expiry
openssl x509 -in cert.pem -noout -enddate
```

### 6. Secure Private Keys

```bash
# Set restrictive permissions
chmod 600 /etc/certs/*.key
chown root:root /etc/certs/*.key

# Use hardware security modules (HSM) for production
```

### 7. Implement Defense in Depth

Don't rely solely on mTLS:

```yaml
routes:
  - path: "/api"
    upstream: backend
    mtls:
      verification_mode: required

    # Also use other auth methods
    auth:
      jwt:
        enabled: true

    # Rate limiting
    ratelimit:
      requests_per_second: 100
```

## Integration with Other Features

### JWT + mTLS

Combine certificate authentication with JWT:

```yaml
routes:
  - path: "/api"
    upstream: backend

    # Require client certificate
    mtls:
      verification_mode: required

    # Also require JWT
    auth:
      jwt:
        enabled: true
        secret: "your-secret"
```

### Rate Limiting per Certificate

Use client certificate CN for rate limiting:

```yaml
routes:
  - path: "/api"
    upstream: backend
    mtls:
      verification_mode: required
    ratelimit:
      requests_per_second: 100
      # Backend can use X-Client-Cert-CN for per-client limits
```

### CORS with mTLS

```yaml
routes:
  - path: "/api"
    upstream: backend
    mtls:
      verification_mode: optional
    cors:
      enabled: true
      allow_origins: ["https://example.com"]
```

## See Also

- [TLS Configuration](TLS.md)
- [Certificate Hot Reload](HOT_RELOAD.md)
- [Security Best Practices](SECURITY.md)
- [Monitoring](MONITORING.md)

## FAQ

**Q: Can I use self-signed client certificates?**

A: Yes, use `verification_mode: optional_no_ca` and implement fingerprint whitelisting.

**Q: How do I rotate client certificates without downtime?**

A: Use optional mode during rotation, or add both old and new fingerprints to whitelist.

**Q: Can I use different CAs for different routes?**

A: Not currently. All routes use the same CA configuration. Use fingerprint whitelisting per route instead.

**Q: What happens if the CA certificate expires?**

A: New client connections will fail. Renew and reload the CA certificate before expiration.

**Q: Can I extract custom certificate extensions?**

A: Not currently. Standard fields (DN, serial, etc.) are extracted. Custom extensions require code changes.

**Q: How does mTLS affect performance?**

A: Minimal impact (< 1ms per request). TLS handshake is slightly slower but happens once per connection.

---

**mTLS Status:** ✅ Production Ready
**Last Updated:** October 30, 2025
