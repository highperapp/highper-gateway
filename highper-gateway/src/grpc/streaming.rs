//! gRPC streaming utilities and frame handling
//!
//! Provides utilities for handling gRPC streaming including:
//! - Frame processing without buffering
//! - Streaming context management
//! - Backpressure handling

use bytes::{Bytes, BytesMut, Buf};
use std::pin::Pin;
use std::task::{Context, Poll};
use http_body::{Body, Frame};
use anyhow::{Result, anyhow};

/// gRPC frame header (5 bytes)
///
/// Layout:
/// - Byte 0: Compressed-Flag (0 = uncompressed, 1 = compressed)
/// - Bytes 1-4: Message-Length (32-bit big-endian unsigned integer)
#[derive(Debug, Clone, Copy)]
pub struct GrpcFrameHeader {
    pub compressed: bool,
    pub length: u32,
}

impl GrpcFrameHeader {
    /// Header size in bytes
    pub const SIZE: usize = 5;

    /// Parse header from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(anyhow!("Incomplete gRPC frame header"));
        }

        let compressed = bytes[0] != 0;
        let length = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);

        Ok(Self { compressed, length })
    }

    /// Encode header to bytes
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut header = [0u8; Self::SIZE];
        header[0] = if self.compressed { 1 } else { 0 };
        header[1..5].copy_from_slice(&self.length.to_be_bytes());
        header
    }
}

/// gRPC frame with header and data
#[derive(Debug, Clone)]
pub struct GrpcFrame {
    pub header: GrpcFrameHeader,
    pub data: Bytes,
}

impl GrpcFrame {
    /// Create a new uncompressed frame
    pub fn new(data: Bytes) -> Self {
        Self {
            header: GrpcFrameHeader {
                compressed: false,
                length: data.len() as u32,
            },
            data,
        }
    }

    /// Encode frame to bytes (header + data)
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(GrpcFrameHeader::SIZE + self.data.len());
        buf.extend_from_slice(&self.header.encode());
        buf.extend_from_slice(&self.data);
        buf.freeze()
    }

    /// Parse frame from buffer (returns frame and remaining bytes)
    pub fn parse(mut buf: Bytes) -> Result<Option<(Self, Bytes)>> {
        if buf.len() < GrpcFrameHeader::SIZE {
            // Not enough data for header
            return Ok(None);
        }

        let header = GrpcFrameHeader::parse(&buf[..GrpcFrameHeader::SIZE])?;
        let total_size = GrpcFrameHeader::SIZE + header.length as usize;

        if buf.len() < total_size {
            // Not enough data for complete frame
            return Ok(None);
        }

        // Extract frame data
        buf.advance(GrpcFrameHeader::SIZE);
        let data = buf.split_to(header.length as usize);

        Ok(Some((Self { header, data }, buf)))
    }
}

/// Streaming body wrapper that preserves gRPC framing
///
/// This is a transparent pass-through that doesn't parse or modify frames,
/// ensuring zero-copy streaming for all gRPC call types.
pub struct GrpcStreamingBody<B> {
    inner: B,
}

impl<B> GrpcStreamingBody<B> {
    pub fn new(body: B) -> Self {
        Self { inner: body }
    }

    pub fn into_inner(self) -> B {
        self.inner
    }
}

impl<B> Body for GrpcStreamingBody<B>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // Pass through frames directly without modification
        Pin::new(&mut self.inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

/// Statistics for gRPC streaming
#[derive(Debug, Default, Clone)]
pub struct StreamingStats {
    /// Number of frames sent
    pub frames_sent: usize,
    /// Total bytes sent
    pub bytes_sent: usize,
    /// Number of frames received
    pub frames_received: usize,
    /// Total bytes received
    pub bytes_received: usize,
}

impl StreamingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_sent_frame(&mut self, size: usize) {
        self.frames_sent += 1;
        self.bytes_sent += size;
    }

    pub fn record_received_frame(&mut self, size: usize) {
        self.frames_received += 1;
        self.bytes_received += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_frame_header_parse() {
        let bytes = [0u8, 0, 0, 0, 10]; // Uncompressed, length 10
        let header = GrpcFrameHeader::parse(&bytes).unwrap();
        assert!(!header.compressed);
        assert_eq!(header.length, 10);

        let bytes = [1u8, 0, 0, 1, 0]; // Compressed, length 256
        let header = GrpcFrameHeader::parse(&bytes).unwrap();
        assert!(header.compressed);
        assert_eq!(header.length, 256);
    }

    #[test]
    fn test_grpc_frame_header_encode() {
        let header = GrpcFrameHeader {
            compressed: false,
            length: 42,
        };
        let bytes = header.encode();
        assert_eq!(bytes, [0u8, 0, 0, 0, 42]);

        let header = GrpcFrameHeader {
            compressed: true,
            length: 1024,
        };
        let bytes = header.encode();
        assert_eq!(bytes, [1u8, 0, 0, 4, 0]);
    }

    #[test]
    fn test_grpc_frame_encode() {
        let frame = GrpcFrame::new(Bytes::from("hello"));
        let encoded = frame.encode();

        // Header: [0, 0, 0, 0, 5] + Data: "hello"
        assert_eq!(encoded.len(), 10);
        assert_eq!(&encoded[0..5], &[0, 0, 0, 0, 5]);
        assert_eq!(&encoded[5..], b"hello");
    }

    #[test]
    fn test_grpc_frame_parse() {
        // Create a frame
        let data = Bytes::from("test message");
        let frame = GrpcFrame::new(data.clone());
        let encoded = frame.encode();

        // Parse it back
        let (parsed, remaining) = GrpcFrame::parse(encoded).unwrap().unwrap();
        assert_eq!(parsed.data, data);
        assert_eq!(parsed.header.length, 12);
        assert!(!parsed.header.compressed);
        assert_eq!(remaining.len(), 0);
    }

    #[test]
    fn test_grpc_frame_parse_incomplete() {
        // Only 3 bytes (not enough for header)
        let buf = Bytes::from(&[0, 0, 0][..]);
        assert!(GrpcFrame::parse(buf).unwrap().is_none());

        // Header complete but data incomplete
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[0, 0, 0, 0, 10]); // Header says 10 bytes
        buf.extend_from_slice(b"short"); // Only 5 bytes of data
        assert!(GrpcFrame::parse(buf.freeze()).unwrap().is_none());
    }

    #[test]
    fn test_streaming_stats() {
        let mut stats = StreamingStats::new();
        stats.record_sent_frame(100);
        stats.record_sent_frame(200);
        stats.record_received_frame(150);

        assert_eq!(stats.frames_sent, 2);
        assert_eq!(stats.bytes_sent, 300);
        assert_eq!(stats.frames_received, 1);
        assert_eq!(stats.bytes_received, 150);
    }
}
