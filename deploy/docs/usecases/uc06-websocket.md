# Use Case 06: WebSocket Load Balancer

WebSocket load balancing with session affinity, ping/pong keepalive, and compression.

## Overview

| Property | Value |
|----------|-------|
| Protocol | WebSocket (WS/WSS) |
| Ports | 80, 443, 8080 |
| TLS Required | Optional |
| Privileged | Optional |
| Scaling | Horizontal (with affinity) |

## When to Use

- Real-time applications (chat, gaming, live updates)
- Bidirectional communication
- Long-lived connections
- Event streaming

## Architecture

```
                         ┌──────────────────────┐
    WS:80      ────────▶ │                      │
                         │   Highper Gateway    │ ────▶ WS Backend Pool
    WSS:443    ────────▶ │  (WebSocket LB)      │    (sticky sessions)
                         │                      │
                         └──────────────────────┘
```

## Quick Start

```bash
cp configs/yaml/uc06-websocket.yaml /etc/highper-gateway/config.yaml
systemctl start highper-gateway
```

## Configuration

### WebSocket Listener

```yaml
listeners:
  - name: ws-main
    protocol: http
    bind: "0.0.0.0:80"
    websocket:
      enabled: true
      ping_interval: 30
      ping_timeout: 10
    routes:
      - match:
          path_prefix: /ws
          headers:
            - name: Upgrade
              value: websocket
        backend: ws-servers
```

### Session Affinity

Use `ip_hash` for sticky sessions:

```yaml
backends:
  - name: ws-servers
    strategy: ip_hash
    servers:
      - address: "ws-1:8080"
      - address: "ws-2:8080"
    connection:
      timeout: 3600000  # 1 hour
```

### WebSocket Settings

```yaml
websocket:
  handshake_timeout: 10000
  max_frame_size: 65536
  max_message_size: 1048576
  compression:
    enabled: true
    level: 6
  subprotocols:
    - graphql-ws
    - json
```

## Keepalive

Server-side ping/pong to detect dead connections:

```yaml
websocket:
  ping_interval: 30   # Send ping every 30s
  ping_timeout: 10    # Wait 10s for pong
```

## Secure WebSocket (WSS)

```yaml
listeners:
  - name: wss-main
    protocol: https
    bind: "0.0.0.0:443"
    tls: default
    websocket:
      enabled: true
```

## Testing

```bash
# Using websocat
websocat ws://localhost/ws

# Using wscat
wscat -c ws://localhost/ws
```

## Scaling Considerations

1. **Session Affinity** - Use `ip_hash` or external session store
2. **Connection Limits** - Increase `max_connections`
3. **Timeouts** - Set long timeouts for idle connections

## Related Use Cases

- [UC03: HTTPS/TLS](./uc03-https-tls.md) - Add TLS
- [UC10: Hybrid](./uc10-hybrid.md) - Mix with HTTP/gRPC
