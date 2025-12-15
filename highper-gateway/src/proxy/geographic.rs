//! Geographic load balancing using GeoIP
//!
//! Routes requests to the nearest backend server based on client IP geolocation.
//! Supports both MaxMind GeoLite2/GeoIP2 and IP2Location databases using Adapter pattern.

use crate::config::{GeoIpProvider, GeoLocation};
use anyhow::{Context, Result};
use std::net::IpAddr;
use std::path::Path;
use tracing::{debug, warn};

/// Geo IP database adapter trait
trait GeoIpAdapter: Send + Sync {
    /// Lookup geographic location for IP address
    fn lookup(&self, ip: IpAddr) -> Option<GeoLocationResult>;
}

/// MaxMind GeoLite2/GeoIP2 adapter
struct MaxMindAdapter {
    reader: maxminddb::Reader<Vec<u8>>,
}

impl MaxMindAdapter {
    fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let reader = maxminddb::Reader::open_readfile(db_path.as_ref())
            .context("Failed to open MaxMind database")?;

        debug!("MaxMind GeoIP database loaded from: {:?}", db_path.as_ref());
        Ok(Self { reader })
    }
}

impl GeoIpAdapter for MaxMindAdapter {
    fn lookup(&self, ip: IpAddr) -> Option<GeoLocationResult> {
        match self.reader.lookup::<maxminddb::geoip2::City>(ip) {
            Ok(city) => {
                let location = city.location.as_ref()?;
                Some(GeoLocationResult {
                    latitude: location.latitude?,
                    longitude: location.longitude?,
                })
            }
            Err(e) => {
                debug!("MaxMind GeoIP lookup failed for {}: {}", ip, e);
                None
            }
        }
    }
}

/// IP2Location adapter
struct Ip2LocationAdapter {
    db: std::sync::Mutex<ip2location::DB>,
}

impl Ip2LocationAdapter {
    fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db = ip2location::DB::from_file(db_path.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to open IP2Location database: {}", e))?;

        debug!("IP2Location database loaded from: {:?}", db_path.as_ref());
        Ok(Self {
            db: std::sync::Mutex::new(db),
        })
    }
}

impl GeoIpAdapter for Ip2LocationAdapter {
    fn lookup(&self, ip: IpAddr) -> Option<GeoLocationResult> {
        let mut db = self.db.lock().ok()?;
        match db.ip_lookup(ip) {
            Ok(record) => {
                // Extract LocationRecord from the Record enum
                // IP2Location 0.4.x returns Record::LocationDb(LocationRecord)
                let location_record = match record {
                    ip2location::Record::LocationDb(rec) => rec,
                    _ => return None,
                };

                // Extract latitude and longitude from the record
                // IP2Location 0.4.x provides latitude and longitude as Option<f32>
                // Note: Requires DB5+ or commercial database packages with lat/lon data
                let latitude = location_record.latitude.map(|lat| lat as f64);
                let longitude = location_record.longitude.map(|lon| lon as f64);

                match (latitude, longitude) {
                    (Some(lat), Some(lon)) => {
                        debug!("IP2Location: Found location for {}: ({}, {})", ip, lat, lon);
                        Some(GeoLocationResult {
                            latitude: lat,
                            longitude: lon,
                        })
                    }
                    _ => {
                        debug!(
                            "IP2Location: No lat/lon data for {}. Ensure you're using a database package that includes geographic coordinates (DB5+)",
                            ip
                        );
                        None
                    }
                }
            }
            Err(e) => {
                debug!("IP2Location lookup failed for {}: {}", ip, e);
                None
            }
        }
    }
}

/// Geographic load balancer
pub struct GeoLoadBalancer {
    adapter: Option<Box<dyn GeoIpAdapter>>,
}

/// Server with geographic location
#[derive(Debug, Clone)]
pub struct GeoServer {
    pub index: usize,
    pub location: GeoLocation,
    pub region: Option<String>,
}

impl GeoLoadBalancer {
    /// Create new geographic load balancer
    pub fn new<P: AsRef<Path>>(provider: GeoIpProvider, db_path: Option<P>) -> Result<Self> {
        let adapter: Option<Box<dyn GeoIpAdapter>> = if let Some(path) = db_path {
            match provider {
                GeoIpProvider::MaxMind => {
                    match MaxMindAdapter::new(path) {
                        Ok(adapter) => Some(Box::new(adapter)),
                        Err(e) => {
                            warn!("Failed to load MaxMind database: {}. Geographic load balancing disabled.", e);
                            None
                        }
                    }
                }
                GeoIpProvider::Ip2Location => {
                    match Ip2LocationAdapter::new(path) {
                        Ok(adapter) => Some(Box::new(adapter)),
                        Err(e) => {
                            warn!("Failed to load IP2Location database: {}. Geographic load balancing disabled.", e);
                            None
                        }
                    }
                }
            }
        } else {
            warn!("No GeoIP database path configured. Geographic load balancing disabled.");
            None
        };

        Ok(Self { adapter })
    }

    /// Select nearest server based on client IP
    pub fn select_nearest(
        &self,
        client_ip: Option<&str>,
        servers: &[GeoServer],
    ) -> Option<usize> {
        if servers.is_empty() {
            return None;
        }

        // Get client IP address
        let client_ip = client_ip?;
        let ip_addr: IpAddr = match client_ip.parse() {
            Ok(addr) => addr,
            Err(e) => {
                debug!("Failed to parse client IP '{}': {}", client_ip, e);
                return None;
            }
        };

        // Lookup client location using adapter
        let adapter = self.adapter.as_ref()?;
        let client_location = adapter.lookup(ip_addr)?;

        debug!(
            "Client IP {} located at ({}, {})",
            client_ip, client_location.latitude, client_location.longitude
        );

        // Find nearest server
        let mut nearest_index = None;
        let mut min_distance = f64::MAX;

        for server in servers {
            let distance = calculate_distance(
                client_location.latitude,
                client_location.longitude,
                server.location.lat,
                server.location.lon,
            );

            debug!(
                "Distance to server {} (region: {:?}): {:.2} km",
                server.index,
                server.region,
                distance
            );

            if distance < min_distance {
                min_distance = distance;
                nearest_index = Some(server.index);
            }
        }

        if let Some(index) = nearest_index {
            debug!(
                "Selected nearest server {} at distance {:.2} km",
                index, min_distance
            );
        }

        nearest_index
    }

    /// Check if GeoIP database is available
    pub fn is_available(&self) -> bool {
        self.adapter.is_some()
    }
}

/// Geographic location result from GeoIP lookup
#[derive(Debug, Clone)]
struct GeoLocationResult {
    latitude: f64,
    longitude: f64,
}

/// Calculate distance between two geographic points using Haversine formula
///
/// Returns distance in kilometers
pub fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_KM * c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_new_york_to_london() {
        // New York: 40.7128° N, 74.0060° W
        // London: 51.5074° N, 0.1278° W
        let distance = calculate_distance(40.7128, -74.0060, 51.5074, -0.1278);

        // Actual distance is approximately 5570 km
        assert!((distance - 5570.0).abs() < 100.0, "Distance: {}", distance);
    }

    #[test]
    fn test_distance_sydney_to_tokyo() {
        // Sydney: 33.8688° S, 151.2093° E
        // Tokyo: 35.6762° N, 139.6503° E
        let distance = calculate_distance(-33.8688, 151.2093, 35.6762, 139.6503);

        // Actual distance is approximately 7800 km
        assert!((distance - 7800.0).abs() < 200.0, "Distance: {}", distance);
    }

    #[test]
    fn test_distance_same_location() {
        let distance = calculate_distance(40.7128, -74.0060, 40.7128, -74.0060);
        assert!(distance < 0.1, "Distance should be nearly zero: {}", distance);
    }

    #[test]
    fn test_distance_antipodal_points() {
        // Points on opposite sides of Earth
        // Should be approximately half Earth's circumference
        let distance = calculate_distance(0.0, 0.0, 0.0, 180.0);

        // Half Earth's circumference is approximately 20,037 km
        assert!(
            (distance - 20037.0).abs() < 500.0,
            "Distance: {}",
            distance
        );
    }

    #[test]
    fn test_geo_load_balancer_without_db() {
        // Should not panic without database (MaxMind)
        let result = GeoLoadBalancer::new(GeoIpProvider::MaxMind, None::<&str>);
        assert!(result.is_ok());

        let lb = result.expect("GeoLoadBalancer creation should succeed without DB");
        assert!(!lb.is_available());

        // Should not panic without database (IP2Location)
        let result = GeoLoadBalancer::new(GeoIpProvider::Ip2Location, None::<&str>);
        assert!(result.is_ok());

        let lb = result.expect("GeoLoadBalancer creation should succeed without DB");
        assert!(!lb.is_available());
    }

    #[test]
    fn test_select_nearest_with_empty_servers() {
        let lb = GeoLoadBalancer::new(GeoIpProvider::MaxMind, None::<&str>)
            .expect("GeoLoadBalancer creation should succeed");
        let servers: Vec<GeoServer> = vec![];

        let result = lb.select_nearest(Some("1.2.3.4"), &servers);
        assert!(result.is_none());
    }

    #[test]
    fn test_select_nearest_without_client_ip() {
        let lb = GeoLoadBalancer::new(GeoIpProvider::MaxMind, None::<&str>)
            .expect("GeoLoadBalancer creation should succeed");
        let servers = vec![GeoServer {
            index: 0,
            location: GeoLocation {
                lat: 40.7128,
                lon: -74.0060,
            },
            region: Some("us-east-1".to_string()),
        }];

        let result = lb.select_nearest(None, &servers);
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_distance_north_pole_to_south_pole() {
        // North Pole to South Pole
        let distance = calculate_distance(90.0, 0.0, -90.0, 0.0);

        // Should be approximately half Earth's circumference
        assert!(
            (distance - 20037.0).abs() < 500.0,
            "Distance: {}",
            distance
        );
    }

    #[test]
    fn test_calculate_distance_equator_quarter_circle() {
        // Quarter of Earth's circumference along equator
        let distance = calculate_distance(0.0, 0.0, 0.0, 90.0);

        // Should be approximately quarter circumference (10,018 km)
        assert!(
            (distance - 10018.0).abs() < 200.0,
            "Distance: {}",
            distance
        );
    }
}
