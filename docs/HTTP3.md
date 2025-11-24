# HTTP/3 (QUIC) Support

## Overview

HTTP/3 is the latest version of the HTTP protocol, built on top of QUIC (Quick UDP Internet Connections) instead of TCP. This document explains how to enable, configure, and use HTTP/3 in Highper Gateway.

## What is HTTP/3?

### Protocol Evolution

```
HTTP/1.1 ──→ HTTP/2 ──→ HTTP/3
   │            │           │
  TCP          TCP        QUIC (UDP)
   │            │           │
Single stream  Multiplexed  Multiplexed + 0-RTT
Head-of-line   Head-of-line No head-of-line
blocking       blocking     blocking
```

### Key Benefits

1. **0-RTT Connection Establishment** - Resume connections instantly without handshake overhead
2. **No Head-of-Line Blocking** - Independent streams at the transport layer
3. **Connection Migration** - Connections survive IP address changes (ideal for mobile)
4. **Improved Congestion Control** - Built into QUIC protocol
5. **Mandatory Encryption** - TLS 1.3 integrated directly into QUIC
6. **Better Performance on Lossy Networks** - Improved packet loss handling

## Browser Support

| Browser  | HTTP/3 Support | Notes                    |
|----------|---------------|--------------------------|
| Chrome   | ✅ Yes        | Enabled by default       |
| Firefox  | ✅ Yes        | Enabled by default       |
| Safari   | ✅ Yes        | iOS 14+, macOS 11+       |
| Edge     | ✅ Yes        | Chromium-based           |
| Opera    | ✅ Yes        | Chromium-based           |

## Configuration

### Basic Configuration

```yaml
server:
  # Enable HTTP/3 protocol
  protocols:
    - http1
    - http2
    - http3

  # HTTP/3 configuration
  http3:
    enabled: true
    port: 443              # UDP port (typically same as HTTPS)
    bind: "0.0.0.0"
    enable_0rtt: false     # Enable 0-RTT (use with caution)
    max_concurrent_streams: 100
    initial_window_size: 10485760  # 10 MB
    max_datagram_size: 1350
    idle_timeout_secs: 30

# TLS is REQUIRED for HTTP/3
tls:
  certificates:
    - domain: "example.com"
      cert_file: "/path/to/cert.pem"
      key_file: "/path/to/key.pem"
```

### Configuration Options

| Option                 | Type    | Default | Description                                    |
|-----------------------|---------|---------|------------------------------------------------|
| `enabled`             | boolean | false   | Enable HTTP/3 server                           |
| `port`                | integer | 443     | UDP port for HTTP/3                            |
| `bind`                | string  | "0.0.0.0" | Bind address                                 |
| `enable_0rtt`         | boolean | false   | Enable 0-RTT connection resumption             |
| `max_concurrent_streams` | integer | 100  | Max concurrent streams per connection          |
| `initial_window_size` | integer | 10485760 | Flow control window size (bytes)            |
| `max_datagram_size`   | integer | 1350    | Maximum datagram size (bytes)                  |
| `idle_timeout_secs`   | integer | 30      | Connection idle timeout (seconds)              |

## Protocol Discovery

### Alt-Svc Header

HTTP/3 uses the `Alt-Svc` (Alternative Service) header to advertise availability:

```http
HTTP/2 200 OK
Alt-Svc: h3=":443"; ma=2592000
Content-Type: text/html
```

When a client receives this header:
1. It remembers that HTTP/3 is available on port 443
2. Subsequent requests will use HTTP/3 instead of HTTP/2
3. The cache duration is 30 days (2592000 seconds)

### How It Works

```
┌─────────┐                    ┌─────────┐
│ Client  │                    │ Server  │
└────┬────┘                    └────┬────┘
     │                              │
     │ 1. HTTP/2 Request (TCP)      │
     │─────────────────────────────>│
     │                              │
     │ 2. Response + Alt-Svc Header │
     │<─────────────────────────────│
     │    "h3=\":443\"; ma=2592000"  │
     │                              │
     │ 3. HTTP/3 Request (UDP)      │
     │─────────────────────────────>│
     │                              │
     │ 4. HTTP/3 Response           │
     │<─────────────────────────────│
     │                              │
```

## 0-RTT (Zero Round Trip Time)

### What is 0-RTT?

0-RTT allows clients to send application data in the first packet during connection resumption, eliminating handshake latency.

### Benefits

- **Instant Connection** - No handshake delay for returning clients
- **Improved User Experience** - Faster page loads for repeat visitors
- **Reduced Latency** - Up to 100ms+ saved on mobile networks

### Security Considerations

⚠️ **Warning**: 0-RTT is vulnerable to replay attacks

- An attacker can capture and replay the first packet
- Only use 0-RTT for idempotent operations (GET requests)
- Avoid using 0-RTT for state-changing operations (POST, PUT, DELETE)

### Configuration

```yaml
server:
  http3:
    enabled: true
    enable_0rtt: true  # Enable with caution
```

## Performance Tuning

### Optimal Settings for Different Use Cases

#### High-Throughput File Transfer

```yaml
server:
  http3:
    enabled: true
    max_concurrent_streams: 200
    initial_window_size: 20971520  # 20 MB
    max_datagram_size: 1450
    idle_timeout_secs: 60
```

#### Real-Time API

```yaml
server:
  http3:
    enabled: true
    max_concurrent_streams: 100
    initial_window_size: 5242880  # 5 MB
    max_datagram_size: 1350
    idle_timeout_secs: 30
```

#### Mobile-Optimized

```yaml
server:
  http3:
    enabled: true
    max_concurrent_streams: 50
    initial_window_size: 2097152  # 2 MB
    max_datagram_size: 1200  # Smaller for mobile networks
    idle_timeout_secs: 45
```

### OS-Level Tuning

For production deployments, increase UDP buffer sizes:

```bash
# Linux
sudo sysctl -w net.core.rmem_max=2500000
sudo sysctl -w net.core.wmem_max=2500000
sudo sysctl -w net.ipv4.udp_mem="65536 131072 262144"

# Make permanent
echo "net.core.rmem_max=2500000" | sudo tee -a /etc/sysctl.conf
echo "net.core.wmem_max=2500000" | sudo tee -a /etc/sysctl.conf
```

## Testing HTTP/3

### Using curl

```bash
# Install curl with HTTP/3 support
# On Ubuntu/Debian: build from source with --with-quiche
# On macOS: brew install curl (includes HTTP/3)

# Test HTTP/3 endpoint
curl --http3 https://example.com

# Verbose output to see protocol negotiation
curl --http3 -v https://example.com

# Force HTTP/3 only
curl --http3-only https://example.com
```

### Using Browser DevTools

1. Open Chrome/Firefox DevTools (F12)
2. Go to Network tab
3. Add "Protocol" column (right-click on column headers)
4. Make a request to your site
5. Check if protocol shows "h3" or "h3-29"

### Using h3 CLI Tool

```bash
# Install h3 tool
cargo install h3-cli

# Test HTTP/3 endpoint
h3 https://example.com

# With verbose logging
RUST_LOG=debug h3 https://example.com
```

## Debugging

### Enable Debug Logging

```bash
# Set environment variable
export RUST_LOG=quinn=debug,h3=debug

# Run proxy
./highper-gateway --config config.yaml
```

### Common Issues

#### 1. UDP Port Blocked

**Symptoms**: Connection timeout, HTTP/3 not working

**Solution**:
```bash
# Check firewall rules
sudo ufw status
sudo ufw allow 443/udp

# For iptables
sudo iptables -A INPUT -p udp --dport 443 -j ACCEPT
```

#### 2. Certificate Issues

**Symptoms**: TLS handshake failed

**Solution**:
- Verify certificate is valid and not expired
- Ensure TLS 1.3 is supported
- Check certificate chain is complete
- Verify private key matches certificate

#### 3. 0-RTT Not Working

**Symptoms**: Every connection does full handshake

**Solution**:
- Verify `enable_0rtt: true` is set
- Check client supports 0-RTT
- Ensure session tickets are working

### Packet Capture

```bash
# Capture QUIC traffic
sudo tcpdump -i any -w quic.pcap 'udp port 443'

# Analyze with Wireshark
wireshark quic.pcap
```

## Monitoring

### Metrics

Highper Gateway exposes HTTP/3 metrics via Prometheus:

```
# HTTP/3 connections
http3_connections_total
http3_connections_active

# HTTP/3 requests
http3_requests_total
http3_request_duration_seconds

# QUIC metrics
quic_packets_sent_total
quic_packets_lost_total
quic_bytes_transferred_total
```

### Example Prometheus Query

```promql
# HTTP/3 adoption rate
rate(http3_requests_total[5m]) / (
  rate(http1_requests_total[5m]) +
  rate(http2_requests_total[5m]) +
  rate(http3_requests_total[5m])
)

# Average connection duration
avg(http3_connection_duration_seconds)
```

## Migration Strategy

### Gradual Rollout

1. **Phase 1**: Enable HTTP/3 alongside HTTP/2
2. **Phase 2**: Monitor adoption and performance
3. **Phase 3**: Optimize based on metrics
4. **Phase 4**: Consider deprecating HTTP/1.1

### Fallback Behavior

HTTP/3 is designed to be optional:

```
Client tries HTTP/3 (UDP)
  ↓ Fails?
Client falls back to HTTP/2 (TCP)
  ↓ Fails?
Client falls back to HTTP/1.1 (TCP)
```

## Security Considerations

### TLS 1.3 Requirement

HTTP/3 requires TLS 1.3:
- Mandatory encryption
- Improved handshake
- Forward secrecy
- No legacy cipher suites

### DDoS Mitigation

QUIC includes built-in protection:
- Connection ID validation
- Address validation tokens
- Amplification attack prevention

### Best Practices

1. **Use Strong Certificates** - 2048-bit RSA or 256-bit ECDSA
2. **Keep Software Updated** - Regular security updates
3. **Monitor Traffic** - Watch for unusual patterns
4. **Rate Limiting** - Implement connection rate limits
5. **Firewall Rules** - Restrict UDP ports appropriately

## Future Enhancements

### Planned Features

- **WebTransport** - Bidirectional streaming over HTTP/3
- **MASQUE** - Proxying UDP over HTTP/3
- **HTTP/3 Prioritization** - Request prioritization support
- **QUIC Multipath** - Use multiple network paths simultaneously
- **Full Request/Response Handling** - Complete h3 0.0.8 API integration

## References

- [RFC 9114: HTTP/3](https://www.rfc-editor.org/rfc/rfc9114.html)
- [RFC 9000: QUIC](https://www.rfc-editor.org/rfc/rfc9000.html)
- [QUIC Working Group](https://quicwg.org/)
- [Can I use HTTP/3?](https://caniuse.com/http3)

## Support

For issues or questions:
- GitHub Issues: https://github.com/yourusername/highper-gateway/issues
- Documentation: https://docs.highper-gateway.dev

---

**Last Updated**: October 31, 2025
**Version**: Phase 2.1
