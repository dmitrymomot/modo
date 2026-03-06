use crate::validate::UploadValidator;

/// An uploaded file fully buffered in memory.
pub struct UploadedFile {
    name: String,
    file_name: String,
    content_type: String,
    data: bytes::Bytes,
}

impl UploadedFile {
    /// Create from an axum multipart field (consumes the field).
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
        let data = field
            .bytes()
            .await
            .map_err(|e| modo::HttpError::BadRequest.with_message(format!("{e}")))?;
        Ok(Self {
            name,
            file_name,
            content_type,
            data,
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

    /// The raw file bytes.
    pub fn data(&self) -> &bytes::Bytes {
        &self.data
    }

    /// File size in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// File extension from the original filename (lowercase, without dot).
    pub fn extension(&self) -> Option<String> {
        self.file_name.rsplit('.').next().and_then(|ext| {
            if ext == self.file_name {
                None
            } else {
                Some(ext.to_ascii_lowercase())
            }
        })
    }

    /// Whether the file is empty (zero bytes).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Test helper — construct an `UploadedFile` without multipart parsing.
    #[doc(hidden)]
    pub fn __test_new(name: &str, file_name: &str, content_type: &str, data: &[u8]) -> Self {
        Self {
            name: name.to_owned(),
            file_name: file_name.to_owned(),
            content_type: content_type.to_owned(),
            data: bytes::Bytes::copy_from_slice(data),
        }
    }

    /// Start building a validation chain.
    pub fn validate(&self) -> UploadValidator<'_> {
        UploadValidator::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_with_name(file_name: &str) -> UploadedFile {
        UploadedFile::__test_new("f", file_name, "application/octet-stream", b"")
    }

    #[test]
    fn extension_lowercase() {
        assert_eq!(file_with_name("photo.JPG").extension(), Some("jpg".into()));
    }

    #[test]
    fn extension_compound() {
        assert_eq!(
            file_with_name("archive.tar.gz").extension(),
            Some("gz".into())
        );
    }

    #[test]
    fn extension_dotfile() {
        assert_eq!(
            file_with_name(".gitignore").extension(),
            Some("gitignore".into())
        );
    }

    #[test]
    fn extension_none() {
        assert_eq!(file_with_name("noext").extension(), None);
    }
}
