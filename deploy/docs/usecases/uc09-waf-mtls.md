# Use Case 09: WAF + Mutual TLS

Web Application Firewall with mutual TLS (client certificates) for zero-trust security.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTPS (mTLS) |
| Ports | 443 |
| TLS Required | Yes (mutual) |
| Privileged | Yes |
| Scaling | Edge/PoP |

## When to Use

- Zero-trust architectures
- B2B API authentication
- PCI-DSS compliance
- Protection against OWASP Top 10

## Architecture

```
                         ┌──────────────────────┐
                         │   WAF Rules          │
    mTLS:443   ────────▶ │   ├── SQL Injection  │ ────▶ Backend
    (client cert)        │   ├── XSS            │
                         │   ├── Path Traversal │
                         │   └── Rate Limiting  │
                         │                      │
                         │   Client Cert Auth   │
                         └──────────────────────┘
```

## Quick Start

### 1. Set Up PKI

```bash
# Create CA
openssl genrsa -out ca.key 4096
openssl req -x509 -new -nodes -key ca.key -days 3650 -out ca.crt

# Create client certificate
openssl genrsa -out client.key 2048
openssl req -new -key client.key -out client.csr
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out client.crt -days 365
```

### 2. Deploy

```bash
cp configs/yaml/uc09-waf-mtls.yaml /etc/highper-gateway/config.yaml
cp ca.crt /etc/highper-gateway/certs/
systemctl start highper-gateway
```

## mTLS Configuration

```yaml
tls:
  mtls:
    cert: /etc/highper-gateway/certs/server.crt
    key: /etc/highper-gateway/certs/server.key
    ca: /etc/highper-gateway/certs/ca.crt
    client_auth: required  # required, request, or none
    verify_depth: 3
    crl:
      enabled: true
      path: /etc/highper-gateway/certs/crl.pem
```

## WAF Rules

### SQL Injection Protection

```yaml
waf:
  rules:
    sql_injection:
      enabled: true
      sensitivity: high
```

### XSS Protection

```yaml
waf:
  rules:
    xss:
      enabled: true
      sensitivity: high
```

### Request Limits

```yaml
waf:
  rules:
    request_limits:
      max_request_size: 10485760
      max_uri_length: 8192
      max_header_size: 16384
```

### IP Reputation

```yaml
waf:
  ip_reputation:
    enabled: true
    blocklist: /etc/highper-gateway/rules/ip-blocklist.txt
    allowlist: /etc/highper-gateway/rules/ip-allowlist.txt
```

## Client Certificate Headers

Pass client cert info to backends:

```yaml
client_cert:
  extract_dn: true
  headers:
    - name: X-Client-Cert-DN
      value: "$client_cert_dn"
    - name: X-Client-Cert-Serial
      value: "$client_cert_serial"
```

## WAF Modes

| Mode | Behavior |
|------|----------|
| `block` | Block malicious requests |
| `detect` | Log only, don't block |
| `off` | Disabled |

## Testing

```bash
# Test with client certificate
curl --cert client.crt --key client.key https://localhost/

# Test WAF (should be blocked)
curl "https://localhost/?id=1' OR '1'='1"
```

## Monitoring

- `highper_gateway_waf_blocked_total`
- `highper_gateway_waf_detected_total`
- `highper_gateway_mtls_auth_failures_total`

## Related Use Cases

- [UC03: HTTPS/TLS](./uc03-https-tls.md) - Server TLS only
- [UC04: API Gateway](./uc04-api-gateway.md) - JWT auth instead
