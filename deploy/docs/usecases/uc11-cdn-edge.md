# Use Case 11: CDN Edge Caching

Edge caching proxy with cache control, compression, and origin shielding.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP/HTTPS |
| Ports | 80, 443 |
| TLS Required | Yes |
| Privileged | Yes |
| Scaling | Edge/PoP |

## When to Use

- Static asset caching
- Origin offloading
- Reduced latency for cached content
- Bandwidth optimization

## Architecture

```
    Client ────▶ ┌──────────────────────┐
                 │   Edge Cache         │
                 │   ├── Memory Cache   │ ──(miss)──▶ Origin
                 │   └── Disk Cache     │
                 │                      │
                 │   • Compression      │
                 │   • Cache-Control    │
                 └──────────────────────┘
```

## Configuration

### Cache Settings

```yaml
cache:
  enabled: true
  storage:
    type: memory
    size: 1073741824  # 1GB
  default_ttl: 3600
  max_object_size: 10485760
  stale_if_error: 86400
  stale_while_revalidate: 60
```

### Route-Based Caching

```yaml
routes:
  - match:
      path_prefix: /static
    backend: origin
    cache:
      enabled: true
      ttl: 86400  # 24 hours

  - match:
      path_prefix: /api
    backend: origin
    cache:
      enabled: false  # Don't cache API
```

### Compression

```yaml
compression:
  enabled: true
  algorithms:
    - br      # Brotli (preferred)
    - gzip
  min_size: 1024
  types:
    - text/html
    - text/css
    - application/javascript
    - application/json
```

### Cache Headers

```yaml
headers:
  response:
    add:
      - name: X-Cache-Status
        value: "$cache_status"  # HIT, MISS, STALE
```

## Cache Behavior

| Header | Behavior |
|--------|----------|
| `Cache-Control: no-cache` | Validate with origin |
| `Cache-Control: no-store` | Never cache |
| `Cache-Control: private` | Don't cache |
| `Cache-Control: max-age=N` | Cache for N seconds |

## Cache Bypass

Bypass cache for specific requests:

```yaml
cache:
  bypass_headers:
    - Authorization
    - Cookie
  bypass_query_params:
    - nocache
```

## Purging

```bash
# Purge single URL
curl -X PURGE http://localhost/static/style.css

# Purge by pattern
curl -X PURGE "http://localhost/static/*"
```

## Monitoring

- `highper_gateway_cache_hits_total`
- `highper_gateway_cache_misses_total`
- `highper_gateway_cache_size_bytes`

## Related Use Cases

- [UC15: Geo LB](./uc15-geo-lb.md) - Add geographic routing
