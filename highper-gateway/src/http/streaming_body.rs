//! Streaming HTTP response body types
//!
//! Provides memory-efficient body types for serving large files and proxied responses.

use bytes::Bytes;
use futures_util::TryStreamExt; // For map_ok
use http_body::{Body, Frame, SizeHint};
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, StreamBody};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

/// Flexible response body that can be either buffered or streamed
pub enum ResponseBody {
    /// Empty body (0 bytes)
    Empty,

    /// Small buffered body (already in memory)
    Buffered(Full<Bytes>),

    /// Streaming body from async reader (for large files)
    Stream(UnsyncBoxBody<Bytes, std::io::Error>),
}

impl ResponseBody {
    /// Create empty body
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Create buffered body from bytes
    pub fn buffered(bytes: Bytes) -> Self {
        Self::Buffered(Full::new(bytes))
    }

    /// Create streaming body from async reader
    pub fn from_reader<R>(reader: R) -> Self
    where
        R: AsyncRead + Send + 'static,
    {
        // Convert AsyncRead to Stream of Bytes
        let reader_stream = ReaderStream::new(reader);

        // Convert to StreamBody and box it
        let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));

        Self::Stream(stream_body.boxed_unsync())
    }

    /// Create streaming body from tokio file
    pub fn from_file(file: tokio::fs::File) -> Self {
        Self::from_reader(file)
    }
}

impl Body for ResponseBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.get_mut() {
            ResponseBody::Empty => Poll::Ready(None),

            ResponseBody::Buffered(full) => Pin::new(full)
                .poll_frame(cx)
                .map_err(|never| match never {}),

            ResponseBody::Stream(stream) => Pin::new(stream)
                .poll_frame(cx)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            ResponseBody::Empty => true,
            ResponseBody::Buffered(full) => full.is_end_stream(),
            ResponseBody::Stream(stream) => stream.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            ResponseBody::Empty => SizeHint::with_exact(0),
            ResponseBody::Buffered(full) => full.size_hint(),
            ResponseBody::Stream(stream) => stream.size_hint(),
        }
    }
}

impl Default for ResponseBody {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt as _;

    #[tokio::test]
    async fn test_empty_body() {
        let mut body = ResponseBody::empty();
        let collected = body.collect().await.unwrap();
        assert_eq!(collected.to_bytes().len(), 0);
    }

    #[tokio::test]
    async fn test_buffered_body() {
        let data = Bytes::from("test data");
        let mut body = ResponseBody::buffered(data.clone());
        let collected = body.collect().await.unwrap();
        assert_eq!(collected.to_bytes(), data);
    }

    #[tokio::test]
    async fn test_streaming_body() {
        use std::io::Cursor;

        let data = b"streaming test data";
        let cursor = Cursor::new(data.to_vec());

        // Wrap in a type that implements AsyncRead
        let async_cursor = tokio::io::BufReader::new(cursor);

        let mut body = ResponseBody::from_reader(async_cursor);
        let collected = body.collect().await.unwrap();
        assert_eq!(collected.to_bytes().as_ref(), data);
    }
}
