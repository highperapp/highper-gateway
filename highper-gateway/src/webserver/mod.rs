//! Web Server Module
//!
//! Provides static file serving and PHP-FPM support for Nginx replacement functionality.
//!
//! ## Overview
//!
//! This module transforms the reverse proxy into a complete web server capable of:
//! - Serving static files with zero-copy I/O
//! - Processing PHP requests via PHP-FPM (FastCGI)
//! - MIME type detection and caching
//! - Range request support
//! - Compression (gzip, brotli)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         HTTP Request Handler            │
//! └─────────────────┬───────────────────────┘
//!                   │
//!           ┌───────┴────────┐
//!           │                │
//!    ┌──────▼──────┐  ┌─────▼──────┐
//!    │   Static    │  │  PHP-FPM   │
//!    │    Files    │  │   Handler  │
//!    └──────┬──────┘  └─────┬──────┘
//!           │                │
//!    ┌──────▼──────┐  ┌─────▼──────┐
//!    │  sendfile() │  │  FastCGI   │
//!    │  + kTLS     │  │  Protocol  │
//!    └─────────────┘  └─────┬──────┘
//!                           │
//!                     ┌─────▼──────┐
//!                     │ PHP-FPM    │
//!                     │ Pool       │
//!                     └────────────┘
//! ```
//!
//! ## Performance
//!
//! Static File Serving:
//! - Zero-copy sendfile: **2-3x faster** than read/write
//! - With kTLS: **Near-zero CPU** for TLS encryption
//! - MIME caching: **O(1) lookup** for common types
//!
//! PHP-FPM:
//! - Connection pooling: **40-60% overhead reduction**
//! - Persistent connections: **No connect() syscall overhead**
//! - FastCGI protocol: **Binary protocol, low overhead**

pub mod static_files;
pub mod php_fpm;
pub mod mime;
pub mod config;
pub mod security;
pub mod resource_limits;
pub mod observability;

pub use static_files::*;
pub use php_fpm::*;
pub use mime::*;
pub use config::*;
pub use security::*;
pub use resource_limits::*;
pub use observability::*;
