# Core Magic Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-wire core features (templates, i18n, CSRF, cookies) via feature flags + config, eliminating manual setup boilerplate.

**Architecture:** Feature flag on = feature active. `AppBuilder::run()` reads `AppConfig` sections and wires each enabled feature internally. Template functions/filters are auto-discovered via `inventory` macros. `CookieManager` extractor provides high-level cookie API used by handlers and CSRF.

**Tech Stack:** Rust, axum 0.8, axum-extra (cookie jars), MiniJinja, inventory 0.3, serde, proc-macro2/syn/quote

---

## File Structure

### New files
- `modo/src/cookies/mod.rs` — module declarations and re-exports
- `modo/src/cookies/config.rs` — `CookieConfig`, `CookieOptions`, `SameSite`
- `modo/src/cookies/manager.rs` — `CookieManager` extractor implementation

### Modified files
- `modo/Cargo.toml` — no new deps needed (axum-extra cookie features already enabled)
- `modo/src/config.rs` — add `AppConfig` struct
- `modo/src/app.rs` — rename `server_config()` → `config()`, add `templates()`, auto-wiring in `run()`
- `modo/src/templates/mod.rs` — add `TemplateFunctionEntry`, `TemplateFilterEntry`, re-exports
- `modo/src/templates/engine.rs` — add inventory collection registration helper
- `modo/src/csrf/middleware.rs` — refactor to use `CookieManager` internally
- `modo/src/lib.rs` — add cookies module, re-export `AppConfig`, `CookieManager`
- `modo-macros/src/lib.rs` — add `template_function`, `template_filter` proc macros
- `modo-macros/src/template_function.rs` — new macro implementation
- `modo-macros/src/template_filter.rs` — new macro implementation
- `examples/hello/src/main.rs` — `ServerConfig` → `AppConfig`, `server_config()` → `config()`
- `examples/templates/src/main.rs` — full simplification with macro-based template functions
- `examples/jobs/src/main.rs` — flatten `AppConfig`, `server_config()` → `config()`
- `examples/todo-api/src/main.rs` — same
- `examples/upload/src/main.rs` — same

---

## Chunk 1: Template Inventory Types + Macros

### Task 1: Add TemplateFunctionEntry and TemplateFilterEntry

**Files:**
- Modify: `modo/src/templates/mod.rs:1-31`
- Test: existing tests should still pass

- [ ] **Step 1: Add inventory types to templates/mod.rs**

Add after the existing re-exports in `modo/src/templates/mod.rs`:

```rust
/// Registration entry for auto-discovered template functions.
pub struct TemplateFunctionEntry {
    pub name: &'static str,
    pub register_fn: fn(&mut minijinja::Environment<'static>),
}
inventory::collect!(TemplateFunctionEntry);

/// Registration entry for auto-discovered template filters.
pub struct TemplateFilterEntry {
    pub name: &'static str,
    pub register_fn: fn(&mut minijinja::Environment<'static>),
}
inventory::collect!(TemplateFilterEntry);
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p modo --features templates`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add modo/src/templates/mod.rs
git commit -m "feat(templates): add TemplateFunctionEntry and TemplateFilterEntry inventory types"
```

---

### Task 2: Add #[template_function] proc macro

**Files:**
- Create: `modo-macros/src/template_function.rs`
- Modify: `modo-macros/src/lib.rs`

- [ ] **Step 1: Create template_function.rs**

Create `modo-macros/src/template_function.rs`:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, Result, parse2};

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let func: ItemFn = parse2(item)?;
    let func_name = &func.sig.ident;

    // Parse optional name = "custom_name" attribute
    let template_name = if attr.is_empty() {
        func_name.to_string()
    } else {
        let name_value: syn::MetaNameValue = parse2(attr)?;
        if name_value.path.is_ident("name") {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &name_value.value
            {
                s.value()
            } else {
                return Err(syn::Error::new_spanned(
                    &name_value.value,
                    "expected string literal for `name`",
                ));
            }
        } else {
            return Err(syn::Error::new_spanned(
                &name_value.path,
                "unknown attribute, expected `name`",
            ));
        }
    };

    Ok(quote! {
        #func

        #[cfg(feature = "templates")]
        ::modo::inventory::submit! {
            ::modo::templates::TemplateFunctionEntry {
                name: #template_name,
                register_fn: |env: &mut ::modo::minijinja::Environment<'static>| {
                    env.add_function(#template_name, #func_name);
                },
            }
        }
    })
}
```

- [ ] **Step 2: Register the macro in lib.rs**

Add to `modo-macros/src/lib.rs`:

```rust
mod template_function;

/// Auto-register a function as a MiniJinja template function.
#[proc_macro_attribute]
pub fn template_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    template_function::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

And add the re-export to `modo/src/lib.rs`:

```rust
#[cfg(feature = "templates")]
pub use modo_macros::template_function;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo-macros && cargo check -p modo --features templates`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add modo-macros/src/template_function.rs modo-macros/src/lib.rs modo/src/lib.rs
git commit -m "feat(macros): add #[template_function] proc macro"
```

---

### Task 3: Add #[template_filter] proc macro

**Files:**
- Create: `modo-macros/src/template_filter.rs`
- Modify: `modo-macros/src/lib.rs`

- [ ] **Step 1: Create template_filter.rs**

Create `modo-macros/src/template_filter.rs`:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, Result, parse2};

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let func: ItemFn = parse2(item)?;
    let func_name = &func.sig.ident;

    // Parse optional name = "custom_name" attribute
    let filter_name = if attr.is_empty() {
        func_name.to_string()
    } else {
        let name_value: syn::MetaNameValue = parse2(attr)?;
        if name_value.path.is_ident("name") {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &name_value.value
            {
                s.value()
            } else {
                return Err(syn::Error::new_spanned(
                    &name_value.value,
                    "expected string literal for `name`",
                ));
            }
        } else {
            return Err(syn::Error::new_spanned(
                &name_value.path,
                "unknown attribute, expected `name`",
            ));
        }
    };

    Ok(quote! {
        #func

        #[cfg(feature = "templates")]
        ::modo::inventory::submit! {
            ::modo::templates::TemplateFilterEntry {
                name: #filter_name,
                register_fn: |env: &mut ::modo::minijinja::Environment<'static>| {
                    env.add_filter(#filter_name, #func_name);
                },
            }
        }
    })
}
```

- [ ] **Step 2: Register the macro in lib.rs**

Add to `modo-macros/src/lib.rs`:

```rust
mod template_filter;

/// Auto-register a function as a MiniJinja template filter.
#[proc_macro_attribute]
pub fn template_filter(attr: TokenStream, item: TokenStream) -> TokenStream {
    template_filter::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

And add the re-export to `modo/src/lib.rs`:

```rust
#[cfg(feature = "templates")]
pub use modo_macros::template_filter;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo-macros && cargo check -p modo --features templates`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add modo-macros/src/template_filter.rs modo-macros/src/lib.rs modo/src/lib.rs
git commit -m "feat(macros): add #[template_filter] proc macro"
```

---

## Chunk 2: CookieManager + AppConfig

### Task 4: Add CookieConfig

**Files:**
- Create: `modo/src/cookies/mod.rs`
- Create: `modo/src/cookies/config.rs`

- [ ] **Step 1: Write the test for CookieConfig defaults**

Add to `modo/src/cookies/config.rs` (tests section):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cookie_config() {
        let config = CookieConfig::default();
        assert!(config.secret.is_none());
        assert!(config.domain.is_none());
        assert_eq!(config.path, "/");
        assert!(config.secure);
        assert!(config.http_only);
        assert_eq!(config.same_site, SameSite::Lax);
        assert!(config.max_age.is_none());
    }

    #[test]
    fn cookie_options_inherits_from_config() {
        let config = CookieConfig {
            domain: Some("example.com".to_string()),
            secure: true,
            ..Default::default()
        };
        let opts = CookieOptions::from_config(&config);
        assert_eq!(opts.domain.as_deref(), Some("example.com"));
        assert!(opts.secure);
    }

    #[test]
    fn cookie_options_override() {
        let config = CookieConfig {
            secure: true,
            ..Default::default()
        };
        let opts = CookieOptions::from_config(&config).secure(false);
        assert!(!opts.secure);
    }
}
```

- [ ] **Step 2: Implement CookieConfig**

Create `modo/src/cookies/config.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Default for SameSite {
    fn default() -> Self {
        Self::Lax
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CookieConfig {
    pub secret: Option<String>,
    pub domain: Option<String>,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    /// Max age in seconds. YAML config accepts integer only (e.g. `86400`).
    /// Duration string parsing (e.g. `"24h"`) may be added later.
    pub max_age: Option<u64>,
}

impl Default for CookieConfig {
    fn default() -> Self {
        Self {
            secret: None,
            domain: None,
            path: "/".to_string(),
            secure: true,
            http_only: true,
            same_site: SameSite::default(),
            max_age: None,
        }
    }
}

/// Per-cookie options that inherit from global CookieConfig.
#[derive(Debug, Clone)]
pub struct CookieOptions {
    pub path: String,
    pub domain: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    pub max_age: Option<u64>,
}

impl CookieOptions {
    pub fn from_config(config: &CookieConfig) -> Self {
        Self {
            path: config.path.clone(),
            domain: config.domain.clone(),
            secure: config.secure,
            http_only: config.http_only,
            same_site: config.same_site.clone(),
            max_age: config.max_age,
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    pub fn max_age(mut self, secs: u64) -> Self {
        self.max_age = Some(secs);
        self
    }

    pub fn session(mut self) -> Self {
        self.max_age = None;
        self
    }
}
```

- [ ] **Step 3: Create mod.rs (config only — manager added in Task 5)**

Create `modo/src/cookies/mod.rs`:

```rust
pub mod config;

pub use config::{CookieConfig, CookieOptions, SameSite};
```

- [ ] **Step 4: Add cookies module to lib.rs**

In `modo/src/lib.rs`, add (unconditional — cookies are always available):

```rust
pub mod cookies;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p modo -- cookies`
Expected: tests pass

- [ ] **Step 6: Commit**

```bash
git add modo/src/cookies/ modo/src/lib.rs
git commit -m "feat(cookies): add CookieConfig and CookieOptions"
```

---

### Task 5: Add CookieManager extractor

**Files:**
- Create: `modo/src/cookies/manager.rs`
- Modify: `modo/src/cookies/mod.rs`

- [ ] **Step 1: Update mod.rs to include manager module**

Update `modo/src/cookies/mod.rs`:

```rust
pub mod config;
pub mod manager;

pub use config::{CookieConfig, CookieOptions, SameSite};
pub use manager::CookieManager;
```

- [ ] **Step 2: Write tests for CookieManager**

Add tests at the bottom of `modo/src/cookies/manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, ServiceRegistry};
    use crate::config::ServerConfig;
    use axum::body::Body;
    use axum::Router;
    use axum::routing::get;
    use axum_extra::extract::cookie::Key;
    use http::Request;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        let cookie_config = CookieConfig {
            secret: Some("test-secret-at-least-32-bytes-long-for-key".to_string()),
            ..Default::default()
        };
        let services = ServiceRegistry::new().with(cookie_config);
        AppState {
            services,
            server_config: ServerConfig {
                secret_key: "test-secret-at-least-32-bytes-long-for-key".to_string(),
                ..Default::default()
            },
            cookie_key: Key::generate(),
        }
    }

    #[tokio::test]
    async fn set_and_read_plain_cookie() {
        let state = test_state();
        let app = Router::new()
            .route(
                "/set",
                get(|mut cookies: CookieManager| async move {
                    cookies.set("test", "hello");
                    cookies
                }),
            )
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/set").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let set_cookie = response
            .headers()
            .get(http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(set_cookie.contains("test=hello"));
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("Path=/"));
    }
}
```

- [ ] **Step 3: Implement CookieManager**

Create `modo/src/cookies/manager.rs`. This is a substantial file — the core API:

```rust
use super::config::{CookieConfig, CookieOptions, SameSite};
use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SignedCookieJar};
use cookie::time::Duration;
use axum::extract::FromRef;

/// High-level cookie extractor with plain, signed, and encrypted cookie support.
///
/// Uses global `CookieConfig` for defaults. Each setter accepts optional
/// `CookieOptions` overrides.
pub struct CookieManager {
    config: CookieConfig,
    jar: axum_extra::extract::CookieJar,
    signed_jar: SignedCookieJar,
    private_jar: PrivateCookieJar,
}

impl<S> FromRequestParts<S> for CookieManager
where
    S: Send + Sync,
    crate::app::AppState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = crate::app::AppState::from_ref(state);
        let config = app_state
            .services
            .get::<CookieConfig>()
            .map(|c| (*c).clone())
            .unwrap_or_default();

        let jar = axum_extra::extract::CookieJar::from_request_parts(parts, state)
            .await
            .unwrap();
        let signed_jar = SignedCookieJar::from_request_parts(parts, state)
            .await
            .unwrap();
        let private_jar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .unwrap();

        Ok(Self {
            config,
            jar,
            signed_jar,
            private_jar,
        })
    }
}

impl CookieManager {
    // --- Plain cookies ---

    pub fn get(&self, name: &str) -> Option<String> {
        self.jar.get(name).map(|c| c.value().to_string())
    }

    pub fn set(&mut self, name: &str, value: &str) {
        let opts = CookieOptions::from_config(&self.config);
        self.set_with(name, value, opts);
    }

    pub fn set_with(&mut self, name: &str, value: &str, opts: CookieOptions) {
        let cookie = build_cookie(name, value, &opts);
        self.jar = self.jar.clone().add(cookie);
    }

    pub fn remove(&mut self, name: &str) {
        self.jar = self.jar.clone().remove(Cookie::from(name.to_string()));
    }

    // --- Signed cookies (HMAC, tamper-proof but readable) ---

    pub fn get_signed(&self, name: &str) -> Option<String> {
        self.signed_jar.get(name).map(|c| c.value().to_string())
    }

    pub fn set_signed(&mut self, name: &str, value: &str) {
        let opts = CookieOptions::from_config(&self.config);
        self.set_signed_with(name, value, opts);
    }

    pub fn set_signed_with(&mut self, name: &str, value: &str, opts: CookieOptions) {
        let cookie = build_cookie(name, value, &opts);
        self.signed_jar = self.signed_jar.clone().add(cookie);
    }

    pub fn remove_signed(&mut self, name: &str) {
        self.signed_jar = self.signed_jar.clone().remove(Cookie::from(name.to_string()));
    }

    // --- Encrypted cookies (requires secret) ---

    pub fn get_encrypted(&self, name: &str) -> Option<String> {
        self.private_jar.get(name).map(|c| c.value().to_string())
    }

    pub fn set_encrypted(&mut self, name: &str, value: &str) {
        let opts = CookieOptions::from_config(&self.config);
        self.set_encrypted_with(name, value, opts);
    }

    pub fn set_encrypted_with(&mut self, name: &str, value: &str, opts: CookieOptions) {
        let cookie = build_cookie(name, value, &opts);
        self.private_jar = self.private_jar.clone().add(cookie);
    }

    pub fn remove_encrypted(&mut self, name: &str) {
        self.private_jar = self.private_jar.clone().remove(Cookie::from(name.to_string()));
    }

    // --- JSON convenience ---

    pub fn get_json<T: serde::de::DeserializeOwned>(&self, name: &str) -> Option<T> {
        self.get(name)
            .and_then(|v| serde_json::from_str(&v).ok())
    }

    pub fn set_json<T: serde::Serialize>(&mut self, name: &str, value: &T) {
        if let Ok(json) = serde_json::to_string(value) {
            self.set(name, &json);
        }
    }

    /// Default options from the global config — useful as a starting point for overrides.
    pub fn default_options(&self) -> CookieOptions {
        CookieOptions::from_config(&self.config)
    }
}

impl IntoResponseParts for CookieManager {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let res = self.jar.into_response_parts(res)?;
        let res = self.signed_jar.into_response_parts(res)?;
        self.private_jar.into_response_parts(res)
    }
}

impl IntoResponse for CookieManager {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

fn build_cookie<'a>(name: &str, value: &str, opts: &CookieOptions) -> Cookie<'a> {
    let mut cookie = Cookie::new(name.to_string(), value.to_string());
    cookie.set_path(opts.path.clone());
    cookie.set_http_only(opts.http_only);
    cookie.set_secure(opts.secure);

    match opts.same_site {
        SameSite::Strict => cookie.set_same_site(cookie::SameSite::Strict),
        SameSite::Lax => cookie.set_same_site(cookie::SameSite::Lax),
        SameSite::None => cookie.set_same_site(cookie::SameSite::None),
    }

    if let Some(domain) = &opts.domain {
        cookie.set_domain(domain.clone());
    }

    if let Some(max_age) = opts.max_age {
        cookie.set_max_age(Duration::seconds(max_age as i64));
    }

    cookie
}
```

Note: The exact API may need adjustments based on axum-extra's `CookieJar` API (the jars are consumed on `.add()` — hence the `self.jar.clone().add()` pattern). Verify this compiles correctly with the actual axum-extra version.

- [ ] **Step 4: Run tests**

Run: `cargo test -p modo -- cookies`
Expected: tests pass

- [ ] **Step 5: Commit**

```bash
git add modo/src/cookies/
git commit -m "feat(cookies): add CookieManager extractor with plain, signed, and encrypted support"
```

---

### Task 6: Add AppConfig

**Files:**
- Modify: `modo/src/config.rs`
- Modify: `modo/src/lib.rs`

- [ ] **Step 1: Write test for AppConfig**

Add to `modo/src/config.rs` tests module:

```rust
#[test]
fn test_app_config_defaults() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.server.port, 3000);
    assert_eq!(cfg.cookies.path, "/");
}

#[test]
fn test_app_config_yaml_minimal() {
    let yaml = "server:\n  port: 8080\n";
    let cfg: AppConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(cfg.server.port, 8080);
    // cookies get defaults
    assert_eq!(cfg.cookies.path, "/");
}
```

- [ ] **Step 2: Add AppConfig struct**

Add to `modo/src/config.rs` after `ServerConfig`:

```rust
/// Unified application configuration.
///
/// Top-level config type for `#[modo::main]`. Includes server settings
/// and optional feature-specific sections (cookies, templates, i18n, CSRF).
/// All feature sections use `#[serde(default)]` — absent in YAML means defaults apply.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub cookies: crate::cookies::CookieConfig,
    #[cfg(feature = "templates")]
    pub templates: crate::templates::TemplateConfig,
    #[cfg(feature = "i18n")]
    pub i18n: crate::i18n::I18nConfig,
    #[cfg(feature = "csrf")]
    pub csrf: crate::csrf::CsrfConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            cookies: crate::cookies::CookieConfig::default(),
            #[cfg(feature = "templates")]
            templates: crate::templates::TemplateConfig::default(),
            #[cfg(feature = "i18n")]
            i18n: crate::i18n::I18nConfig::default(),
            #[cfg(feature = "csrf")]
            csrf: crate::csrf::CsrfConfig::default(),
        }
    }
}
```

- [ ] **Step 3: Add re-export to lib.rs**

In `modo/src/lib.rs`, add to the `pub use config::{...}` line:

```rust
pub use config::{AppConfig, HttpConfig, RateLimitConfig, SecurityHeadersConfig, TrailingSlash};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p modo -- test_app_config`
Expected: tests pass

- [ ] **Step 5: Commit**

```bash
git add modo/src/config.rs modo/src/lib.rs
git commit -m "feat(config): add unified AppConfig with feature-gated sections"
```

---

## Chunk 3: AppBuilder Auto-Wiring

### Task 7: Rename server_config() to config() and accept AppConfig

**Files:**
- Modify: `modo/src/app.rs:77-120`

- [ ] **Step 1: Add config() method that accepts AppConfig**

In `modo/src/app.rs`, change the `AppBuilder` struct to store `AppConfig` instead of `ServerConfig`:

Replace the `server_config` field (line 78):
```rust
// Old
server_config: ServerConfig,
// New
app_config: Option<crate::config::AppConfig>,
```

Update `new()` (line 99):
```rust
// Old
server_config: ServerConfig::default(),
// New
app_config: None,
```

Replace `server_config()` method (lines 117-120):
```rust
pub fn config(mut self, config: crate::config::AppConfig) -> Self {
    self.app_config = Some(config);
    self
}

/// Deprecated: use `config()` with `AppConfig` instead.
pub fn server_config(mut self, config: ServerConfig) -> Self {
    let mut app_config = self.app_config.take().unwrap_or_default();
    app_config.server = config;
    self.app_config = Some(app_config);
    self
}
```

Update `ensure_http_override()` (line 243-248):
```rust
fn ensure_http_override(&mut self) -> &mut HttpConfig {
    if self.override_http.is_none() {
        let http = self
            .app_config
            .as_ref()
            .map(|c| c.server.http.clone())
            .unwrap_or_default();
        self.override_http = Some(http);
    }
    self.override_http.as_mut().unwrap()
}
```

- [ ] **Step 2: Update run() to use AppConfig**

In `run()`, replace the opening config resolution (lines 251-269) with:

```rust
let app_config = self.app_config.unwrap_or_default();
let mut server_config = app_config.server.clone();
```

The rest of the config override logic stays the same.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo --all-features`
Expected: success (existing examples will break — that's expected, we'll fix them in Task 11)

- [ ] **Step 4: Commit**

```bash
git add modo/src/app.rs
git commit -m "refactor(app): rename server_config() to config(), accept AppConfig"
```

---

### Task 8: Add templates() callback and auto-wiring in run()

**Files:**
- Modify: `modo/src/app.rs`

- [ ] **Step 1: Add templates callback field to AppBuilder**

Add to `AppBuilder` struct fields:

```rust
#[cfg(feature = "templates")]
templates_callback: Option<Box<dyn FnOnce(&mut crate::templates::TemplateEngine) + Send>>,
```

Initialize in `new()`:

```rust
#[cfg(feature = "templates")]
templates_callback: None,
```

Add method:

```rust
/// Configure the template engine before it's registered as a service.
///
/// The callback receives a `&mut TemplateEngine` after auto-discovery
/// of `#[template_function]` and `#[template_filter]` macros, but before
/// the engine is registered. Use this for advanced `env_mut()` access.
#[cfg(feature = "templates")]
pub fn templates(
    mut self,
    f: impl FnOnce(&mut crate::templates::TemplateEngine) + Send + 'static,
) -> Self {
    self.templates_callback = Some(Box::new(f));
    self
}
```

- [ ] **Step 2: Add template auto-wiring in run()**

In `run()`, after creating `app_config` and before building the service registry, add the template auto-wiring block. Insert before the `AppState` creation (around line 280):

```rust
// --- Auto-wire templates ---
#[cfg(feature = "templates")]
{
    use crate::templates::{TemplateFunctionEntry, TemplateFilterEntry};

    let mut engine = crate::templates::engine(&app_config.templates)?;

    // Register inventory-discovered functions
    for entry in inventory::iter::<TemplateFunctionEntry> {
        (entry.register_fn)(engine.env_mut());
    }

    // Register inventory-discovered filters
    for entry in inventory::iter::<TemplateFilterEntry> {
        (entry.register_fn)(engine.env_mut());
    }

    // Auto-wire i18n template functions if both features enabled
    #[cfg(feature = "i18n")]
    {
        let i18n_store = crate::i18n::load(&app_config.i18n)?;
        crate::i18n::register_template_functions(engine.env_mut(), i18n_store.clone());
        // Store i18n_store for layer wiring later (load() returns Arc<TranslationStore>)
        self.services.insert(
            std::any::TypeId::of::<crate::i18n::TranslationStore>(),
            i18n_store,
        );
    }

    // Auto-wire CSRF template functions if both features enabled
    #[cfg(feature = "csrf")]
    crate::csrf::register_template_functions(engine.env_mut());

    // Run user callback
    if let Some(callback) = self.templates_callback {
        callback(&mut engine);
    }

    // Register engine as service
    self.services.insert(
        std::any::TypeId::of::<crate::templates::TemplateEngine>(),
        std::sync::Arc::new(engine),
    );
}
```

- [ ] **Step 3: Add i18n auto-wiring (when templates not enabled)**

After the templates block, add standalone i18n wiring:

```rust
// --- Auto-wire i18n (standalone, without templates) ---
#[cfg(all(feature = "i18n", not(feature = "templates")))]
{
    let i18n_store = crate::i18n::load(&app_config.i18n)?;
    self.services.insert(
        std::any::TypeId::of::<crate::i18n::TranslationStore>(),
        i18n_store,
    );
}
```

- [ ] **Step 4: Add i18n layer wiring in the middleware stack**

In the middleware stack section (after user layers, before rate limiter), add:

```rust
// --- i18n layer (auto-wired) ---
#[cfg(feature = "i18n")]
if let Some(store) = state.services.get::<crate::i18n::TranslationStore>() {
    router = router.layer(crate::i18n::layer(store));
}
```

Note: `state.services.get::<TranslationStore>()` returns `Option<Arc<TranslationStore>>` — which is exactly what `crate::i18n::layer()` expects.

- [ ] **Step 5: Auto-register CookieConfig and CsrfConfig as services**

In `run()`, before `AppState` creation, add:

```rust
// --- Auto-wire CookieConfig ---
self.services.insert(
    std::any::TypeId::of::<crate::cookies::CookieConfig>(),
    std::sync::Arc::new(app_config.cookies.clone()),
);

// --- Auto-wire CsrfConfig ---
#[cfg(feature = "csrf")]
self.services.insert(
    std::any::TypeId::of::<crate::csrf::CsrfConfig>(),
    std::sync::Arc::new(app_config.csrf.clone()),
);
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p modo --all-features`
Expected: success

- [ ] **Step 7: Run existing tests**

Run: `cargo test -p modo`
Expected: all pass (tests don't depend on examples)

- [ ] **Step 8: Commit**

```bash
git add modo/src/app.rs
git commit -m "feat(app): auto-wire templates, i18n, CSRF, cookies in run()"
```

---

### Task 9: Re-export new types from lib.rs

**Files:**
- Modify: `modo/src/lib.rs`

- [ ] **Step 1: Update lib.rs with all new re-exports**

Ensure `modo/src/lib.rs` has:

```rust
// New macro re-exports
#[cfg(feature = "templates")]
pub use modo_macros::template_filter;
#[cfg(feature = "templates")]
pub use modo_macros::template_function;

// New module
pub mod cookies;

// Updated config re-export
pub use config::{AppConfig, HttpConfig, RateLimitConfig, SecurityHeadersConfig, TrailingSlash};
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p modo --all-features`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add modo/src/lib.rs
git commit -m "refactor(lib): add re-exports for cookies, AppConfig, template macros"
```

---

## Chunk 4: Examples Update

### Task 10: Update hello example

**Files:**
- Modify: `examples/hello/src/main.rs:36-42`

- [ ] **Step 1: Update main function**

Replace:
```rust
#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: modo::config::ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    app.server_config(config).run().await
}
```

With:
```rust
#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: modo::config::AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    app.config(config).run().await
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p hello`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add examples/hello/src/main.rs
git commit -m "refactor(examples/hello): use AppConfig and config()"
```

---

### Task 11: Update templates example

**Files:**
- Modify: `examples/templates/src/main.rs`

- [ ] **Step 1: Simplify to use macros**

Replace entire file:
```rust
#[modo::view("pages/home.html", htmx = "partials/clock.html")]
struct HomePage {
    time: String,
    date: String,
    time_hour: u32,
}

#[modo::handler(GET, "/")]
async fn home() -> HomePage {
    let now = chrono::Local::now();
    HomePage {
        time: now.format("%H:%M:%S").to_string(),
        date: now.format("%A, %B %d, %Y").to_string(),
        time_hour: chrono::Timelike::hour(&now),
    }
}

#[modo::template_function]
fn greeting(hour: u32) -> String {
    match hour {
        0..=11 => "Good morning".to_string(),
        12..=17 => "Good afternoon".to_string(),
        _ => "Good evening".to_string(),
    }
}

#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: modo::config::AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    app.config(config).run().await
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p templates`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add examples/templates/src/main.rs
git commit -m "refactor(examples/templates): simplify with #[template_function] and AppConfig"
```

---

### Task 12: Update jobs example

**Files:**
- Modify: `examples/jobs/src/main.rs:8-15,87`

- [ ] **Step 1: Update config struct and main**

Change the config struct:
```rust
#[derive(Default, Deserialize)]
struct Config {
    #[serde(flatten)]
    core: modo::config::AppConfig,
    database: DatabaseConfig,
    #[serde(default)]
    jobs: modo_jobs::JobsConfig,
}
```

Change the main function to use `.config(config.core)`:
```rust
app.config(config.core)
    .managed_service(db)
    .managed_service(jobs)
    .run()
    .await
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p jobs`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add examples/jobs/src/main.rs
git commit -m "refactor(examples/jobs): use AppConfig and config()"
```

---

### Task 13: Update todo-api example

**Files:**
- Modify: `examples/todo-api/src/main.rs:7-12,110`

- [ ] **Step 1: Update config struct and main**

Change:
```rust
#[derive(Default, Deserialize)]
struct Config {
    #[serde(flatten)]
    core: modo::config::AppConfig,
    database: DatabaseConfig,
}
```

And main:
```rust
app.config(config.core)
    .managed_service(db)
    .run()
    .await
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p todo-api`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add examples/todo-api/src/main.rs
git commit -m "refactor(examples/todo-api): use AppConfig and config()"
```

---

### Task 14: Update upload example

**Files:**
- Modify: `examples/upload/src/main.rs:36-54`

- [ ] **Step 1: Update config struct and main**

Change:
```rust
#[derive(Default, Deserialize)]
struct Config {
    #[serde(flatten)]
    core: modo::config::AppConfig,
    #[serde(default)]
    upload: UploadConfig,
}
```

And main:
```rust
let storage = modo_upload::storage(&config.upload)?;
app.config(config.core)
    .service(storage)
    .run()
    .await
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p upload`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add examples/upload/src/main.rs
git commit -m "refactor(examples/upload): use AppConfig and config()"
```

---

### Task 15: Refactor CSRF to use CookieManager (follow-up)

**Files:**
- Modify: `modo/src/csrf/middleware.rs`

Note: This is a refactor — CSRF already works. The goal is to replace raw `build_set_cookie()` and `read_cookie()` calls with `CookieManager` for consistency. This task can be deferred if the other tasks run long.

- [ ] **Step 1: Replace build_set_cookie with CookieManager in handle_safe_request**

In `modo/src/csrf/middleware.rs`, the `handle_safe_request` function (line 62) currently builds a raw Set-Cookie header string via `build_set_cookie()`. Refactor to use `CookieManager`'s signed cookie API instead, reading the `CookieConfig` from AppState services for cookie defaults.

The key changes:
- Replace `read_cookie(&parts.headers, &config.cookie_name)` with signed cookie reads
- Replace `build_set_cookie(...)` with `CookieManager` signed cookie writes
- Remove the local `build_set_cookie` helper function

- [ ] **Step 2: Run CSRF tests**

Run: `cargo test -p modo -- csrf`
Expected: all existing CSRF tests pass

- [ ] **Step 3: Commit**

```bash
git add modo/src/csrf/middleware.rs
git commit -m "refactor(csrf): use CookieManager for cookie operations"
```

---

### Task 16: Run full check

- [ ] **Step 1: Format**

Run: `just fmt`

- [ ] **Step 2: Lint**

Run: `just lint`
Expected: success (no warnings with -D warnings)

- [ ] **Step 3: Test**

Run: `just test`
Expected: all tests pass

- [ ] **Step 4: Fix any issues found**

If lint/test fails, fix and re-run.

- [ ] **Step 5: Commit any formatting/lint fixes**

```bash
git add -A
git commit -m "style: fix formatting and lint issues"
```
