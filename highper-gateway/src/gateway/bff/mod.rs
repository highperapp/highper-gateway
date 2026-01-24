//! BFF (Backend for Frontend) Pattern
//!
//! Provides client-specific API endpoints with automatic response shaping.
//!
//! ## Overview
//!
//! The BFF pattern allows you to create optimized API endpoints for different
//! client types (mobile, web, desktop, IoT). Each client type can have:
//!
//! - Custom route configurations
//! - Client-specific response transformations
//! - Per-client caching strategies
//! - Different rate limits
//!
//! ## Example
//!
//! ```rust,ignore
//! use highper_gateway::gateway::bff::*;
//!
//! let config = BffConfig {
//!     profiles: HashMap::from([
//!         ("mobile".to_string(), ClientProfile {
//!             name: "mobile".to_string(),
//!             routes: vec![
//!                 BffRoute {
//!                     path: "/api/dashboard".to_string(),
//!                     aggregation: AggregationConfig {
//!                         backends: vec![
//!                             BackendCall {
//!                                 name: "user".to_string(),
//!                                 upstream: "user_service".to_string(),
//!                                 path: "/user/summary".to_string(),
//!                                 ..Default::default()
//!                             },
//!                         ],
//!                         ..Default::default()
//!                     },
//!                     ..Default::default()
//!                 },
//!             ],
//!             transformations: ResponseTransformConfig {
//!                 compact: true,
//!                 remove_fields: vec!["debug_info".to_string()],
//!                 ..Default::default()
//!             },
//!             ..Default::default()
//!         }),
//!     ]),
//!     ..Default::default()
//! };
//!
//! let executor = AggregationExecutor::new(client);
//! let handler = BffHandler::new(config, executor);
//!
//! // Detect client and resolve route
//! let client_type = handler.detect_client(&request);
//! if let Some(profile) = handler.get_profile(&client_type) {
//!     if let Some(route_match) = handler.resolve_route(&request, profile) {
//!         // Execute aggregation and transform response
//!         let response = handler.transform_response(profile, aggregated_response);
//!     }
//! }
//! ```
//!
//! ## Client Detection
//!
//! Supports multiple detection methods:
//!
//! - **Header**: Uses `X-Client-Type` header (default)
//! - **UserAgent**: Parses User-Agent for mobile/web/desktop patterns
//! - **QueryParam**: Uses `?client=mobile` query parameter
//! - **PathPrefix**: Uses `/mobile/api/...` path prefix
//!
//! ## Response Transformations
//!
//! - Remove sensitive/unnecessary fields
//! - Rename fields for client consistency
//! - Add computed fields using JSONPath
//! - Compact JSON for bandwidth-sensitive clients

pub mod config;
pub mod handler;

pub use config::{
    BffConfig,
    BffRoute,
    ClientProfile,
    ClientDetectionConfig,
    DetectionMethod,
    ResponseTransformConfig,
    ComputedField,
    ClientCacheConfig,
    ClientRateLimitConfig,
};

pub use handler::{
    BffHandler,
    ClientType,
    RouteMatch,
};
