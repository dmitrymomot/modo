use bytes::Bytes;
use futures_util::stream;
use std::pin::Pin;
use tokio::io::AsyncRead;

/// A streaming uploaded file — data arrives as chunks, not one contiguous buffer.
///
/// During multipart parsing, chunks are collected from the request body into
/// an internal buffer. The stream can then be consumed incrementally via
/// `chunk()` or converted to an `AsyncRead` via `into_reader()`.
pub struct UploadStream {
    name: String,
    file_name: String,
    content_type: String,
    chunks: Vec<Bytes>,
    pos: usize,
}

impl UploadStream {
    /// Create from an axum multipart field by draining its chunks.
    #[doc(hidden)]
    pub async fn from_field(
        field: axum::extract::multipart::Field<'_>,
    ) -> Result<Self, modo::Error> {
        let name = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().unwrap_or_default().to_owned();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        // Collect chunks from the borrowed field into an owned Vec<Bytes>
        let mut chunks = Vec::new();
        let mut field = field;
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            modo::HttpError::BadRequest.with_message(format!("Failed to read multipart chunk: {e}"))
        })? {
            chunks.push(chunk);
        }

        Ok(Self {
            name,
            file_name,
            content_type,
            chunks,
            pos: 0,
        })
    }

    /// The multipart field name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The original filename provided by the client.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// The MIME content type.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Read the next chunk. Returns `None` when all chunks are consumed.
    pub async fn chunk(&mut self) -> Option<Result<Bytes, std::io::Error>> {
        if self.pos < self.chunks.len() {
            let chunk = self.chunks[self.pos].clone();
            self.pos += 1;
            Some(Ok(chunk))
        } else {
            None
        }
    }

    /// Convert into an `AsyncRead` for use with tokio I/O.
    pub fn into_reader(self) -> Pin<Box<dyn AsyncRead + Send>> {
        let chunks = self.chunks;
        let s = stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
        Box::pin(tokio_util::io::StreamReader::new(s))
    }

    /// Total size of all collected chunks in bytes.
    pub fn size(&self) -> usize {
        self.chunks.iter().map(|c| c.len()).sum()
    }
}
