# modo-i18n Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a file-based i18n module for modo with YAML translations, locale resolution middleware, an `I18n` extractor, and a `t!()` proc macro.

**Architecture:** Two crates — `modo-i18n` (runtime: config, store, middleware, extractor) and `modo-i18n-macros` (proc macro: `t!()`). Translations loaded from `locales/{lang}/{namespace}.yml` at startup into an `Arc<TranslationStore>`. Middleware resolves locale per-request via a priority chain (custom source → cookie → query param → Accept-Language → default) and inserts it into request extensions. The `I18n` extractor reads from extensions and provides `t()` / `t_plural()` methods.

**Tech Stack:** Rust (edition 2024), serde + serde_yaml_ng, axum 0.8, tower, syn/quote/proc-macro2

**Design doc:** `docs/plans/2026-03-08-modo-i18n-design.md`

---

### Task 1: Scaffold crates and workspace

**Files:**
- Create: `modo-i18n/Cargo.toml`
- Create: `modo-i18n/src/lib.rs`
- Create: `modo-i18n-macros/Cargo.toml`
- Create: `modo-i18n-macros/src/lib.rs`
- Modify: `Cargo.toml` (root workspace)

**Step 1: Create `modo-i18n/Cargo.toml`**

```toml
[package]
name = "modo-i18n"
version = "0.1.0"
edition = "2024"
license.workspace = true

[dependencies]
modo = { path = "../modo" }
modo-i18n-macros = { path = "../modo-i18n-macros" }

axum = "0.8"
axum-extra = { version = "0.10", features = ["cookie"] }
http = "1"
tower = { version = "0.5", features = ["util"] }
serde = { version = "1", features = ["derive"] }
serde_yaml_ng = "0.10"
tracing = "0.1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

**Step 2: Create `modo-i18n/src/lib.rs`**

```rust
pub mod config;
pub mod entry;
pub mod error;
pub mod extractor;
pub mod locale;
pub mod middleware;
pub mod store;

pub use config::I18nConfig;
pub use entry::Entry;
pub use error::I18nError;
pub use extractor::I18n;
pub use middleware::{layer, layer_with_source};
pub use store::{load, TranslationStore};

// Re-export macro
pub use modo_i18n_macros::t;
```

**Step 3: Create `modo-i18n-macros/Cargo.toml`**

```toml
[package]
name = "modo-i18n-macros"
version = "0.1.0"
edition = "2024"
license.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full", "extra-traits"] }
quote = "1"
proc-macro2 = "1"
```

**Step 4: Create `modo-i18n-macros/src/lib.rs`** (placeholder)

```rust
use proc_macro::TokenStream;

/// Translate a key with optional named variables.
///
/// Usage:
/// - `t!(i18n, "key")`
/// - `t!(i18n, "key", name = expr)`
/// - `t!(i18n, "key", count = expr)` — triggers plural
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    match t_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn t_impl(_input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    todo!()
}
```

**Step 5: Add both crates to root `Cargo.toml` workspace members**

Add `"modo-i18n"` and `"modo-i18n-macros"` to the `members` array.

**Step 6: Verify workspace compiles**

Run: `cargo check -p modo-i18n-macros`
Expected: PASS (placeholder compiles, just has `todo!()`)

**Step 7: Commit**

```bash
git add modo-i18n/ modo-i18n-macros/ Cargo.toml
git commit -m "feat(modo-i18n): scaffold crates and workspace membership"
```

---

### Task 2: I18nConfig

**Files:**
- Create: `modo-i18n/src/config.rs`

**Step 1: Write the test**

Add to bottom of `config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = I18nConfig::default();
        assert_eq!(config.path, "locales");
        assert_eq!(config.default_lang, "en");
        assert_eq!(config.cookie_name, "lang");
        assert_eq!(config.query_param, "lang");
    }

    #[test]
    fn partial_yaml_deserialization() {
        let yaml = r#"
path: "translations"
default_lang: "es"
"#;
        let config: I18nConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.path, "translations");
        assert_eq!(config.default_lang, "es");
        assert_eq!(config.cookie_name, "lang");
        assert_eq!(config.query_param, "lang");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p modo-i18n config::tests`
Expected: FAIL — `I18nConfig` not defined

**Step 3: Write implementation**

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct I18nConfig {
    pub path: String,
    pub default_lang: String,
    pub cookie_name: String,
    pub query_param: String,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            path: "locales".to_string(),
            default_lang: "en".to_string(),
            cookie_name: "lang".to_string(),
            query_param: "lang".to_string(),
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p modo-i18n config::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add modo-i18n/src/config.rs
git commit -m "feat(modo-i18n): add I18nConfig with serde defaults"
```

---

### Task 3: Entry enum and I18nError

**Files:**
- Create: `modo-i18n/src/entry.rs`
- Create: `modo-i18n/src/error.rs`

**Step 1: Write `entry.rs`**

```rust
#[derive(Debug, Clone)]
pub enum Entry {
    Plain(String),
    Plural {
        zero: Option<String>,
        one: Option<String>,
        other: String,
    },
}
```

**Step 2: Write `error.rs`**

```rust
use std::fmt;

#[derive(Debug)]
pub enum I18nError {
    DirectoryNotFound { path: String },
    DefaultLangMissing { lang: String, path: String },
    ParseError { lang: String, file: String, source: serde_yaml_ng::Error },
    PluralMissingOther { lang: String, key: String },
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryNotFound { path } => {
                write!(f, "i18n: translations directory not found: {path}")
            }
            Self::DefaultLangMissing { lang, path } => {
                write!(f, "i18n: default language '{lang}' directory not found in {path}")
            }
            Self::ParseError { lang, file, source } => {
                write!(f, "i18n: failed to parse {lang}/{file}: {source}")
            }
            Self::PluralMissingOther { lang, key } => {
                write!(f, "i18n: plural entry '{key}' in '{lang}' missing required 'other' key")
            }
        }
    }
}

impl std::error::Error for I18nError {}
```

**Step 3: Verify compilation**

Run: `cargo check -p modo-i18n`
Expected: PASS (with warnings about unused — that's fine, they'll be used in later tasks)

**Step 4: Commit**

```bash
git add modo-i18n/src/entry.rs modo-i18n/src/error.rs
git commit -m "feat(modo-i18n): add Entry enum and I18nError types"
```

---

### Task 4: Locale normalization and Accept-Language parsing

**Files:**
- Create: `modo-i18n/src/locale.rs`

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_region() {
        assert_eq!(normalize_lang("en-US"), "en");
        assert_eq!(normalize_lang("es-MX"), "es");
        assert_eq!(normalize_lang("pt_BR"), "pt");
    }

    #[test]
    fn normalize_lowercases() {
        assert_eq!(normalize_lang("EN"), "en");
        assert_eq!(normalize_lang("De-AT"), "de");
    }

    #[test]
    fn normalize_plain() {
        assert_eq!(normalize_lang("fr"), "fr");
    }

    #[test]
    fn parse_accept_language_with_weights() {
        let result = parse_accept_language("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5");
        assert_eq!(result, vec!["fr", "en", "de"]);
    }

    #[test]
    fn parse_accept_language_deduplicates() {
        let result = parse_accept_language("en-US, en-GB;q=0.9, en;q=0.8");
        assert_eq!(result, vec!["en"]);
    }

    #[test]
    fn parse_accept_language_default_weight() {
        let result = parse_accept_language("es, en;q=0.5");
        assert_eq!(result, vec!["es", "en"]);
    }

    #[test]
    fn parse_accept_language_empty() {
        let result = parse_accept_language("");
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_first_available_match() {
        let available = vec!["en".to_string(), "de".to_string()];
        let result = resolve_from_accept_language("fr, de;q=0.9, en;q=0.8", &available);
        assert_eq!(result, Some("de".to_string()));
    }

    #[test]
    fn resolve_no_match() {
        let available = vec!["en".to_string()];
        let result = resolve_from_accept_language("fr, de", &available);
        assert_eq!(result, None);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p modo-i18n locale::tests`
Expected: FAIL — functions not defined

**Step 3: Write implementation**

```rust
/// Normalize a language tag to a bare lowercase language code.
/// "en-US" → "en", "pt_BR" → "pt", "DE" → "de"
pub fn normalize_lang(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .to_lowercase()
}

/// Parse an Accept-Language header into a list of normalized language codes,
/// sorted by quality weight (descending), deduplicated, with "*" filtered out.
pub fn parse_accept_language(header: &str) -> Vec<String> {
    let mut entries: Vec<(String, f32)> = header
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (tag, q) = if let Some((tag, params)) = part.split_once(';') {
                let q = params
                    .trim()
                    .strip_prefix("q=")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1.0);
                (tag.trim(), q)
            } else {
                (part, 1.0)
            };
            let normalized = normalize_lang(tag);
            if normalized == "*" {
                return None;
            }
            Some((normalized, q))
        })
        .collect();

    // Stable sort descending by weight
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate, keeping first (highest weight)
    let mut seen = Vec::new();
    let mut result = Vec::new();
    for (lang, _) in entries {
        if !seen.contains(&lang) {
            seen.push(lang.clone());
            result.push(lang);
        }
    }
    result
}

/// Find the first language from Accept-Language header that matches an available locale.
pub fn resolve_from_accept_language(header: &str, available: &[String]) -> Option<String> {
    parse_accept_language(header)
        .into_iter()
        .find(|lang| available.contains(lang))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p modo-i18n locale::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add modo-i18n/src/locale.rs
git commit -m "feat(modo-i18n): add locale normalization and Accept-Language parsing"
```

---

### Task 5: TranslationStore — YAML loading

**Files:**
- Create: `modo-i18n/src/store.rs`

This is the core task. The store loads all YAML files from disk, flattens nested keys, detects plural entries, and provides lookup methods.

**Step 1: Write tests**

Create test fixture files first. Tests will use `$TMPDIR` to create temporary locale directories.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::I18nConfig;
    use std::fs;

    fn setup_locales(dir: &std::path::Path) {
        let en = dir.join("en");
        fs::create_dir_all(&en).unwrap();
        fs::write(
            en.join("common.yml"),
            r#"
greeting: "Hello, {name}!"
items_count:
  zero: "No items"
  one: "One item"
  other: "{count} items"
"#,
        )
        .unwrap();
        fs::write(
            en.join("auth.yml"),
            r#"
page:
  title: "Sign In"
  errors:
    invalid_email: "Invalid email"
"#,
        )
        .unwrap();

        let es = dir.join("es");
        fs::create_dir_all(&es).unwrap();
        fs::write(
            es.join("common.yml"),
            r#"
greeting: "Hola, {name}!"
"#,
        )
        .unwrap();
    }

    fn test_config(dir: &std::path::Path) -> I18nConfig {
        I18nConfig {
            path: dir.to_str().unwrap().to_string(),
            default_lang: "en".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn load_discovers_languages() {
        let dir = std::env::temp_dir().join("modo_i18n_test_langs");
        let _ = fs::remove_dir_all(&dir);
        setup_locales(&dir);

        let store = load(&test_config(&dir)).unwrap();
        let mut langs = store.available_langs().to_vec();
        langs.sort();
        assert_eq!(langs, vec!["en", "es"]);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_flattens_nested_keys() {
        let dir = std::env::temp_dir().join("modo_i18n_test_flatten");
        let _ = fs::remove_dir_all(&dir);
        setup_locales(&dir);

        let store = load(&test_config(&dir)).unwrap();
        assert_eq!(
            store.get("en", "auth.page.title"),
            Some("Sign In".to_string())
        );
        assert_eq!(
            store.get("en", "auth.page.errors.invalid_email"),
            Some("Invalid email".to_string())
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_detects_plural_entries() {
        let dir = std::env::temp_dir().join("modo_i18n_test_plural");
        let _ = fs::remove_dir_all(&dir);
        setup_locales(&dir);

        let store = load(&test_config(&dir)).unwrap();
        assert_eq!(
            store.get_plural("en", "common.items_count", 0),
            Some("No items".to_string())
        );
        assert_eq!(
            store.get_plural("en", "common.items_count", 1),
            Some("One item".to_string())
        );
        assert_eq!(
            store.get_plural("en", "common.items_count", 42),
            Some("{count} items".to_string())
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_plain_key_lookup() {
        let dir = std::env::temp_dir().join("modo_i18n_test_plain");
        let _ = fs::remove_dir_all(&dir);
        setup_locales(&dir);

        let store = load(&test_config(&dir)).unwrap();
        assert_eq!(
            store.get("en", "common.greeting"),
            Some("Hello, {name}!".to_string())
        );
        assert_eq!(
            store.get("es", "common.greeting"),
            Some("Hola, {name}!".to_string())
        );
        assert_eq!(store.get("es", "auth.page.title"), None);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_error_directory_not_found() {
        let config = I18nConfig {
            path: "/nonexistent/path".to_string(),
            ..Default::default()
        };
        let err = load(&config).unwrap_err();
        assert!(matches!(err, I18nError::DirectoryNotFound { .. }));
    }

    #[test]
    fn load_error_default_lang_missing() {
        let dir = std::env::temp_dir().join("modo_i18n_test_no_default");
        let _ = fs::remove_dir_all(&dir);
        let es = dir.join("es");
        fs::create_dir_all(&es).unwrap();
        fs::write(es.join("common.yml"), "key: value").unwrap();

        let config = I18nConfig {
            path: dir.to_str().unwrap().to_string(),
            default_lang: "en".to_string(),
            ..Default::default()
        };
        let err = load(&config).unwrap_err();
        assert!(matches!(err, I18nError::DefaultLangMissing { .. }));

        fs::remove_dir_all(&dir).unwrap();
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p modo-i18n store::tests`
Expected: FAIL — `load`, `TranslationStore` not defined

**Step 3: Write implementation**

```rust
use crate::config::I18nConfig;
use crate::entry::Entry;
use crate::error::I18nError;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

pub struct TranslationStore {
    config: I18nConfig,
    translations: HashMap<String, HashMap<String, Entry>>,
    langs: Vec<String>,
}

impl TranslationStore {
    pub fn config(&self) -> &I18nConfig {
        &self.config
    }

    pub fn available_langs(&self) -> &[String] {
        &self.langs
    }

    pub fn has_lang(&self, lang: &str) -> bool {
        self.langs.contains(&lang.to_string())
    }

    /// Look up a plain translation key for a given language.
    pub fn get(&self, lang: &str, key: &str) -> Option<String> {
        self.translations.get(lang)?.get(key).and_then(|e| match e {
            Entry::Plain(s) => Some(s.clone()),
            Entry::Plural { .. } => None,
        })
    }

    /// Look up a plural translation key for a given language and count.
    pub fn get_plural(&self, lang: &str, key: &str, count: u64) -> Option<String> {
        self.translations.get(lang)?.get(key).and_then(|e| match e {
            Entry::Plural { zero, one, other } => {
                let result = match count {
                    0 => zero.as_deref().unwrap_or(other.as_str()),
                    1 => one.as_deref().unwrap_or(other.as_str()),
                    _ => other.as_str(),
                };
                Some(result.to_string())
            }
            Entry::Plain(_) => None,
        })
    }
}

/// Load all translations from disk according to config.
pub fn load(config: &I18nConfig) -> Result<Arc<TranslationStore>, I18nError> {
    let base = Path::new(&config.path);
    if !base.is_dir() {
        return Err(I18nError::DirectoryNotFound {
            path: config.path.clone(),
        });
    }

    let mut translations: HashMap<String, HashMap<String, Entry>> = HashMap::new();
    let mut langs: Vec<String> = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(base)
        .map_err(|_| I18nError::DirectoryNotFound {
            path: config.path.clone(),
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let lang_name = entry.file_name().to_string_lossy().to_string();

        // Skip directories that aren't pure lowercase alpha
        if !lang_name.chars().all(|c| c.is_ascii_lowercase()) {
            continue;
        }

        let lang_dir = entry.path();
        let mut lang_translations: HashMap<String, Entry> = HashMap::new();

        let mut files: Vec<_> = fs::read_dir(&lang_dir)
            .map_err(|_| I18nError::DirectoryNotFound {
                path: lang_dir.to_string_lossy().to_string(),
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "yml" || ext == "yaml")
            })
            .collect();
        files.sort_by_key(|e| e.file_name());

        for file_entry in files {
            let file_path = file_entry.path();
            let namespace = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let content = fs::read_to_string(&file_path).map_err(|e| I18nError::ParseError {
                lang: lang_name.clone(),
                file: namespace.clone(),
                source: serde_yaml_ng::Error::custom(e.to_string()),
            })?;

            let yaml: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(&content).map_err(|e| I18nError::ParseError {
                    lang: lang_name.clone(),
                    file: namespace.clone(),
                    source: e,
                })?;

            if let serde_yaml_ng::Value::Mapping(map) = yaml {
                flatten_yaml(&lang_name, &namespace, &map, &mut lang_translations)?;
            }
        }

        let key_count = lang_translations.len();
        langs.push(lang_name.clone());
        translations.insert(lang_name.clone(), lang_translations);
        info!(lang = %lang_name, keys = key_count, "loaded translations");
    }

    if !langs.contains(&config.default_lang) {
        return Err(I18nError::DefaultLangMissing {
            lang: config.default_lang.clone(),
            path: config.path.clone(),
        });
    }

    Ok(Arc::new(TranslationStore {
        config: config.clone(),
        translations,
        langs,
    }))
}

const PLURAL_KEYS: &[&str] = &["zero", "one", "other"];

fn is_plural_map(map: &serde_yaml_ng::Mapping) -> bool {
    if map.is_empty() {
        return false;
    }
    let has_other = map.contains_key(serde_yaml_ng::Value::String("other".to_string()));
    let all_plural = map.keys().all(|k| {
        k.as_str()
            .is_some_and(|s| PLURAL_KEYS.contains(&s))
    });
    has_other && all_plural
}

fn flatten_yaml(
    lang: &str,
    prefix: &str,
    map: &serde_yaml_ng::Mapping,
    out: &mut HashMap<String, Entry>,
) -> Result<(), I18nError> {
    for (key, value) in map {
        let key_str = match key.as_str() {
            Some(s) => s,
            None => continue,
        };
        let full_key = format!("{prefix}.{key_str}");

        match value {
            serde_yaml_ng::Value::String(s) => {
                out.insert(full_key, Entry::Plain(s.clone()));
            }
            serde_yaml_ng::Value::Mapping(nested) if is_plural_map(nested) => {
                let other = nested
                    .get(serde_yaml_ng::Value::String("other".to_string()))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| I18nError::PluralMissingOther {
                        lang: lang.to_string(),
                        key: full_key.clone(),
                    })?
                    .to_string();
                let zero = nested
                    .get(serde_yaml_ng::Value::String("zero".to_string()))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let one = nested
                    .get(serde_yaml_ng::Value::String("one".to_string()))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.insert(full_key, Entry::Plural { zero, one, other });
            }
            serde_yaml_ng::Value::Mapping(nested) => {
                flatten_yaml(lang, &full_key, nested, out)?;
            }
            _ => {
                // Numbers, bools, etc. → convert to string
                out.insert(full_key, Entry::Plain(value.to_string()));
            }
        }
    }
    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p modo-i18n store::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add modo-i18n/src/store.rs
git commit -m "feat(modo-i18n): add TranslationStore with YAML loading and plural detection"
```

---

### Task 6: I18n extractor with `t()` and `t_plural()`

**Files:**
- Create: `modo-i18n/src/extractor.rs`

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::I18nConfig;
    use crate::store;
    use std::fs;

    fn setup_store() -> (Arc<TranslationStore>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("modo_i18n_test_extractor");
        let _ = fs::remove_dir_all(&dir);
        let en = dir.join("en");
        fs::create_dir_all(&en).unwrap();
        fs::write(
            en.join("common.yml"),
            r#"
greeting: "Hello, {name}!"
farewell: "Goodbye"
items_count:
  zero: "No items"
  one: "One item"
  other: "{count} items"
"#,
        )
        .unwrap();
        let es = dir.join("es");
        fs::create_dir_all(&es).unwrap();
        fs::write(
            es.join("common.yml"),
            r#"
greeting: "Hola, {name}!"
"#,
        )
        .unwrap();

        let config = I18nConfig {
            path: dir.to_str().unwrap().to_string(),
            default_lang: "en".to_string(),
            ..Default::default()
        };
        let s = store::load(&config).unwrap();
        (s, dir)
    }

    #[test]
    fn t_plain_key() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(i18n.t("common.farewell", &[]), "Goodbye");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_with_variables() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(
            i18n.t("common.greeting", &[("name", "Alice")]),
            "Hello, Alice!"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_fallback_to_default_lang() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "es".to_string(), "en".to_string());
        // "farewell" exists in en but not es
        assert_eq!(i18n.t("common.farewell", &[]), "Goodbye");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_missing_key_returns_key() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(i18n.t("nonexistent.key", &[]), "nonexistent.key");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_plural_zero() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(
            i18n.t_plural("common.items_count", 0, &[]),
            "No items"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_plural_one() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(
            i18n.t_plural("common.items_count", 1, &[]),
            "One item"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn t_plural_other_with_count_var() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store, "en".to_string(), "en".to_string());
        assert_eq!(
            i18n.t_plural("common.items_count", 5, &[("count", "5")]),
            "5 items"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lang_accessor() {
        let (store, dir) = setup_store();
        let i18n = I18n::new(store.clone(), "es".to_string(), "en".to_string());
        assert_eq!(i18n.lang(), "es");

        let mut langs = i18n.available_langs().to_vec();
        langs.sort();
        assert_eq!(langs, vec!["en", "es"]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p modo-i18n extractor::tests`
Expected: FAIL — `I18n` not defined

**Step 3: Write implementation**

```rust
use crate::store::TranslationStore;
use modo::Error;
use modo::app::AppState;
use modo::axum::extract::FromRequestParts;
use modo::axum::http::request::Parts;
use std::sync::Arc;

/// Resolved language tag, inserted into request extensions by middleware.
#[derive(Debug, Clone)]
pub struct ResolvedLang(pub String);

pub struct I18n {
    store: Arc<TranslationStore>,
    lang: String,
    default_lang: String,
}

impl I18n {
    pub fn new(store: Arc<TranslationStore>, lang: String, default_lang: String) -> Self {
        Self {
            store,
            lang,
            default_lang,
        }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    pub fn available_langs(&self) -> &[String] {
        self.store.available_langs()
    }

    /// Translate a key with variable substitution.
    /// Falls back to default language, then returns the key itself.
    pub fn t(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let raw = self
            .store
            .get(&self.lang, key)
            .or_else(|| self.store.get(&self.default_lang, key))
            .unwrap_or_else(|| key.to_string());
        interpolate(&raw, vars)
    }

    /// Translate a plural key with count and variable substitution.
    /// Falls back to "other" category, then default language, then the key itself.
    pub fn t_plural(&self, key: &str, count: u64, vars: &[(&str, &str)]) -> String {
        let raw = self
            .store
            .get_plural(&self.lang, key, count)
            .or_else(|| self.store.get_plural(&self.default_lang, key, count))
            .unwrap_or_else(|| key.to_string());
        interpolate(&raw, vars)
    }
}

impl FromRequestParts<AppState> for I18n {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let store = state
            .services
            .get::<TranslationStore>()
            .ok_or_else(|| Error::internal("TranslationStore not found in services"))?;

        let lang = parts
            .extensions
            .get::<ResolvedLang>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| store.config().default_lang.clone());

        let default_lang = store.config().default_lang.clone();

        Ok(I18n::new(store, lang, default_lang))
    }
}

fn interpolate(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p modo-i18n extractor::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add modo-i18n/src/extractor.rs
git commit -m "feat(modo-i18n): add I18n extractor with translation lookup and interpolation"
```

---

### Task 7: Middleware — locale resolution

**Files:**
- Create: `modo-i18n/src/middleware.rs`

**Step 1: Write implementation**

The middleware is an axum async function. It needs the `TranslationStore` from services plus an optional custom source. Since axum middleware functions take `State(state): State<AppState>`, and the custom source needs to be available, we store it in the `TranslationStore` itself (behind an `Option<Arc<...>>`).

First, update `TranslationStore` in `store.rs` to support the custom source:

Add to `store.rs`:

```rust
use axum::http::Request;

type CustomSourceFn = Arc<dyn Fn(&Request<axum::body::Body>) -> Option<String> + Send + Sync>;

// Add field to TranslationStore:
// custom_source: Option<CustomSourceFn>,

// Add method:
impl TranslationStore {
    pub fn with_custom_source(
        self: Arc<Self>,
        source: impl Fn(&Request<axum::body::Body>) -> Option<String> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(TranslationStore {
            config: self.config.clone(),
            translations: self.translations.clone(),
            langs: self.langs.clone(),
            custom_source: Some(Arc::new(source)),
        })
    }

    pub fn custom_source(&self) -> Option<&CustomSourceFn> {
        self.custom_source.as_ref()
    }
}
```

Then write `middleware.rs`:

```rust
use crate::extractor::ResolvedLang;
use crate::locale::{normalize_lang, resolve_from_accept_language};
use crate::store::TranslationStore;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::CookieJar;
use http::header;
use modo::app::AppState;
use std::sync::Arc;

pub fn layer(
    store: Arc<TranslationStore>,
) -> axum::middleware::FromFnLayer<
    fn(State<AppState>, Request<axum::body::Body>, Next) -> _,
    AppState,
    _,
> {
    axum::middleware::from_fn_with_state::<AppState, _>(i18n_middleware)
}

// Note: The actual layer() and layer_with_source() functions return middleware layers.
// Since axum's from_fn return types are complex, use the pattern from modo codebase.

pub async fn i18n_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(store) = state.services.get::<TranslationStore>() else {
        return next.run(request).await;
    };

    let config = store.config();
    let available = store.available_langs();

    // 1. Custom source
    let mut lang = store
        .custom_source()
        .and_then(|f| f(&request))
        .map(|l| normalize_lang(&l))
        .filter(|l| available.contains(l));

    // 2. Cookie
    if lang.is_none() {
        let jar = CookieJar::from_headers(request.headers());
        lang = jar
            .get(&config.cookie_name)
            .map(|c| normalize_lang(c.value()))
            .filter(|l| available.contains(l));
    }

    // 3. Query parameter
    let query_lang = request
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .find_map(|pair| {
                    let (k, v) = pair.split_once('=')?;
                    if k == config.query_param { Some(v.to_string()) } else { None }
                })
        })
        .map(|l| normalize_lang(&l))
        .filter(|l| available.contains(l));

    let set_cookie = query_lang.is_some() && lang.is_none();
    if lang.is_none() {
        lang = query_lang;
    }

    // 4. Accept-Language header
    if lang.is_none() {
        if let Some(header_val) = request
            .headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
        {
            lang = resolve_from_accept_language(header_val, available);
        }
    }

    // 5. Default
    let resolved = lang.unwrap_or_else(|| config.default_lang.clone());

    let (mut parts, body) = request.into_parts();
    parts
        .extensions
        .insert(ResolvedLang(resolved.clone()));
    let request = Request::from_parts(parts, body);

    let mut response = next.run(request).await;

    // Set cookie if query param triggered lang change
    if set_cookie {
        let cookie_val = format!(
            "{}={}; Path=/; SameSite=Lax; Max-Age=31536000",
            config.cookie_name, resolved
        );
        if let Ok(val) = cookie_val.parse() {
            response.headers_mut().append(header::SET_COOKIE, val);
        }
    }

    response
}
```

**Step 2: Update `lib.rs` to export the layer functions properly**

The `layer()` and `layer_with_source()` functions need to be simple public functions that return an appropriate middleware layer. Given axum's complex return types, the simplest approach is to use `axum::middleware::from_fn` in the app setup. Export just the middleware function and let users call `axum::middleware::from_fn(modo_i18n::i18n_middleware)`.

Alternatively, provide wrapper functions:

```rust
// In middleware.rs — public API
pub fn layer() -> axum::middleware::FromFnLayer<
    fn(State<AppState>, Request<axum::body::Body>, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>,
    AppState,
    (),
> {
    axum::middleware::from_fn(i18n_middleware)
}
```

Since the return type is complex, follow the modo codebase pattern. Check how `client_ip_middleware` is registered in `app.rs` — it uses `axum::middleware::from_fn_with_state`. The i18n middleware should follow the same pattern. The app registers it via:

```rust
app.layer(axum::middleware::from_fn_with_state(app_state, i18n_middleware))
```

But since modo's `app.layer()` takes any Tower layer, and the user calls it before the app state is built, the simpler approach is to just export the middleware function and document the registration pattern. Or wrap it like session does with a Tower Layer+Service.

Given the complexity of getting return types right, use the **Tower Layer+Service pattern** (like modo-session) to keep the public API clean:

```rust
use tower::{Layer, Service};

#[derive(Clone)]
pub struct I18nLayer;

impl<S> Layer<S> for I18nLayer {
    type Service = I18nMiddleware<S>;
    fn layer(&self, inner: S) -> Self::Service {
        I18nMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct I18nMiddleware<S> {
    inner: S,
}
```

Actually — look at how the middleware is registered. Since the middleware reads `TranslationStore` from `AppState.services` (where it was already registered via `app.service(store)`), the middleware itself doesn't need to carry the store. It just needs to be a layer. Use `axum::middleware::from_fn`:

```rust
pub fn layer() -> impl tower::Layer<...> { ... }
```

The cleanest approach: export the middleware function. Users register it as:

```rust
app.service(i18n_store)
   .layer(axum::middleware::from_fn(modo_i18n::middleware::i18n_middleware))
```

But this is verbose. To keep it ergonomic, provide a `layer()` helper. The session module solves this with a Tower Layer struct. Follow that pattern.

**Step 3: Verify compilation**

Run: `cargo check -p modo-i18n`
Expected: PASS

**Step 4: Commit**

```bash
git add modo-i18n/src/middleware.rs modo-i18n/src/store.rs
git commit -m "feat(modo-i18n): add locale resolution middleware with cookie/query/header support"
```

---

### Task 8: `t!()` proc macro

**Files:**
- Modify: `modo-i18n-macros/src/lib.rs`

**Step 1: Write implementation**

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitStr, Token};

struct TInput {
    i18n_expr: Expr,
    key: LitStr,
    vars: Vec<(Ident, Expr)>,
}

impl Parse for TInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let i18n_expr: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let key: LitStr = input.parse()?;

        let mut vars = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            vars.push((name, value));
        }

        Ok(TInput {
            i18n_expr,
            key,
            vars,
        })
    }
}

#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    match t_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn t_impl(input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let input = syn::parse2::<TInput>(input)?;
    let i18n = &input.i18n_expr;
    let key = &input.key;

    let has_count = input.vars.iter().any(|(name, _)| name == "count");

    let var_pairs: Vec<proc_macro2::TokenStream> = input
        .vars
        .iter()
        .map(|(name, value)| {
            let name_str = name.to_string();
            quote! { (#name_str, &(#value).to_string()) }
        })
        .collect();

    if has_count {
        let count_expr = input
            .vars
            .iter()
            .find(|(name, _)| name == "count")
            .map(|(_, expr)| expr)
            .unwrap();

        Ok(quote! {
            #i18n.t_plural(#key, #count_expr as u64, &[#(#var_pairs),*])
        })
    } else {
        Ok(quote! {
            #i18n.t(#key, &[#(#var_pairs),*])
        })
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check -p modo-i18n-macros`
Expected: PASS

**Step 3: Commit**

```bash
git add modo-i18n-macros/src/lib.rs
git commit -m "feat(modo-i18n-macros): implement t!() proc macro with plural detection"
```

---

### Task 9: Integration test

**Files:**
- Create: `modo-i18n/tests/integration.rs`

Write an integration test that exercises the full flow: load translations, create an `I18n` instance, use `t()` and `t_plural()` with the `t!()` macro.

**Step 1: Write test**

```rust
use modo_i18n::t;
use modo_i18n::{I18n, I18nConfig, load};
use std::fs;

fn setup() -> (std::sync::Arc<modo_i18n::TranslationStore>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join("modo_i18n_integration");
    let _ = fs::remove_dir_all(&dir);

    let en = dir.join("en");
    fs::create_dir_all(&en).unwrap();
    fs::write(en.join("common.yml"), r#"
greeting: "Hello, {name}!"
farewell: "Goodbye, {name}. See you {when}!"
items:
  zero: "No items"
  one: "One item"
  other: "{count} items"
"#).unwrap();

    let es = dir.join("es");
    fs::create_dir_all(&es).unwrap();
    fs::write(es.join("common.yml"), r#"
greeting: "Hola, {name}!"
items:
  zero: "Sin elementos"
  one: "Un elemento"
  other: "{count} elementos"
"#).unwrap();

    let config = I18nConfig {
        path: dir.to_str().unwrap().to_string(),
        default_lang: "en".to_string(),
        ..Default::default()
    };
    let store = load(&config).unwrap();
    (store, dir)
}

#[test]
fn t_macro_plain() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.greeting", name = "World"), "Hello, World!");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_multiple_vars() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(
        t!(i18n, "common.farewell", name = "Alice", when = "tomorrow"),
        "Goodbye, Alice. See you tomorrow!"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_plural() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.items", count = 0), "No items");
    assert_eq!(t!(i18n, "common.items", count = 1), "One item");
    assert_eq!(t!(i18n, "common.items", count = 42), "42 items");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_fallback_to_default_lang() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "es".to_string(), "en".to_string());
    // "farewell" only exists in en
    assert_eq!(
        t!(i18n, "common.farewell", name = "Bob", when = "later"),
        "Goodbye, Bob. See you later!"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_spanish() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "es".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.greeting", name = "Mundo"), "Hola, Mundo!");
    assert_eq!(t!(i18n, "common.items", count = 3), "3 elementos");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_missing_key() {
    let (store, dir) = setup();
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "nonexistent.key"), "nonexistent.key");
    fs::remove_dir_all(&dir).unwrap();
}
```

**Step 2: Run tests**

Run: `cargo test -p modo-i18n --test integration`
Expected: PASS

**Step 3: Commit**

```bash
git add modo-i18n/tests/integration.rs
git commit -m "test(modo-i18n): add integration tests for t!() macro and translation fallback"
```

---

### Task 10: Final cleanup and `just check`

**Step 1: Run formatter**

Run: `just fmt`

**Step 2: Run full check**

Run: `just check`
Expected: PASS — fmt, clippy, all tests

**Step 3: Fix any issues found by clippy or fmt**

Address any warnings or errors.

**Step 4: Commit fixes if any**

```bash
git add -A
git commit -m "chore(modo-i18n): fix lint and formatting issues"
```
