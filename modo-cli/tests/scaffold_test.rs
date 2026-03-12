use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn modo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_modo"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modo-test-{}-{}", name, std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    dir
}

#[test]
fn scaffold_minimal() {
    let dir = temp_dir("minimal");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "minimal"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(dir.join("Cargo.toml").exists());
    assert!(dir.join("src/main.rs").exists());
    assert!(dir.join("src/config.rs").exists());
    assert!(dir.join("config/development.yaml").exists());
    assert!(dir.join("config/production.yaml").exists());
    assert!(dir.join(".env").exists());
    assert!(dir.join(".env.example").exists());
    assert!(dir.join(".gitignore").exists());
    assert!(dir.join("CLAUDE.md").exists());
    assert!(dir.join("justfile").exists());
    // No database files
    assert!(!dir.join("docker-compose.yaml").exists());
    assert!(!dir.join("src/handlers").exists());

    // Verify Cargo.toml content
    let cargo = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("{{"));
    assert!(!cargo.contains("modo-db"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn scaffold_api_sqlite() {
    let dir = temp_dir("api-sqlite");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "api"])
        .output()
        .unwrap();
    assert!(output.status.success());

    assert!(dir.join("src/handlers/mod.rs").exists());
    assert!(dir.join("src/models/mod.rs").exists());
    assert!(!dir.join("docker-compose.yaml").exists());

    let cargo = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("sqlite"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn scaffold_api_postgres() {
    let dir = temp_dir("api-pg");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "api", "--postgres"])
        .output()
        .unwrap();
    assert!(output.status.success());

    assert!(dir.join("docker-compose.yaml").exists());

    let cargo = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("postgres"));

    let dc = fs::read_to_string(dir.join("docker-compose.yaml")).unwrap();
    assert!(dc.contains("postgres:18-alpine"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn scaffold_web() {
    let dir = temp_dir("web");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "web"])
        .output()
        .unwrap();
    assert!(output.status.success());

    // All directories present
    assert!(dir.join("src/handlers/mod.rs").exists());
    assert!(dir.join("src/models/mod.rs").exists());
    assert!(dir.join("src/tasks/mod.rs").exists());
    assert!(dir.join("src/views/mod.rs").exists());
    assert!(dir.join("assets/src/app.css").exists());
    assert!(dir.join("templates/app/base.html").exists());
    assert!(dir.join("templates/app/index.html").exists());

    // Cargo.toml has all features
    let cargo = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("modo-auth"));
    assert!(cargo.contains("modo-session"));
    assert!(cargo.contains("modo-jobs"));
    assert!(cargo.contains("modo-email"));
    assert!(cargo.contains("modo-upload"));
    assert!(cargo.contains("modo-tenant"));

    // Config has email and i18n overrides
    let dev_cfg = fs::read_to_string(dir.join("config/development.yaml")).unwrap();
    assert!(dev_cfg.contains("templates_path: templates/emails"));
    assert!(dev_cfg.contains("path: locales"));
    assert!(dev_cfg.contains("backend: local"));

    let prod_cfg = fs::read_to_string(dir.join("config/production.yaml")).unwrap();
    assert!(prod_cfg.contains("backend: s3"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn scaffold_worker() {
    let dir = temp_dir("worker");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "worker"])
        .output()
        .unwrap();
    assert!(output.status.success());

    assert!(dir.join("src/tasks/mod.rs").exists());
    assert!(!dir.join("src/handlers").exists());
    assert!(!dir.join("src/views").exists());

    let main_rs = fs::read_to_string(dir.join("src/main.rs")).unwrap();
    assert!(main_rs.contains("modo_jobs::start"));
    assert!(main_rs.contains("/health"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn error_existing_directory() {
    let dir = temp_dir("exists");
    fs::create_dir_all(&dir).unwrap();

    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "minimal"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn error_db_flag_with_minimal() {
    let dir = temp_dir("minimal-pg");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "minimal", "--postgres"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not use a database"));

    // Directory should NOT have been created
    assert!(!dir.exists());
}

#[test]
fn error_conflicting_db_flags() {
    let dir = temp_dir("conflict");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "api", "--postgres", "--sqlite"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn no_unrendered_placeholders() {
    let dir = temp_dir("placeholders");
    let output = modo_bin()
        .args(["new", dir.to_str().unwrap(), "-t", "web"])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Walk all files and check for unrendered {{ }} (but skip raw Jinja in HTML templates)
    fn check_dir(dir: &std::path::Path) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().unwrap() != ".git" {
                    check_dir(&path);
                }
                continue;
            }
            // Skip HTML files (they contain MiniJinja syntax for the app)
            if path.extension().map_or(false, |e| e == "html") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            // Check for any unrendered MiniJinja placeholder
            assert!(
                !content.contains("{{ ") && !content.contains("{{project") && !content.contains("{{db"),
                "unrendered placeholder in {}",
                path.display()
            );
        }
    }
    check_dir(&dir);

    fs::remove_dir_all(&dir).unwrap();
}
