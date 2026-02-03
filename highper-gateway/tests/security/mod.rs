//! Security Testing Framework
//!
//! Comprehensive vulnerability analysis and penetration testing framework
//! for Highper Gateway's 15 deployment scenarios.
//!
//! # Test Categories
//!
//! - **Protocol Security**: TCP, HTTP, QUIC, WebSocket, gRPC
//! - **Authentication**: JWT, mTLS, API keys, OAuth2
//! - **Authorization**: RBAC, route-based access control
//! - **Injection**: SQL, XSS, LDAP, command injection
//! - **Cryptographic**: TLS configuration, cipher suites
//! - **DoS/Rate Limiting**: Connection limits, request throttling
//! - **Cache Security**: Poisoning, deception attacks
//!
//! # Running Tests
//!
//! ```bash
//! # Run all security tests
//! cargo test --test security_pentest -- --ignored --test-threads=1
//!
//! # Run specific scenario tests
//! cargo test --test security_pentest scenario_01 -- --ignored
//!
//! # Run specific attack category
//! cargo test --test security_pentest sql_injection -- --ignored
//! ```

pub mod common;
pub mod payloads;
pub mod scenarios;

// Re-export commonly used items
pub use common::*;
pub use payloads::*;
