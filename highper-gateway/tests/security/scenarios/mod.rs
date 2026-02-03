//! Per-Scenario Security Tests
//!
//! Each module contains security tests specific to a deployment scenario.

pub mod scenario_01_tcp;
pub mod scenario_02_http;
pub mod scenario_03_tls;
pub mod scenario_04_api_gateway;
pub mod scenario_05_http3;
pub mod scenario_06_websocket;
pub mod scenario_07_grpc;
pub mod scenario_08_database;
pub mod scenario_09_waf_mtls;
pub mod scenario_10_hybrid;
pub mod scenario_11_cdn_cache;
pub mod scenario_12_microservices;
pub mod scenario_13_graphql;
pub mod scenario_14_static_php;
pub mod scenario_15_geo_routing;
