# Use Case 05: HTTP/3 QUIC

Next-generation HTTP/3 with QUIC transport for reduced latency and improved mobile performance.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP/3 (QUIC/UDP) |
| Ports | 443 (UDP + TCP fallback) |
| TLS Required | Yes (TLS 1.3) |
| Privileged | Yes |
| Scaling | Horizontal |

## When to Use

- Mobile-first applications
- High-latency networks
- Lossy network conditions
- 0-RTT connection resumption
- Multiplexed streams without head-of-line blocking

## Architecture

```
                         ┌──────────────────────┐
    HTTP/3:443/UDP ────▶ │                      │
                         │   Highper Gateway    │ ────▶ Backend
    HTTP/2:443/TCP ────▶ │    (HTTP/3 + H2)     │
    (fallback)           │                      │
                         └──────────────────────┘
```

## Requirements

- Linux Kernel 5.11+ (for optimal QUIC support)
- TLS 1.3 certificates
- UDP port 443 open in firewall

## Quick Start

```bash
# Deploy configuration
cp configs/yaml/uc05-http3-quic.yaml /etc/highper-gateway/config.yaml

# Open UDP port
ufw allow 443/udp

# Start gateway
systemctl start highper-gateway
```

## Configuration

### HTTP/3 Listener

```yaml
listeners:
  - name: http3-main
    protocol: http3
    bind: "0.0.0.0:443"
    tls: default
    quic:
      max_idle_timeout: 30000
      initial_max_data: 10485760
      initial_max_streams_bidi: 100
```

### Alt-Svc Header

Advertise HTTP/3 support to clients:

```yaml
headers:
  response:
    add:
      - name: Alt-Svc
        value: 'h3=":443"; ma=86400'
```

### QUIC Parameters

```yaml
quic:
  max_idle_timeout: 30000           # Connection timeout
  max_udp_payload_size: 1350        # MTU-safe payload
  initial_max_data: 10485760        # 10MB flow control
  initial_max_stream_data: 1048576  # 1MB per stream
  initial_max_streams_bidi: 100     # Concurrent streams
```

## TLS Requirements

HTTP/3 requires TLS 1.3:

```yaml
tls:
  default:
    protocols:
      - TLSv1.3
    alpn:
      - h3
      - h2
      - http/1.1
```

## Testing

```bash
# Using curl with HTTP/3 support
curl --http3 https://localhost/

# Check Alt-Svc header
curl -I https://localhost/ | grep -i alt-svc
```

## Browser Support

| Browser | HTTP/3 Support |
|---------|----------------|
| Chrome 87+ | ✅ |
| Firefox 88+ | ✅ |
| Safari 14+ | ✅ |
| Edge 87+ | ✅ |

## Monitoring

Key metrics for HTTP/3:
- `highper_gateway_quic_connections_total`
- `highper_gateway_quic_streams_total`
- `highper_gateway_quic_0rtt_accepted_total`

## Related Use Cases

- [UC03: HTTPS/TLS](./uc03-https-tls.md) - HTTP/2 only
- [UC11: CDN Edge](./uc11-cdn-edge.md) - Add caching
