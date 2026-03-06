# Design: modo-db

> Database layer for modo, extracted as an independent extension crate.

## Status

Accepted (2026-03-06)

## Crates

- `modo-db` — runtime: connection, extractor, schema sync, migrations, ID generation
- `modo-db-macros` — proc macros: `#[entity]`, `#[migration]`

## Feature Flags

- `sqlite` (default) — SQLite backend via `sea-orm/sqlx-sqlite`
- `postgres` — PostgreSQL backend via `sea-orm/sqlx-postgres`

## Core Types

| Type | Purpose |
|------|---------|
| `DatabaseConfig` | Plain serde struct (url, pool settings). No behavior. |
| `DbPool` | Newtype around `sea_orm::DatabaseConnection` |
| `Db(pool)` | Axum extractor, pulls `DbPool` from service registry |
| `EntityRegistration` | Inventory-collected entity metadata |
| `MigrationRegistration` | Inventory-collected migration metadata |

## Public API

```rust
// Connect to database (auto-detects backend from URL scheme)
modo_db::connect(&DatabaseConfig) -> Result<DbPool, Error>

// Schema sync (from entity registrations) + run pending migrations
modo_db::sync_and_migrate(&DbPool) -> Result<(), Error>

// ID generation helpers
modo_db::generate_ulid() -> String
modo_db::generate_nanoid() -> String
```

## DatabaseConfig

Flat struct, backend-agnostic. Irrelevant fields are silently ignored (e.g., SQLite ignores SSL settings). Loaded via `modo::config::load()` from YAML with env var substitution.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,           // sqlite:// or postgres://
    pub max_connections: u32,
    pub min_connections: u32,
}
```

### YAML example

```yaml
database:
  url: ${DATABASE_URL:-sqlite://data.db?mode=rwc}
  max_connections: 5
  min_connections: 1
```

## Connection Setup

`modo_db::connect()` is a free function that:

1. Parses URL scheme to detect backend
2. Creates `sea_orm::ConnectOptions` with pool settings
3. Calls `sea_orm::Database::connect(opts).await`
4. Applies backend-specific setup:
   - **SQLite:** `PRAGMA journal_mode=WAL`, `busy_timeout=5000`, `synchronous=NORMAL`, `foreign_keys=ON`
   - **Postgres:** pool sizing, connection validation

## Db Extractor

Pulls `DbPool` from the service registry (registered via `app.service(db)`). Provides ergonomic destructuring:

```rust
pub struct Db(pub DbPool);

impl FromRequestParts<AppState> for Db {
    // internally: state.services.get::<DbPool>()
}
```

## Schema Sync & Migrations

`sync_and_migrate(&db)` combines two phases in fixed order:

1. **Schema sync** — collect `EntityRegistration` entries via `inventory`, register with SeaORM schema builder, run addition-only sync
2. **Migrations** — collect `MigrationRegistration` entries, check `_modo_migrations` table, run pending migrations in version order

Combined into one function to enforce correct ordering (entities first, then migrations).

## Entity Macro

Lives in `modo-db-macros`. Used as `#[modo_db::entity(table = "users")]`.

Struct-level: `timestamps`, `soft_delete`, `index(columns = [...], unique)`
Field-level: `primary_key`, `auto = "ulid"|"nanoid"`, `unique`, `indexed`, `column_type`, `default_value`, `nullable`, `belongs_to`, `has_many`, `has_one`, `via`, `renamed_from`

Generates: SeaORM model, relation enum, related impls, ActiveModelBehavior, inventory registration.

## Migration Macro

Lives in `modo-db-macros`. Used as `#[modo_db::migration(version = 1, description = "...")]`.

Generates inventory registration for the async migration function.

## End-User Code

```rust
use modo::prelude::*;
use modo_db::{Db, DatabaseConfig};

#[derive(Deserialize)]
struct AppConfig {
    #[serde(flatten)]
    server: ServerConfig,
    database: DatabaseConfig,
}

#[modo::main]
async fn main(app: AppBuilder) -> Result<(), Box<dyn std::error::Error>> {
    let config: AppConfig = modo::config::load()?;
    let db = modo_db::connect(&config.database).await?;
    modo_db::sync_and_migrate(&db).await?;
    app.service(db).run().await
}

#[modo_db::entity(table = "users")]
#[entity(timestamps)]
pub struct User {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    #[entity(unique)]
    pub email: String,
    pub name: String,
}

#[modo::handler(GET, "/users")]
async fn list_users(Db(db): Db) -> Result<Json<Vec<user::Model>>> {
    let users = user::Entity::find().all(&db).await?;
    Ok(Json(users))
}

#[modo_db::migration(version = 1, description = "Add display_name to users")]
async fn add_display_name(db: &DatabaseConnection) -> Result<(), Error> {
    db.execute_unprepared("ALTER TABLE users ADD COLUMN display_name TEXT").await?;
    Ok(())
}
```

## Integration Pattern

modo-db integrates with core via the existing service registry + extractor pattern. Core knows nothing about databases.

- **Registration:** `app.service(db)` stores `DbPool` in `ServiceRegistry`
- **Extraction:** `Db(db): Db` pulls from `ServiceRegistry` via `Service<DbPool>`
- **Config:** `DatabaseConfig` loaded via `modo::config::load()` (YAML + env vars)
- **Auto-discovery:** entities and migrations use `inventory` (same as routes)

## Decisions

- `DbPool` is a newtype (not raw `DatabaseConnection`) for encapsulation
- `DatabaseConfig` is plain data — no methods, `connect()` is a free function
- Single `sync_and_migrate()` — not split, enforces correct ordering
- Flat config struct — backend-specific fields silently ignored
- Both SQLite and Postgres implemented, Postgres tests gated behind env var
- Entity/migration macros in `modo-db-macros` (not in core macros)
