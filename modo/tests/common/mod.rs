use modo::templates::{TemplateConfig, TemplateEngine, engine};
use std::io::Write;
use tempfile::TempDir;

/// Create a temporary template directory and engine for testing.
///
/// Each `(name, content)` pair creates a template file under the temp dir.
/// Returns the `TempDir` guard (must be kept alive) and the engine.
pub fn setup_engine(templates: &[(&str, &str)]) -> (TempDir, TemplateEngine) {
    let dir = TempDir::new().unwrap();
    for (name, content) in templates {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
    let config = TemplateConfig {
        path: dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let eng = engine(&config).unwrap();
    (dir, eng)
}
