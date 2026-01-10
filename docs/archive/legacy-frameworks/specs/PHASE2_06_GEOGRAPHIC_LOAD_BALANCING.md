# Phase 2.6: Geographic Load Balancing - Implementation Specification

**Duration:** 1 week
**Priority:** Low-Medium
**Difficulty:** Low
**Impact:** +1% load balancing score

---

## Executive Summary

Implement geographic load balancing to route requests to the nearest backend server based on client location. Improves latency for global deployments.

---

## Configuration

```yaml
upstreams:
  - name: "global-backend"
    load_balancing:
      algorithm: "geographic"

      # GeoIP database
      geoip_db_path: "/usr/share/GeoIP/GeoLite2-City.mmdb"

    servers:
      # US East
      - url: "http://us-east.example.com:8000"
        weight: 1
        region: "us-east-1"
        location:
          lat: 39.0481
          lon: -77.4728

      # Europe
      - url: "http://eu-west.example.com:8000"
        weight: 1
        region: "eu-west-1"
        location:
          lat: 53.3498
          lon: -6.2603

      # Asia Pacific
      - url: "http://ap-south.example.com:8000"
        weight: 1
        region: "ap-south-1"
        location:
          lat: 19.0760
          lon: 72.8777
```

---

## Implementation

```rust
// File: highper-gateway/src/proxy/loadbalancer.rs (enhance)

use maxminddb::{geoip2, Reader};

pub struct GeographicLoadBalancer {
    servers: Vec<ServerWithLocation>,
    geoip_reader: Reader<Vec<u8>>,
}

#[derive(Clone)]
struct ServerWithLocation {
    url: String,
    weight: u32,
    region: String,
    latitude: f64,
    longitude: f64,
}

impl GeographicLoadBalancer {
    pub fn new(servers: Vec<ServerWithLocation>, geoip_db_path: &str) -> anyhow::Result<Self> {
        let geoip_reader = maxminddb::Reader::open_readfile(geoip_db_path)?;

        Ok(Self {
            servers,
            geoip_reader,
        })
    }

    pub fn select(&self, client_ip: Option<&str>) -> Option<String> {
        let client_ip = client_ip?;

        // Parse IP address
        let ip: std::net::IpAddr = client_ip.parse().ok()?;

        // Lookup location
        let location: geoip2::City = self.geoip_reader.lookup(ip).ok()?;

        let client_lat = location.location.as_ref()?.latitude?;
        let client_lon = location.location.as_ref()?.longitude?;

        // Find nearest server
        let mut nearest = None;
        let mut min_distance = f64::MAX;

        for server in &self.servers {
            let distance = calculate_distance(
                client_lat,
                client_lon,
                server.latitude,
                server.longitude,
            );

            if distance < min_distance {
                min_distance = distance;
                nearest = Some(server.url.clone());
            }
        }

        nearest
    }
}

/// Calculate distance between two points (Haversine formula)
fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km

    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();

    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos()
            * lat2.to_radians().cos()
            * (dlon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    r * c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_calculation() {
        // New York to London
        let distance = calculate_distance(40.7128, -74.0060, 51.5074, -0.1278);
        assert!((distance - 5570.0).abs() < 100.0); // ~5570 km
    }

    #[test]
    fn test_geographic_selection() {
        // Test selecting nearest server based on location
    }
}
```

---

## Dependencies

```toml
[dependencies]
# GeoIP lookup
maxminddb = "0.24"
```

---

## GeoIP Database

Download free GeoLite2 database:
```bash
# Download GeoLite2-City database
wget https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-City.mmdb
mv GeoLite2-City.mmdb /usr/share/GeoIP/
```

---

## Acceptance Criteria

- [ ] GeoIP lookup works
- [ ] Nearest server selected
- [ ] Distance calculation accurate
- [ ] Fallback to other algorithms if GeoIP fails
- [ ] Unit tests pass

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
