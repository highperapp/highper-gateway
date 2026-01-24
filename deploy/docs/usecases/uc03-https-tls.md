# Use Case 03: HTTPS/TLS Termination

TLS termination with modern cipher suites, OCSP stapling, and session resumption.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTPS (Layer 7) |
| Ports | 443, 8443 |
| TLS Required | Yes |
| Privileged | Yes |
| Scaling | Horizontal |

## When to Use

- Centralized TLS certificate management
- Offloading TLS from backend servers
- Enforcing TLS 1.2+ and modern ciphers
- HSTS and security headers

## Architecture

```
                         ┌──────────────────────┐
    HTTPS:443  ────────▶ │                      │ ────▶ HTTP Backend
                         │   Highper Gateway    │
    (TLS 1.2/1.3)        │  (TLS Termination)   │ ────▶ HTTP Backend
                         │                      │
                         └──────────────────────┘
```

## Quick Start

### 1. Generate Certificates

```bash
# Production: Use Let's Encrypt or your CA
# Testing: Self-signed
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/highper-gateway/certs/server.key \
  -out /etc/highper-gateway/certs/server.crt \
  -subj "/CN=gateway.example.com"
```

### 2. Deploy Configuration

```bash
cp configs/yaml/uc03-https-tls.yaml /etc/highper-gateway/config.yaml
systemctl start highper-gateway
```

## TLS Configuration

### Modern TLS Settings

```yaml
tls:
  default:
    cert: /etc/highper-gateway/certs/server.crt
    key: /etc/highper-gateway/certs/server.key
    protocols:
      - TLSv1.2
      - TLSv1.3
    ciphers:
      - TLS_AES_256_GCM_SHA384
      - TLS_CHACHA20_POLY1305_SHA256
      - ECDHE-ECDSA-AES256-GCM-SHA384
      - ECDHE-RSA-AES256-GCM-SHA384
    prefer_server_ciphers: true
```

### Session Resumption

```yaml
tls:
  default:
    session_cache: shared
    session_timeout: 3600
    session_tickets: true
```

### OCSP Stapling

```yaml
tls:
  default:
    ocsp_stapling: true
    ocsp_stapling_verify: true
```

## Security Headers

### HSTS

```yaml
hsts:
  enabled: true
  max_age: 31536000
  include_subdomains: true
  preload: true
```

### Response Headers

```yaml
headers:
  response:
    add:
      - name: X-Content-Type-Options
        value: nosniff
      - name: X-Frame-Options
        value: DENY
      - name: X-XSS-Protection
        value: "1; mode=block"
```

## Certificate Rotation

Use the TLS rotation playbook for zero-downtime certificate updates:

```bash
ansible-playbook playbooks/tls-rotate.yml \
  -e "cert_file=new-cert.pem" \
  -e "key_file=new-key.pem"
```

## Testing

```bash
# Check TLS configuration
openssl s_client -connect localhost:443 -tls1_3

# Verify certificate
curl -v https://localhost:443/health --insecure
```

## Related Use Cases

- [UC04: API Gateway](./uc04-api-gateway.md) - Add authentication
- [UC09: WAF + mTLS](./uc09-waf-mtls.md) - Add client certificates
