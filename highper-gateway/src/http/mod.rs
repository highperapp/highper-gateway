//! HTTP protocol implementations

pub mod protocol;
pub mod http3; // Quinn-based implementation (deprecated)
pub mod http3_quiche; // Quiche-based implementation (recommended)
pub mod alt_svc;
pub mod body_utils;
pub mod streaming_body;
pub mod proxy_streaming;

pub use protocol::*;
pub use body_utils::*;
pub use streaming_body::*;
pub use proxy_streaming::*;

// Re-export quiche implementation as default HTTP/3
pub use http3_quiche::Http3Server;
