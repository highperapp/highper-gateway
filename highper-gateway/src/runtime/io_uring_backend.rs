//! io_uring backend implementation for AsyncIoBackend trait
//!
//! This module adapts the io_uring shim layer to implement the AsyncIoBackend trait,
//! making it swappable with other I/O backends.

#![cfg(all(feature = "io-uring", target_os = "linux"))]

use super::io_backend::{AsyncIoBackend, BackendStats};
use super::io_uring_shim::GLOBAL_IO_URING;
use async_trait::async_trait;
use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use tokio::net::TcpStream;

/// io_uring backend implementation
///
/// This wraps the io_uring shim layer and exposes it via the AsyncIoBackend trait.
pub struct IoUringBackend {
    // Currently uses the global singleton, but could be extended to support
    // multiple io_uring instances for different workloads
}

impl IoUringBackend {
    /// Create a new io_uring backend
    ///
    /// This initializes the io_uring runtime (via the global singleton)
    pub fn new() -> io::Result<Self> {
        // Force initialization of GLOBAL_IO_URING by accessing it
        let _ = &*GLOBAL_IO_URING;

        Ok(Self {})
    }
}

#[async_trait]
impl AsyncIoBackend for IoUringBackend {
    async fn accept(&self, listener_fd: RawFd) -> io::Result<(TcpStream, SocketAddr)> {
        GLOBAL_IO_URING.accept(listener_fd).await
    }

    async fn read(&self, fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        GLOBAL_IO_URING.read(fd, buf).await
    }

    async fn write(&self, fd: RawFd, buf: &[u8]) -> io::Result<usize> {
        GLOBAL_IO_URING.write(fd, buf).await
    }

    async fn close(&self, fd: RawFd) -> io::Result<()> {
        GLOBAL_IO_URING.close(fd).await
    }

    fn name(&self) -> &'static str {
        "io_uring"
    }

    fn stats(&self) -> BackendStats {
        let io_uring_stats = GLOBAL_IO_URING.stats();

        BackendStats {
            pending_operations: io_uring_stats.pending_operations,
            pending_accepts: io_uring_stats.pending_accepts,
            total_operations: io_uring_stats.total_operations,
            backend_info: format!(
                "io_uring (queue_depth=4096, ops={}, pending={})",
                io_uring_stats.total_operations,
                io_uring_stats.pending_operations + io_uring_stats.pending_accepts
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_uring_backend_creation() {
        let result = IoUringBackend::new();
        match result {
            Ok(backend) => {
                assert_eq!(backend.name(), "io_uring");
                let stats = backend.stats();
                assert_eq!(stats.pending_operations, 0);
            }
            Err(e) => {
                println!("io_uring backend creation failed (expected on some systems): {}", e);
            }
        }
    }
}
