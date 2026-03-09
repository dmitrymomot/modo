use minijinja::{Environment, Error, ErrorKind, State};

/// Register CSRF template functions on the MiniJinja environment.
///
/// Registers:
/// - `csrf_field()` — returns `<input type="hidden" ...>` HTML for forms
/// - `csrf_token()` — returns the raw token string (for meta tags / JS)
///
/// Both read `csrf_token` from the template render context, injected by
/// the CSRF middleware via `TemplateContext`.
pub fn register_template_functions(env: &mut Environment<'static>) {
    env.add_function("csrf_field", |state: &State| -> Result<String, Error> {
        let token = state
            .lookup("csrf_token")
            .map(|v: minijinja::Value| v.to_string())
            .unwrap_or_default();

        if token.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "csrf_token not found in template context — is the CSRF middleware active?",
            ));
        }

        let field_name = state
            .lookup("csrf_field_name")
            .map(|v: minijinja::Value| v.to_string())
            .unwrap_or_else(|| "_csrf_token".to_string());

        let escaped = html_escape(&token);
        Ok(format!(
            r#"<input type="hidden" name="{field_name}" value="{escaped}">"#
        ))
    });

    env.add_function("csrf_token", |state: &State| -> Result<String, Error> {
        let token = state
            .lookup("csrf_token")
            .map(|v: minijinja::Value| v.to_string())
            .unwrap_or_default();

        if token.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "csrf_token not found in template context — is the CSRF middleware active?",
            ));
        }

        Ok(token)
    });
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}
