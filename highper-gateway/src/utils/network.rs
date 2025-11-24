//! Network utilities

use std::net::SocketAddr;

/// Parse a socket address from a string
pub fn parse_addr(addr: &str) -> Result<SocketAddr, std::net::AddrParseError> {
    addr.parse()
}
