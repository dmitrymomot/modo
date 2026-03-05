# Session Entity-First Refactor + Feature Flags

## Overview

Replace the trait-based session store with a concrete `#[modo::entity]` session entity and `SessionStore` struct. Add granular feature flags so different app types (web, API, worker) only pull in what they need. Default features: none (core HTTP only).

## Feature Flag Map

```toml
[features]
default = []

# Internal feature (enabled by session or templates)
cookies = []  # axum-extra cookie features, cookie crate

# User-facing
session = ["cookies"]
auth = ["session"]
templates = ["cookies", "dep:askama", "dep:askama_web"]
jobs = ["dep:tokio-util", "dep:cron"]
sentry = ["dep:sentry"]
```

Dependency graph:
```
auth -> session -> cookies
templates -> cookies
jobs (independent)
sentry (independent)
```

### What Each Feature Gates

| Feature | Modules/Files | Deps Made Optional |
|---------|--------------|-------------------|
| `cookies` | `cookie_key` in AppState, `FromRef<AppState> for Key` | `cookie` (key-expansion), `axum-extra` cookie features |
| `session` | `session/` module, `middleware/session.rs`, session config fields, session entity | `sha2`, `ipnet` |
| `auth` | `extractors/auth.rs` (Auth, OptionalAuth, UserProvider) | (none beyond session) |
| `templates` | `templates/` module (flash, BaseContext, HTMX), `middleware/csrf.rs` | `askama`, `askama_web` |
| `jobs` | `jobs/` module | `tokio-util`, `cron` |
| `sentry` | sentry re-export, sentry config fields | `sentry` |

### Always-On Core

axum, tokio, tower, tracing, sea-orm, inventory, serde, serde_json, thiserror, anyhow, dotenvy, rand, ulid, nanoid, chrono.

Modules: `app`, `config`, `db`, `error`, `router`, `extractors/db.rs`, `extractors/service.rs`.

### Typical Usage

- Web app: `features = ["auth", "templates", "jobs"]`
- API server: `features = ["auth"]`
- Worker: `features = ["jobs"]`
- Minimal HTTP: `default-features = false` (or just `features = []`)

## Session Entity

Replaces the `SessionData` struct as the source of truth. Uses `#[modo::entity]` macro for auto-discovery and schema sync.

```rust
#[modo::entity(table = "_modo_sessions")]
#[entity(timestamps)]  // adds created_at, updated_at
pub struct Session {
    #[entity(primary_key, auto_increment = false)]
    pub id: String,              // ULID

    #[entity(indexed, unique)]
    pub token: String,           // 64-char hex, cookie lookup key

    #[entity(indexed)]
    pub user_id: String,

    pub ip_address: String,
    pub user_agent: String,
    pub device_name: String,
    pub device_type: String,
    pub fingerprint: String,

    #[entity(column_type = "Text")]
    pub data: String,            // JSON blob

    pub expires_at: DateTimeUtc,
    // created_at, updated_at auto-added by timestamps
}
```

Key decisions:
- `updated_at` replaces `last_active_at` (same semantic, no extra column)
- `data` is `String` in DB, converted to/from `serde_json::Value` in store methods
- Registers as framework entity (`is_framework: true`)
- `SessionId` and `SessionToken` newtypes stay in `types.rs` (domain types, not DB types)

## Concrete SessionStore

Replaces `SessionStore` trait + `SessionStoreDyn` trait + blanket impl.

```rust
pub struct SessionStore {
    db: DatabaseConnection,
    ttl: Duration,
}

impl SessionStore {
    pub fn new(db: DatabaseConnection, ttl: Duration) -> Self;

    pub async fn create(&self, user_id: &str, meta: &SessionMeta) -> Result<SessionData, Error>;
    pub async fn create_with(&self, user_id: &str, meta: &SessionMeta, data: Value) -> Result<SessionData, Error>;
    pub async fn read(&self, id: &SessionId) -> Result<Option<SessionData>, Error>;
    pub async fn read_by_token(&self, token: &SessionToken) -> Result<Option<SessionData>, Error>;
    pub async fn touch(&self, id: &SessionId, ttl: Duration) -> Result<(), Error>;
    pub async fn update_data(&self, id: &SessionId, data: Value) -> Result<(), Error>;
    pub async fn update_token(&self, id: &SessionId, new_token: &SessionToken) -> Result<(), Error>;
    pub async fn destroy(&self, id: &SessionId) -> Result<(), Error>;
    pub async fn destroy_all_for_user(&self, user_id: &str) -> Result<(), Error>;
    pub async fn destroy_all_except(&self, user_id: &str, except_id: &SessionId) -> Result<(), Error>;
    pub async fn cleanup_expired(&self) -> Result<u64, Error>;
}
```

Each method uses SeaORM queries against the session entity.

## What Gets Deleted

- `SessionStore` trait (63 lines)
- `SessionStoreDyn` trait (60 lines)
- Blanket `impl<T: SessionStore> SessionStoreDyn for T` (78 lines)
- `MemoryStore` test helper (~100 lines)
- All `Arc<dyn SessionStoreDyn>` casts in tests (~20 occurrences)
- `.session_store()` builder method on `AppBuilder` (auto-wired instead)

Total: ~350 lines of abstraction plumbing removed.

## What Gets Added

- Session entity definition (~30 lines)
- Concrete `SessionStore` with SeaORM queries (~170 lines)
- `impl From<session::Model> for SessionData` conversion (~20 lines)
- `#[cfg]` annotations across the codebase

Net: ~150 fewer lines, simpler code, full SeaORM queryability.

## Auto-Wiring

In `AppBuilder::run()`, when `session` feature is enabled and DB is available:

```rust
#[cfg(feature = "session")]
let session_store = db.as_ref().map(|db| {
    Arc::new(SessionStore::new(db.clone(), config.session_ttl))
});
```

No manual `.session_store()` call needed. Session entity syncs via `sync_and_migrate()` (handles framework entities).

## User-Facing Queries

Because session is a real SeaORM entity, users can query it directly:

```rust
use modo::session::Entity as Session;
use modo::session::Column;

let sessions = Session::find()
    .filter(Column::UserId.eq("user123"))
    .filter(Column::ExpiresAt.gt(Utc::now()))
    .order_by_desc(Column::UpdatedAt)
    .limit(10)
    .all(&db)
    .await?;
```

## Impact on Existing Code

### SessionManager (minimal changes)
- `store: Arc<dyn SessionStoreDyn>` -> `store: Arc<SessionStore>` (concrete type)
- All public API unchanged
- Gated under `#[cfg(feature = "session")]`

### Session Middleware (minimal changes)
- Same type change for store reference
- Gated under `#[cfg(feature = "session")]`

### Auth Extractors
- Gated under `#[cfg(feature = "auth")]`
- Still reads `SessionData` from extensions (no direct store dependency)

### Templates/CSRF
- `middleware/csrf.rs` -> `#[cfg(feature = "templates")]`
- `templates/` module -> `#[cfg(feature = "templates")]`

### Config
- Session fields (`session_ttl`, `session_cookie_name`, etc.) gated under `#[cfg(feature = "session")]`
- `trusted_proxies` gated under `#[cfg(feature = "session")]`
- `secret_key` stays always-on in config; `cookie_key: Key` in AppState gated under `#[cfg(feature = "cookies")]`

### Proc Macros
- `#[modo::main]` must emit `#[cfg(feature = "session")]` / `#[cfg(not(feature = "session"))]` branches for session middleware registration
- `#[modo::context]` must emit conditional branches for session/auth fields

## Testing Strategy

- Session store tests use `:memory:` SQLite (real DB, no mocks)
- CI must verify feature combinations compile:
  - `cargo check --no-default-features`
  - `cargo check --features session`
  - `cargo check --features auth`
  - `cargo check --features templates`
  - `cargo check --features "auth,templates,jobs"`
  - `cargo check --all-features`
- `just check` updated to verify minimum set of combinations

## Cleanup Job

- When both `session` and `jobs` features are enabled, wire `cleanup_expired()` as a periodic cron job
- When only `session` is enabled, expired sessions filtered out at query time (`expires_at > now`), accumulate in DB until manual cleanup or user-defined cron
