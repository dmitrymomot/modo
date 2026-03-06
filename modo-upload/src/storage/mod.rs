#[cfg(feature = "local")]
pub mod local;
#[cfg(feature = "opendal")]
pub mod opendal;

use crate::file::UploadedFile;
use crate::stream::UploadStream;

/// Metadata for a stored file.
pub struct StoredFile {
    /// Relative path within the storage backend (e.g. `avatars/01HXK3Q1A2B3.jpg`).
    pub path: String,
    /// File size in bytes.
    pub size: u64,
}

/// Trait for persisting uploaded files to a storage backend.
#[async_trait::async_trait]
pub trait FileStorage: Send + Sync + 'static {
    /// Store a buffered file under `prefix/`. Returns the stored path and size.
    async fn store(&self, prefix: &str, file: &UploadedFile) -> Result<StoredFile, modo::Error>;

    /// Store a streaming file under `prefix/`. Returns the stored path and size.
    async fn store_stream(
        &self,
        prefix: &str,
        stream: &mut UploadStream,
    ) -> Result<StoredFile, modo::Error>;

    /// Delete a file by its storage path.
    async fn delete(&self, path: &str) -> Result<(), modo::Error>;

    /// Check if a file exists at the given storage path.
    async fn exists(&self, path: &str) -> Result<bool, modo::Error>;
}

/// Generate a unique filename: `{ulid}.{ext}`.
pub(crate) fn generate_filename(original: &str) -> String {
    let id = ulid::Ulid::new().to_string().to_lowercase();
    match original.rsplit('.').next() {
        Some(ext) if ext != original => format!("{id}.{ext}"),
        _ => id,
    }
}
