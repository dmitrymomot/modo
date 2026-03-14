# modo-db Domain Model Refactoring — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor modo-db so the `#[entity]` macro preserves the user's struct as a domain model with CRUD methods, hiding SeaORM internals behind a `Record` trait.

**Architecture:** Thin proc macro generates SeaORM module alongside the preserved struct plus `Record` trait impl. Runtime library provides `Record` trait with CRUD helpers, `EntityQuery` auto-converting wrapper, and `DefaultHooks` blanket trait for lifecycle hooks via inherent method priority.

**Tech Stack:** Rust, SeaORM v2 RC, syn/quote/proc-macro2 for macro, inventory for auto-discovery.

**Spec:** `docs/superpowers/specs/2026-03-14-modo-db-domain-model-design.md`

---

## File Map

### New files (modo-db/src/)

| File | Responsibility |
|------|---------------|
| `record.rs` | `Record` trait definition with default impls for `find_all`, `query`, `update_many`, `delete_many` |
| `hooks.rs` | `DefaultHooks` blanket trait (no-op `before_save`, `after_save`, `before_delete`) |
| `query.rs` | `EntityQuery<T,E>`, `EntityUpdateMany<E>`, `EntityDeleteMany<E>` wrappers |
| `helpers.rs` | `do_insert`, `do_update`, `do_delete` shared CRUD logic |
| `error.rs` | `db_err_to_error()` conversion function |

### Modified files (modo-db/src/)

| File | Change |
|------|--------|
| `lib.rs` | Add `mod` plus `pub use` for new modules |

### Rewritten files (modo-db-macros/src/)

| File | Change |
|------|--------|
| `entity.rs` | Rewrite to preserve user struct, generate SeaORM module alongside, emit `Record` impl |

### New test files (modo-db/tests/)

| File | Tests |
|------|-------|
| `record_trait.rs` | CRUD operations via `Record` trait with in-memory SQLite |
| `entity_query.rs` | `EntityQuery` filter/order/limit/pagination/escape-hatch |
| `hooks.rs` | DefaultHooks fallback, inherent method priority |
| `soft_delete.rs` | Soft-delete CRUD, scoped queries, restore, force-delete |
| `error_conversion.rs` | `db_err_to_error` mapping |

### Modified files (modo-db/tests/)

| File | Change |
|------|--------|
| `entity_macro.rs` | Rewrite to test new macro output (struct preserved, Default, From, Record impl) |

### Modified example files

| File | Change |
|------|--------|
| `examples/todo-api/src/entity.rs` | Keep struct, use new macro output |
| `examples/todo-api/src/handlers.rs` | Use `Record` methods instead of SeaORM traits |
| `examples/todo-api/src/types.rs` | Possibly simplify (response types may merge with entity) |
| `examples/sse-chat/src/entity.rs` | Keep struct, use new macro output |
| `examples/sse-chat/src/chat.rs` | Use `Record` methods, `EntityQuery` |

### Documentation

| File | Change |
|------|--------|
| `modo-db/README.md` | Rewrite with new API examples |
| `modo-db-macros/README.md` | Update macro documentation |

---

## Chunk 1: modo-db Runtime Library

### Task 1: DefaultHooks blanket trait

**Files:**
- Create: `modo-db/src/hooks.rs`
- Modify: `modo-db/src/lib.rs`

- [ ] **Step 1: Create hooks.rs with DefaultHooks trait**

```rust
// modo-db/src/hooks.rs
pub trait DefaultHooks {
    fn before_save(&mut self) -> Result<(), modo::Error> {
        Ok(())
    }
    fn after_save(&self) -> Result<(), modo::Error> {
        Ok(())
    }
    fn before_delete(&self) -> Result<(), modo::Error> {
        Ok(())
    }
}

impl<T> DefaultHooks for T {}
```

- [ ] **Step 2: Add mod and pub use in lib.rs**

Add to `modo-db/src/lib.rs`:
```rust
mod hooks;
pub use hooks::DefaultHooks;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo-db`
Expected: clean

- [ ] **Step 4: Commit**

```
git add modo-db/src/hooks.rs modo-db/src/lib.rs
git commit -m "feat(modo-db): add DefaultHooks blanket trait for lifecycle hooks"
```

---

### Task 2: Error conversion helper

**Files:**
- Create: `modo-db/src/error.rs`
- Create: `modo-db/tests/error_conversion.rs`
- Modify: `modo-db/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
// modo-db/tests/error_conversion.rs
use modo_db::db_err_to_error;
use sea_orm::DbErr;

#[test]
fn record_not_found_maps_to_404() {
    let err = db_err_to_error(DbErr::RecordNotFound("test".into()));
    // Assert status is 404 - check modo::Error API for exact assertion
    assert!(err.to_string().contains("Not Found") || format!("{:?}", err).contains("404"));
}

#[test]
fn other_errors_map_to_500() {
    let err = db_err_to_error(DbErr::Custom("boom".into()));
    assert!(err.to_string().contains("boom"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `TMPDIR=/private/tmp/claude-501 cargo test -p modo-db --test error_conversion`
Expected: FAIL - `db_err_to_error` not found

- [ ] **Step 3: Create error.rs**

```rust
// modo-db/src/error.rs
pub fn db_err_to_error(e: sea_orm::DbErr) -> modo::Error {
    match e.sql_err() {
        Some(sea_orm::error::SqlErr::UniqueConstraintViolation(_)) => {
            modo::Error::from(modo::HttpError::Conflict)
        }
        _ => match e {
            sea_orm::DbErr::RecordNotFound(_) => {
                modo::Error::from(modo::HttpError::NotFound)
            }
            _ => modo::Error::internal(e.to_string()),
        },
    }
}
```

- [ ] **Step 4: Add mod and pub use in lib.rs**

```rust
mod error;
pub use error::db_err_to_error;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `TMPDIR=/private/tmp/claude-501 cargo test -p modo-db --test error_conversion`
Expected: PASS

Note: The test assertions depend on how `modo::Error` exposes status. Check the actual `modo::Error` API and adjust assertions accordingly.

- [ ] **Step 6: Commit**

```
git add modo-db/src/error.rs modo-db/src/lib.rs modo-db/tests/error_conversion.rs
git commit -m "feat(modo-db): add db_err_to_error conversion helper"
```

---

### Task 3: EntityQuery wrapper

**Files:**
- Create: `modo-db/src/query.rs`
- Modify: `modo-db/src/lib.rs`

- [ ] **Step 1: Create query.rs with EntityQuery, EntityUpdateMany, EntityDeleteMany**

`EntityQuery<T, E>` wraps `Select<E>` with chainable methods (`filter`, `order_by_asc`, `order_by_desc`, `limit`, `offset`) and terminal methods (`all`, `one`, `count`, `paginate`, `paginate_cursor`) that auto-convert results via `T::from(model)`. Includes `into_select()` escape hatch.

`EntityUpdateMany<E>` wraps `UpdateMany<E>` with `filter`, `col_expr`, and `exec`.

`EntityDeleteMany<E>` wraps `DeleteMany<E>` with `filter` and `exec`.

All error conversions use `db_err_to_error` internally.

Refer to spec section "Query Wrappers" for exact signatures.

- [ ] **Step 2: Add mod and pub use in lib.rs**

```rust
mod query;
pub use query::{EntityDeleteMany, EntityQuery, EntityUpdateMany};
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo-db`
Expected: clean (may need to adjust imports based on actual SeaORM v2 API)

- [ ] **Step 4: Commit**

```
git add modo-db/src/query.rs modo-db/src/lib.rs
git commit -m "feat(modo-db): add EntityQuery, EntityUpdateMany, EntityDeleteMany wrappers"
```

---

### Task 4: Record trait and CRUD helpers

**Files:**
- Create: `modo-db/src/record.rs`
- Create: `modo-db/src/helpers.rs`
- Modify: `modo-db/src/lib.rs`

- [ ] **Step 1: Create helpers.rs with shared CRUD logic**

Three functions: `do_insert<T: Record>`, `do_update<T: Record>`, `do_delete<T: Record>`. These handle the ActiveModel conversion, `apply_auto_fields`, and SeaORM calls. They do NOT call hooks - hooks are called by the macro-generated wrapper.

Refer to spec section "Record Trait" method flows for exact logic.

- [ ] **Step 2: Create record.rs with Record trait**

Define the trait with associated types (`Entity`, `ActiveModel`) and required methods (`from_model`, `into_active_model_full`, `into_active_model`, `apply_auto_fields`). Add default impls for `find_all`, `query`, `update_many`, `delete_many`.

The CRUD methods (`find_by_id`, `delete_by_id`, `insert`, `update`, `delete`) are declared but NOT given default impls - the macro generates them per entity.

- [ ] **Step 3: Add mod and pub use in lib.rs**

```rust
mod helpers;
mod record;

pub use helpers::{do_delete, do_insert, do_update};
pub use record::Record;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p modo-db`
Expected: clean

- [ ] **Step 5: Commit**

```
git add modo-db/src/record.rs modo-db/src/helpers.rs modo-db/src/lib.rs
git commit -m "feat(modo-db): add Record trait and CRUD helper functions"
```

---

## Chunk 2: Entity Macro Rewrite

### Task 5: Refactor entity macro - struct preservation and SeaORM module

**Files:**
- Modify: `modo-db-macros/src/entity.rs`

This is the largest task. The current macro (~760 lines) must be rewritten to:
1. Preserve the user's struct (strip `#[entity]` attrs, remove relation fields, add timestamp/soft-delete fields, add `#[derive(Clone, Debug, serde::Serialize)]`)
2. Generate the SeaORM module alongside (same structure as today)
3. Still generate `ActiveModelBehavior` (empty - logic moved to Record)

Key change: instead of only emitting `pub mod mod_name { ... }`, emit both the cleaned user struct AND the module.

- [ ] **Step 1: Modify expand() to emit preserved struct alongside SeaORM module**

- [ ] **Step 2: Verify it compiles and existing entity_macro tests can be adapted**

Run: `cargo check -p modo-db`

- [ ] **Step 3: Commit**

```
git add modo-db-macros/src/entity.rs
git commit -m "refactor(modo-db-macros): preserve user struct alongside SeaORM module"
```

---

### Task 6: Generate Default and From impls

**Files:**
- Modify: `modo-db-macros/src/entity.rs`

- [ ] **Step 1: Generate Default impl**

For each field: auto="ulid" calls `generate_ulid()`, auto="nanoid" calls `generate_nanoid()`, timestamps call `Utc::now()`, String defaults to `String::new()`, bool to `false`, numeric to `0`, Option to `None`.

- [ ] **Step 2: Generate From<Model> impl**

Maps each field from `model.field_name` to `Self { field_name: model.field_name }`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p modo-db`

- [ ] **Step 4: Commit**

```
git add modo-db-macros/src/entity.rs
git commit -m "feat(modo-db-macros): generate Default and From<Model> impls"
```

---

### Task 7: Generate Record trait impl

**Files:**
- Modify: `modo-db-macros/src/entity.rs`

- [ ] **Step 1: Generate impl Record**

Emit `from_model`, `into_active_model_full`, `into_active_model`, `apply_auto_fields` (required methods). Emit `find_by_id`, `delete_by_id`, `insert`, `update`, `delete` (macro-generated, not defaults).

For `insert`/`update`/`delete`: emit `use modo_db::DefaultHooks;` then `self.before_save()?;` before delegating to `do_insert`/`do_update`/`do_delete`.

Refer to spec "Macro Output" section 5 for exact code.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p modo-db`

- [ ] **Step 3: Commit**

```
git add modo-db-macros/src/entity.rs
git commit -m "feat(modo-db-macros): generate Record trait impl with hooks and CRUD"
```

---

### Task 8: Generate relation accessors

**Files:**
- Modify: `modo-db-macros/src/entity.rs`

- [ ] **Step 1: Generate belongs_to accessors**

For `#[entity(belongs_to = "Target")]` on `user_id: String`, generate `pub async fn user(&self, db) -> Result<Option<Target>>` that calls `Target::find_by_id`.

- [ ] **Step 2: Generate has_many accessors**

For `#[entity(has_many)]` on `members: Vec<Member>`, generate `pub async fn members(&self, db) -> Result<Vec<Member>>` that uses `Member::query().filter(...)`.

- [ ] **Step 3: Generate has_one accessors**

Same pattern, returns `Result<Option<T>>` using `.one(db)`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p modo-db`

- [ ] **Step 5: Commit**

```
git add modo-db-macros/src/entity.rs
git commit -m "feat(modo-db-macros): generate relation accessor methods"
```

---

### Task 9: Soft-delete method generation

**Files:**
- Modify: `modo-db-macros/src/entity.rs`

- [ ] **Step 1: Override delete/query for soft-delete entities**

When `#[entity(soft_delete)]`: override `delete` to SET deleted_at, override `find_by_id`/`find_all`/`query` to filter `deleted_at IS NULL`, override `delete_many` to UPDATE instead of DELETE.

- [ ] **Step 2: Generate soft-delete specific methods**

`with_deleted()`, `only_deleted()`, `restore()`, `force_delete()`, `force_delete_by_id()`, `force_delete_many()`.

- [ ] **Step 3: Add auto-index for deleted_at**

Add index SQL to `EntityRegistration.extra_sql`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p modo-db`

- [ ] **Step 5: Commit**

```
git add modo-db-macros/src/entity.rs
git commit -m "feat(modo-db-macros): generate soft-delete overrides and methods"
```

---

## Chunk 3: Tests

### Task 10: Rewrite entity_macro tests

**Files:**
- Modify: `modo-db/tests/entity_macro.rs`

- [ ] **Step 1: Update tests for new macro output**

Verify: struct preserved, Default generates ULID/timestamps, From round-trips, into_active_model works, relation fields removed, derives present, soft-delete adds deleted_at and index, composite PKs work.

- [ ] **Step 2: Run tests**

Run: `TMPDIR=/private/tmp/claude-501 cargo test -p modo-db --test entity_macro`

- [ ] **Step 3: Commit**

```
git add modo-db/tests/entity_macro.rs
git commit -m "test(modo-db): rewrite entity macro tests for struct-preserving output"
```

---

### Task 11: Record trait integration tests

**Files:**
- Create: `modo-db/tests/record_trait.rs`

- [ ] **Step 1: Write integration tests with in-memory SQLite**

Define test entity, test: insert and find, update, delete, delete_by_id, find_all, query with filter, query one, find_by_id returns 404, insert duplicate returns 409, update_many, delete_many, pagination.

Each test: connect in-memory SQLite, sync_and_migrate, perform ops, assert.

- [ ] **Step 2: Run tests**

Run: `TMPDIR=/private/tmp/claude-501 cargo test -p modo-db --test record_trait`

- [ ] **Step 3: Commit**

```
git add modo-db/tests/record_trait.rs
git commit -m "test(modo-db): add Record trait integration tests"
```

---

### Task 12: Hooks and soft-delete tests

**Files:**
- Create: `modo-db/tests/hooks.rs`
- Create: `modo-db/tests/soft_delete.rs`

- [ ] **Step 1: Write hooks tests**

Define entity with inherent `before_save` that uppercases name. Verify hook fires on insert/update. Verify entity without hooks uses default no-op.

- [ ] **Step 2: Write soft-delete tests**

Define soft-delete entity. Test: delete sets deleted_at, find excludes deleted, with_deleted includes all, only_deleted, restore, force_delete.

- [ ] **Step 3: Run tests**

Run: `TMPDIR=/private/tmp/claude-501 cargo test -p modo-db --test hooks --test soft_delete`

- [ ] **Step 4: Commit**

```
git add modo-db/tests/hooks.rs modo-db/tests/soft_delete.rs
git commit -m "test(modo-db): add hooks and soft-delete integration tests"
```

---

## Chunk 4: Examples, Documentation, Cleanup

### Task 13: Update todo-api example

**Files:**
- Modify: `examples/todo-api/src/entity.rs`
- Modify: `examples/todo-api/src/handlers.rs`
- Modify: `examples/todo-api/src/types.rs`

- [ ] **Step 1: Update handlers to use Record methods**

Replace SeaORM trait imports with `use modo_db::Record;`. Use `Todo::find_by_id`, `todo.insert`, etc.

- [ ] **Step 2: Verify builds and runs**

Run: `cargo build -p todo-api`

- [ ] **Step 3: Commit**

```
git add examples/todo-api/
git commit -m "refactor(examples): update todo-api to use Record API"
```

---

### Task 14: Update sse-chat example

**Files:**
- Modify: `examples/sse-chat/src/entity.rs`
- Modify: `examples/sse-chat/src/chat.rs`

- [ ] **Step 1: Update entity and handlers**

Same pattern as todo-api.

- [ ] **Step 2: Verify builds**

Run: `cargo build -p sse-chat`

- [ ] **Step 3: Commit**

```
git add examples/sse-chat/
git commit -m "refactor(examples): update sse-chat to use Record API"
```

---

### Task 15: Update documentation

**Files:**
- Modify: `modo-db/README.md`
- Modify: `modo-db-macros/README.md`
- Modify: `modo-db/src/lib.rs` (crate docs)
- Modify: `modo-db/src/record.rs` (doc comments)
- Modify: `modo-db/src/query.rs` (doc comments)

- [ ] **Step 1: Rewrite modo-db README with all usage patterns from spec**

Entity definition, CRUD, partial updates, filtered queries, pagination, bulk ops, transactions, relations, hooks, escape hatch.

- [ ] **Step 2: Update modo-db-macros README**

- [ ] **Step 3: Add doc comments to new source files**

- [ ] **Step 4: Verify docs build**

Run: `TMPDIR=/private/tmp/claude-501 cargo doc -p modo-db -p modo-db-macros --no-deps`

- [ ] **Step 5: Commit**

```
git add modo-db/README.md modo-db-macros/README.md modo-db/src/
git commit -m "docs(modo-db): rewrite documentation for domain model API"
```

---

### Task 16: Final verification

- [ ] **Step 1: Run full test suite**

Run: `TMPDIR=/private/tmp/claude-501 just test`

- [ ] **Step 2: Run linter**

Run: `just lint`

- [ ] **Step 3: Run format check**

Run: `just fmt`

- [ ] **Step 4: Build all examples**

Run: `cargo build -p hello -p jobs -p sse-chat -p sse-dashboard -p templates -p todo-api -p upload`

- [ ] **Step 5: Commit any remaining fixes**

```
git add -A
git commit -m "chore: final cleanup for modo-db v0.3 domain model refactoring"
```
