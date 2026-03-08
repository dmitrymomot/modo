use modo_i18n::t;
use modo_i18n::{I18n, I18nConfig, load};
use std::fs;

fn setup(name: &str) -> (std::sync::Arc<modo_i18n::TranslationStore>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("modo_i18n_integration_{name}"));
    let _ = fs::remove_dir_all(&dir);

    let en = dir.join("en");
    fs::create_dir_all(&en).unwrap();
    fs::write(
        en.join("common.yml"),
        r#"
greeting: "Hello, {name}!"
farewell: "Goodbye, {name}. See you {when}!"
items:
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
items:
  zero: "Sin elementos"
  one: "Un elemento"
  other: "{count} elementos"
"#,
    )
    .unwrap();

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
    let (store, dir) = setup("plain");
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.greeting", name = "World"), "Hello, World!");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_multiple_vars() {
    let (store, dir) = setup("multiple_vars");
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(
        t!(i18n, "common.farewell", name = "Alice", when = "tomorrow"),
        "Goodbye, Alice. See you tomorrow!"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_plural() {
    let (store, dir) = setup("plural");
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.items", count = 0), "No items");
    assert_eq!(t!(i18n, "common.items", count = 1), "One item");
    assert_eq!(t!(i18n, "common.items", count = 42), "42 items");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_fallback_to_default_lang() {
    let (store, dir) = setup("fallback");
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
    let (store, dir) = setup("spanish");
    let i18n = I18n::new(store, "es".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "common.greeting", name = "Mundo"), "Hola, Mundo!");
    assert_eq!(t!(i18n, "common.items", count = 3), "3 elementos");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn t_macro_missing_key() {
    let (store, dir) = setup("missing_key");
    let i18n = I18n::new(store, "en".to_string(), "en".to_string());
    assert_eq!(t!(i18n, "nonexistent.key"), "nonexistent.key");
    fs::remove_dir_all(&dir).unwrap();
}
