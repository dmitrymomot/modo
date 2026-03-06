use crate::file::UploadedFile;

/// Fluent validator for uploaded files.
pub struct UploadValidator<'a> {
    file: &'a UploadedFile,
    errors: Vec<String>,
}

impl<'a> UploadValidator<'a> {
    pub(crate) fn new(file: &'a UploadedFile) -> Self {
        Self {
            file,
            errors: Vec::new(),
        }
    }

    /// Reject if the file exceeds `max` bytes.
    pub fn max_size(mut self, max: usize) -> Self {
        if self.file.size() > max {
            self.errors
                .push(format!("File exceeds maximum size of {}", format_size(max)));
        }
        self
    }

    /// Reject if the content type doesn't match `pattern`.
    /// Supports exact types (`image/png`) and wildcard subtypes (`image/*`).
    pub fn accept(mut self, pattern: &str) -> Self {
        if !mime_matches(self.file.content_type(), pattern) {
            self.errors.push(format!("File type must match {pattern}"));
        }
        self
    }

    /// Finish validation. Returns `Ok(())` or a validation error.
    pub fn check(self) -> Result<(), modo::Error> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(modo::validate::validation_error(vec![(
                self.file.name(),
                self.errors,
            )]))
        }
    }
}

/// Check if a content type matches a pattern (e.g. `image/*` matches `image/png`).
pub fn mime_matches(content_type: &str, pattern: &str) -> bool {
    if pattern == "*/*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        content_type.starts_with(prefix)
            && content_type
                .as_bytes()
                .get(prefix.len())
                .is_some_and(|&b| b == b'/')
    } else {
        content_type == pattern
    }
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{}GB", bytes / (1024 * 1024 * 1024))
    } else if bytes >= 1024 * 1024 {
        format!("{}MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{bytes}B")
    }
}

/// Convert megabytes to bytes.
pub fn mb(n: usize) -> usize {
    n * 1024 * 1024
}

/// Convert kilobytes to bytes.
pub fn kb(n: usize) -> usize {
    n * 1024
}

/// Convert gigabytes to bytes.
pub fn gb(n: usize) -> usize {
    n * 1024 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_exact_match() {
        assert!(mime_matches("image/png", "image/png"));
        assert!(!mime_matches("image/jpeg", "image/png"));
    }

    #[test]
    fn mime_wildcard_match() {
        assert!(mime_matches("image/png", "image/*"));
        assert!(mime_matches("image/jpeg", "image/*"));
        assert!(!mime_matches("text/plain", "image/*"));
    }

    #[test]
    fn mime_any_match() {
        assert!(mime_matches("anything/here", "*/*"));
    }

    #[test]
    fn size_helpers() {
        assert_eq!(kb(1), 1024);
        assert_eq!(mb(1), 1024 * 1024);
        assert_eq!(gb(1), 1024 * 1024 * 1024);
        assert_eq!(mb(5), 5 * 1024 * 1024);
    }

    #[test]
    fn format_size_display() {
        assert_eq!(format_size(500), "500B");
        assert_eq!(format_size(1024), "1KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5MB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2GB");
    }
}
