//! Protocol detection and handling for TCP proxy
//!
//! Supports automatic protocol detection for:
//! - MySQL (port 3306)
//! - PostgreSQL (port 5432)
//! - Redis (port 6379)
//! - Generic TCP

use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Supported protocols
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// MySQL protocol
    Mysql,

    /// PostgreSQL protocol
    Postgresql,

    /// Redis protocol (RESP)
    Redis,

    /// Generic TCP (no protocol awareness)
    #[default]
    Generic,
}

impl Protocol {
    /// Get default port for this protocol
    pub fn default_port(&self) -> u16 {
        match self {
            Protocol::Mysql => 3306,
            Protocol::Postgresql => 5432,
            Protocol::Redis => 6379,
            Protocol::Generic => 0,
        }
    }

    /// Check if this protocol supports pipelining
    pub fn supports_pipelining(&self) -> bool {
        match self {
            Protocol::Redis => true,
            Protocol::Mysql => false,
            Protocol::Postgresql => false,
            Protocol::Generic => false,
        }
    }

    /// Get protocol name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Mysql => "mysql",
            Protocol::Postgresql => "postgresql",
            Protocol::Redis => "redis",
            Protocol::Generic => "generic",
        }
    }
}

/// Protocol detector
pub struct ProtocolDetector;

impl ProtocolDetector {
    /// Detect protocol from first bytes
    pub async fn detect<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Protocol> {
        let mut buf = [0u8; 16];

        // Peek at first few bytes
        let n = reader.read(&mut buf).await?;

        if n == 0 {
            return Ok(Protocol::Generic);
        }

        // MySQL handshake starts with packet length (3 bytes) + sequence (1 byte) + protocol version (1 byte = 0x0a)
        if n >= 5 && buf[4] == 0x0a {
            return Ok(Protocol::Mysql);
        }

        // PostgreSQL startup message starts with length (4 bytes) + protocol version (4 bytes)
        // Protocol version is usually 0x00030000 (3.0) or 0x00030001 (3.1)
        if n >= 8 {
            let proto = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
            if proto == 0x00030000 || proto == 0x00030001 || proto == 0x00030002 {
                return Ok(Protocol::Postgresql);
            }
        }

        // Redis RESP protocol - commands start with '*' (array), '+' (simple string), '-' (error), ':' (integer), '$' (bulk string)
        if n >= 1 && matches!(buf[0], b'*' | b'+' | b'-' | b':' | b'$') {
            return Ok(Protocol::Redis);
        }

        Ok(Protocol::Generic)
    }

    /// Detect protocol from port number
    pub fn detect_from_port(port: u16) -> Protocol {
        match port {
            3306 => Protocol::Mysql,
            5432 => Protocol::Postgresql,
            6379 => Protocol::Redis,
            _ => Protocol::Generic,
        }
    }
}

/// MySQL protocol handler
pub struct MysqlProtocol;

impl MysqlProtocol {
    /// Check if buffer contains a complete MySQL packet
    pub fn is_complete_packet(buf: &[u8]) -> bool {
        if buf.len() < 4 {
            return false;
        }

        // MySQL packet format: 3 bytes length + 1 byte sequence + payload
        let payload_len = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]) as usize;
        buf.len() >= payload_len + 4
    }

    /// Extract packet length from MySQL packet header
    pub fn packet_length(buf: &[u8]) -> Option<usize> {
        if buf.len() < 4 {
            return None;
        }

        let payload_len = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]) as usize;
        Some(payload_len + 4) // +4 for header
    }

    /// Create MySQL ping packet
    pub fn ping_packet() -> Vec<u8> {
        vec![
            0x01, 0x00, 0x00, // Length: 1 byte
            0x00,             // Sequence: 0
            0x0e,             // COM_PING command
        ]
    }

    /// Check if packet is a MySQL error response
    pub fn is_error_packet(buf: &[u8]) -> bool {
        buf.len() >= 5 && buf[4] == 0xff
    }
}

/// PostgreSQL protocol handler
pub struct PostgresqlProtocol;

impl PostgresqlProtocol {
    /// Check if buffer contains a complete PostgreSQL message
    pub fn is_complete_message(buf: &[u8]) -> bool {
        if buf.len() < 5 {
            return false;
        }

        // PostgreSQL message format: 1 byte type + 4 bytes length (including length itself) + payload
        let msg_len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
        buf.len() >= msg_len + 1 // +1 for type byte
    }

    /// Extract message length from PostgreSQL message header
    pub fn message_length(buf: &[u8]) -> Option<usize> {
        if buf.len() < 5 {
            return None;
        }

        let msg_len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
        Some(msg_len + 1) // +1 for type byte
    }

    /// Create PostgreSQL simple query (for health check)
    pub fn simple_query(sql: &str) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.push(b'Q'); // Query message type

        let len = (sql.len() + 5) as u32; // +4 for length field, +1 for null terminator
        packet.extend_from_slice(&len.to_be_bytes());
        packet.extend_from_slice(sql.as_bytes());
        packet.push(0); // Null terminator

        packet
    }

    /// Check if message is an error response
    pub fn is_error_message(buf: &[u8]) -> bool {
        buf.len() >= 1 && buf[0] == b'E'
    }
}

/// Redis protocol handler (RESP - REdis Serialization Protocol)
pub struct RedisProtocol;

impl RedisProtocol {
    /// Check if buffer contains a complete RESP message
    pub fn is_complete_message(buf: &[u8]) -> bool {
        if buf.is_empty() {
            return false;
        }

        match buf[0] {
            b'+' | b'-' | b':' => {
                // Simple string, error, integer - terminated by \r\n
                buf.windows(2).any(|w| w == b"\r\n")
            }
            b'$' => {
                // Bulk string
                if let Some(len) = Self::parse_bulk_length(buf) {
                    if len == -1 {
                        // Null bulk string
                        return buf.windows(2).any(|w| w == b"\r\n");
                    }
                    if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                        let header_len = pos + 1;
                        buf.len() >= header_len + len as usize + 2 // +2 for \r\n
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            b'*' => {
                // Array - need to parse recursively
                // For simplicity, just check for complete simple messages
                buf.windows(2).any(|w| w == b"\r\n")
            }
            _ => false,
        }
    }

    /// Parse bulk string length from RESP
    fn parse_bulk_length(buf: &[u8]) -> Option<i64> {
        if buf.len() < 3 || buf[0] != b'$' {
            return None;
        }

        let end = buf.iter().position(|&b| b == b'\r')?;
        let len_str = std::str::from_utf8(&buf[1..end]).ok()?;
        len_str.parse().ok()
    }

    /// Create Redis PING command
    pub fn ping_command() -> Vec<u8> {
        b"*1\r\n$4\r\nPING\r\n".to_vec()
    }

    /// Create Redis command
    pub fn command(args: &[&str]) -> Vec<u8> {
        let mut cmd = format!("*{}\r\n", args.len());
        for arg in args {
            cmd.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }
        cmd.into_bytes()
    }

    /// Check if message is an error response
    pub fn is_error_message(buf: &[u8]) -> bool {
        buf.len() >= 1 && buf[0] == b'-'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_default_ports() {
        assert_eq!(Protocol::Mysql.default_port(), 3306);
        assert_eq!(Protocol::Postgresql.default_port(), 5432);
        assert_eq!(Protocol::Redis.default_port(), 6379);
        assert_eq!(Protocol::Generic.default_port(), 0);
    }

    #[test]
    fn test_protocol_pipelining() {
        assert!(Protocol::Redis.supports_pipelining());
        assert!(!Protocol::Mysql.supports_pipelining());
        assert!(!Protocol::Postgresql.supports_pipelining());
        assert!(!Protocol::Generic.supports_pipelining());
    }

    #[test]
    fn test_mysql_ping_packet() {
        let ping = MysqlProtocol::ping_packet();
        assert_eq!(ping.len(), 5);
        assert_eq!(ping[4], 0x0e); // COM_PING
    }

    #[test]
    fn test_mysql_packet_length() {
        let packet = vec![0x05, 0x00, 0x00, 0x00, 0x0e, 0x01, 0x02, 0x03, 0x04];
        assert_eq!(MysqlProtocol::packet_length(&packet), Some(9));
    }

    #[test]
    fn test_postgresql_simple_query() {
        let query = PostgresqlProtocol::simple_query("SELECT 1");
        assert!(query.len() > 0);
        assert_eq!(query[0], b'Q');
    }

    #[test]
    fn test_redis_ping_command() {
        let ping = RedisProtocol::ping_command();
        assert_eq!(ping, b"*1\r\n$4\r\nPING\r\n");
    }

    #[test]
    fn test_redis_command() {
        let cmd = RedisProtocol::command(&["GET", "key"]);
        assert_eq!(cmd, b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n");
    }

    #[test]
    fn test_redis_is_complete_message() {
        assert!(RedisProtocol::is_complete_message(b"+OK\r\n"));
        assert!(RedisProtocol::is_complete_message(b"-ERR\r\n"));
        assert!(RedisProtocol::is_complete_message(b":123\r\n"));
        assert!(!RedisProtocol::is_complete_message(b"+OK"));
    }

    #[test]
    fn test_detect_from_port() {
        assert_eq!(ProtocolDetector::detect_from_port(3306), Protocol::Mysql);
        assert_eq!(ProtocolDetector::detect_from_port(5432), Protocol::Postgresql);
        assert_eq!(ProtocolDetector::detect_from_port(6379), Protocol::Redis);
        assert_eq!(ProtocolDetector::detect_from_port(8080), Protocol::Generic);
    }
}
