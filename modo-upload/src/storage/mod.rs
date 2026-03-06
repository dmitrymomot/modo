#[cfg(feature = "local")]
pub mod local;
#[cfg(feature = "opendal")]
pub mod opendal;

use crate::file::UploadedFile;
use crate::stream::UploadStream;
use std::path::{Component, Path, PathBuf};

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

/// Validate that `path` stays within `base` by rejecting `..`, absolute paths, and other
/// non-normal components. Returns the resolved path under `base`.
pub(crate) fn ensure_within(base: &Path, path: &Path) -> Result<PathBuf, modo::Error> {
    let mut result = base.to_path_buf();
    for component in path.components() {
        match component {
            Component::Normal(c) => result.push(c),
            Component::CurDir => {}
            _ => return Err(modo::Error::internal("Invalid storage path")),
        }
    }
    Ok(result)
}

/// Validate that a logical path (for object stores) contains no `..` or leading `/`.
#[cfg(feature = "opendal")]
pub(crate) fn validate_logical_path(path: &str) -> Result<(), modo::Error> {
    if path.starts_with('/') {
        return Err(modo::Error::internal("Invalid storage path"));
    }
    for segment in path.split('/') {
        if segment == ".." {
            return Err(modo::Error::internal("Invalid storage path"));
        }
    }
    Ok(())
}

/// Generate a unique filename: `{ulid}.{ext}`.
pub(crate) fn generate_filename(original: &str) -> String {
    let id = ulid::Ulid::new().to_string().to_lowercase();
    match original.rsplit('.').next() {
        Some(ext) if ext != original => format!("{id}.{ext}"),
        _ => id,
    }
}
