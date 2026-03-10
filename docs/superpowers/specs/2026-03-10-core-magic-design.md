# Core Magic — Auto-Wiring & Simplified API

**Date:** 2026-03-10
**Status:** Approved
**Approach:** C — Auto-wire with override

## Goal

Move feature wiring from user code into the framework. Features are enabled via Cargo feature flags, configured via YAML config sections (with sensible defaults), and auto-wired during `AppBuilder::run()`. Users only write code for customization and user-provided types.

## Principles

- **Feature flag on = feature active.** No explicit opt-in calls.
- **Config section optional.** Absent = defaults. Present = override defaults.
- **Customization only when needed.** `.templates(|engine| { ... })` escape hatch, not the primary path.
- **Macro-based discovery.** Template functions/filters use `inventory` like handlers and jobs.
- **No changes to external crates.** modo-db, modo-session, modo-jobs, modo-auth, modo-tenant, modo-upload stay as-is.

## Unified Config

`ServerConfig` replaced by `AppConfig` as the top-level config type.

```yaml
# config.yaml
server:
  host: "0.0.0.0"
  port: 3000

cookies:
  secret: "${COOKIE_SECRET}"
  domain: "example.com"        # optional
  path: "/"                    # default: "/"
  secure: true                 # default: true
  http_only: true              # default: true
  same_site: "lax"             # default: "lax"
  max_age: 86400               # default: session cookie

templates:                      # feature = "templates"
  path: "templates"            # default: "templates"
  strict: true                 # default: true

i18n:                           # feature = "i18n"
  path: "translations"         # default: "translations"
  default_locale: "en"         # default: "en"

csrf:                           # feature = "csrf"
  cookie_name: "csrf_token"    # default: "csrf_token"
  token_length: 32             # default: 32
  ttl: "1h"                    # default: "1h"
```

```rust
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub cookies: CookieConfig,
    #[cfg(feature = "templates")]
    #[serde(default)]
    pub templates: TemplateConfig,
    #[cfg(feature = "i18n")]
    #[serde(default)]
    pub i18n: I18nConfig,
    #[cfg(feature = "csrf")]
    #[serde(default)]
    pub csrf: CsrfConfig,
}
```

All feature sections use `#[serde(default)]` — absent in YAML means defaults apply.

## CookieManager

Core extractor for cookie operations. Used by handlers directly and by CSRF internally. Available to external crates (modo-session) via public API.

### Config

```rust
pub struct CookieConfig {
    pub secret: Option<String>,   // required for encrypted/signed cookies
    pub domain: Option<String>,
    pub path: String,             // default: "/"
    pub secure: bool,             // default: true
    pub http_only: bool,          // default: true
    pub same_site: SameSite,      // default: Lax
    pub max_age: Option<u64>,     // default: None (session cookie)
}
```

### API

```rust
pub struct CookieManager { /* config + request jar + response jar */ }

// Plain cookies
cookies.get("name") -> Option<String>
cookies.set("name", "value")
cookies.set_with("name", "value", CookieOptions { ... })
cookies.remove("name")

// Encrypted cookies (requires secret)
cookies.get_encrypted("name") -> Option<String>
cookies.set_encrypted("name", "value")
cookies.set_encrypted_with("name", "value", CookieOptions { ... })
cookies.remove_encrypted("name")

// Signed cookies (HMAC, tamper-proof but readable)
cookies.get_signed("name") -> Option<String>
cookies.set_signed("name", "value")

// JSON convenience
cookies.get_json::<T>("name") -> Option<T>
cookies.set_json("name", &value)
```

`CookieOptions` inherits from global `CookieConfig` with per-call overrides.

## AppBuilder Changes

```rust
impl AppBuilder {
    pub fn config(mut self, config: AppConfig) -> Self { ... }

    #[cfg(feature = "templates")]
    pub fn templates(mut self, f: impl FnOnce(&mut TemplateEngine) + 'static) -> Self { ... }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> { ... }
}
```

### Auto-wiring order inside `run()`

1. **Cookies** — register `CookieConfig` as service (always)
2. **Templates** (feature = "templates")
   - `engine(&config.templates)` — create `TemplateEngine`
   - Collect `inventory<TemplateFunctionEntry>` — register on engine
   - Collect `inventory<TemplateFilterEntry>` — register on engine
   - If feature `i18n` also on: `register_template_functions(engine)`
   - If feature `csrf` also on: register `csrf_token()` template function
   - Run user's `.templates(|engine|)` callback if provided
   - Register engine as service (auto-wires RenderLayer + ContextLayer)
3. **i18n** (feature = "i18n")
   - `i18n::load(&config.i18n)` — create `TranslationStore`
   - Wire `I18nLayer` into middleware stack
4. **CSRF** (feature = "csrf")
   - Register `CsrfConfig` as service
   - CSRF middleware already attached via `#[middleware]` on handlers — no global layer
5. Existing server middleware stack
6. User layers (`.layer()` calls)
7. Router assembly + server start

## Macro-Based Template Functions & Filters

### `#[modo::template_function]`

```rust
#[modo::template_function]
fn greeting(hour: u32) -> String {
    match hour {
        0..=11 => "Good morning".to_string(),
        12..=17 => "Good afternoon".to_string(),
        _ => "Good evening".to_string(),
    }
}
```

Expands to:

```rust
fn greeting(hour: u32) -> String { /* original body */ }

#[cfg(feature = "templates")]
inventory::submit! {
    modo::templates::TemplateFunctionEntry {
        name: "greeting",
        register_fn: |env: &mut minijinja::Environment<'static>| {
            env.add_function("greeting", greeting);
        },
    }
}
```

### `#[modo::template_filter]`

```rust
#[modo::template_filter]
fn shout(value: String) -> String {
    value.to_uppercase()
}
```

Same pattern using `env.add_filter()`.

### Custom name

```rust
#[modo::template_function(name = "greet")]
fn greeting(hour: u32) -> String { ... }
```

### Registration types

```rust
pub struct TemplateFunctionEntry {
    pub name: &'static str,
    pub register_fn: fn(&mut minijinja::Environment<'static>),
}
inventory::collect!(TemplateFunctionEntry);

pub struct TemplateFilterEntry {
    pub name: &'static str,
    pub register_fn: fn(&mut minijinja::Environment<'static>),
}
inventory::collect!(TemplateFilterEntry);
```

## Example: Before & After

### Before

```rust
use modo::templates::{TemplateConfig, engine};

#[modo::view("pages/home.html", htmx = "partials/clock.html")]
struct HomePage { time: String, date: String, time_hour: u32 }

#[modo::handler(GET, "/")]
async fn home() -> HomePage { /* ... */ }

#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: modo::config::ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let template_config = TemplateConfig::default();
    let mut engine = engine(&template_config)?;
    engine.env_mut().add_function("greeting", |hour: u32| -> String {
        match hour {
            0..=11 => "Good morning".to_string(),
            12..=17 => "Good afternoon".to_string(),
            _ => "Good evening".to_string(),
        }
    });
    app.server_config(config).service(engine).run().await
}
```

### After

```rust
#[modo::view("pages/home.html", htmx = "partials/clock.html")]
struct HomePage { time: String, date: String, time_hour: u32 }

#[modo::handler(GET, "/")]
async fn home() -> HomePage { /* ... */ }

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

## Files to Modify

### `modo/`
- `Cargo.toml` — cookie deps (axum_extra cookie jars, encryption)
- `src/config.rs` — new `AppConfig` struct
- `src/app.rs` — `server_config()` → `config()`, `templates()`, auto-wiring in `run()`
- `src/cookies/` — new module: `CookieConfig`, `CookieManager`, `CookieOptions`
- `src/templates/` — `TemplateFunctionEntry`, `TemplateFilterEntry`, inventory collection
- `src/csrf/` — refactor to use `CookieManager` internally
- `src/i18n/` — wiring moves from user code into `run()`
- `src/lib.rs` — re-export `AppConfig`, `CookieManager`, cookie module

### `modo-macros/`
- `src/lib.rs` — `#[template_function]`, `#[template_filter]` proc macros
- `src/main_macro.rs` — update to `load_or_default::<AppConfig>()`

### Examples
- All examples: `ServerConfig` → `AppConfig`, `server_config()` → `config()`
- `templates/` example: simplified with macros

### Unchanged
- Handler/module/view macros
- External crates (modo-db, modo-session, modo-jobs, modo-auth, modo-tenant, modo-upload)
- `#[middleware]` pattern
- `.service()`, `.managed_service()`, `.layer()` methods

## Breaking Changes

- `ServerConfig` → `AppConfig` in `#[modo::main]` signature
- `server_config()` → `config()` on `AppBuilder`
- Users no longer manually create `TemplateEngine` or `TranslationStore`
- `CsrfConfig` no longer manually registered as service

## New Public API

- `AppConfig` — unified config type
- `CookieManager` — extractor
- `CookieConfig`, `CookieOptions` — cookie configuration
- `#[modo::template_function]`, `#[modo::template_filter]` — macros
- `AppBuilder::templates(|engine| { ... })` — escape hatch
- `TemplateFunctionEntry`, `TemplateFilterEntry` — inventory types
