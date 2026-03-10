use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CsrfConfig {
    pub cookie_name: String,
    pub field_name: String,
    pub header_name: String,
    pub cookie_max_age: u64,
    pub token_length: usize,
    pub secure: bool,
    pub max_body_bytes: usize,
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            cookie_name: "_csrf".to_string(),
            field_name: "_csrf_token".to_string(),
            header_name: "x-csrf-token".to_string(),
            cookie_max_age: 86400,
            token_length: 32,
            secure: true,
            max_body_bytes: 1_048_576,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = CsrfConfig::default();
        assert_eq!(config.cookie_name, "_csrf");
        assert_eq!(config.field_name, "_csrf_token");
        assert_eq!(config.header_name, "x-csrf-token");
        assert_eq!(config.cookie_max_age, 86400);
        assert_eq!(config.token_length, 32);
        assert!(config.secure);
        assert_eq!(config.max_body_bytes, 1_048_576);
    }

    #[test]
    fn partial_yaml_deserialization() {
        let yaml = r#"
cookie_name: "my_csrf"
secure: false
max_body_bytes: 2097152
"#;
        let config: CsrfConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.cookie_name, "my_csrf");
        assert!(!config.secure);
        assert_eq!(config.max_body_bytes, 2_097_152);
        // Defaults preserved
        assert_eq!(config.field_name, "_csrf_token");
        assert_eq!(config.header_name, "x-csrf-token");
        assert_eq!(config.cookie_max_age, 86400);
        assert_eq!(config.token_length, 32);
    }
}
