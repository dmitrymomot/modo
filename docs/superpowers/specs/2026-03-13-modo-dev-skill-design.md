# modo-dev Skill Plugin — Design Spec

## Overview

A Claude Code plugin that provides a skill for building applications with the modo Rust web framework. The skill guides Claude through requirement gathering, directs it to the right reference documentation, and enforces modo conventions and built-in functionality usage.

**Target audience:** Developers building apps with modo (not framework development).
**Assumed starting point:** Project scaffolded by `modo-cli` with required modules enabled.

## Plugin Location & Installation

The plugin lives in the modo repo at `claude-plugin/`. A marketplace entry at the repo root enables installation.

### Installation flow

```
/plugin marketplace add dmitrymomot/modo
/plugin install modo-dev@modo
```

### Repository structure

```
modo/                                   # existing repo root
├── .claude-plugin/
│   └── marketplace.json                # marketplace catalog
├── claude-plugin/                      # plugin root
│   ├── .claude-plugin/
│   │   └── plugin.json
│   └── skills/
│       └── modo/
│           ├── SKILL.md
│           └── references/
│               ├── conventions.md
│               ├── handlers.md
│               ├── database.md
│               ├── jobs.md
│               ├── email.md
│               ├── auth-sessions.md
│               ├── templates-htmx.md
│               ├── upload.md
│               ├── tenant.md
│               ├── config.md
│               └── testing.md
├── Cargo.toml
├── modo/
├── modo-macros/
└── ...
```

## Plugin Manifest

### `.claude-plugin/marketplace.json` (repo root)

```json
{
  "name": "modo",
  "owner": {
    "name": "Dmytro Momot"
  },
  "plugins": [
    {
      "name": "modo-dev",
      "source": {
        "source": "git-subdir",
        "url": "https://github.com/dmitrymomot/modo.git",
        "path": "claude-plugin"
      },
      "description": "Skills for building apps with the modo Rust web framework"
    }
  ]
}
```

### `claude-plugin/.claude-plugin/plugin.json`

```json
{
  "name": "modo-dev",
  "description": "Skills for building applications with the modo Rust web framework",
  "version": "0.1.0",
  "author": { "name": "Dmytro Momot" },
  "repository": "https://github.com/dmitrymomot/modo",
  "license": "Apache-2.0"
}
```

## Skill Design

### SKILL.md (~800-1,000 words)

**Frontmatter:**

```yaml
---
name: modo
description: >
  This skill should be used when the user is building an application with the
  modo Rust web framework, asks about modo handlers, modules, middleware,
  database entities, migrations, jobs, email, sessions, authentication,
  templates, HTMX, SSE, uploads, multi-tenancy, configuration, or testing
  patterns. Also use when the user references modo macros like #[handler],
  #[module], #[main], #[entity], #[job], #[view], or FromMultipart.
---
```

**Body structure (4 sections):**

#### 1. Hard Rules

Two non-negotiable rules at the top:

- **Requirement-gathering gate:** Before writing any code, use AskUserQuestion to clarify what the user wants to build (endpoint, entity, job, auth flow, etc.), understand specifics (field types, relations, validation, modules involved, error handling), then describe the approach and wait for approval. Do not skip even for simple tasks.
- **Use built-in functionality first:** Always prefer modo's built-in macros, extractors, middleware, config, and error types over manual implementations or external crates. Use `#[entity]` not manual SeaORM models. Use `HandlerResult` not custom error types. Use `modo::Json` not `axum::Json`. Use built-in validation/sanitization derives. Only reach for manual approaches when no built-in exists.

#### 2. Assumed Starting Point

Project scaffolded by `modo-cli` with all needed modules enabled. This skill focuses on building features on top of that structure.

#### 3. Macro Cheat Sheet

Compact table:

| Macro | Crate | Purpose |
|-------|-------|---------|
| `#[modo::handler(METHOD, PATH)]` | modo-macros | Route registration |
| `#[modo::module(prefix = "/path")]` | modo-macros | Route prefix grouping |
| `#[modo::main]` / `#[modo::main(static_assets = "path/")]` | modo-macros | App bootstrap (optional embedded static assets) |
| `#[modo::error_handler]` | modo-macros | Custom error handling |
| `#[modo::view]` | modo-macros | Template rendering |
| `#[modo::template_function]` | modo-macros | Custom template function |
| `#[modo::template_filter]` | modo-macros | Custom template filter |
| `modo::t!(i18n, "key")` | modo-macros | i18n translation (function-like macro) |
| `#[derive(Sanitize)]` | modo-macros | Input sanitization |
| `#[derive(Validate)]` | modo-macros | Input validation |
| `#[modo_db::entity]` | modo-db-macros | Entity + migration generation |
| `#[modo_db::migration]` | modo-db-macros | Versioned SQL migration |
| `#[modo_jobs::job]` | modo-jobs-macros | Job definition |
| `#[derive(FromMultipart)]` | modo-upload-macros | Multipart form parsing |

#### 4. Topic Index

Maps tasks to reference files:

| Task | Read |
|------|------|
| File organization, error patterns, custom error handlers, gotchas | `references/conventions.md` |
| Handlers, routing, modules, middleware, rate limiting, CORS, security headers, static files | `references/handlers.md` |
| Entities, migrations, queries, pagination | `references/database.md` |
| Background jobs, cron scheduling | `references/jobs.md` |
| Email templates, transports | `references/email.md` |
| Authentication, sessions, password hashing | `references/auth-sessions.md` |
| Templates, HTMX, SSE, CSRF, i18n | `references/templates-htmx.md` |
| File uploads, multipart, storage backends | `references/upload.md` |
| Multi-tenancy resolver patterns | `references/tenant.md` |
| YAML config, env interpolation, feature flags | `references/config.md` |
| Testing middleware, cookies, inventory | `references/testing.md` |

## Reference Files

Each reference file (2,000-4,000 words) follows this structure:

1. **Documentation header** — docs.rs links for relevant crates
2. **Built-in first** — leads with modo's built-in way to do things
3. **Patterns + inline code examples** — extracted from actual crate source, examples, and tests
4. **Common recipes** — hand-written walkthroughs connecting patterns together
5. **Gotchas** — topic-specific pitfalls

### Reference file → docs.rs mapping

| File | docs.rs links |
|------|---------------|
| `conventions.md` | https://docs.rs/modo |
| `handlers.md` | https://docs.rs/modo, https://docs.rs/modo-macros |
| `database.md` | https://docs.rs/modo-db, https://docs.rs/modo-db-macros |
| `jobs.md` | https://docs.rs/modo-jobs, https://docs.rs/modo-jobs-macros |
| `email.md` | https://docs.rs/modo-email |
| `auth-sessions.md` | https://docs.rs/modo-auth, https://docs.rs/modo-session |
| `templates-htmx.md` | https://docs.rs/modo (features: templates, sse, csrf, i18n) |
| `upload.md` | https://docs.rs/modo-upload, https://docs.rs/modo-upload-macros |
| `tenant.md` | https://docs.rs/modo-tenant |
| `config.md` | https://docs.rs/modo |
| `testing.md` | All relevant crates |

### Reference file content summaries

**conventions.md** — File organization rules (`mod.rs` only for imports/re-exports, handlers and views in separate files), middleware stacking order (Global → Module → Handler), error handling patterns (`HandlerResult<T>`, `JsonResult<T>`, `#[error_handler]` for custom error handling), `modo::Json` not `axum::Json`, ULID session IDs (no UUID), feature flag syntax (`dep:name`), gotchas (inventory linking in tests, `ExprTrait` conflicts with `Ord::max`/`Ord::min`, HTMX 200-only rendering, re-exports must be alphabetically sorted).

**handlers.md** — `#[handler(GET, "/path")]` and `#[handler(POST, "/path")]` patterns, `#[module(prefix = "/path")]` for grouping, path params with partial extraction via `..`, extractors (Query, Json, Path), `#[derive(Validate)]` and `#[derive(Sanitize)]` for input processing, per-handler middleware, `HandlerResult<T>` and `JsonResult<T>` return types, `modo::Json` wrapper, built-in middleware (`RateLimitConfig`, `SecurityHeadersConfig`, `CorsConfig`, `TrailingSlash`, `ClientIp`, `RateLimitInfo`), static file serving (`static-fs` for dev, `static-embed` for prod), health check endpoints.

**database.md** — `#[entity(table = "name")]` macro for model + migration generation, field types and attributes, `Db` extractor, CRUD patterns, query building, offset and cursor pagination helpers, `#[migration]` for versioned SQL, group-scoped database sync, SeaORM v2 RC specifics.

**jobs.md** — `#[job(queue = "name")]` macro, `JobQueue` extractor for enqueuing, cron scheduling (in-memory only, not persisted), retry with exponential backoff, graceful shutdown with drain timeout, compile-time registration via `inventory`.

**email.md** — `Mailer` service, Markdown templates with HTML rendering + plain-text fallback, SMTP (lettre) and Resend transports, template filesystem provider with layouts, integration pattern: mailer registered on jobs builder (`.service(email)`), app enqueues `SendEmailPayload`, job worker sends. Template var syntax overlap with scaffold-time Jinja vars — use raw blocks when both appear.

**auth-sessions.md** — `UserProvider` trait for pluggable user repos, `Auth<U>` and `OptionalAuth<U>` extractors, Argon2id password hashing with `PasswordHasher`/`PasswordConfig`, `SessionManager` extractor, `SessionStore`, `SessionConfig`, `user_id_from_extensions(&parts.extensions)` for session user ID access, database-backed sessions with SHA-256 hashed tokens, ULID session IDs, LRU eviction for multi-device, sliding expiry, optional cleanup job via `cleanup-job` feature, `UserContextLayer` for template injection.

**templates-htmx.md** — MiniJinja `#[view]` macro, auto-registered template layer (no manual `.layer()`), `#[template_function]` and `#[template_filter]`, HTMX rendering rules (render on `HX-Request`, always HTTP 200, non-200 skips render), SSE with `SseBroadcastManager` and `SseEvent` builder, CSRF double-submit cookie with `CsrfToken` extractor, i18n with `I18n` extractor and `#[t]` macro.

**upload.md** — `#[derive(FromMultipart)]` for declarative multipart parsing, per-field validation (size limits, MIME type filtering), `FileStorage` trait, local filesystem (default) and S3-compatible via OpenDAL, `StorageBackend` enum in config, `UploadedFile` type, `MultipartForm<T>` extractor.

**tenant.md** — `TenantResolver` trait for custom resolution, subdomain/header/path resolution strategies, `Tenant` and `OptionalTenant` extractors, type-erased service pattern with `Arc<dyn TenantResolverDyn<T>>`, `TenantContextLayer` for template injection.

**config.md** — YAML config loaded from `config/{MODO_ENV}.yaml`, environment variable interpolation (`${VAR}` and `${VAR:-default}`), config struct with serde deserialization, nested sections (server, cookies, database, jobs, email, upload, tenant, auth, session), feature-gated fields with `#[cfg(feature = "...")]` in struct, `Default`, and `from_env()`.

**testing.md** — Tower middleware testing with `Router::new().route(...).layer(mw).oneshot(request)` (no AppState needed, handler reads `Extension<T>`), cookie attribute testing (create `AppState` with custom `CookieConfig`, fire request, assert `Set-Cookie` header), inventory force-linking with `use crate::entity::foo as _;`, feature-gated testing with `cargo test -p crate --features feat`.

## Scope Boundaries

The skill helps with **building features on top of a scaffolded modo project**. It does NOT cover:

- Framework development (modifying modo itself, writing new macros)
- Project scaffolding (that's `modo-cli`)
- Deployment, Docker, CI/CD
- External crate selection beyond modo's ecosystem

## Reference Depth

Reference files cover the **80% use-case patterns**, not exhaustive API surface. docs.rs links are provided for full API details. Content summaries below are rough guides for topic coverage — not strict checklists. The implementer should use judgment on what patterns are most useful, based on what the actual source code reveals.

## Example → Reference Mapping

| Example | Primary reference |
|---------|-------------------|
| `hello` | `handlers.md` |
| `todo-api` | `handlers.md`, `database.md` |
| `jobs` | `jobs.md` |
| `upload` | `upload.md` |
| `templates` | `templates-htmx.md` |
| `sse-chat` | `templates-htmx.md`, `auth-sessions.md` |
| `sse-dashboard` | `templates-htmx.md` |

## Implementation Approach

Reference content will be derived from the actual crate source code, read one crate at a time in depth:

1. Read the crate's `src/` files (public API, structs, traits, macros)
2. Read the crate's examples and tests
3. Read the crate's README
4. Cross-reference with relevant examples in `examples/`
5. Write the reference file with extracted patterns + hand-written narrative

No guessing or hallucinating APIs. Every code example in a reference file must be verified against the actual source.

## Build Sequence

1. Create directory structure and manifests (`marketplace.json`, `plugin.json`)
2. Write `SKILL.md`
3. Write `references/conventions.md` (foundational — other refs depend on it)
4. Write remaining reference files one at a time, reading each crate in depth:
   - `handlers.md` (modo + modo-macros)
   - `database.md` (modo-db + modo-db-macros)
   - `jobs.md` (modo-jobs + modo-jobs-macros)
   - `email.md` (modo-email)
   - `auth-sessions.md` (modo-auth + modo-session)
   - `templates-htmx.md` (modo features: templates, sse, csrf, i18n)
   - `upload.md` (modo-upload + modo-upload-macros)
   - `tenant.md` (modo-tenant)
   - `config.md` (modo config module)
   - `testing.md` (patterns across crates)
5. Review all files for accuracy
