# Session Entity-First Refactor + Feature Flags — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace trait-based session store with concrete entity + store, add granular feature flags (default = none).

**Architecture:** Session becomes a `#[modo::entity]` framework entity auto-synced at startup. `SessionStore` becomes a concrete struct with SeaORM queries. Feature flags gate session, auth, templates, cookies, jobs, and sentry independently.

**Tech Stack:** SeaORM v2 RC (entity + queries), inventory (auto-discovery), axum 0.8 (extractors/middleware)

**Design doc:** `docs/plans/2026-03-06-session-entity-feature-flags-design.md`

---

### Task 1: Feature Flags in Cargo.toml

Makes `sha2`, `ipnet`, `cookie`, `axum-extra` cookie features optional. Changes default to `[]`.

**Files:**
- Modify: `modo/Cargo.toml`

**Step 1: Update Cargo.toml**

```toml
[dependencies]
# Core (always on)
modo-macros = { path = "../modo-macros" }
axum = "0.8"
axum-extra = { version = "0.10", default-features = false }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
inventory = "0.3"
sea-orm = { version = "2.0.0-rc", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros", "schema-sync"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "registry"] }
dotenvy = "0.15"
rand = "0.9"
ulid = "1"
nanoid = "0.4"
chrono = { version = "0.4", features = ["serde"] }

# Optional deps (gated by features)
cookie = { version = "0.18", features = ["key-expansion"], optional = true }
sha2 = { version = "0.10", optional = true }
ipnet = { version = "2", optional = true }
askama = { version = "0.13", optional = true }
askama_web = { version = "0.14", features = ["axum-0.8"], optional = true }
sentry = { version = "0.46", features = ["tracing"], optional = true }
tokio-util = { version = "0.7", features = ["rt"], optional = true }
cron = { version = "0.15", optional = true }

[features]
default = []
cookies = ["dep:cookie", "axum-extra/cookie-signed", "axum-extra/cookie-private"]
session = ["cookies", "dep:sha2", "dep:ipnet"]
auth = ["session"]
templates = ["cookies", "dep:askama", "dep:askama_web"]
jobs = ["dep:tokio-util", "dep:cron"]
sentry = ["dep:sentry"]
```

Key points:
- `axum-extra` is always-on (may have non-cookie uses) but cookie features are gated via `cookies`
- `rand`, `ulid`, `nanoid` stay always-on (used by core: CSRF, request IDs, DB IDs)
- `sha2` and `ipnet` are session-only

**Step 2: Verify it compiles with no features**

Run: `cargo check --no-default-features -p modo`

This WILL fail with many errors -- expected. The remaining tasks fix each one.

**Step 3: Commit**

```
feat: restructure feature flags in Cargo.toml

Default features are now empty (core HTTP only).
New features: cookies, session, auth, templates.
Jobs default changed from on to off.
```

---

### Task 2: Gate `cookies` -- AppState and Key

Gate `cookie_key` field and `FromRef<AppState> for Key` behind `#[cfg(feature = "cookies")]`.

**Files:**
- Modify: `modo/src/app.rs` (lines 6, 16-30, 125-136)

**Step 1: Gate cookie_key in AppState**

In `modo/src/app.rs`:

```rust
// Line 6: conditional import
#[cfg(feature = "cookies")]
use axum_extra::extract::cookie::Key;

// AppState struct:
#[derive(Clone)]
pub struct AppState {
    pub db: Option<DatabaseConnection>,
    pub services: ServiceRegistry,
    pub config: AppConfig,
    #[cfg(feature = "cookies")]
    pub cookie_key: Key,
    #[cfg(feature = "session")]
    pub session_store: Option<Arc<SessionStore>>,  // will fix in Task 5
    #[cfg(feature = "jobs")]
    pub job_queue: Option<crate::jobs::JobQueue>,
}

// FromRef impl:
#[cfg(feature = "cookies")]
impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}
```

In `AppBuilder::run()`, gate cookie_key generation (lines 125-136):

```rust
#[cfg(feature = "cookies")]
let cookie_key = if self.config.secret_key.is_empty() {
    if self.config.environment == Environment::Production {
        warn!("MODO_SECRET_KEY is empty in production! ...");
    } else {
        warn!("MODO_SECRET_KEY is empty -- generating random key");
    }
    axum_extra::extract::cookie::Key::generate()
} else {
    axum_extra::extract::cookie::Key::derive_from(self.config.secret_key.as_bytes())
};
```

And in AppState construction:

```rust
let state = AppState {
    db,
    services: ServiceRegistry { services: Arc::new(services) },
    config: self.config.clone(),
    #[cfg(feature = "cookies")]
    cookie_key,
    #[cfg(feature = "session")]
    session_store: todo!(),  // Task 5 will implement
    #[cfg(feature = "jobs")]
    job_queue,
};
```

**Step 2: Verify**

Run: `cargo check --no-default-features -p modo`

Expect fewer errors (cookie-related ones resolved).

**Step 3: Commit**

```
feat: gate cookie_key behind cookies feature flag
```

---

### Task 3: Gate `session` module in lib.rs and config.rs

**Files:**
- Modify: `modo/src/lib.rs` (line 12)
- Modify: `modo/src/config.rs` (lines 15-19, 45-49, 85-125)

**Step 1: Gate session module in lib.rs**

```rust
// modo/src/lib.rs line 12
#[cfg(feature = "session")]
pub mod session;
```

**Step 2: Gate session config fields**

In `modo/src/config.rs`, apply `#[cfg(feature = "session")]` to:

Struct fields (lines 15-19):
```rust
#[cfg(feature = "session")]
pub session_ttl: Duration,
#[cfg(feature = "session")]
pub session_cookie_name: String,
#[cfg(feature = "session")]
pub session_validate_fingerprint: bool,
#[cfg(feature = "session")]
pub session_touch_interval: Duration,
#[cfg(feature = "session")]
pub trusted_proxies: Vec<ipnet::IpNet>,
```

Default impl (lines 45-49):
```rust
#[cfg(feature = "session")]
session_ttl: Duration::from_secs(30 * 24 * 60 * 60),
#[cfg(feature = "session")]
session_cookie_name: "_session".to_string(),
#[cfg(feature = "session")]
session_validate_fingerprint: true,
#[cfg(feature = "session")]
session_touch_interval: Duration::from_secs(5 * 60),
#[cfg(feature = "session")]
trusted_proxies: Vec::new(),
```

`from_env()` (lines 85-125): wrap the session fields block:
```rust
#[cfg(feature = "session")]
session_ttl: Duration::from_secs({ ... }),
#[cfg(feature = "session")]
session_cookie_name: env::var("MODO_SESSION_COOKIE_NAME")
    .unwrap_or_else(|_| "_session".to_string()),
#[cfg(feature = "session")]
session_validate_fingerprint: env::var("MODO_SESSION_VALIDATE_FINGERPRINT")
    .map(|v| v != "false" && v != "0")
    .unwrap_or(true),
#[cfg(feature = "session")]
session_touch_interval: Duration::from_secs({ ... }),
#[cfg(feature = "session")]
trusted_proxies: env::var("MODO_TRUSTED_PROXIES").unwrap_or_default()
    .split(',')
    .filter(|s| !s.trim().is_empty())
    .filter_map(|s| { ... })
    .collect(),
```

**Step 3: Verify**

Run: `cargo check --no-default-features -p modo`

**Step 4: Commit**

```
feat: gate session module and config behind session feature flag
```

---

### Task 4: Gate `templates` and `auth` modules

**Files:**
- Modify: `modo/src/lib.rs` (line 13)
- Modify: `modo/src/middleware/mod.rs`
- Modify: `modo/src/extractors/mod.rs`

**Step 1: Gate templates module**

In `modo/src/lib.rs`:
```rust
#[cfg(feature = "templates")]
pub mod templates;
```

In `modo/src/middleware/mod.rs`:
```rust
#[cfg(feature = "templates")]
pub mod csrf;
#[cfg(feature = "session")]
pub mod session;

#[cfg(feature = "templates")]
pub use csrf::{CsrfToken, csrf_protection};
#[cfg(feature = "session")]
pub use session::session;
```

**Step 2: Gate auth extractor**

In `modo/src/extractors/mod.rs`:
```rust
#[cfg(feature = "auth")]
pub mod auth;
pub mod db;
pub mod service;
```

**Step 3: Verify**

Run: `cargo check --no-default-features -p modo`

**Step 4: Commit**

```
feat: gate templates, csrf, and auth behind feature flags
```

---

### Task 5: Session Entity Definition

Create the SeaORM entity for `_modo_sessions` following the jobs entity pattern (manual SeaORM derives + `inventory::submit!`).

**Files:**
- Create: `modo/src/session/entity.rs`
- Modify: `modo/src/session/mod.rs`

**Step 1: Create session entity**

```rust
// modo/src/session/entity.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "_modo_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique, indexed)]
    pub token: String,
    #[sea_orm(indexed)]
    pub user_id: String,
    pub ip_address: String,
    pub user_agent: String,
    pub device_name: String,
    pub device_type: String,
    pub fingerprint: String,
    #[sea_orm(column_type = "Text")]
    pub data: String,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

inventory::submit! {
    crate::db::EntityRegistration {
        table_name: "_modo_sessions",
        register_fn: |sb| sb.register(Entity),
        is_framework: true,
        extra_sql: &[],
    }
}
```

Note: Using `String` for timestamps (like the jobs entity does) because SQLite stores dates as text. The store will handle conversion to/from `chrono::DateTime<Utc>`.

**Step 2: Add to session mod.rs**

```rust
// modo/src/session/mod.rs
pub mod entity;
// ... existing modules ...
```

**Step 3: Verify entity compiles**

Run: `cargo check --features session -p modo`

**Step 4: Commit**

```
feat: add session entity for _modo_sessions table
```

---

### Task 6: Concrete SessionStore

Replace the trait-based store with a concrete struct using SeaORM queries.

**Files:**
- Rewrite: `modo/src/session/store.rs`
- Modify: `modo/src/session/types.rs` (add `SessionId::from_raw`)

**Step 1: Add `from_raw` to SessionId**

In `modo/src/session/types.rs`, add to `impl SessionId`:
```rust
pub(crate) fn from_raw(s: String) -> Self {
    Self(s)
}
```

**Step 2: Write concrete SessionStore**

Replace entire `modo/src/session/store.rs`:

```rust
use crate::error::Error;
use crate::session::entity::{self, ActiveModel, Column, Entity};
use crate::session::meta::SessionMeta;
use crate::session::types::{SessionData, SessionId, SessionToken};
use chrono::Utc;
use sea_orm::*;
use std::time::Duration;

pub struct SessionStore {
    db: DatabaseConnection,
    ttl: Duration,
}

impl SessionStore {
    pub fn new(db: DatabaseConnection, ttl: Duration) -> Self {
        Self { db, ttl }
    }

    pub async fn create(
        &self,
        user_id: &str,
        meta: &SessionMeta,
    ) -> Result<SessionData, Error> {
        self.create_with(user_id, meta, serde_json::json!({})).await
    }

    pub async fn create_with(
        &self,
        user_id: &str,
        meta: &SessionMeta,
        data: serde_json::Value,
    ) -> Result<SessionData, Error> {
        let id = SessionId::new();
        let token = SessionToken::generate();
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(self.ttl)
                .unwrap_or_else(|_| chrono::Duration::hours(1));

        let model = ActiveModel {
            id: Set(id.as_str().to_string()),
            token: Set(token.as_str().to_string()),
            user_id: Set(user_id.to_string()),
            ip_address: Set(meta.ip_address.clone()),
            user_agent: Set(meta.user_agent.clone()),
            device_name: Set(meta.device_name.clone()),
            device_type: Set(meta.device_type.clone()),
            fingerprint: Set(meta.fingerprint.clone()),
            data: Set(serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())),
            expires_at: Set(expires_at.to_rfc3339()),
            created_at: Set(now.to_rfc3339()),
            updated_at: Set(now.to_rfc3339()),
        };

        Entity::insert(model).exec(&self.db).await?;

        Ok(SessionData {
            id,
            token,
            user_id: user_id.to_string(),
            ip_address: meta.ip_address.clone(),
            user_agent: meta.user_agent.clone(),
            device_name: meta.device_name.clone(),
            device_type: meta.device_type.clone(),
            fingerprint: meta.fingerprint.clone(),
            data,
            created_at: now,
            last_active_at: now,
            expires_at,
        })
    }

    pub async fn read(&self, id: &SessionId) -> Result<Option<SessionData>, Error> {
        let model = Entity::find_by_id(id.as_str().to_string())
            .one(&self.db)
            .await?;
        Ok(model.map(model_to_session_data))
    }

    pub async fn read_by_token(
        &self,
        token: &SessionToken,
    ) -> Result<Option<SessionData>, Error> {
        let model = Entity::find()
            .filter(Column::Token.eq(token.as_str()))
            .one(&self.db)
            .await?;
        Ok(model.map(model_to_session_data))
    }

    pub async fn touch(&self, id: &SessionId, ttl: Duration) -> Result<(), Error> {
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(ttl)
                .unwrap_or_else(|_| chrono::Duration::hours(1));

        Entity::update_many()
            .filter(Column::Id.eq(id.as_str()))
            .col_expr(Column::UpdatedAt, Expr::value(now.to_rfc3339()))
            .col_expr(Column::ExpiresAt, Expr::value(expires_at.to_rfc3339()))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn update_data(
        &self,
        id: &SessionId,
        data: serde_json::Value,
    ) -> Result<(), Error> {
        let json = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        Entity::update_many()
            .filter(Column::Id.eq(id.as_str()))
            .col_expr(Column::Data, Expr::value(json))
            .col_expr(Column::UpdatedAt, Expr::value(Utc::now().to_rfc3339()))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn update_token(
        &self,
        id: &SessionId,
        new_token: &SessionToken,
    ) -> Result<(), Error> {
        Entity::update_many()
            .filter(Column::Id.eq(id.as_str()))
            .col_expr(Column::Token, Expr::value(new_token.as_str()))
            .col_expr(Column::UpdatedAt, Expr::value(Utc::now().to_rfc3339()))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn destroy(&self, id: &SessionId) -> Result<(), Error> {
        Entity::delete_by_id(id.as_str().to_string())
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn destroy_all_for_user(&self, user_id: &str) -> Result<(), Error> {
        Entity::delete_many()
            .filter(Column::UserId.eq(user_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn destroy_all_except(
        &self,
        user_id: &str,
        except_id: &SessionId,
    ) -> Result<(), Error> {
        Entity::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.ne(except_id.as_str()))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired(&self) -> Result<u64, Error> {
        let result = Entity::delete_many()
            .filter(Column::ExpiresAt.lt(Utc::now().to_rfc3339()))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}

fn model_to_session_data(m: entity::Model) -> SessionData {
    SessionData {
        id: SessionId::from_raw(m.id),
        token: SessionToken::from_raw(m.token),
        user_id: m.user_id,
        ip_address: m.ip_address,
        user_agent: m.user_agent,
        device_name: m.device_name,
        device_type: m.device_type,
        fingerprint: m.fingerprint,
        data: serde_json::from_str(&m.data).unwrap_or(serde_json::json!({})),
        created_at: chrono::DateTime::parse_from_rfc3339(&m.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_active_at: chrono::DateTime::parse_from_rfc3339(&m.updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        expires_at: chrono::DateTime::parse_from_rfc3339(&m.expires_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    }
}
```

**Step 3: Verify**

Run: `cargo check --features session -p modo`

**Step 4: Commit**

```
feat: implement concrete SessionStore with SeaORM queries

Replaces SessionStore trait, SessionStoreDyn trait, and blanket impl
with a concrete struct that queries the _modo_sessions entity directly.
```

---

### Task 7: Update SessionManager and Middleware

Change `Arc<dyn SessionStoreDyn>` to `Arc<SessionStore>` (concrete).

**Files:**
- Modify: `modo/src/session/manager.rs` (lines 4, 23)
- Modify: `modo/src/session/mod.rs`
- Modify: `modo/src/middleware/session.rs` (line 33)
- Modify: `modo/src/app.rs` (lines 3, 21, 65, 82-85, 145-148, 208)
- Modify: `modo/src/extractors/auth.rs` (lines 159, 201)

**Step 1: Update SessionManager internals**

In `modo/src/session/manager.rs`:

Line 4: Change import:
```rust
use crate::session::store::SessionStore;
```

Line 23: Change field type:
```rust
pub struct SessionManagerState {
    pub action: Arc<Mutex<SessionAction>>,
    pub meta: SessionMeta,
    pub store: Arc<SessionStore>,  // was Arc<dyn SessionStoreDyn>
    pub current_session: Option<SessionData>,
}
```

All method calls on `self.state.store` stay identical (same method names).

**Step 2: Update session mod.rs exports**

```rust
// modo/src/session/mod.rs
mod device;
pub mod entity;
mod fingerprint;
pub(crate) mod manager;
mod meta;
mod store;
mod types;

pub(crate) use device::{parse_device_name, parse_device_type};
pub(crate) use fingerprint::compute_fingerprint;
pub use manager::SessionManager;
pub use meta::SessionMeta;
pub use store::SessionStore;
pub use types::{SessionData, SessionId, SessionToken};
```

Removed: `SessionStoreDyn` export (deleted).

**Step 3: Update app.rs**

Remove `SessionStore` trait import, `SessionStoreDyn` import. Change store type:

```rust
// Line 3: remove old import, add new
#[cfg(feature = "session")]
use crate::session::SessionStore;

// AppState:
#[cfg(feature = "session")]
pub session_store: Option<Arc<SessionStore>>,

// AppBuilder: remove session_store field and .session_store() method entirely
```

In `AppBuilder::run()`, auto-wire session store (replace lines 145-148):

```rust
#[cfg(feature = "session")]
let session_store = db.as_ref().map(|db| {
    Arc::new(SessionStore::new(db.clone(), self.config.session_ttl))
});
#[cfg(feature = "session")]
if session_store.is_some() {
    info!("Session store auto-initialized");
}
```

And in AppState construction:
```rust
#[cfg(feature = "session")]
session_store,
```

Remove the `session_store` field from `AppBuilder` and the `.session_store()` method (lines 65, 74, 82-85).

**Step 4: Update auth extractors**

In `modo/src/extractors/auth.rs`, the `state.session_store` references (lines 159, 201) are fine -- the whole file is already gated under `#[cfg(feature = "auth")]` which implies `session`.

**Step 5: Update middleware/session.rs**

Line 33-34: `state.session_store` type is now `Option<Arc<SessionStore>>` -- no code change needed, just the type flows through.

**Step 6: Verify with session feature**

Run: `cargo check --features session -p modo`

**Step 7: Commit**

```
feat: wire concrete SessionStore through manager, middleware, and app

Remove SessionStoreDyn, remove .session_store() builder method.
Session store auto-wired from DB connection when session feature is on.
```

---

### Task 8: Update Proc Macros for Feature Flags

Gate session and auth references in generated code.

**Files:**
- Modify: `modo-macros/src/context.rs` (lines 112-132)

**Step 1: Gate session extraction in context macro**

In `modo-macros/src/context.rs`, change lines 126-132:

```rust
// Generate session extraction code
let session_extraction = if let Some(session_name) = &session_field {
    quote! {
        #[cfg(feature = "session")]
        let #session_name = parts.extensions.get::<modo::session::SessionData>().cloned();
        #[cfg(not(feature = "session"))]
        let #session_name: Option<()> = None;
    }
} else {
    quote! {}
};
```

Also gate the user extraction (which uses `OptionalAuth` from auth module), lines 112-123:

```rust
let user_extraction = if let (Some(user_name), Some(inner_ty)) =
    (&user_field, &user_inner_type)
{
    quote! {
        #[cfg(feature = "auth")]
        let #user_name = match modo::extractors::auth::OptionalAuth::<#inner_ty>::from_request_parts(parts, state).await {
            Ok(modo::extractors::auth::OptionalAuth(Some(auth_data))) => Some(auth_data.user),
            Ok(modo::extractors::auth::OptionalAuth(None)) => None,
            Err(never) => match never {},
        };
        #[cfg(not(feature = "auth"))]
        let #user_name: Option<#inner_ty> = None;
    }
} else {
    quote! {}
};
```

**Step 2: Verify**

Run: `cargo check --features "auth,templates" -p modo`

**Step 3: Commit**

```
feat: gate session and auth extraction in context macro
```

---

### Task 9: Update Integration Tests

Gate session tests behind feature flags, rewrite to use `:memory:` SQLite.

**Files:**
- Modify: `modo/tests/integration.rs`

**Step 1: Gate and rewrite**

Wrap the `session_integration` module:

```rust
#[cfg(all(feature = "session", feature = "auth"))]
mod session_integration {
    // ... entire module ...
}
```

Update `build_test_router()` (lines 18-37):

```rust
fn build_test_router() -> axum::Router {
    let state = AppState {
        db: None,
        services: Default::default(),
        config: modo::config::AppConfig::default(),
        #[cfg(feature = "cookies")]
        cookie_key: axum_extra::extract::cookie::Key::generate(),
        #[cfg(feature = "session")]
        session_store: None,
        #[cfg(feature = "jobs")]
        job_queue: None,
    };
    // ... rest unchanged ...
}
```

Inside `session_integration` module, replace `MemoryStore` with `:memory:` SQLite:

```rust
use sea_orm::{Database, DatabaseConnection, ConnectionTrait};

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys=ON").await.unwrap();
    modo::db::sync_and_migrate(&db).await.unwrap();
    db
}
```

Replace `build_session_router` to be async and use real DB:

```rust
async fn build_session_router() -> (axum::Router, AppState) {
    let db = setup_test_db().await;
    let config = modo::config::AppConfig {
        session_validate_fingerprint: true,
        ..Default::default()
    };
    let store = Arc::new(modo::session::SessionStore::new(
        db.clone(),
        config.session_ttl,
    ));
    let services =
        ServiceRegistry::new().with(UserProviderService::<TestUser>::new(TestUserProvider));
    let state = AppState {
        db: Some(db),
        services,
        config,
        cookie_key: axum_extra::extract::cookie::Key::generate(),
        session_store: Some(store),
        #[cfg(feature = "jobs")]
        job_queue: None,
    };

    let router = axum::Router::new()
        .route("/login", post(login_handler))
        .route("/check", get(check_handler))
        .route("/auth", get(auth_handler))
        .route("/optional-auth", get(optional_auth_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            modo::middleware::session::session,
        ))
        .with_state(state.clone());

    (router, state)
}
```

Update each test to call `build_session_router().await`:

```rust
#[tokio::test]
async fn session_authenticate_sets_cookie() {
    let (app, _) = build_session_router().await;
    // ... rest of test unchanged ...
}
```

Delete `MemoryStore`, `FailingReadStore`, and their impls (lines 117-305).

Delete the `session_transient_db_error_preserves_cookie` test -- `FailingReadStore` no longer exists, and transient DB errors are a middleware concern, not store concern.

For `session_expired_removes_stale_cookie`: use raw SQL instead of in-memory mutation:

```rust
#[tokio::test]
async fn session_expired_removes_stale_cookie() {
    let (app, state) = build_session_router().await;
    let db = state.db.as_ref().unwrap();

    // Login
    let response = app.clone().oneshot(/* login request */).await.unwrap();
    let cookie = extract_session_cookie(&response, COOKIE_NAME).unwrap();

    // Expire all sessions in the DB
    db.execute_unprepared(
        "UPDATE \"_modo_sessions\" SET expires_at = datetime('now', '-1 hour')"
    ).await.unwrap();

    // Request with expired cookie
    let response = app.oneshot(/* check request with cookie */).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"no_session");
}
```

For `session_fingerprint_mismatch_destroys_session`: verify via DB query instead of accessing store internals:

```rust
// Verify session destroyed in DB
let count: u64 = modo::session::entity::Entity::find()
    .filter(modo::session::entity::Column::Id.eq(session_id.as_str()))
    .count(db)
    .await
    .unwrap();
assert_eq!(count, 0, "session should be destroyed after fingerprint mismatch");
```

**Step 2: Also update SessionManager unit tests**

In `modo/src/session/manager.rs` (lines 299-890): Replace `MemoryStore` with `:memory:` SQLite + `sync_and_migrate()`. Replace `Arc<dyn SessionStoreDyn>` with `Arc<SessionStore>`. Update `make_manager()` to accept `Arc<SessionStore>`.

The `FailingDestroyStore` test (around line 870) should be deleted -- it was testing the manager's behavior when destroy fails, which is a store-level concern now tested via the real store.

**Step 3: Verify all tests pass with session feature**

Run: `cargo test --features "session,auth" -p modo`

**Step 4: Verify tests pass without session feature**

Run: `cargo test --no-default-features -p modo`

**Step 5: Commit**

```
test: rewrite session tests to use :memory: SQLite

Remove MemoryStore and FailingReadStore test helpers.
All session tests now use real SQLite with sync_and_migrate().
Tests gated behind session/auth feature flags.
```

---

### Task 10: Verify All Feature Combinations

**Files:** None (verification only)

**Step 1: Check each combination compiles**

```
cargo check --no-default-features -p modo
cargo check --features session -p modo
cargo check --features auth -p modo
cargo check --features templates -p modo
cargo check --features jobs -p modo
cargo check --features sentry -p modo
cargo check --features "auth,templates" -p modo
cargo check --features "auth,templates,jobs" -p modo
cargo check --all-features -p modo
```

Each must succeed with no errors.

**Step 2: Run tests for each**

```
cargo test --no-default-features -p modo
cargo test --features session -p modo
cargo test --features auth -p modo
cargo test --features "auth,templates,jobs" -p modo
cargo test --all-features -p modo
```

**Step 3: Run full check**

```
just fmt
just check
```

**Step 4: Commit any fixes**

```
fix: resolve feature flag compilation issues
```

---

### Task 11: Update CLAUDE.md and Documentation

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update CLAUDE.md**

Add to **Key Decisions**:
- Feature flags: `default = []` (core HTTP only). Enable session, auth, templates, jobs, sentry as needed.
- Session store: concrete `SessionStore` struct with SeaORM queries (no trait abstraction)
- Session entity: framework entity `_modo_sessions`, auto-synced at startup
- Session auto-wiring: store created automatically when session feature + DB available

Update **Conventions**:
- Feature flags: session, auth, templates, cookies (internal), jobs, sentry
- Session entity: `modo::session::entity::{Entity, Column, Model}` -- queryable with SeaORM
- Session store: `modo::session::SessionStore` -- concrete struct, not a trait

Update **Gotchas**:
- When adding fields to AppState, gate them with `#[cfg(feature = "...")]` and update `modo/tests/integration.rs`
- `axum-extra` has `default-features = false`; cookie features enabled via `cookies` feature flag
- `rand`, `ulid`, `nanoid` are always-on (used by core, not feature-gated)

**Step 2: Commit**

```
docs: update CLAUDE.md for feature flags and concrete session store
```

---

## Task Dependencies

```
Task 1 (Cargo.toml) --+---> Task 2 (cookies gate)
                       +---> Task 3 (session/config gate)
                       +---> Task 4 (templates/auth gate)
                              |
Task 5 (entity) ------------> Task 6 (concrete store)
                                     |
Task 2 + Task 3 + Task 4 + Task 6 -> Task 7 (wire everything)
                                     |
                                     +---> Task 8 (proc macros)
                                     +---> Task 9 (tests)
                                              |
                                              +---> Task 10 (verify)
                                                       |
                                                       +---> Task 11 (docs)
```

Tasks 2, 3, 4 can run in parallel after Task 1.
Tasks 5, 6 can run in parallel with Tasks 2-4 (independent).
Task 7 needs everything before it.
Tasks 8, 9 can run in parallel after Task 7.
Task 10 needs everything.
