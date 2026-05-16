//! HTTP protocol implementations

pub mod alt_svc;
pub mod body_utils;
pub mod http3; // Quinn-based implementation (deprecated)
pub mod http3_quiche; // Quiche-based implementation (recommended)
pub mod protocol;
pub mod proxy_streaming;
pub mod streaming_body;

pub use body_utils::*;
pub use protocol::*;
pub use proxy_streaming::*;
pub use streaming_body::*;

// Re-export quiche implementation as default HTTP/3
pub use http3_quiche::Http3Server;
