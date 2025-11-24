//! epoll/kqueue backend implementation for AsyncIoBackend trait
//!
//! This module provides a fallback I/O backend that uses standard tokio
//! (which internally uses epoll on Linux, kqueue on macOS/BSD).
//!
//! This backend is used when:
//! - io_uring is not available (old kernel, disabled via sysctl)
//! - Running on non-Linux platforms (macOS, BSD, Windows)
//! - io_uring feature is not enabled at compile time

use super::io_backend::{AsyncIoBackend, BackendStats};
use async_trait::async_trait;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::{FromRawFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// epoll/kqueue backend implementation
///
/// This uses standard tokio for I/O operations, which provides:
/// - epoll on Linux
/// - kqueue on macOS/BSD
/// - IOCP on Windows
pub struct EpollBackend {
    /// Counter for total operations (for stats)
    total_operations: AtomicU64,
}

impl EpollBackend {
    /// Create a new epoll/kqueue backend
    pub fn new() -> Self {
        Self {
            total_operations: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl AsyncIoBackend for EpollBackend {
    async fn accept(&self, listener_fd: RawFd) -> io::Result<(TcpStream, SocketAddr)> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Convert raw fd to std::net::TcpListener, then to tokio::net::TcpListener
        let std_listener = unsafe {
            // Safety: We assume the fd is valid and points to a listening socket
            // The fd is NOT closed when std_listener is dropped (we duplicate it)
            std::net::TcpListener::from_raw_fd(libc::dup(listener_fd))
        };

        std_listener.set_nonblocking(true)?;
        let tokio_listener = tokio::net::TcpListener::from_std(std_listener)?;

        // Accept connection using tokio
        tokio_listener.accept().await
    }

    async fn read(&self, fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Convert raw fd to tokio TcpStream
        let std_stream = unsafe {
            // Safety: Duplicate fd to avoid ownership issues
            std::net::TcpStream::from_raw_fd(libc::dup(fd))
        };

        std_stream.set_nonblocking(true)?;
        let mut tokio_stream = TcpStream::from_std(std_stream)?;

        // Read using tokio
        tokio_stream.read(buf).await
    }

    async fn write(&self, fd: RawFd, buf: &[u8]) -> io::Result<usize> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Convert raw fd to tokio TcpStream
        let std_stream = unsafe {
            // Safety: Duplicate fd to avoid ownership issues
            std::net::TcpStream::from_raw_fd(libc::dup(fd))
        };

        std_stream.set_nonblocking(true)?;
        let mut tokio_stream = TcpStream::from_std(std_stream)?;

        // Write using tokio
        tokio_stream.write(buf).await
    }

    async fn close(&self, fd: RawFd) -> io::Result<()> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Close the file descriptor
        let result = unsafe { libc::close(fd) };

        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn name(&self) -> &'static str {
        #[cfg(target_os = "linux")]
        return "epoll";

        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        return "kqueue";

        #[cfg(target_os = "windows")]
        return "iocp";

        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "windows"
        )))]
        return "unknown";
    }

    fn stats(&self) -> BackendStats {
        let total = self.total_operations.load(Ordering::Relaxed);

        BackendStats {
            pending_operations: 0, // tokio doesn't expose this
            pending_accepts: 0,
            total_operations: total,
            backend_info: format!("{} (via tokio, total_ops={})", self.name(), total),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;

    #[test]
    fn test_epoll_backend_creation() {
        let backend = EpollBackend::new();

        #[cfg(target_os = "linux")]
        assert_eq!(backend.name(), "epoll");

        #[cfg(target_os = "macos")]
        assert_eq!(backend.name(), "kqueue");

        let stats = backend.stats();
        assert_eq!(stats.total_operations, 0);
    }

    #[tokio::test]
    async fn test_epoll_backend_accept() {
        let backend = EpollBackend::new();

        // Create a test listener
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let fd = listener.as_raw_fd();

        // Connect in background
        tokio::spawn(async move {
            let _stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Accept using backend
        let result = backend.accept(fd).await;
        assert!(result.is_ok());

        let stats = backend.stats();
        assert_eq!(stats.total_operations, 1);
    }
}
