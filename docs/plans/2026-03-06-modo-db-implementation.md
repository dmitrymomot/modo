# modo-db Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract the database layer from the legacy monolith into `modo-db` + `modo-db-macros` crates with SQLite and Postgres support.

**Architecture:** `modo-db` is a standalone extension crate that depends on `modo` (core) for `AppState`, `ServiceRegistry`, `Error`, and `inventory`. It provides `DbPool` (newtype around `sea_orm::DatabaseConnection`), a `Db` extractor, `DatabaseConfig`, schema sync + migrations, and ID generation. `modo-db-macros` provides `#[entity]` and `#[migration]` proc macros. Integration with core is via the service registry pattern.

**Tech Stack:** SeaORM v2 RC (`schema-sync` feature), SQLite + Postgres via feature flags, `inventory` for auto-discovery, `chrono` for timestamps.

---

### Task 1: Create `modo-db-macros` crate

**Files:**
- Create: `modo-db-macros/Cargo.toml`
- Create: `modo-db-macros/src/lib.rs`
- Create: `modo-db-macros/src/entity.rs`
- Create: `modo-db-macros/src/migration.rs`

**Step 1: Create `modo-db-macros/Cargo.toml`**

```toml
[package]
name = "modo-db-macros"
version = "0.1.0"
edition = "2024"
license.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full", "extra-traits"] }
quote = "1"
proc-macro2 = "1"
```

**Step 2: Create `modo-db-macros/src/lib.rs`**

```rust
use proc_macro::TokenStream;

mod entity;
mod migration;

/// Attribute macro for declaring database entities with auto-registration.
///
/// Usage: `#[modo_db::entity(table = "users")]`
#[proc_macro_attribute]
pub fn entity(attr: TokenStream, item: TokenStream) -> TokenStream {
    entity::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Attribute macro for declaring escape-hatch migrations with auto-registration.
///
/// Usage: `#[modo_db::migration(version = 1, description = "...")]`
#[proc_macro_attribute]
pub fn migration(attr: TokenStream, item: TokenStream) -> TokenStream {
    migration::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

**Step 3: Copy and adapt `modo-db-macros/src/entity.rs`**

Copy from `_legacy/modo-macros/src/entity.rs` (740 lines). Change all generated code references in `quote!` blocks:
- `modo::sea_orm::` to `modo_db::sea_orm::`
- `modo::chrono::` to `modo_db::chrono::`
- `modo::db::generate_ulid()` to `modo_db::generate_ulid()`
- `modo::db::generate_nanoid()` to `modo_db::generate_nanoid()`
- `modo::db::EntityRegistration` to `modo_db::EntityRegistration`
- `modo::inventory::submit!` to `modo_db::inventory::submit!`

All changes are in the `quote!` blocks (generated code), NOT in the macro logic itself.

**Step 4: Copy and adapt `modo-db-macros/src/migration.rs`**

Copy from `_legacy/modo-macros/src/migration.rs` (70 lines). Change generated code references:

The `quote!` block in `expand()` becomes:

```rust
Ok(quote! {
    #func

    modo_db::inventory::submit! {
        modo_db::MigrationRegistration {
            version: #version,
            description: #description,
            handler: |db| Box::pin(#func_name(db)),
        }
    }
})
```

**Step 5: Verify it compiles**

Run: `cargo check -p modo-db-macros`
Expected: success (proc macro crate has no external deps besides syn/quote)

**Step 6: Commit**

```
feat(modo-db-macros): add entity and migration proc macros

Extracted from legacy modo-macros, adapted to reference modo_db
instead of modo for generated code paths.
```

---

### Task 2: Create `modo-db` runtime crate

**Files:**
- Create: `modo-db/Cargo.toml`
- Create: `modo-db/src/lib.rs`
- Create: `modo-db/src/config.rs`
- Create: `modo-db/src/pool.rs`
- Create: `modo-db/src/extractor.rs`
- Create: `modo-db/src/entity.rs`
- Create: `modo-db/src/migration.rs`
- Create: `modo-db/src/id.rs`
- Create: `modo-db/src/sync.rs`
- Create: `modo-db/src/connect.rs`

**Step 1: Create `modo-db/Cargo.toml`**

```toml
[package]
name = "modo-db"
version = "0.1.0"
edition = "2024"
license.workspace = true

[features]
default = ["sqlite"]
sqlite = ["sea-orm/sqlx-sqlite"]
postgres = ["sea-orm/sqlx-postgres"]

[dependencies]
modo = { path = "../modo" }
modo-db-macros = { path = "../modo-db-macros" }

sea-orm = { version = "2.0.0-rc", features = [
    "runtime-tokio-native-tls",
    "macros",
    "schema-sync",
] }

inventory = "0.3"
serde = { version = "1", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
ulid = "1"
nanoid = "0.4"
tracing = "0.1"
thiserror = "2"

[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
serde_yaml_ng = "0.10"
```

**Step 2: Create `modo-db/src/entity.rs`**

```rust
use sea_orm::schema::SchemaBuilder;

/// Registration info for a SeaORM entity, collected via `inventory`.
///
/// The `#[modo_db::entity]` macro generates an `inventory::submit!` block
/// for each entity. Framework entities (migrations, sessions)
/// register themselves identically with `is_framework: true`.
pub struct EntityRegistration {
    pub table_name: &'static str,
    pub register_fn: fn(SchemaBuilder) -> SchemaBuilder,
    pub is_framework: bool,
    pub extra_sql: &'static [&'static str],
}

inventory::collect!(EntityRegistration);
```

**Step 3: Create `modo-db/src/migration.rs`**

```rust
use std::future::Future;
use std::pin::Pin;

/// The `_modo_migrations` table tracks which migrations have been executed.
pub(crate) mod migration_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "_modo_migrations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub version: i64,
        pub description: String,
        pub executed_at: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Type alias for migration handler functions.
pub type MigrationFn =
    fn(
        &sea_orm::DatabaseConnection,
    ) -> Pin<Box<dyn Future<Output = Result<(), modo::Error>> + Send + '_>>;

/// Registration info for a migration, collected via `inventory`.
pub struct MigrationRegistration {
    pub version: u64,
    pub description: &'static str,
    pub handler: MigrationFn,
}

inventory::collect!(MigrationRegistration);

// Register _modo_migrations as a framework entity
inventory::submit! {
    crate::EntityRegistration {
        table_name: "_modo_migrations",
        register_fn: |sb| sb.register(migration_entity::Entity),
        is_framework: true,
        extra_sql: &[],
    }
}
```

**Step 4: Create `modo-db/src/id.rs`**

```rust
/// Generate a new ULID string (26 chars, Crockford Base32).
pub fn generate_ulid() -> String {
    ulid::Ulid::new().to_string()
}

/// Generate a new NanoID (21 chars, default alphabet).
pub fn generate_nanoid() -> String {
    nanoid::nanoid!()
}
```

**Step 5: Create `modo-db/src/config.rs`**

```rust
use serde::Deserialize;

/// Database configuration, deserialized from YAML via `modo::config::load()`.
///
/// Backend is auto-detected from the URL scheme (`sqlite://` or `postgres://`).
/// Irrelevant fields are silently ignored for the active backend.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// Connection URL (e.g., `sqlite://data.db?mode=rwc` or `postgres://localhost/myapp`).
    pub url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Minimum number of connections in the pool.
    pub min_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://data.db?mode=rwc".to_string(),
            max_connections: 5,
            min_connections: 1,
        }
    }
}
```

**Step 6: Create `modo-db/src/pool.rs`**

```rust
use sea_orm::DatabaseConnection;
use std::ops::Deref;

/// Newtype around `sea_orm::DatabaseConnection`.
///
/// Registered as a service via `app.service(db)` and extracted
/// in handlers via the `Db` extractor.
#[derive(Debug, Clone)]
pub struct DbPool(pub(crate) DatabaseConnection);

impl DbPool {
    /// Access the underlying SeaORM connection.
    pub fn connection(&self) -> &DatabaseConnection {
        &self.0
    }
}

impl Deref for DbPool {
    type Target = DatabaseConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
```

**Step 7: Create `modo-db/src/connect.rs`**

```rust
use crate::config::DatabaseConfig;
use crate::pool::DbPool;
use sea_orm::{ConnectOptions, Database};
use tracing::info;

/// Connect to the database using the provided configuration.
///
/// Auto-detects the backend from the URL scheme and applies
/// backend-specific settings (SQLite pragmas, Postgres pool tuning).
pub async fn connect(config: &DatabaseConfig) -> Result<DbPool, modo::Error> {
    let mut opts = ConnectOptions::new(&config.url);
    opts.max_connections(config.max_connections)
        .min_connections(config.min_connections);

    let conn = Database::connect(opts)
        .await
        .map_err(|e| modo::Error::internal(format!("Database connection failed: {e}")))?;

    // Apply backend-specific settings
    if config.url.starts_with("sqlite://") || config.url.starts_with("sqlite:") {
        apply_sqlite_pragmas(&conn).await?;
    }

    info!(url = %redact_url(&config.url), "Database connected");
    Ok(DbPool(conn))
}

#[cfg(feature = "sqlite")]
async fn apply_sqlite_pragmas(
    conn: &sea_orm::DatabaseConnection,
) -> Result<(), modo::Error> {
    use sea_orm::ConnectionTrait;

    conn.execute_unprepared("PRAGMA journal_mode=WAL").await?;
    conn.execute_unprepared("PRAGMA busy_timeout=5000").await?;
    conn.execute_unprepared("PRAGMA synchronous=NORMAL").await?;
    conn.execute_unprepared("PRAGMA foreign_keys=ON").await?;
    Ok(())
}

#[cfg(not(feature = "sqlite"))]
async fn apply_sqlite_pragmas(
    _conn: &sea_orm::DatabaseConnection,
) -> Result<(), modo::Error> {
    Err(modo::Error::internal(
        "SQLite URL provided but `sqlite` feature is not enabled",
    ))
}

/// Redact credentials from database URL for logging.
fn redact_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let prefix = &url[..scheme_end + 3];
            let suffix = &url[at_pos..];
            return format!("{prefix}***{suffix}");
        }
    }
    url.to_string()
}
```

**Step 8: Create `modo-db/src/extractor.rs`**

```rust
use crate::pool::DbPool;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use modo::app::AppState;
use modo::error::Error;

/// Axum extractor for the database connection pool.
///
/// Usage: `Db(db): Db` in handler parameters.
///
/// Requires `DbPool` to be registered via `app.service(db)`.
#[derive(Debug, Clone)]
pub struct Db(pub DbPool);

impl FromRequestParts<AppState> for Db {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        state
            .services
            .get::<DbPool>()
            .map(|pool| Db(DbPool(pool.connection().clone())))
            .ok_or_else(|| {
                Error::internal(
                    "Database not configured. Register DbPool via app.service(db).",
                )
            })
    }
}
```

**Step 9: Create `modo-db/src/sync.rs`**

```rust
use crate::entity::EntityRegistration;
use crate::migration::MigrationRegistration;
use crate::pool::DbPool;
use sea_orm::{ConnectionTrait, Schema};
use tracing::info;

/// Synchronize database schema from registered entities, then run pending migrations.
///
/// 1. Bootstrap `_modo_migrations` table (must exist before schema sync)
/// 2. Collect all `EntityRegistration` entries from `inventory`
/// 3. Register framework entities first, then user entities
/// 4. Run `SchemaBuilder::sync()` (addition-only, topo-sorted by SeaORM)
/// 5. Execute extra SQL (composite indices, partial unique indices)
/// 6. Run pending migrations (version-ordered, tracked in `_modo_migrations`)
pub async fn sync_and_migrate(db: &DbPool) -> Result<(), modo::Error> {
    let conn = db.connection();

    // 1. Bootstrap _modo_migrations
    conn.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS _modo_migrations (\
            version INTEGER PRIMARY KEY, \
            description TEXT NOT NULL, \
            executed_at TEXT NOT NULL DEFAULT (datetime('now'))\
        )",
    )
    .await?;

    // 2. Collect and register all entities
    let backend = conn.get_database_backend();
    let schema = Schema::new(backend);
    let mut builder = schema.builder();

    // Framework entities first, then user entities
    for reg in inventory::iter::<EntityRegistration> {
        if reg.is_framework {
            builder = (reg.register_fn)(builder);
        }
    }
    for reg in inventory::iter::<EntityRegistration> {
        if !reg.is_framework {
            builder = (reg.register_fn)(builder);
        }
    }

    // 3. Sync (addition-only)
    builder.sync(conn).await?;
    info!("Schema sync complete");

    // 4. Run extra SQL
    for reg in inventory::iter::<EntityRegistration> {
        for sql in reg.extra_sql {
            if let Err(e) = conn.execute_unprepared(sql).await {
                tracing::error!(
                    table = reg.table_name,
                    sql = sql,
                    error = %e,
                    "Failed to execute extra SQL for entity"
                );
                return Err(e.into());
            }
        }
    }

    // 5. Run pending migrations
    run_pending_migrations(conn).await?;

    Ok(())
}

async fn run_pending_migrations(
    db: &sea_orm::DatabaseConnection,
) -> Result<(), modo::Error> {
    use crate::migration::migration_entity;
    use sea_orm::EntityTrait;
    use std::collections::HashSet;

    let mut migrations: Vec<&MigrationRegistration> =
        inventory::iter::<MigrationRegistration>
            .into_iter()
            .collect();

    if migrations.is_empty() {
        return Ok(());
    }

    // Check for duplicate versions
    let mut seen = HashSet::new();
    for m in &migrations {
        if !seen.insert(m.version) {
            return Err(modo::Error::internal(format!(
                "Duplicate migration version: {}",
                m.version
            )));
        }
    }

    migrations.sort_by_key(|m| m.version);

    // Query already-executed versions
    let executed: Vec<migration_entity::Model> =
        migration_entity::Entity::find().all(db).await?;
    let executed_versions: HashSet<u64> =
        executed.iter().map(|m| m.version as u64).collect();

    // Run pending
    for migration in &migrations {
        if executed_versions.contains(&migration.version) {
            continue;
        }
        info!(
            "Running migration v{}: {}",
            migration.version, migration.description
        );

        (migration.handler)(db).await?;

        // Record migration as executed
        let version_i64 = i64::try_from(migration.version).map_err(|_| {
            modo::Error::internal(format!(
                "Migration version {} exceeds maximum ({})",
                migration.version,
                i64::MAX
            ))
        })?;
        let record = migration_entity::ActiveModel {
            version: sea_orm::Set(version_i64),
            description: sea_orm::Set(migration.description.to_string()),
            executed_at: sea_orm::Set(chrono::Utc::now().to_rfc3339()),
        };
        migration_entity::Entity::insert(record)
            .exec(db)
            .await?;
        info!("Migration v{} complete", migration.version);
    }

    Ok(())
}
```

**Step 10: Create `modo-db/src/lib.rs`**

```rust
pub mod config;
pub mod connect;
pub mod entity;
pub mod extractor;
pub mod id;
pub mod migration;
pub mod pool;
pub mod sync;

// Public API
pub use config::DatabaseConfig;
pub use connect::connect;
pub use entity::EntityRegistration;
pub use extractor::Db;
pub use id::{generate_nanoid, generate_ulid};
pub use migration::MigrationRegistration;
pub use pool::DbPool;
pub use sync::sync_and_migrate;

// Re-export proc macros
pub use modo_db_macros::{entity, migration};

// Re-exports for macro-generated code
pub use chrono;
pub use inventory;
pub use sea_orm;
```

**Step 11: Verify it compiles**

Run: `cargo check -p modo-db`
Expected: success

**Step 12: Commit**

```
feat(modo-db): add database runtime crate

Includes DbPool, Db extractor, DatabaseConfig, connect(),
sync_and_migrate(), entity/migration registration, and
ID generation. Supports sqlite and postgres feature flags.
```

---

### Task 3: Update workspace

**Files:**
- Modify: `Cargo.toml` (workspace root)

**Step 1: Add new crates to workspace members**

Add `"modo-db"` and `"modo-db-macros"` to the `members` list:

```toml
[workspace]
resolver = "2"
members = ["modo", "modo-macros", "modo-db", "modo-db-macros", "modo-upload", "modo-upload-macros", "examples/*"]
```

**Step 2: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: success

**Step 3: Commit**

```
chore: add modo-db and modo-db-macros to workspace
```

---

### Task 4: Write tests for `modo-db`

**Files:**
- Create: `modo-db/tests/config.rs`
- Create: `modo-db/tests/pool.rs`
- Create: `modo-db/tests/entity_macro.rs`
- Create: `modo-db/tests/migration_macro.rs`
- Create: `modo-db/tests/connect.rs`
- Create: `modo-db/tests/id.rs`

**Step 1: Write config tests** (`modo-db/tests/config.rs`)

```rust
use modo_db::DatabaseConfig;

#[test]
fn test_default_config() {
    let config = DatabaseConfig::default();
    assert_eq!(config.url, "sqlite://data.db?mode=rwc");
    assert_eq!(config.max_connections, 5);
    assert_eq!(config.min_connections, 1);
}

#[test]
fn test_config_deserialize() {
    let yaml = r#"
url: "postgres://localhost/myapp"
max_connections: 10
min_connections: 2
"#;
    let config: DatabaseConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.url, "postgres://localhost/myapp");
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 2);
}

#[test]
fn test_config_deserialize_defaults() {
    let yaml = r#"
url: "sqlite://test.db"
"#;
    let config: DatabaseConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.url, "sqlite://test.db");
    assert_eq!(config.max_connections, 5);
    assert_eq!(config.min_connections, 1);
}
```

**Step 2: Write pool tests** (`modo-db/tests/pool.rs`)

```rust
use modo_db::DbPool;

#[test]
fn test_dbpool_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<DbPool>();
    assert_sync::<DbPool>();
}

#[test]
fn test_dbpool_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<DbPool>();
}
```

**Step 3: Write entity macro tests** (`modo-db/tests/entity_macro.rs`)

Adapted from `_legacy/modo/tests/entity_macro.rs`. Key changes:
- `modo::entity` to `modo_db::entity`
- `modo::db::EntityRegistration` to `modo_db::EntityRegistration`

Full test file covers: basic entity, belongs_to, timestamps, soft_delete, composite indices, junction tables, auto ULID/NanoID, renamed_from, framework flag check.

**Step 4: Write migration macro tests** (`modo-db/tests/migration_macro.rs`)

Adapted from `_legacy/modo/tests/migration_macro.rs`. Key changes:
- `modo::migration` to `modo_db::migration`
- `modo::db::MigrationRegistration` to `modo_db::MigrationRegistration`
- `modo::error::Error` to `modo::Error`

**Step 5: Write connect + sync integration tests** (`modo-db/tests/connect.rs`)

```rust
use modo_db::DatabaseConfig;

#[tokio::test]
async fn test_connect_sqlite_in_memory() {
    let config = DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };
    let db = modo_db::connect(&config).await.unwrap();
    use sea_orm::ConnectionTrait;
    db.execute_unprepared("SELECT 1").await.unwrap();
}

#[tokio::test]
async fn test_sync_and_migrate_empty() {
    let config = DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };
    let db = modo_db::connect(&config).await.unwrap();
    modo_db::sync_and_migrate(&db).await.unwrap();

    use sea_orm::ConnectionTrait;
    db.execute_unprepared("SELECT * FROM _modo_migrations")
        .await
        .unwrap();
}
```

**Step 6: Write ID generation tests** (`modo-db/tests/id.rs`)

```rust
#[test]
fn test_generate_ulid() {
    let id = modo_db::generate_ulid();
    assert_eq!(id.len(), 26);
}

#[test]
fn test_generate_ulid_unique() {
    let a = modo_db::generate_ulid();
    let b = modo_db::generate_ulid();
    assert_ne!(a, b);
}

#[test]
fn test_generate_nanoid() {
    let id = modo_db::generate_nanoid();
    assert_eq!(id.len(), 21);
}

#[test]
fn test_generate_nanoid_unique() {
    let a = modo_db::generate_nanoid();
    let b = modo_db::generate_nanoid();
    assert_ne!(a, b);
}
```

**Step 7: Run all tests**

Run: `cargo test -p modo-db`
Expected: all tests pass

**Step 8: Commit**

```
test(modo-db): add tests for config, pool, macros, connect, sync, and ID generation
```

---

### Task 5: Run full workspace check

**Step 1: Format**

Run: `just fmt`

**Step 2: Lint**

Run: `just lint`
Expected: no warnings (fix any that appear)

**Step 3: Test all**

Run: `just test`
Expected: all tests pass across all crates

**Step 4: Commit any fixes**

```
fix(modo-db): address lint warnings
```

---

### Task 6: Build `todo-api` example

**Files:**
- Create: `examples/todo-api/Cargo.toml`
- Create: `examples/todo-api/src/main.rs`
- Create: `examples/todo-api/config/development.yaml`

**Step 1: Create `examples/todo-api/Cargo.toml`**

```toml
[package]
name = "todo-api"
version = "0.1.0"
edition = "2024"

[dependencies]
modo = { path = "../../modo" }
modo-db = { path = "../../modo-db", features = ["sqlite"] }
serde = { version = "1", features = ["derive"] }
```

**Step 2: Create `examples/todo-api/config/development.yaml`**

```yaml
port: 3001
secret_key: ${SECRET_KEY:-todo-api-dev-secret-key-change-in-production}
database:
  url: ${DATABASE_URL:-sqlite://todos.db?mode=rwc}
  max_connections: 5
  min_connections: 1
```

**Step 3: Create `examples/todo-api/src/main.rs`**

```rust
use modo::prelude::*;
use modo_db::{Db, DatabaseConfig};
use serde::{Deserialize, Serialize};

// --- Config ---

#[derive(Deserialize)]
struct AppConfig {
    #[serde(flatten)]
    server: modo::config::ServerConfig,
    database: DatabaseConfig,
}

// --- Entity ---

#[modo_db::entity(table = "todos")]
#[entity(timestamps)]
pub struct Todo {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    pub title: String,
    #[entity(default_value = false)]
    pub completed: bool,
}

// --- DTOs ---

#[derive(Deserialize)]
struct CreateTodo {
    title: String,
}

#[derive(Serialize)]
struct TodoResponse {
    id: String,
    title: String,
    completed: bool,
}

impl From<todo::Model> for TodoResponse {
    fn from(m: todo::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            completed: m.completed,
        }
    }
}

// --- Handlers ---

#[modo::handler(GET, "/todos")]
async fn list_todos(Db(db): Db) -> Result<axum::Json<Vec<TodoResponse>>, modo::Error> {
    use sea_orm::EntityTrait;
    let todos = todo::Entity::find().all(&db).await?;
    Ok(axum::Json(todos.into_iter().map(TodoResponse::from).collect()))
}

#[modo::handler(POST, "/todos")]
async fn create_todo(
    Db(db): Db,
    axum::Json(input): axum::Json<CreateTodo>,
) -> Result<axum::Json<TodoResponse>, modo::Error> {
    use sea_orm::{ActiveModelTrait, Set};
    let model = todo::ActiveModel {
        title: Set(input.title),
        ..Default::default()
    };
    let result = model.insert(&db).await?;
    Ok(axum::Json(TodoResponse::from(result)))
}

#[modo::handler(DELETE, "/todos/:id")]
async fn delete_todo(
    Db(db): Db,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, modo::Error> {
    use sea_orm::{EntityTrait, ModelTrait};
    let todo = todo::Entity::find_by_id(&id)
        .one(&db)
        .await?
        .ok_or(modo::HttpError::NotFound)?;
    todo.delete(&db).await?;
    Ok(axum::Json(serde_json::json!({"deleted": id})))
}

// --- Main ---

#[modo::main]
async fn main(app: modo::app::AppBuilder) -> Result<(), Box<dyn std::error::Error>> {
    let config: AppConfig = modo::config::load_or_default()?;
    let db = modo_db::connect(&config.database).await?;
    modo_db::sync_and_migrate(&db).await?;
    app.service(db).run().await
}
```

**Step 4: Verify it compiles**

Run: `cargo build -p todo-api`
Expected: success

**Step 5: Commit**

```
feat(examples): add todo-api example for modo-db validation
```

---

### Task 7: Final check and cleanup

**Step 1: Run full workspace check**

Run: `just check`
Expected: all green (fmt + lint + test)

**Step 2: Verify current core macros don't export entity/migration**

Check `modo-macros/src/lib.rs` does NOT export entity or migration macros (those now live in `modo-db-macros`).

**Step 3: Commit if any cleanup needed**

```
chore: final cleanup after modo-db extraction
```
