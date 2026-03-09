use modo_upload::{StorageBackend, UploadConfig};

#[test]
fn test_default_config() {
    let config = UploadConfig::default();
    assert_eq!(config.path, "./uploads");
    assert_eq!(config.backend, StorageBackend::Local);
}

#[tokio::test]
async fn test_local_storage_from_default_config() {
    let config = UploadConfig::default();
    let storage = modo_upload::storage(&config);
    assert!(storage.is_ok(), "storage() should succeed with default config");
}

#[test]
fn test_config_deserialize_defaults() {
    let json = "{}";
    let config: UploadConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.path, "./uploads");
    assert_eq!(config.backend, StorageBackend::Local);
}

#[test]
fn test_config_deserialize_custom_path() {
    let json = r#"{"path": "/data/files"}"#;
    let config: UploadConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.path, "/data/files");
    assert_eq!(config.backend, StorageBackend::Local);
}
