//! gRPC Proxying Support
//!
//! Provides transparent gRPC proxying with support for:
//! - gRPC over HTTP/2 (standard)
//! - gRPC health checks (grpc.health.v1.Health)
//! - gRPC load balancing
//! - gRPC metadata forwarding
//! - Streaming (unary, server-streaming, client-streaming, bidirectional)

pub mod detector;
pub mod handler;
pub mod health;
pub mod streaming;

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// gRPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Enable gRPC support
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum message size in bytes (4MB default)
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Connection timeout
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Enable gRPC health checking
    #[serde(default = "default_true")]
    pub health_check_enabled: bool,

    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub health_check_interval: u64,

    /// gRPC health check service name (empty = check server health)
    #[serde(default)]
    pub health_check_service: Option<String>,

    /// Enable gRPC reflection
    #[serde(default)]
    pub reflection_enabled: bool,

    /// Load balancing strategy for gRPC
    #[serde(default)]
    pub load_balancing: GrpcLoadBalancing,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_message_size: 4 * 1024 * 1024, // 4MB
            timeout_seconds: 30,
            health_check_enabled: true,
            health_check_interval: 10,
            health_check_service: None,
            reflection_enabled: false,
            load_balancing: GrpcLoadBalancing::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_message_size() -> usize {
    4 * 1024 * 1024
}

fn default_timeout() -> u64 {
    30
}

fn default_health_interval() -> u64 {
    10
}

/// gRPC load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcLoadBalancing {
    /// Load balancing policy
    #[serde(default)]
    pub policy: GrpcLoadBalancingPolicy,

    /// Enable connection affinity (sticky connections)
    #[serde(default)]
    pub enable_affinity: bool,

    /// Affinity key (header name to use for affinity)
    #[serde(default)]
    pub affinity_key: Option<String>,
}

impl Default for GrpcLoadBalancing {
    fn default() -> Self {
        Self {
            policy: GrpcLoadBalancingPolicy::RoundRobin,
            enable_affinity: false,
            affinity_key: None,
        }
    }
}

/// gRPC load balancing policies
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrpcLoadBalancingPolicy {
    /// Round-robin across backends
    RoundRobin,
    /// Least active requests
    LeastRequest,
    /// Random selection
    Random,
    /// Power of two random choices
    PowerOfTwo,
    /// Consistent hashing (based on metadata)
    ConsistentHash,
}

impl Default for GrpcLoadBalancingPolicy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// gRPC call type detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcCallType {
    /// Unary call (single request, single response)
    Unary,
    /// Client streaming (stream of requests, single response)
    ClientStreaming,
    /// Server streaming (single request, stream of responses)
    ServerStreaming,
    /// Bidirectional streaming (stream of requests and responses)
    Bidirectional,
}

/// gRPC status codes (from grpc/status.proto)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GrpcStatusCode {
    Ok = 0,
    Cancelled = 1,
    Unknown = 2,
    InvalidArgument = 3,
    DeadlineExceeded = 4,
    NotFound = 5,
    AlreadyExists = 6,
    PermissionDenied = 7,
    ResourceExhausted = 8,
    FailedPrecondition = 9,
    Aborted = 10,
    OutOfRange = 11,
    Unimplemented = 12,
    Internal = 13,
    Unavailable = 14,
    DataLoss = 15,
    Unauthenticated = 16,
}

impl GrpcStatusCode {
    /// Convert to HTTP status code for error responses
    pub fn to_http_status(&self) -> u16 {
        match self {
            GrpcStatusCode::Ok => 200,
            GrpcStatusCode::Cancelled => 499, // Client closed request
            GrpcStatusCode::Unknown => 500,
            GrpcStatusCode::InvalidArgument => 400,
            GrpcStatusCode::DeadlineExceeded => 504,
            GrpcStatusCode::NotFound => 404,
            GrpcStatusCode::AlreadyExists => 409,
            GrpcStatusCode::PermissionDenied => 403,
            GrpcStatusCode::ResourceExhausted => 429,
            GrpcStatusCode::FailedPrecondition => 400,
            GrpcStatusCode::Aborted => 409,
            GrpcStatusCode::OutOfRange => 400,
            GrpcStatusCode::Unimplemented => 501,
            GrpcStatusCode::Internal => 500,
            GrpcStatusCode::Unavailable => 503,
            GrpcStatusCode::DataLoss => 500,
            GrpcStatusCode::Unauthenticated => 401,
        }
    }

    /// Get status message
    pub fn message(&self) -> &'static str {
        match self {
            GrpcStatusCode::Ok => "OK",
            GrpcStatusCode::Cancelled => "Cancelled",
            GrpcStatusCode::Unknown => "Unknown",
            GrpcStatusCode::InvalidArgument => "Invalid Argument",
            GrpcStatusCode::DeadlineExceeded => "Deadline Exceeded",
            GrpcStatusCode::NotFound => "Not Found",
            GrpcStatusCode::AlreadyExists => "Already Exists",
            GrpcStatusCode::PermissionDenied => "Permission Denied",
            GrpcStatusCode::ResourceExhausted => "Resource Exhausted",
            GrpcStatusCode::FailedPrecondition => "Failed Precondition",
            GrpcStatusCode::Aborted => "Aborted",
            GrpcStatusCode::OutOfRange => "Out of Range",
            GrpcStatusCode::Unimplemented => "Unimplemented",
            GrpcStatusCode::Internal => "Internal",
            GrpcStatusCode::Unavailable => "Unavailable",
            GrpcStatusCode::DataLoss => "Data Loss",
            GrpcStatusCode::Unauthenticated => "Unauthenticated",
        }
    }
}

/// gRPC metadata (headers/trailers)
pub type GrpcMetadata = Vec<(String, String)>;

/// gRPC request information
#[derive(Debug, Clone)]
pub struct GrpcRequest {
    /// Service and method (e.g., "/package.Service/Method")
    pub path: String,

    /// Call type
    pub call_type: GrpcCallType,

    /// Metadata (headers)
    pub metadata: GrpcMetadata,

    /// Timeout/deadline
    pub timeout: Option<Duration>,

    /// Content type (should be "application/grpc" or "application/grpc+proto")
    pub content_type: String,
}
