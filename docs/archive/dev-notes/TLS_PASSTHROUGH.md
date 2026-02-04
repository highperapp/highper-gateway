# TLS Passthrough Configuration Guide

## Overview

TLS passthrough allows the reverse proxy to route encrypted HTTPS traffic to backend servers **without terminating TLS**. The proxy inspects the SNI (Server Name Indication) field in the TLS ClientHello handshake to determine the routing destination, then forwards the encrypted connection directly to the appropriate backend.

## TLS Termination vs TLS Passthrough

### TLS Termination
```
Client → [Encrypted TLS] → Proxy → [Decrypts] → [Plain HTTP] → Backend
```
- Proxy decrypts TLS traffic
- Proxy can inspect HTTP headers, modify requests, apply middleware
- Proxy manages certificates
- Backend receives plain HTTP traffic

### TLS Passthrough
```
Client → [Encrypted TLS] → Proxy → [Reads SNI] → [Encrypted TLS] → Backend
```
- Proxy does NOT decrypt TLS traffic
- Proxy can only route based on SNI hostname
- Backend manages its own certificates
- Backend receives encrypted TLS traffic
- End-to-end encryption maintained

## Key Benefits

1. **End-to-End Encryption**: Traffic remains encrypted from client to backend
2. **Certificate Management**: Backends handle their own certificates
3. **Zero Trust Architecture**: Proxy cannot see decrypted traffic
4. **Compliance**: Useful for regulatory requirements (HIPAA, PCI-DSS)
5. **Multi-Tenant**: Each backend can use different certificates
6. **Performance**: No encryption/decryption overhead at proxy level

## How SNI-Based Routing Works

### 1. Client Initiates TLS Connection
```
Client → TCP Connection → Proxy (port 9443)
```

### 2. Client Sends TLS ClientHello
```
TLS ClientHello {
  Version: TLS 1.3
  Cipher Suites: [...]
  Extensions: [
    SNI: "api.example.com"  ← Proxy reads this
    ALPN: [h2, http/1.1]
  ]
}
```

### 3. Proxy Extracts SNI
```rust
// Proxy reads SNI from ClientHello
let sni_info = extract_sni(&mut client_stream).await?;
// sni_info.server_name = "api.example.com"
```

### 4. Proxy Routes Based on SNI
```
SNI: "api.example.com" → Route to api_backend (10.0.1.10:8443)
```

### 5. Proxy Forwards Encrypted Traffic
```rust
// Replay ClientHello to backend
backend.write_all(&sni_info.client_hello).await?;

// Bidirectional copy of encrypted data
tokio::io::copy_bidirectional(&mut client, &mut backend).await?;
```

### 6. Backend Handles TLS Handshake
```
Backend receives ClientHello → Backend sends ServerHello + Certificate
→ TLS handshake completes → Application data exchange
```

## Configuration

### Basic Configuration

```yaml
tls:
  passthrough:
    enabled: true

    # Bind address for passthrough (separate port)
    bind:
      - "0.0.0.0:9443"

    # Connection timeout
    timeout: "60s"

    # SNI-based routes
    routes:
      - server_name: "api.example.com"
        upstream: "api_backend"
        enabled: true
```

### Multiple Routes

```yaml
tls:
  passthrough:
    enabled: true
    bind:
      - "0.0.0.0:9443"

    routes:
      # Exact hostname match
      - server_name: "api.example.com"
        upstream: "api_backend"
        enabled: true

      # Another hostname
      - server_name: "app.example.com"
        upstream: "app_backend"
        enabled: true

      # Wildcard subdomain
      - server_name: "*.internal.example.com"
        upstream: "internal_services"
        enabled: true

    # Default backend for non-matching SNI
    default_backend: "default_https_backend"
```

### Upstream Configuration

Backends must use HTTPS URLs when using TLS passthrough:

```yaml
upstreams:
  - name: "api_backend"
    servers:
      - url: "https://10.0.1.10:8443"
        weight: 100
      - url: "https://10.0.1.11:8443"
        weight: 100

    load_balancing:
      policy: "round_robin"

    health_checks:
      active:
        enabled: true
        interval: "10s"
        path: "/health"
```

### Combining TLS Termination and Passthrough

You can run both TLS termination and TLS passthrough simultaneously on different ports:

```yaml
server:
  # TLS Termination (proxy decrypts)
  tls_bind:
    - "0.0.0.0:8443"

tls:
  # Certificates for TLS termination
  certificates:
    - domains:
        - "admin.example.com"
      cert_file: "/etc/certs/admin.crt"
      key_file: "/etc/certs/admin.key"

  # TLS Passthrough (proxy forwards encrypted)
  passthrough:
    enabled: true
    bind:
      - "0.0.0.0:9443"  # Different port!

    routes:
      - server_name: "api.example.com"
        upstream: "api_backend"
```

**Port Mapping:**
- Port 8080: HTTP traffic (plain)
- Port 8443: HTTPS traffic (TLS termination - proxy decrypts)
- Port 9443: HTTPS traffic (TLS passthrough - proxy forwards encrypted)

## Wildcard Matching

### Exact Match
```yaml
- server_name: "api.example.com"
  upstream: "api_backend"
```
Matches: `api.example.com`
Does NOT match: `www.api.example.com`, `v2.api.example.com`

### Wildcard Match
```yaml
- server_name: "*.example.com"
  upstream: "wildcard_backend"
```
Matches: `api.example.com`, `app.example.com`, `anything.example.com`
Does NOT match: `example.com`, `sub.api.example.com`

### Catch-All Default
```yaml
tls:
  passthrough:
    default_backend: "fallback_backend"
```
Routes any SNI that doesn't match specific routes to the default backend.

## Use Cases

### 1. Microservices with Individual Certificates

Each microservice manages its own certificate:

```yaml
tls:
  passthrough:
    enabled: true
    bind: ["0.0.0.0:9443"]

    routes:
      - server_name: "users.myapp.com"
        upstream: "users_service"

      - server_name: "orders.myapp.com"
        upstream: "orders_service"

      - server_name: "payments.myapp.com"
        upstream: "payments_service"
```

### 2. Multi-Tenant SaaS

Each tenant has their own domain and certificate:

```yaml
tls:
  passthrough:
    enabled: true
    bind: ["0.0.0.0:9443"]

    routes:
      - server_name: "acmecorp.myapp.com"
        upstream: "tenant_acmecorp"

      - server_name: "globex.myapp.com"
        upstream: "tenant_globex"

      # Wildcard for all tenants
      - server_name: "*.myapp.com"
        upstream: "tenant_router"
```

### 3. Legacy Applications

Route to legacy apps that handle their own TLS:

```yaml
tls:
  passthrough:
    enabled: true
    bind: ["0.0.0.0:9443"]

    routes:
      - server_name: "legacy-app.example.com"
        upstream: "legacy_backend"
```

### 4. Internal Services (Zero Trust)

Keep traffic encrypted even within internal network:

```yaml
tls:
  passthrough:
    enabled: true
    bind: ["0.0.0.0:9443"]

    routes:
      - server_name: "*.internal.corp"
        upstream: "internal_services"
```

## Load Balancing

TLS passthrough supports all load balancing policies:

### Round Robin
```yaml
upstreams:
  - name: "api_backend"
    servers:
      - url: "https://10.0.1.10:8443"
      - url: "https://10.0.1.11:8443"
      - url: "https://10.0.1.12:8443"

    load_balancing:
      policy: "round_robin"
```

### Least Connections
```yaml
load_balancing:
  policy: "least_conn"
```
Recommended for long-lived connections.

### IP Hash (Session Affinity)
```yaml
load_balancing:
  policy: "ip_hash"
```
Routes same client IP to same backend. Useful for stateful applications.

### Weighted Round Robin
```yaml
upstreams:
  - name: "api_backend"
    servers:
      - url: "https://10.0.1.10:8443"
        weight: 100
      - url: "https://10.0.1.11:8443"
        weight: 50  # Gets half the traffic
```

## Health Checks

Active health checks verify backend availability:

```yaml
upstreams:
  - name: "api_backend"
    servers:
      - url: "https://10.0.1.10:8443"

    health_checks:
      active:
        enabled: true
        interval: "10s"
        timeout: "5s"
        path: "/health"
        healthy_threshold: 2
        unhealthy_threshold: 3
```

**Note**: Health checks for TLS passthrough backends use HTTPS to connect to the backend.

## Monitoring and Observability

### Metrics

TLS passthrough connections are tracked separately:

```
# Prometheus metrics
proxy_tls_passthrough_connections_total{server_name="api.example.com"} 1250
proxy_tls_passthrough_bytes_sent{server_name="api.example.com"} 15728640
proxy_tls_passthrough_bytes_received{server_name="api.example.com"} 52428800
proxy_tls_passthrough_connection_duration_seconds{server_name="api.example.com"} 45.2
```

### Logging

```json
{
  "timestamp": "2025-01-15T10:30:45Z",
  "level": "info",
  "message": "TLS passthrough connection established",
  "sni": "api.example.com",
  "client_ip": "203.0.113.42",
  "backend": "10.0.1.10:8443",
  "upstream": "api_backend"
}
```

### Tracing

TLS passthrough connections are traced with limited information:

```
Span: tls_passthrough
  - sni: api.example.com
  - upstream: api_backend
  - backend: 10.0.1.10:8443
  - duration: 45.2s
  - bytes_sent: 15728640
  - bytes_received: 52428800
```

**Note**: Since traffic is encrypted, the proxy cannot see HTTP headers, URLs, or status codes.

## Troubleshooting

### Connection Fails with "No SNI extension found"

**Problem**: Client doesn't send SNI in TLS ClientHello.

**Solution**:
- Ensure client supports SNI (very old clients may not)
- Configure a `default_backend` to handle connections without SNI

```yaml
tls:
  passthrough:
    default_backend: "fallback_backend"
```

### Backend Certificate Mismatch

**Problem**: Backend certificate doesn't match SNI hostname.

**Symptoms**: Client receives certificate error.

**Solution**:
- Ensure backend certificate's Common Name or SAN matches the SNI hostname
- Backend must serve correct certificate for the requested hostname

Example:
```
SNI: api.example.com
Backend Certificate CN: *.example.com  ← This works
Backend Certificate CN: example.com    ← This does NOT work
```

### "Connection refused" or Timeout

**Problem**: Backend not reachable or not listening on HTTPS port.

**Solution**:
- Verify backend is running and listening on the configured port
- Check firewall rules between proxy and backend
- Test direct connection: `openssl s_client -connect backend:8443 -servername api.example.com`

### SNI Not Matching Any Route

**Problem**: SNI hostname doesn't match any configured route.

**Symptoms**: Connection rejected or routed to default backend.

**Solution**:
- Check SNI value in logs
- Verify route configuration matches the SNI hostname
- Add wildcard route or default backend

### Load Balancing Not Working

**Problem**: All traffic goes to single backend.

**Solution**:
- Verify multiple backend servers are configured
- Check health check status (unhealthy backends are excluded)
- For IP hash policy, verify client IPs are different

### Performance Issues

**Problem**: TLS passthrough slower than expected.

**Solution**:
- Check connection timeout setting (increase if needed)
- Enable TCP tuning options:

```yaml
server:
  performance:
    tcp_nodelay: true
    tcp_fastopen: true
    keepalive: true
    keepalive_timeout: "65s"
```

- Consider using io_uring for maximum performance (enabled by default)

## Security Considerations

### 1. Certificate Validation

The proxy does NOT validate backend certificates in passthrough mode. Ensure:
- Backend certificates are valid and not expired
- Backend certificates match the SNI hostname
- Certificate chains are complete

### 2. SNI Privacy

SNI is sent in plaintext in TLS ClientHello. Consider:
- **ECH (Encrypted Client Hello)** - TLS 1.3 feature that encrypts SNI (future support)
- For maximum privacy, use TLS termination with a single wildcard certificate

### 3. DDoS Protection

TLS passthrough connections can be resource-intensive:

```yaml
tls:
  passthrough:
    timeout: "30s"  # Lower timeout for faster cleanup

    # Rate limiting (future feature)
    rate_limiting:
      enabled: true
      max_connections_per_ip: 100
      window: "1m"
```

### 4. Backend Authentication

Since proxy cannot see traffic, implement backend authentication:
- Mutual TLS (mTLS) between proxy and backend
- VPN or private network between proxy and backends
- Firewall rules restricting backend access

## Testing

### Test SNI Extraction

```bash
# Send ClientHello with SNI
openssl s_client -connect localhost:9443 \
  -servername api.example.com \
  -showcerts

# Expected: Connection to backend, valid certificate returned
```

### Test Load Balancing

```bash
# Make multiple requests
for i in {1..10}; do
  curl -k https://api.example.com:9443/test
done

# Check logs for backend distribution
```

### Test Health Checks

```bash
# Stop one backend
systemctl stop backend1

# Wait for health check interval
sleep 15

# Verify traffic routes to healthy backends only
curl -k https://api.example.com:9443/test
```

### Test Wildcard Matching

```bash
# Test different subdomains
curl -k https://app1.internal.example.com:9443/
curl -k https://app2.internal.example.com:9443/
curl -k https://app3.internal.example.com:9443/

# All should route to same upstream
```

## Performance Benchmarks

TLS passthrough performance depends on:
- Network bandwidth
- Backend response time
- Number of concurrent connections

### Typical Performance

| Metric | Value |
|--------|-------|
| Latency overhead | < 1ms |
| Throughput | 10+ Gbps (with io_uring) |
| Concurrent connections | 100,000+ |
| CPU usage | ~5% for 10k connections |
| Memory usage | ~50MB + 4KB per connection |

### Optimization Tips

1. **Enable io_uring** (enabled by default on Linux 5.1+)
2. **Use TCP Fast Open**:
   ```yaml
   server:
     performance:
       tcp_fastopen: true
   ```

3. **Tune buffer sizes**:
   ```yaml
   server:
     performance:
       read_buffer_size: 65536
       write_buffer_size: 65536
   ```

4. **Increase worker threads** for high concurrency:
   ```yaml
   server:
     workers: "16"  # Or "auto" for num_cpus
   ```

## Complete Example

See `config/tls-passthrough-example.yaml` for a complete working configuration.

## Further Reading

- [RFC 6066 - TLS Server Name Indication](https://tools.ietf.org/html/rfc6066)
- [TLS 1.3 RFC 8446](https://tools.ietf.org/html/rfc8446)
- [SNI-based routing best practices](https://www.nginx.com/blog/nginx-ssl/)
