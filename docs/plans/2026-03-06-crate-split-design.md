# ADR: Split modo Into Core + Extension Crates

> **Priority: HIGHEST — must be implemented before any other work.**

## Status

Accepted (2026-03-06)

## Context

The current monolithic `modo` crate bundles everything: HTTP, DB, sessions, auth, jobs, templates, CSRF. Every change ripples across the entire framework. This causes:

1. **Compile time waste** — users pay for session/jobs/templates even if unused
2. **Rigidity** — users can't swap session/auth/jobs implementations or DB backends
3. **Maintenance burden** — independent features are coupled in one crate
4. **Developer fatigue** — any change requires fixes across the whole framework
5. **SQLite lock-in** — Postgres is sometimes the better choice, but the framework hardcodes SQLite

## Decision

Split modo into a minimal core crate + independent extension crates (Approach C: extension-to-extension deps, no core traits for session/auth/jobs/db).

### Core (`modo`)

Stable foundation that rarely changes. **No database dependency.**

- axum HTTP server, app builder, lifecycle (graceful shutdown)
- Cookie jar (signed + private) — a primitive, not a feature
- Service registry + `Service<T>` extractor
- Error types
- Config loading (env + .env)
- Router + `inventory` auto-discovery
- `#[handler]`, `#[main]`, `#[module]` macros
- Re-exports (axum, tokio, tracing, etc.)

### Extension Crates

| Crate | Contains | Depends on |
|---|---|---|
| `modo-db` | DB connection, `Db` extractor, entity-first migrations, schema sync, `#[entity]`/`#[migration]` macros. Features: `sqlite`, `postgres` | `modo` (core) |
| `modo-session` | Session types, store trait+impl, manager, middleware, fingerprint, device | `modo`, `modo-db` |
| `modo-auth` | `UserProvider` trait, `Auth<User>`, `OptionalAuth<User>` extractors | `modo`, `modo-session` |
| `modo-jobs` | Queue, runner, cron, job entity/store, `#[job]` macro | `modo`, `modo-db` |
| `modo-templates` | Askama integration, BaseContext, flash, HTMX helpers, `#[context]` macro | `modo` (core) |
| `modo-csrf` | CSRF double-submit cookie middleware | `modo` (core) |

### Monorepo Structure

All crates live in one Cargo workspace (standard Rust practice for related crates):

```
modo/
  Cargo.toml          # workspace root
  modo/               # core (no DB)
  modo-macros/        # proc macros (core macros only)
  modo-db/            # database layer
  modo-session/
  modo-auth/
  modo-jobs/
  modo-templates/
  modo-csrf/
```

### End-User DX

**SQLite app (recommended default):**

```toml
[dependencies]
modo = "0.1"
modo-db = { version = "0.1", features = ["sqlite"] }
modo-session = "0.1"
```

**Postgres app:**

```toml
[dependencies]
modo = "0.1"
modo-db = { version = "0.1", features = ["postgres"] }
modo-session = "0.1"
```

**No-DB app (webhook receiver, proxy, etc.):**

```toml
[dependencies]
modo = "0.1"
```

**App code:**

```rust
use modo::prelude::*;
use modo_db::Db;
use modo_session::{SessionManager, SqliteSessionStore};

#[modo::main]
async fn main(app: modo::App) {
    // DB setup — backend-specific pragmas/pool handled internally
    let db = modo_db::connect_from_env().await;
    let session_store = SqliteSessionStore::new(&db).await;

    app.service(db)
        .service(session_store)
        .middleware(modo_session::layer())
        .run()
        .await
}

#[modo::handler(GET, "/users")]
async fn list_users(Db(db): Db) -> Result<Json<Vec<User>>> {
    // works the same regardless of sqlite or postgres
}

#[modo::handler(GET, "/dashboard")]
async fn dashboard(session: SessionManager) -> Result<String> {
    let user_id: Option<String> = session.get("user_id").await?;
    match user_id {
        Some(id) => Ok(format!("Welcome back, {id}")),
        None => Ok("Not logged in".into()),
    }
}
```

Extension crates integrate via the existing service registry + middleware + extractor pattern. Core knows nothing about databases, sessions, auth, or jobs.

### modo-db Backend Details

`modo-db` uses SeaORM under the hood and exposes a unified API regardless of backend. Backend-specific behavior is behind feature flags:

- **`sqlite` feature:** WAL mode, `busy_timeout`, `synchronous=NORMAL`, `foreign_keys=ON`
- **`postgres` feature:** connection pool sizing, SSL mode, statement caching
- **Entity macros** (`#[entity]`, `#[migration]`) live in `modo-db` (or `modo-db-macros`) — not in core
- **Schema sync** works the same for both backends (SeaORM's `schema-sync` feature)

## Consequences

**Positive:**
- Core becomes a rock — almost never needs to change
- "Fix one thing, touch everything" disappears
- Users only compile what they use
- Extensions evolve and version independently
- Matches Rust ecosystem conventions (axum/axum-extra/axum-login pattern)
- Postgres support without workarounds — just change a feature flag
- No-DB apps are possible (webhooks, proxies, static servers)

**Negative:**
- Users add multiple crates to `Cargo.toml` (mitigated by clear examples)
- Extension-to-extension version compatibility must be managed
- Some proc macros (`#[entity]`, `#[job]`, `#[context]`) move to extension crates
- Two DB backends to test and maintain in modo-db

## Implementation Order

1. Extract `modo-db` (move DB out of core, add sqlite/postgres features)
2. Extract `modo-session` (depends on modo-db)
3. Extract `modo-auth` (depends on modo-session)
4. Extract `modo-jobs` (depends on modo-db)
5. Extract `modo-templates`
6. Extract `modo-csrf`
7. Clean up core — remove all DB/feature flags, leave only HTTP + cookies + services

## Example Apps

Build after each extraction step to validate the framework works end-to-end. Each example lives in `examples/<name>/` as a standalone binary with its own `Cargo.toml` (workspace member).

| # | Example | Validates | Built after |
|---|---|---|---|
| 1 | `hello` | Routes, error handling, core only — no DB | Step 7 (core cleanup) |
| 2 | `todo-api` | JSON REST CRUD, DB entities, migrations, `Db` extractor, `Service<T>` | Step 1 (modo-db) |
| 3 | `blog` | Templates, flash messages, CSRF forms, HTML rendering | Steps 5-6 (templates + csrf) |
| 4 | `auth-app` | Signup/login/logout, sessions, protected routes, password hashing | Steps 2-3 (session + auth) |
| 5 | `saas-starter` | Full stack: auth + DB + jobs + templates + CSRF — the "real app" smoke test | Step 6 (all extracted) |

### Example Descriptions

**hello** — Bare minimum. A few GET routes, a JSON error route. Proves core works standalone with zero extensions and no DB.

**todo-api** — JSON REST API. CRUD for todos with SQLite. Entity-first migrations. No auth, no templates. Validates that modo-db + core extractors + error handling work together.

**blog** — Server-rendered blog with Askama templates. Create/edit/delete posts via HTML forms with CSRF protection. Flash messages for success/error feedback. Validates modo-templates + modo-csrf integration.

**auth-app** — Login/signup/logout with session-based auth. Protected dashboard route. Session listing and revocation. Validates modo-session + modo-auth working together.

**saas-starter** — Realistic micro-SaaS skeleton. User auth, a DB-backed resource (e.g., projects), background jobs (e.g., email on signup), server-rendered UI, CSRF. The ultimate integration test — if this works cleanly, the framework is ready.
