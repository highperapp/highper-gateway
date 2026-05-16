//! TLS acceptor with SNI support and mTLS client certificate extraction

use crate::tls::client_cert::ClientCertInfo;
use rustls::ServerConfig;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor as RustlsAcceptor;
use tracing::{debug, error};

/// TLS acceptor that wraps a TCP stream with TLS
pub struct TlsAcceptor {
    inner: RustlsAcceptor,
}

impl TlsAcceptor {
    /// Create a new TLS acceptor from a ServerConfig
    pub fn new(config: Arc<ServerConfig>) -> Self {
        Self {
            inner: RustlsAcceptor::from(config),
        }
    }

    /// Accept a TLS connection
    pub async fn accept(&self, stream: TcpStream) -> io::Result<TlsStream<TcpStream>> {
        debug!("Accepting TLS connection");

        match self.inner.accept(stream).await {
            Ok(tls_stream) => {
                debug!("TLS handshake completed successfully");
                Ok(tls_stream)
            }
            Err(e) => {
                error!("TLS handshake failed: {}", e);
                Err(e)
            }
        }
    }

    /// Extract client certificate information from a TLS stream
    ///
    /// # Arguments
    /// * `tls_stream` - Accepted TLS stream
    ///
    /// # Returns
    /// * `Option<ClientCertInfo>` - Client certificate info if available
    pub fn extract_client_cert(tls_stream: &TlsStream<TcpStream>) -> Option<ClientCertInfo> {
        // Get connection info from TLS stream
        let (_io, conn) = tls_stream.get_ref();

        // Check if peer certificates are available
        if let Some(certs) = conn.peer_certificates() {
            if let Some(first_cert) = certs.first() {
                // Extract info from the first (leaf) certificate
                return ClientCertInfo::from_der(first_cert);
            }
        }

        None
    }
}

/// Wrapper for either a plain TCP stream or a TLS stream
pub enum MaybeTlsStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl AsyncRead for MaybeTlsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            MaybeTlsStream::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTlsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            MaybeTlsStream::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(stream) => Pin::new(stream).poll_flush(cx),
            MaybeTlsStream::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            MaybeTlsStream::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_tls_stream_size() {
        let size = std::mem::size_of::<MaybeTlsStream>();

        // TLS streams contain internal buffers (16KB TLS record buffer)
        // Ensure size is reasonable (< 32KB total)
        assert!(
            size < 32 * 1024,
            "MaybeTlsStream size is {} bytes, expected < 32KB",
            size
        );

        // For monitoring - log actual size
        println!(
            "MaybeTlsStream actual size: {} bytes ({:.1} KB)",
            size,
            size as f64 / 1024.0
        );
    }
}
