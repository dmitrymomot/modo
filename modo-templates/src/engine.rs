use minijinja::Environment;

/// Wraps MiniJinja's `Environment` for use as a modo service.
pub struct TemplateEngine {
    env: Environment<'static>,
}

impl TemplateEngine {
    /// Get a reference to the inner MiniJinja Environment for registering
    /// custom functions, filters, or globals.
    pub fn env(&self) -> &Environment<'static> {
        &self.env
    }

    /// Get a mutable reference to the inner MiniJinja Environment.
    pub fn env_mut(&mut self) -> &mut Environment<'static> {
        &mut self.env
    }

    /// Render a template by name with the given context value.
    pub fn render(&self, name: &str, ctx: minijinja::Value) -> Result<String, crate::TemplateError> {
        let tmpl = self.env.get_template(name)?;
        Ok(tmpl.render(ctx)?)
    }
}

/// Create a template engine from config (follows `modo_i18n::load` pattern).
pub fn engine(config: &crate::TemplateConfig) -> Result<TemplateEngine, crate::TemplateError> {
    let mut env = Environment::new();

    if config.strict {
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    }

    env.set_loader(minijinja::path_loader(&config.path));

    Ok(TemplateEngine { env })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    fn setup_templates(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("modo_tmpl_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.html"), "Hello {{ name }}!").unwrap();
        fs::write(
            dir.join("layout.html"),
            "{% block content %}{% endblock %}",
        )
        .unwrap();
        fs::write(
            dir.join("page.html"),
            r#"{% extends "layout.html" %}{% block content %}Page: {{ title }}{% endblock %}"#,
        )
        .unwrap();
        dir
    }

    fn test_config(dir: &std::path::Path) -> crate::TemplateConfig {
        crate::TemplateConfig {
            path: dir.to_str().unwrap().to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn render_simple_template() {
        let dir = setup_templates("simple");
        let engine = crate::engine(&test_config(&dir)).unwrap();

        let result = engine
            .render("hello.html", minijinja::context! { name => "World" }.into())
            .unwrap();
        assert_eq!(result, "Hello World!");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn render_with_inheritance() {
        let dir = setup_templates("inherit");
        let engine = crate::engine(&test_config(&dir)).unwrap();

        let result = engine
            .render("page.html", minijinja::context! { title => "Home" }.into())
            .unwrap();
        assert_eq!(result, "Page: Home");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn strict_mode_rejects_undefined() {
        let dir = setup_templates("strict");
        let engine = crate::engine(&test_config(&dir)).unwrap();

        let result = engine.render(
            "hello.html",
            minijinja::context! {}.into(), // name is missing
        );
        assert!(result.is_err());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn template_not_found_error() {
        let dir = setup_templates("notfound");
        let engine = crate::engine(&test_config(&dir)).unwrap();

        let result = engine.render("nonexistent.html", minijinja::context! {}.into());
        assert!(matches!(
            result,
            Err(crate::TemplateError::NotFound { .. })
        ));

        fs::remove_dir_all(&dir).unwrap();
    }
}
