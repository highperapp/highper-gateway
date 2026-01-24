# Use Case 15: Geographic Load Balancing

Geographic routing based on client location with region-based failover.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP/HTTPS |
| Ports | 80, 443 |
| TLS Required | Yes |
| Privileged | Yes |
| Scaling | Global |

## When to Use

- Global application deployment
- Latency-sensitive applications
- Data sovereignty requirements
- Regional failover

## Architecture

```
                         ┌──────────────────────┐
                         │   Highper Gateway    │
    Client (US) ───────▶ │                      │ ────▶ US-East
    Client (EU) ───────▶ │   GeoIP Routing      │ ────▶ EU-West
    Client (APAC) ─────▶ │                      │ ────▶ APAC
                         │   Regional Failover  │
                         └──────────────────────┘
```

## Configuration

### GeoIP Database

```yaml
geoip:
  database: /etc/highper-gateway/geoip/GeoLite2-City.mmdb
  auto_update:
    enabled: true
    interval: 604800  # Weekly
```

### Geographic Backend

```yaml
backends:
  - name: geo-routing
    strategy: geo
    geo:
      database: /etc/highper-gateway/geoip/GeoLite2-City.mmdb
      default_region: us-east
    regions:
      - name: us-east
        countries: [US, CA]
        servers:
          - address: "us-east-1.app.global:8080"
          - address: "us-east-2.app.global:8080"

      - name: eu-west
        countries: [GB, IE, FR, DE, ES]
        servers:
          - address: "eu-west-1.app.global:8080"

      - name: asia-pacific
        countries: [JP, KR, SG, AU]
        servers:
          - address: "ap-1.app.global:8080"
```

### Regional Failover

```yaml
failover:
  enabled: true
  threshold: 2
  fallback_regions:
    us-east: [us-west, eu-west]
    eu-west: [eu-central, us-east]
    asia-pacific: [us-west, eu-central]
```

### State-Level Routing (US)

```yaml
regions:
  - name: us-east
    countries: [US]
    states:
      US: [NY, NJ, PA, MA, CT, VA, NC]
    servers:
      - address: "us-east.app.global:8080"

  - name: us-west
    countries: [US]
    states:
      US: [CA, WA, OR, NV, AZ]
    servers:
      - address: "us-west.app.global:8080"
```

## Latency-Based Routing

Combine geo with latency measurements:

```yaml
latency_routing:
  enabled: true
  check_interval: 60
  samples: 5
  weight_factor: 0.7  # 70% geo, 30% latency
```

## Debug Headers

```yaml
headers:
  response:
    add:
      - name: X-Geo-Region
        value: "$geo_region"
      - name: X-Geo-Country
        value: "$geo_country"
```

## MaxMind GeoIP Setup

```bash
# Download GeoLite2 database
mkdir -p /etc/highper-gateway/geoip
cd /etc/highper-gateway/geoip

# Using geoipupdate (recommended)
geoipupdate -v

# Or manual download
curl -o GeoLite2-City.mmdb "https://download.maxmind.com/..."
```

## Testing

```bash
# Test with X-Forwarded-For
curl -H "X-Forwarded-For: 8.8.8.8" https://localhost/
curl -H "X-Forwarded-For: 185.86.151.11" https://localhost/
```

## Monitoring

- `highper_gateway_geo_requests_total{region="us-east"}`
- `highper_gateway_geo_failover_total`
- `highper_gateway_geo_latency_seconds`

## Related Use Cases

- [UC11: CDN Edge](./uc11-cdn-edge.md) - Add caching
- [UC02: HTTP LB](./uc02-http-lb.md) - Single region
