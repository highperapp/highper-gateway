//! PHP-FPM support via FastCGI protocol

use super::PhpFpmConfig;
use dashmap::DashMap;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

/// FastCGI protocol constants
const FCGI_VERSION: u8 = 1;
const FCGI_BEGIN_REQUEST: u8 = 1;
const FCGI_PARAMS: u8 = 4;
const FCGI_STDIN: u8 = 5;
const FCGI_STDOUT: u8 = 6;
const FCGI_STDERR: u8 = 7;
const FCGI_END_REQUEST: u8 = 3;

const FCGI_RESPONDER: u16 = 1;
const FCGI_KEEP_CONN: u8 = 1;

/// PHP-FPM connection pool
pub struct PhpFpmPool {
    config: PhpFpmConfig,
    connections: Arc<DashMap<usize, PooledConnection>>,
    next_id: std::sync::atomic::AtomicUsize,
}

impl PhpFpmPool {
    /// Create a new PHP-FPM connection pool
    pub fn new(config: PhpFpmConfig) -> Self {
        let pool_size = config.pool_size;
        Self {
            config,
            connections: Arc::new(DashMap::with_capacity(pool_size)),
            next_id: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> io::Result<PooledFpmConnection> {
        // Try to find an idle connection
        for entry in self.connections.iter() {
            let conn = entry.value();
            if conn.is_idle() && !conn.is_expired() {
                return Ok(PooledFpmConnection {
                    id: *entry.key(),
                    socket: Connection::Unix(UnixStream::connect(&self.config.socket)?),
                    pool: Arc::clone(&self.connections),
                });
            }
        }

        // No idle connection, create new if under limit
        if self.connections.len() < self.config.pool_size {
            let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let socket = self.create_connection()?;

            let pooled_conn = PooledConnection {
                last_used: Instant::now(),
                in_use: true,
            };

            self.connections.insert(id, pooled_conn);

            return Ok(PooledFpmConnection {
                id,
                socket,
                pool: Arc::clone(&self.connections),
            });
        }

        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Connection pool exhausted"
        ))
    }

    /// Create a new connection to PHP-FPM
    fn create_connection(&self) -> io::Result<Connection> {
        if self.config.socket.starts_with('/') || self.config.socket.starts_with('.') {
            // Unix socket
            let stream = UnixStream::connect(&self.config.socket)?;
            stream.set_read_timeout(Some(Duration::from_secs(self.config.read_timeout)))?;
            stream.set_write_timeout(Some(Duration::from_secs(self.config.write_timeout)))?;
            Ok(Connection::Unix(stream))
        } else {
            // TCP socket
            let stream = TcpStream::connect(&self.config.socket)?;
            stream.set_read_timeout(Some(Duration::from_secs(self.config.read_timeout)))?;
            stream.set_write_timeout(Some(Duration::from_secs(self.config.write_timeout)))?;
            Ok(Connection::Tcp(stream))
        }
    }

    /// Clean up expired connections
    pub fn cleanup_expired(&self) {
        let mut to_remove = Vec::new();

        for entry in self.connections.iter() {
            if entry.value().is_expired() {
                to_remove.push(*entry.key());
            }
        }

        for id in to_remove {
            self.connections.remove(&id);
        }
    }
}

/// Pooled connection state
struct PooledConnection {
    last_used: Instant,
    in_use: bool,
}

impl PooledConnection {
    fn is_idle(&self) -> bool {
        !self.in_use
    }

    fn is_expired(&self) -> bool {
        self.last_used.elapsed() > Duration::from_secs(60)
    }
}

/// Pooled FastCGI connection
pub struct PooledFpmConnection {
    id: usize,
    socket: Connection,
    pool: Arc<DashMap<usize, PooledConnection>>,
}

impl PooledFpmConnection {
    /// Execute FastCGI request
    pub fn execute(&mut self, params: &[(String, String)], stdin: &[u8]) -> io::Result<Vec<u8>> {
        let request_id: u16 = 1;

        // Send BEGIN_REQUEST
        self.send_begin_request(request_id)?;

        // Send PARAMS
        self.send_params(request_id, params)?;

        // Send empty PARAMS (end of params)
        self.send_record(request_id, FCGI_PARAMS, &[])?;

        // Send STDIN
        if !stdin.is_empty() {
            self.send_record(request_id, FCGI_STDIN, stdin)?;
        }

        // Send empty STDIN (end of stdin)
        self.send_record(request_id, FCGI_STDIN, &[])?;

        // Read response
        let stdout = self.read_response(request_id)?;

        Ok(stdout)
    }

    fn send_begin_request(&mut self, request_id: u16) -> io::Result<()> {
        let mut content = Vec::with_capacity(8);
        content.extend_from_slice(&FCGI_RESPONDER.to_be_bytes());
        content.push(FCGI_KEEP_CONN);
        content.extend_from_slice(&[0u8; 5]); // Reserved

        self.send_record(request_id, FCGI_BEGIN_REQUEST, &content)
    }

    fn send_params(&mut self, request_id: u16, params: &[(String, String)]) -> io::Result<()> {
        let mut content = Vec::new();

        for (key, value) in params {
            // Encode key length
            Self::encode_length(&mut content, key.len());
            // Encode value length
            Self::encode_length(&mut content, value.len());
            // Key
            content.extend_from_slice(key.as_bytes());
            // Value
            content.extend_from_slice(value.as_bytes());
        }

        self.send_record(request_id, FCGI_PARAMS, &content)
    }

    fn encode_length(buf: &mut Vec<u8>, length: usize) {
        if length < 128 {
            buf.push(length as u8);
        } else {
            buf.extend_from_slice(&((length as u32) | 0x80000000).to_be_bytes());
        }
    }

    fn send_record(&mut self, request_id: u16, record_type: u8, content: &[u8]) -> io::Result<()> {
        let content_length = content.len().min(0xFFFF);
        let padding_length = (8 - (content_length % 8)) % 8;

        // FastCGI record header (8 bytes)
        let header = [
            FCGI_VERSION,
            record_type,
            (request_id >> 8) as u8,
            (request_id & 0xFF) as u8,
            (content_length >> 8) as u8,
            (content_length & 0xFF) as u8,
            padding_length as u8,
            0, // Reserved
        ];

        self.socket.write_all(&header)?;
        self.socket.write_all(&content[..content_length])?;

        if padding_length > 0 {
            self.socket.write_all(&vec![0u8; padding_length])?;
        }

        Ok(())
    }

    fn read_response(&mut self, _request_id: u16) -> io::Result<Vec<u8>> {
        let mut stdout_data = Vec::new();

        loop {
            // Read record header
            let mut header = [0u8; 8];
            self.socket.read_exact(&mut header)?;

            let record_type = header[1];
            let content_length = ((header[4] as usize) << 8) | (header[5] as usize);
            let padding_length = header[6] as usize;

            // Read content
            let mut content = vec![0u8; content_length];
            self.socket.read_exact(&mut content)?;

            // Read padding
            if padding_length > 0 {
                let mut padding = vec![0u8; padding_length];
                self.socket.read_exact(&mut padding)?;
            }

            match record_type {
                FCGI_STDOUT => {
                    if content.is_empty() {
                        // End of stdout
                        break;
                    }
                    stdout_data.extend_from_slice(&content);
                }
                FCGI_STDERR => {
                    if !content.is_empty() {
                        warn!("PHP-FPM stderr: {}", String::from_utf8_lossy(&content));
                    }
                }
                FCGI_END_REQUEST => {
                    break;
                }
                _ => {}
            }
        }

        Ok(stdout_data)
    }
}

impl Drop for PooledFpmConnection {
    fn drop(&mut self) {
        if let Some(mut conn) = self.pool.get_mut(&self.id) {
            conn.in_use = false;
            conn.last_used = Instant::now();
        }
    }
}

/// Connection type (Unix or TCP socket)
enum Connection {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl Connection {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Unix(s) => s.write_all(buf),
            Connection::Tcp(s) => s.write_all(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match self {
            Connection::Unix(s) => s.read_exact(buf),
            Connection::Tcp(s) => s.read_exact(buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_length_short() {
        let mut buf = Vec::new();
        PooledFpmConnection::encode_length(&mut buf, 100);
        assert_eq!(buf, vec![100]);
    }

    #[test]
    fn test_encode_length_long() {
        let mut buf = Vec::new();
        PooledFpmConnection::encode_length(&mut buf, 1000);
        assert_eq!(buf.len(), 4);
        assert!(buf[0] & 0x80 != 0); // High bit set
    }

    #[test]
    fn test_pool_creation() {
        let config = PhpFpmConfig {
            socket: "/var/run/php/php-fpm.sock".to_string(),
            pool_size: 5,
            ..Default::default()
        };

        let pool = PhpFpmPool::new(config);
        assert_eq!(pool.connections.len(), 0);
    }
}
