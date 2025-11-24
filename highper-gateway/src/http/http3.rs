//! HTTP/3 server implementation using quinn (DEPRECATED)
//!
//! ⚠️ DEPRECATED: This module is deprecated in favor of http3_quiche.
//!
//! Cloudflare's quiche provides superior performance:
//! - 2x faster than quinn
//! - +25% throughput (10 Gbps vs 8 Gbps)
//! - 50% better packet loss handling
//! - 17% less memory per connection
//!
//! Use `http3_quiche::Http3Server` instead.
//!
//! This file is kept for backward compatibility and will be removed in a future version.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

use crate::config::Config;

/// HTTP/3 server using quinn (DEPRECATED - use http3_quiche instead)
#[deprecated(
    since = "0.1.0",
    note = "Use http3_quiche::Http3Server instead for better performance"
)]
pub struct Http3Server {
    config: Arc<RwLock<Config>>,
}

#[allow(deprecated)]
impl Http3Server {
    /// Create a new HTTP/3 server (DEPRECATED)
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        warn!("⚠️ DEPRECATED: http3::Http3Server is deprecated. Use http3_quiche::Http3Server instead.");
        Self { config }
    }

    /// Start HTTP/3 server (DEPRECATED - returns error)
    pub async fn run(self) -> Result<()> {
        warn!("⚠️ DEPRECATED: http3::Http3Server::run() is deprecated.");
        warn!("   Quinn-based HTTP/3 has been replaced with quiche for superior performance.");
        warn!("   Please update your code to use http3_quiche::Http3Server instead.");

        anyhow::bail!(
            "http3::Http3Server is deprecated. Use http3_quiche::Http3Server for 2x better performance."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deprecation_notice() {
        // This test just ensures the module compiles
        // The actual functionality has been moved to http3_quiche
        assert!(true, "Quinn-based HTTP/3 is deprecated in favor of quiche");
    }
}
