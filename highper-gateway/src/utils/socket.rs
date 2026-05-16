//! Socket optimization utilities for production-grade networking
//!
//! This module provides TCP socket optimizations including:
//! - SO_REUSEADDR and SO_REUSEPORT for immediate port reuse
//! - SO_LINGER for controlled connection termination
//! - TCP_NODELAY to disable Nagle's algorithm
//! - TCP_FASTOPEN for reduced connection latency
//! - Receive and send buffer tuning

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, warn};

/// Socket optimization configuration
#[derive(Debug, Clone)]
pub struct SocketConfig {
    /// Enable SO_REUSEADDR (allows immediate port reuse after close)
    pub reuse_addr: bool,

    /// Enable SO_REUSEPORT (multiple processes can bind to same port)
    pub reuse_port: bool,

    /// SO_LINGER timeout (None = default, Some(0) = immediate RST)
    pub linger: Option<Duration>,

    /// Enable TCP_NODELAY (disable Nagle's algorithm for lower latency)
    pub nodelay: bool,

    /// Enable TCP_FASTOPEN (reduces connection setup by 1 RTT)
    pub fastopen: bool,

    /// TCP keepalive settings
    pub keepalive: Option<Duration>,

    /// Receive buffer size (None = OS default)
    pub recv_buffer_size: Option<usize>,

    /// Send buffer size (None = OS default)
    pub send_buffer_size: Option<usize>,

    /// Backlog size for listen() - number of pending connections
    pub backlog: i32,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            reuse_addr: true,
            reuse_port: true,
            linger: Some(Duration::from_secs(0)), // Immediate RST on close
            nodelay: true,
            fastopen: true,
            keepalive: Some(Duration::from_secs(60)),
            recv_buffer_size: Some(256 * 1024), // 256KB
            send_buffer_size: Some(256 * 1024), // 256KB
            backlog: 4096,                      // Large backlog for high concurrency
        }
    }
}

impl SocketConfig {
    /// Create a production-optimized socket configuration
    pub fn production() -> Self {
        Self {
            reuse_addr: true,
            reuse_port: true,
            linger: Some(Duration::from_secs(0)), // Immediate close for port reuse
            nodelay: true,
            fastopen: true,
            keepalive: Some(Duration::from_secs(60)),
            recv_buffer_size: Some(512 * 1024), // 512KB for high throughput
            send_buffer_size: Some(512 * 1024), // 512KB for high throughput
            backlog: 8192,                      // Very large backlog
        }
    }

    /// Create a low-latency optimized configuration
    pub fn low_latency() -> Self {
        Self {
            reuse_addr: true,
            reuse_port: true,
            linger: Some(Duration::from_secs(0)),
            nodelay: true,  // Critical for low latency
            fastopen: true, // Save 1 RTT
            keepalive: Some(Duration::from_secs(30)),
            recv_buffer_size: Some(128 * 1024), // Smaller buffers for lower latency
            send_buffer_size: Some(128 * 1024),
            backlog: 2048,
        }
    }
}

/// Create an optimized TCP socket bound to the given address
pub fn create_optimized_socket(addr: SocketAddr, config: &SocketConfig) -> std::io::Result<Socket> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

    // Apply socket options
    apply_socket_options(&socket, config)?;

    // Bind to address
    let sock_addr: SockAddr = addr.into();
    socket.bind(&sock_addr)?;

    // Start listening with configured backlog
    socket.listen(config.backlog)?;

    debug!(
        "Optimized socket created and bound to {} with backlog={}",
        addr, config.backlog
    );

    Ok(socket)
}

/// Apply socket optimizations to an existing socket
pub fn apply_socket_options(socket: &Socket, config: &SocketConfig) -> std::io::Result<()> {
    // SO_REUSEADDR: Allow immediate port reuse (critical for <10s port release)
    if config.reuse_addr {
        socket.set_reuse_address(true)?;
        debug!("SO_REUSEADDR enabled");
    }

    // SO_REUSEPORT: Allow multiple processes to bind to same port (Linux 3.9+)
    #[cfg(target_os = "linux")]
    if config.reuse_port {
        match socket.set_reuse_port(true) {
            Ok(_) => debug!("SO_REUSEPORT enabled (Linux)"),
            Err(e) => warn!("Failed to set SO_REUSEPORT: {} (requires Linux 3.9+)", e),
        }
    }

    // SO_LINGER: Control socket close behavior
    // - None: OS default (graceful close with timeout)
    // - Some(0): Immediate RST, no TIME_WAIT (best for port reuse)
    // - Some(n): Graceful close with n-second timeout
    socket.set_linger(config.linger)?;
    debug!("SO_LINGER set to {:?}", config.linger);

    // TCP_NODELAY: Disable Nagle's algorithm for lower latency
    // Nagle's algorithm buffers small packets, which adds latency
    if config.nodelay {
        socket.set_nodelay(true)?;
        debug!("TCP_NODELAY enabled");
    }

    // TCP keepalive: Detect dead connections
    if let Some(keepalive_duration) = config.keepalive {
        #[cfg(target_os = "linux")]
        {
            use socket2::TcpKeepalive;
            let keepalive = TcpKeepalive::new()
                .with_time(keepalive_duration)
                .with_interval(Duration::from_secs(10))
                .with_retries(3);
            socket.set_tcp_keepalive(&keepalive)?;
            debug!("TCP keepalive enabled: {}s", keepalive_duration.as_secs());
        }

        #[cfg(not(target_os = "linux"))]
        {
            socket.set_keepalive(true)?;
            debug!("TCP keepalive enabled (platform-specific)");
        }
    }

    // Receive buffer size
    if let Some(size) = config.recv_buffer_size {
        socket.set_recv_buffer_size(size)?;
        debug!("Receive buffer size set to {} bytes", size);
    }

    // Send buffer size
    if let Some(size) = config.send_buffer_size {
        socket.set_send_buffer_size(size)?;
        debug!("Send buffer size set to {} bytes", size);
    }

    // TCP_FASTOPEN: Reduce connection setup by 1 RTT
    // This allows data to be sent in the SYN packet (requires kernel support)
    #[cfg(target_os = "linux")]
    if config.fastopen {
        // The value is the queue length for TFO requests
        // 5 is a reasonable default for server-side TFO
        match set_tcp_fastopen(&socket, 5) {
            Ok(_) => debug!("TCP_FASTOPEN enabled (queue=5)"),
            Err(e) => warn!("Failed to set TCP_FASTOPEN: {} (requires Linux 3.7+, sysctl net.ipv4.tcp_fastopen=3)", e),
        }
    }

    // TCP_QUICKACK: Reduce ACK delay for lower latency
    // Disables delayed ACKs which can add 40-200ms latency
    #[cfg(target_os = "linux")]
    {
        match set_tcp_quickack(&socket, true) {
            Ok(_) => debug!("TCP_QUICKACK enabled"),
            Err(e) => warn!("Failed to set TCP_QUICKACK: {}", e),
        }
    }

    Ok(())
}

/// Set TCP Fast Open on a socket (Linux-specific)
#[cfg(target_os = "linux")]
fn set_tcp_fastopen(socket: &Socket, queue_len: i32) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;

    const TCP_FASTOPEN: i32 = 23; // Linux TCP_FASTOPEN constant

    let ret = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::IPPROTO_TCP,
            TCP_FASTOPEN,
            &queue_len as *const i32 as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Set TCP Quick ACK on a socket (Linux-specific)
/// Disables delayed ACKs for lower latency (can reduce ACK delay from 40-200ms to <1ms)
#[cfg(target_os = "linux")]
fn set_tcp_quickack(socket: &Socket, enable: bool) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;

    const TCP_QUICKACK: i32 = 12; // Linux TCP_QUICKACK constant

    let val: i32 = if enable { 1 } else { 0 };

    let ret = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::IPPROTO_TCP,
            TCP_QUICKACK,
            &val as *const i32 as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Get current socket statistics (for monitoring)
#[derive(Debug, Default)]
pub struct SocketStats {
    pub recv_buffer_size: usize,
    pub send_buffer_size: usize,
    pub recv_queue: usize,
    pub send_queue: usize,
}

impl SocketStats {
    pub fn from_socket(socket: &Socket) -> std::io::Result<Self> {
        let recv_buffer_size = socket.recv_buffer_size()?;
        let send_buffer_size = socket.send_buffer_size()?;

        Ok(Self {
            recv_buffer_size,
            send_buffer_size,
            recv_queue: 0, // Would need ioctl FIONREAD to get
            send_queue: 0, // Would need ioctl TIOCOUTQ to get
        })
    }
}

/// Convert socket2::Socket to tokio::net::TcpListener
pub fn socket_to_listener(socket: Socket) -> std::io::Result<tokio::net::TcpListener> {
    socket.set_nonblocking(true)?;
    let std_listener: std::net::TcpListener = socket.into();
    tokio::net::TcpListener::from_std(std_listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_config_default() {
        let config = SocketConfig::default();
        assert!(config.reuse_addr);
        assert!(config.nodelay);
        assert_eq!(config.linger, Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_socket_config_production() {
        let config = SocketConfig::production();
        assert!(config.reuse_addr);
        assert!(config.reuse_port);
        assert!(config.fastopen);
        assert_eq!(config.backlog, 8192);
    }

    #[tokio::test]
    async fn test_create_optimized_socket() {
        let addr = "127.0.0.1:0".parse().unwrap();
        let config = SocketConfig::default();

        let socket = create_optimized_socket(addr, &config).unwrap();
        let stats = SocketStats::from_socket(&socket).unwrap();

        assert!(stats.recv_buffer_size > 0);
        assert!(stats.send_buffer_size > 0);
    }
}
