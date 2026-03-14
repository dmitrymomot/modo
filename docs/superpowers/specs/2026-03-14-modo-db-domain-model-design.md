# modo-db Domain Model Refactoring — Design Spec

**Date:** 2026-03-14
**Version:** 0.2.0 → 0.3.0 (breaking change)
**Scope:** `modo-db`, `modo-db-macros`, examples, CLI templates, documentation

## Problem

The current `#[modo_db::entity]` macro consumes the user's struct entirely, replacing it with a SeaORM module (`pub mod user { Model, Entity, ActiveModel, ... }`). This means:

- Users define a struct but never use it — it's only input for code generation
- All handlers require SeaORM trait imports (`EntityTrait`, `ActiveModelTrait`, `Set`, etc.)
- No way to add custom domain methods to the model type
- No lifecycle hooks beyond what the macro generates
- SeaORM internals leak into application code

## Solution

Preserve the user's struct as the primary domain type. Generate the SeaORM module alongside it with `From`/`Into` conversions. Provide a `Record` trait with default CRUD implementations so 80% of database operations require zero SeaORM imports.

## Architecture

Three layers with clear responsibilities:

```
+----------------------------------------------+
|  User Code                                   |
|  - struct User { ... }                       |
|  - impl User { custom domain methods }       |
|  - fn before_save / after_save / before_delete|
+----------------------------------------------+
|  Proc Macro (modo-db-macros)                 |
|  - Generates SeaORM module (user::*)         |
|  - Generates From/Default impls              |
|  - Generates impl Record for User            |
|  - Generates relation accessors              |
|  - Generates inventory registration          |
+----------------------------------------------+
|  Runtime Library (modo-db)                   |
|  - Record trait + default CRUD impls         |
|  - EntityQuery<T,E> wrapper                  |
|  - EntityUpdateMany<E>, EntityDeleteMany<E>  |
|  - DefaultHooks blanket trait                |
|  - CRUD helper functions (do_insert, etc.)   |
|  - db_err_to_error() conversion helper       |
|  - Pagination, connection, sync (unchanged)  |
+----------------------------------------------+
```

### What changes

- `modo-db-macros`: entity macro rewritten to preserve user's struct + generate `Record` impl
- `modo-db`: new `Record` trait, query wrappers, `DefaultHooks` blanket trait, CRUD helpers, `db_err_to_error()`

### What stays unchanged

- `DatabaseConfig`, `connect()`, `DbPool`, `Db` extractor
- `sync_and_migrate()`, `sync_and_migrate_group()`, `EntityRegistration`, `inventory::submit!`
- `MigrationRegistration`, `#[migration]` macro
- `generate_ulid()`, `generate_nanoid()`
- `PageParams`, `PageResult`, `CursorParams`, `CursorResult`, `paginate()`, `paginate_cursor()`
- Feature flags (`sqlite`, `postgres`)

### What's deleted

- The current pattern of consuming the user's struct
- Soft-delete module-level functions (replaced by `Record` methods)
- `ActiveModelBehavior` logic (moved to `Record::apply_auto_fields`)

## Record Trait

Core of the runtime library. Generic CRUD lives here as default implementations:

```rust
/// Blanket trait providing default no-op lifecycle hooks for all types.
/// Inherent methods on the user's struct take priority over these defaults.
pub trait DefaultHooks {
    fn before_save(&mut self) -> Result<(), Error> { Ok(()) }
    fn after_save(&self) -> Result<(), Error> { Ok(()) }
    fn before_delete(&self) -> Result<(), Error> { Ok(()) }
}
impl<T> DefaultHooks for T {}

pub trait Record: Sized + Send + Sync {
    type Entity: EntityTrait;
    type ActiveModel: ActiveModelTrait<Entity = Self::Entity> + Send;

    // --- Required (macro generates) ---
    fn from_model(model: <Self::Entity as EntityTrait>::Model) -> Self;
    fn into_active_model_full(&self) -> Self::ActiveModel;  // all fields Set
    fn into_active_model(&self) -> Self::ActiveModel;       // PK Set, rest NotSet
    fn apply_auto_fields(am: &mut Self::ActiveModel, is_insert: bool);

    // --- CRUD (macro-generated — calls hooks in concrete context) ---
    // find_by_id, delete_by_id: macro-generated (PK type varies)
    // insert, update, delete: macro-generated (hook calls need concrete type)
    async fn find_by_id(/* PK type */, db: &impl ConnectionTrait) -> Result<Self, Error>;  // 404 if not found
    async fn delete_by_id(/* PK type */, db: &impl ConnectionTrait) -> Result<(), Error>; // 404 if not found
    async fn insert(self, db: &impl ConnectionTrait) -> Result<Self, Error>;              // 409 on duplicate
    async fn update(&mut self, db: &impl ConnectionTrait) -> Result<(), Error>;           // 404 if gone
    async fn delete(self, db: &impl ConnectionTrait) -> Result<(), Error>;

    // --- Query builders + find_all (default impls — no hooks needed) ---
    async fn find_all(db: &impl ConnectionTrait) -> Result<Vec<Self>, Error>;  // empty Vec if none
    fn query() -> EntityQuery<Self, Self::Entity>;
    // query().one() -> Result<Option<Self>, Error>  (absence is valid)
    // query().all() -> Result<Vec<Self>, Error>     (empty Vec is valid)
    fn update_many() -> EntityUpdateMany<Self::Entity>;
    fn delete_many() -> EntityDeleteMany<Self::Entity>;
}

// Shared CRUD logic in modo-db (called by macro-generated methods):
pub async fn do_insert<T: Record>(record: T, db: &impl ConnectionTrait) -> Result<T, Error>;
pub async fn do_update<T: Record>(record: &T, db: &impl ConnectionTrait) -> Result<T, Error>;
pub async fn do_delete<T: Record>(record: T, db: &impl ConnectionTrait) -> Result<(), Error>;
```

**Why `insert`/`update`/`delete` are macro-generated, not default impls:**

Hook calls (`self.before_save()`) use Rust's inherent method priority, which only works when the concrete type is known. Inside a default trait method, `Self` is generic — the compiler can't see inherent methods. The macro generates these methods per entity in `impl Record for User { ... }` where the type is concrete, so inherent methods are resolved correctly.

The actual DB logic lives in shared helper functions (`do_insert`, `do_update`, `do_delete`) in modo-db. The macro-generated methods are thin wrappers:

```rust
// Macro generates for User:
async fn insert(mut self, db: &impl ConnectionTrait) -> Result<Self, Error> {
    use modo_db::DefaultHooks;          // fallback in scope
    self.before_save()?;                // User::before_save() wins if defined
    let result = modo_db::do_insert(self, db).await?;
    result.after_save()?;               // User::after_save() wins if defined
    Ok(result)
}
```
```

### PK-dependent methods

`find_by_id` and `delete_by_id` are generated by the macro per entity because the PK type varies:

```rust
// String PK:
async fn find_by_id(id: &str, db: &impl ConnectionTrait) -> Result<Self, Error>;    // 404 if not found
async fn delete_by_id(id: &str, db: &impl ConnectionTrait) -> Result<(), Error>;    // 404 if not found

// Composite PK:
async fn find_by_id(id: (String, String), db: &impl ConnectionTrait) -> Result<Self, Error>;
async fn delete_by_id(id: (String, String), db: &impl ConnectionTrait) -> Result<(), Error>;
```

### Method flow

**insert():**

```
1. self.before_save()         <- inherent method or DefaultHooks no-op
2. self -> ActiveModel         <- into_active_model_full()
3. apply_auto_fields(am)      <- fill empty ID, set timestamps
4. am.insert(db)              <- SeaORM INSERT
5. Model -> Self               <- from_model()
6. self.after_save()          <- inherent method or DefaultHooks no-op
```

**update(&mut self):**

```
1. self.before_save()                              <- inherent method or DefaultHooks no-op
2. self -> ActiveModel (all fields Set)            <- into_active_model_full()
3. apply_auto_fields(am)                           <- refresh updated_at
4. am.update(db)                                   <- SeaORM UPDATE
5. Model -> update *self in place                  <- from_model(), mutate self with refreshed values
6. self.after_save()                               <- inherent method or DefaultHooks no-op
```

**delete():**

```
1. self.before_delete()       <- inherent method or DefaultHooks no-op
2. soft_delete? -> UPDATE SET deleted_at = now(), updated_at = now()
   hard delete? -> DELETE FROM
```

## Query Wrappers

### EntityQuery<T, E>

Wraps `Select<E>`, auto-converts results to user's type:

```rust
pub struct EntityQuery<T, E: EntityTrait> {
    select: Select<E>,
    _phantom: PhantomData<T>,
}

impl<T, E> EntityQuery<T, E>
where
    T: Record<Entity = E>,
    E: EntityTrait,
{
    // Terminal — auto-convert:
    pub async fn all(self, db: &impl ConnectionTrait) -> Result<Vec<T>, Error>;
    pub async fn one(self, db: &impl ConnectionTrait) -> Result<Option<T>, Error>;
    pub async fn count(self, db: &impl ConnectionTrait) -> Result<u64, Error>;

    // Pagination — auto-convert:
    pub async fn paginate(self, db: &impl ConnectionTrait, params: &PageParams) -> Result<PageResult<T>, Error>;
    pub async fn paginate_cursor<C, V, F>(self, col: C, cursor_fn: F, db: &impl ConnectionTrait, params: &CursorParams<V>) -> Result<CursorResult<T>, Error>;

    // Chainable:
    pub fn filter(self, f: impl IntoCondition) -> Self;
    pub fn order_by_asc<C: ColumnTrait>(self, col: C) -> Self;
    pub fn order_by_desc<C: ColumnTrait>(self, col: C) -> Self;
    pub fn limit(self, n: u64) -> Self;
    pub fn offset(self, n: u64) -> Self;

    // Escape hatch:
    pub fn into_select(self) -> Select<E>;
}
```

### EntityUpdateMany<E>

```rust
pub struct EntityUpdateMany<E: EntityTrait> {
    update: UpdateMany<E>,
}

impl<E: EntityTrait> EntityUpdateMany<E> {
    pub fn filter(self, f: impl IntoCondition) -> Self;
    pub fn col_expr<T: IntoIden>(self, col: T, expr: SimpleExpr) -> Self;
    pub async fn exec(self, db: &impl ConnectionTrait) -> Result<u64, Error>;
}
```

### EntityDeleteMany<E>

```rust
pub struct EntityDeleteMany<E: EntityTrait> {
    delete: DeleteMany<E>,
}

impl<E: EntityTrait> EntityDeleteMany<E> {
    pub fn filter(self, f: impl IntoCondition) -> Self;
    pub async fn exec(self, db: &impl ConnectionTrait) -> Result<u64, Error>;
}
```

## Lifecycle Hooks

Uses Rust's inherent method priority: when the macro-generated `insert`/`update`/`delete` calls `self.before_save()`, an inherent method on the user's struct takes priority over the `DefaultHooks` blanket trait fallback. No attributes, no extra traits to implement.

```rust
// In modo-db — blanket no-op for all types:
pub trait DefaultHooks {
    fn before_save(&mut self) -> Result<(), Error> { Ok(()) }
    fn after_save(&self) -> Result<(), Error> { Ok(()) }
    fn before_delete(&self) -> Result<(), Error> { Ok(()) }
}
impl<T> DefaultHooks for T {}
```

**No hook needed — just works:**

```rust
#[modo_db::entity(table = "users")]
pub struct User { ... }
// No hook methods defined. DefaultHooks::before_save() is called -> no-op.
// Auto-ID and timestamps work via apply_auto_fields.
```

**Custom hooks — just define methods on your struct:**

```rust
impl User {
    pub fn before_save(&mut self) -> Result<(), modo::Error> {
        self.email = self.email.trim().to_lowercase();
        if self.name.is_empty() {
            return Err(modo::Error::validation("name is required"));
        }
        Ok(())
    }

    pub fn after_save(&self) -> Result<(), modo::Error> {
        tracing::info!(user_id = %self.id, "user saved");
        Ok(())
    }

    pub fn before_delete(&self) -> Result<(), modo::Error> {
        if self.role == "admin" {
            return Err(modo::Error::validation("cannot delete admin"));
        }
        Ok(())
    }
}
```

**How it works:** The macro generates `insert`/`update`/`delete` in `impl Record for User` (concrete type). Inside that block, `self.before_save()` resolves to the inherent method if defined, or falls back to `DefaultHooks::before_save()` (no-op). No `#[entity(hooks)]` attribute needed.

Auto-ID and timestamps are NOT in hooks — they're in `apply_auto_fields`, which always runs after the hook. User hooks contain only domain logic.

**Bulk operations (`update_many`, `delete_many`, `force_delete_many`) skip hooks** — they're bulk SQL operations that don't load records. This is a documented trade-off.

## Macro Output

Given:

```rust
#[modo_db::entity(table = "users")]
#[entity(timestamps)]
pub struct User {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    pub email: String,
    pub name: String,
    #[entity(has_many)]
    pub members: Vec<Member>,
}
```

The macro emits:

### 1. User's struct preserved (with auto fields appended, relation fields removed)

```rust
#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- `members` field removed (relation-only)
- `created_at`, `updated_at` added by `#[entity(timestamps)]`
- `Clone`, `Debug`, `Serialize` auto-derived

### 2. SeaORM module

```rust
pub mod user {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model { id, email, name, created_at, updated_at }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation { ... }

    pub enum Column { Id, Email, Name, CreatedAt, UpdatedAt }

    impl Related<super::member::Entity> for Entity { ... }

    impl ActiveModelBehavior for ActiveModel {}  // empty — logic in Record
}
```

### 3. Default (eager generation)

```rust
impl Default for User {
    fn default() -> Self {
        Self {
            id: modo_db::generate_ulid(),
            email: String::new(),
            name: String::new(),
            created_at: modo_db::chrono::Utc::now(),
            updated_at: modo_db::chrono::Utc::now(),
        }
    }
}
```

### 4. Conversions

```rust
impl From<user::Model> for User {
    fn from(m: user::Model) -> Self {
        Self { id: m.id, email: m.email, name: m.name,
               created_at: m.created_at, updated_at: m.updated_at }
    }
}
```

### 5. Record impl

```rust
impl modo_db::Record for User {
    type Entity = user::Entity;
    type ActiveModel = user::ActiveModel;

    fn from_model(model: user::Model) -> Self { Self::from(model) }

    fn into_active_model_full(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: Set(self.id.clone()),
            email: Set(self.email.clone()),
            name: Set(self.name.clone()),
            created_at: Set(self.created_at),
            updated_at: Set(self.updated_at),
        }
    }

    fn into_active_model(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: Set(self.id.clone()),
            ..Default::default()  // all other fields NotSet
        }
    }

    fn apply_auto_fields(am: &mut user::ActiveModel, is_insert: bool) {
        if is_insert {
            if let sea_orm::ActiveValue::Set(ref id) = am.id {
                if id.is_empty() { am.id = Set(modo_db::generate_ulid()); }
            }
        }
        let now = modo_db::chrono::Utc::now();
        if is_insert {
            if let sea_orm::ActiveValue::Set(ref ts) = am.created_at {
                if *ts == DateTime::UNIX_EPOCH { am.created_at = Set(now); }
            }
        }
        am.updated_at = Set(now);
    }

    // PK-dependent + hook-calling methods (macro-generated, not default impls):
    async fn find_by_id(id: &str, db: &impl ConnectionTrait) -> Result<Option<Self>, Error> { ... }
    async fn delete_by_id(id: &str, db: &impl ConnectionTrait) -> Result<(), Error> { ... }

    async fn insert(mut self, db: &impl ConnectionTrait) -> Result<Self, Error> {
        use modo_db::DefaultHooks;
        self.before_save()?;                        // inherent method wins if defined
        let result = modo_db::do_insert(self, db).await?;
        result.after_save()?;
        Ok(result)
    }

    async fn update(&mut self, db: &impl ConnectionTrait) -> Result<(), Error> {
        use modo_db::DefaultHooks;
        self.before_save()?;
        let refreshed = modo_db::do_update(self, db).await?;
        *self = refreshed;
        self.after_save()?;
        Ok(())
    }

    async fn delete(self, db: &impl ConnectionTrait) -> Result<(), Error> {
        use modo_db::DefaultHooks;
        self.before_delete()?;
        modo_db::do_delete(self, db).await
    }
}
```

### 6. Relation accessors

```rust
impl User {
    pub async fn members(&self, db: &impl ConnectionTrait) -> Result<Vec<Member>, modo::Error> {
        Member::query()
            .filter(member::Column::UserId.eq(&self.id))
            .all(db).await
    }
}
```

### 7. Inventory registration (unchanged)

```rust
modo_db::inventory::submit! {
    modo_db::EntityRegistration {
        table_name: "users",
        group: "default",
        register_fn: |sb| sb.register(user::Entity),
        is_framework: false,
        extra_sql: &[],
    }
}
```

## Soft Delete

For entities with `#[entity(soft_delete)]`, the macro adds `deleted_at: Option<DateTime<Utc>>` to the struct and generates additional behavior.

### Behavior overrides

| Method | Normal entity | Soft-delete entity |
|--------|--------------|-------------------|
| `find_all(&db)` | `SELECT *` | `WHERE deleted_at IS NULL` |
| `find_by_id(id, &db)` | no filter | `+ WHERE deleted_at IS NULL` |
| `query()` | unfiltered | pre-filtered `deleted_at IS NULL` |
| `delete(self, &db)` | `DELETE` | `UPDATE SET deleted_at = now()` |
| `delete_by_id(id, &db)` | `DELETE WHERE id = ?` | `UPDATE SET deleted_at = now() WHERE id = ?` |
| `delete_many()` | `DELETE WHERE ...` | `UPDATE SET deleted_at = now() WHERE ...` |

### Additional methods (soft-delete only)

```rust
impl Todo {
    // Query scopes:
    pub fn with_deleted() -> EntityQuery<Self, todo::Entity>;
    pub fn only_deleted() -> EntityQuery<Self, todo::Entity>;

    // Instance:
    pub async fn restore(&mut self, db: &impl ConnectionTrait) -> Result<(), Error>;
    pub async fn force_delete(self, db: &impl ConnectionTrait) -> Result<(), Error>;

    // Static:
    pub async fn force_delete_by_id(id: &str, db: &impl ConnectionTrait) -> Result<(), Error>;
    pub fn force_delete_many() -> EntityDeleteMany<todo::Entity>;
}
```

### Auto-index

`#[entity(soft_delete)]` automatically generates:

```sql
CREATE INDEX IF NOT EXISTS idx_{table}_deleted_at ON {table} (deleted_at)
```

## Error Handling

`Record` methods convert `sea_orm::DbErr` to `modo::Error` via a helper function (not `From` impl — orphan rule prevents `impl From<DbErr> for modo::Error` in `modo-db` since neither type is local):

```rust
// In modo-db:
pub fn db_err_to_error(e: sea_orm::DbErr) -> modo::Error {
    match e.sql_err() {
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) => {
            modo::Error::from(modo::HttpError::Conflict)
        }
        _ => match e {
            sea_orm::DbErr::RecordNotFound(_) => modo::Error::from(modo::HttpError::NotFound),
            _ => modo::Error::internal(e.to_string()),
        },
    }
}
```

All `Record` default implementations use `db_err_to_error()` internally. Users never need to call it — `Record` methods return `Result<T, modo::Error>` and the `?` operator works directly in handlers.

**Note:** `DbErr::UniqueConstraintViolation` is not a direct variant — constraint violations are accessed via `db_err.sql_err()` which returns `Option<SqlErr>`.

## Usage Examples

### Simple CRUD

```rust
use modo::extractor::JsonReq;
use modo::{Json, JsonResult, HttpError};
use modo_db::{Db, Record};

#[modo::handler(POST, "/users")]
async fn create_user(Db(db): Db, input: JsonReq<CreateUser>) -> JsonResult<User> {
    let user = User {
        email: input.email.clone(),
        name: input.name.clone(),
        ..Default::default()
    }.insert(&*db).await?;
    Ok(Json(user))
}

#[modo::handler(GET, "/users/{id}")]
async fn get_user(Db(db): Db, id: String) -> JsonResult<User> {
    let user = User::find_by_id(&id, &*db).await?;  // 404 automatic
    Ok(Json(user))
}

#[modo::handler(PUT, "/users/{id}")]
async fn update_user(Db(db): Db, id: String, input: JsonReq<UpdateUser>) -> JsonResult<User> {
    let mut user = User::find_by_id(&id, &*db).await?;
    user.name = input.name.clone();
    user.update(&*db).await?;
    Ok(Json(user))
}

#[modo::handler(DELETE, "/users/{id}")]
async fn delete_user(Db(db): Db, id: String) -> JsonResult<()> {
    User::delete_by_id(&id, &*db).await?;
    Ok(Json(()))
}
```

### Partial updates

```rust
impl User {
    pub async fn update_email(
        &mut self,
        email: String,
        db: &impl ConnectionTrait,
    ) -> Result<(), modo::Error> {
        use sea_orm::{ActiveModelTrait, Set};

        let mut am = self.into_active_model();
        am.email = Set(email.clone());
        am.update(db).await?;
        self.email = email;
        Ok(())
    }
}
```

### Filtered queries

```rust
let active_users = User::query()
    .filter(user::Column::Active.eq(true))
    .order_by_desc(user::Column::CreatedAt)
    .all(&*db).await?;

let first = User::query()
    .filter(user::Column::Email.eq("a@b.com"))
    .one(&*db).await?;

let count = User::query()
    .filter(user::Column::Active.eq(true))
    .count(&*db).await?;
```

### Pagination

```rust
use modo::extractor::QueryReq;
use modo_db::PageParams;

#[modo::handler(GET, "/users")]
async fn list_users(
    Db(db): Db,
    params: QueryReq<PageParams>,
) -> JsonResult<PageResult<User>> {
    let page = User::query()
        .order_by_asc(user::Column::Name)
        .paginate(&*db, &params).await?;
    Ok(Json(page))
}
```

### Bulk operations

```rust
// Bulk update:
use sea_orm::prelude::Expr;

User::update_many()
    .filter(user::Column::TenantId.eq(&tid))
    .col_expr(user::Column::Active, Expr::value(false))
    .col_expr(user::Column::UpdatedAt, Expr::value(Utc::now()))
    .exec(&*db).await?;

// Bulk delete:
User::delete_many()
    .filter(user::Column::TenantId.eq(&tid))
    .exec(&*db).await?;
```

### Transactions

```rust
use sea_orm::TransactionTrait;

#[modo::handler(POST, "/teams")]
async fn create_team(Db(db): Db, input: JsonReq<CreateTeam>) -> JsonResult<Tenant> {
    let txn = db.begin().await?;

    let tenant = Tenant {
        name: input.name.clone(),
        slug: input.slug.clone(),
        ..Default::default()
    }.insert(&txn).await?;

    let _member = Member {
        user_id: input.owner_id.clone(),
        tenant_id: tenant.id.clone(),
        role: "owner".into(),
        ..Default::default()
    }.insert(&txn).await?;

    txn.commit().await?;
    Ok(Json(tenant))
}
```

### Relation accessors

```rust
let user = User::find_by_id(&id, &*db).await?;  // 404 automatic
let memberships = user.members(&*db).await?;

for m in &memberships {
    let tenant = m.tenant(&*db).await?;
}
```

### Custom domain methods with joins

```rust
impl User {
    pub async fn tenants_with_roles(
        &self,
        db: &impl ConnectionTrait,
    ) -> Result<Vec<(Tenant, String)>, modo::Error> {
        use modo_db::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let rows = member::Entity::find()
            .filter(member::Column::UserId.eq(&self.id))
            .find_also_related(tenant::Entity)
            .all(db)
            .await?;

        Ok(rows.into_iter()
            .filter_map(|(m, t)| t.map(|t| (Tenant::from(t), m.role)))
            .collect())
    }
}
```

### Escape hatch

```rust
// Raw SeaORM Select:
let select = User::query().into_select();

// Direct module access:
user::Entity::find()
user::Entity::update_many()
user::Column::Email
user::Relation::Member
```

### Lifecycle hooks

```rust
// Just add methods to your struct — no attributes or traits needed:
impl User {
    pub fn before_save(&mut self) -> Result<(), modo::Error> {
        self.email = self.email.trim().to_lowercase();
        if self.name.is_empty() {
            return Err(modo::Error::validation("name is required"));
        }
        Ok(())
    }

    pub fn after_save(&self) -> Result<(), modo::Error> {
        tracing::info!(user_id = %self.id, "user saved");
        Ok(())
    }

    pub fn before_delete(&self) -> Result<(), modo::Error> {
        if self.role == "admin" {
            return Err(modo::Error::validation("cannot delete admin"));
        }
        Ok(())
    }
}
```

## Design Notes

### Escape hatch bypasses auto-fields

When using the SeaORM escape hatch directly (`user::ActiveModel::insert(db)`), `apply_auto_fields` does NOT run — the user is responsible for setting ID and timestamps. `ActiveModelBehavior` is empty. This is a documented trade-off: convenience methods handle auto-fields, escape hatch gives full control.

### Last-write-wins on update

`update()` performs a last-write-wins overwrite with no optimistic locking. If another writer modifies the record between `find_by_id` and `update`, the later write wins. There is no version/timestamp guard. This is a conscious trade-off for simplicity — optimistic locking can be added later if needed.

### delete_by_id loads record to fire hooks

For consistency, `delete_by_id` loads the record first via `find_by_id`, then calls `delete(self, db)` which fires `before_delete`. This costs an extra SELECT but ensures hooks always run for single-record deletes. Bulk operations (`delete_many`, `force_delete_many`) skip hooks.

### Pagination auto-conversion

`EntityQuery::paginate()` internally calls the existing `paginate()` function which returns `PageResult<E::Model>`, then maps via `PageResult::map(T::from_model)` to convert to `PageResult<T>`. Same pattern for `paginate_cursor` with `CursorResult::map`.

### Soft-delete entities override find_all and query

For soft-delete entities, the macro overrides `find_all` and `query` in the `Record` impl to pre-filter `deleted_at IS NULL`. These are not trait defaults — they are macro-generated overrides.

## Deliverables

1. **`modo-db` runtime** — `Record` trait, `DefaultHooks` blanket trait, CRUD helpers (`do_insert`/`do_update`/`do_delete`), `EntityQuery`, `EntityUpdateMany`, `EntityDeleteMany`, `db_err_to_error()`
2. **`modo-db-macros` entity macro** — rewrite to preserve struct, generate `Record` impl, relation accessors, soft-delete methods
3. **Documentation** — `modo-db` crate docs and README with all usage patterns (CRUD, partial updates, queries, pagination, bulk ops, transactions, relations, hooks, escape hatch)
4. **Examples** — update all examples in `examples/` folder to use new API
5. **CLI templates** — update `modo-cli` scaffolding templates if they reference entity patterns
6. **Tests** — `Record` trait defaults, `EntityQuery`, hooks resolution, soft-delete, macro integration

## Testing Strategy

### Runtime library tests (modo-db)

- `Record` trait default methods — using a test entity with in-memory SQLite
- `EntityQuery` — filter, order, limit, pagination, `into_select` escape hatch
- `EntityUpdateMany` / `EntityDeleteMany` — bulk operations
- `DefaultHooks` + inherent method priority — default no-op works, user's inherent method fires when defined
- `db_err_to_error()` — unique constraint -> 409, not found -> 404
- Soft-delete behavior — `delete` does UPDATE, `force_delete` does DELETE, queries exclude deleted

### Macro tests (modo-db-macros / modo-db integration)

- Struct preserved — verify user type is usable, not consumed
- `Default` — auto-ID generated, timestamps populated
- `From<Model>` / `into_active_model_full()` / `into_active_model()` — round-trip correctness
- `apply_auto_fields` — fills empty ID, skips non-empty, always sets timestamps
- Relation accessors — `belongs_to` generates singular method, `has_many` generates collection method
- Soft-delete extras — `with_deleted`, `only_deleted`, `restore`, `force_delete_by_id`, `force_delete_many`
- Composite PKs — `find_by_id` accepts tuple
- Auto derives — `Clone`, `Debug`, `Serialize` present on generated struct

### Examples

- Update all examples in `examples/` to use new `Record` API
- Verify all examples compile and run
