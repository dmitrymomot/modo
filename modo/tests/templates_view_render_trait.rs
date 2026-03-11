#![cfg(feature = "templates")]

use modo::templates::{TemplateConfig, TemplateContext, TemplateEngine, ViewRender, engine};
use std::io::Write;
use tempfile::TempDir;

fn setup_engine(templates: &[(&str, &str)]) -> (TempDir, TemplateEngine) {
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

// A manual ViewRender implementation for testing
// (macro-generated impls tested separately)
struct TestView {
    name: String,
}

impl ViewRender for TestView {
    fn render_with(
        &self,
        engine: &TemplateEngine,
        context: &TemplateContext,
        _is_htmx: bool,
    ) -> Result<String, modo::templates::TemplateError> {
        let user_ctx = minijinja::Value::from_serialize(serde_json::json!({
            "name": self.name,
        }));
        let merged = context.merge_with(user_ctx);
        engine.render("test.html", merged)
    }

    fn has_dual_template(&self) -> bool {
        false
    }
}

#[test]
fn single_view_renders() {
    let (_dir, eng) = setup_engine(&[("test.html", "Hello {{ name }}!")]);
    let ctx = TemplateContext::new();
    let view = TestView {
        name: "World".into(),
    };

    let html = view.render_with(&eng, &ctx, false).unwrap();
    assert_eq!(html, "Hello World!");
}

#[test]
fn tuple_renders_concatenated() {
    let (_dir, eng) = setup_engine(&[("test.html", "Hello {{ name }}!")]);
    let ctx = TemplateContext::new();

    let views = (
        TestView {
            name: "Alice".into(),
        },
        TestView { name: "Bob".into() },
    );
    let html = views.render_with(&eng, &ctx, false).unwrap();
    assert_eq!(html, "Hello Alice!Hello Bob!");
}

#[test]
fn single_view_merges_request_context() {
    let (_dir, eng) = setup_engine(&[("test.html", "{{ name }} at {{ current_url|safe }}")]);
    let mut ctx = TemplateContext::new();
    ctx.insert("current_url", minijinja::Value::from("/home"));
    let view = TestView {
        name: "World".into(),
    };

    let html = view.render_with(&eng, &ctx, false).unwrap();
    assert_eq!(html, "World at /home");
}
