# modo

Rust web framework for micro-SaaS. Single binary, compile-time magic, multi-DB support.

## NEXT SESSION: Major Refactor (2026-03-06)

**IMPORTANT: Read `docs/plans/2026-03-06-crate-split-design.md` before doing anything.**

Plan: Move ALL current code into `_legacy/` subfolder, then rebuild the framework feature-by-feature from that reference. Extract cleanly into separate crates instead of moving files around in-place. This avoids dragging old coupling into the new structure.

Refactor strategy:
1. `git mv modo/ _legacy/modo/` and `git mv modo-macros/ _legacy/modo-macros/`
2. Create fresh `modo/` (core: HTTP, cookies, services — NO DB)
3. Pull code feature-by-feature from `_legacy/` into new crates
4. Follow implementation order in the ADR: modo-db → modo-session → modo-auth → modo-jobs → modo-templates → modo-csrf
5. Build example apps after each extraction to validate
6. Delete `_legacy/` when done

## Stack

- axum 0.8 (HTTP)
- SeaORM v2 RC (database) — use v2 only, not v1.x
- Askama (templates)
- inventory (auto-discovery, not linkme)
- tokio (async runtime)

## Architecture

**Target structure (post-refactor):**
- `modo/` — core crate (HTTP, cookies, services — no DB)
- `modo-macros/` — core proc macros
- `modo-db/` — database layer (features: sqlite, postgres)
- `modo-session/` — session management
- `modo-auth/` — authentication
- `modo-jobs/` — background jobs (**implemented**)
- `modo-jobs-macros/` — `#[job(...)]` proc macro (**implemented**)
- `modo-templates/` — Askama + HTMX + flash
- `modo-csrf/` — CSRF protection
- ADR: `docs/plans/2026-03-06-crate-split-design.md`
- Original design doc: `docs/plans/2026-03-04-modo-architecture-design.md`

## Commands

- `just fmt` — format all code
- `just lint` — clippy with `-D warnings` (all workspace targets/features)
- `just test` — run all workspace tests
- `just check` — fmt-check + lint + test (CI/pre-push)
- `cargo check` — type check
- `cargo build -p hello` — build example
- `cargo run -p hello` — run example server

## Conventions

- Handlers: `#[modo::handler(METHOD, "/path")]`
- Path params: plain `id: String` in handler fn auto-extracted from `{id}` in route path — no need for `Path(id): Path<String>`
- Path params: partial extraction supported — declare only the params you need, others ignored via `..`
- Entry point: `#[modo::main]`
- Routes auto-discovered via `inventory` crate
- DB extractor: `Db(db): Db`
- Service extractor: `Service<MyType>`
- Errors: `Result<T, Error>`
- Modules: `#[modo::module(prefix = "/path", middleware = [...])]`
- CSRF: `#[middleware(modo::middleware::csrf_protection)]` — uses double-submit cookie
- Flash messages: `Flash` (write) / `FlashMessages` (read) — cookie-based, one-shot
- Template context: `BaseContext` extractor — auto-gathers HTMX, flash, CSRF, locale
- Middleware: plain async functions, attached via `#[middleware(fn_name(params))]`
- Middleware stacking order: Global (outermost) → Module → Handler (innermost)
- Services: manually constructed, registered via `.service(instance)`
- Sessions: `app.session_store(my_store)` to register, `SessionManager` in handlers
- SessionManager: `authenticate()` / `logout()` / `logout_all()` / `logout_other()` / `rotate()` — handles cookies automatically
- SessionManager data: `data()` / `get::<T>()` / `set()` / `update_data()` / `remove_key()` — immediate store writes
- Auth: implement `UserProvider` trait, use `Auth<User>` / `OptionalAuth<User>` extractors
- Template context: `#[modo::context]` with `#[base]` + `#[user]` + `#[session]` fields
- BaseContext: includes request_id, is_htmx, current_url, flash_messages, csrf_token, locale

## Jobs (modo-jobs)

- Define jobs: `#[modo_jobs::job(queue = "...", priority = N, max_attempts = N, timeout = "5m")]`
- Cron jobs: `#[modo_jobs::job(cron = "0 0 * * * *", timeout = "5m")]` — in-memory only
- Cron + queue/priority/max_attempts = compile error (mutually exclusive)
- Job params: `payload: T` (Serialize/Deserialize), `Service<T>`, `Db(db): Db`
- Enqueue: `MyJob::enqueue(&queue, &payload).await?` or `MyJob::enqueue_at(&queue, &payload, run_at).await?`
- Extractor: `queue: JobQueue` in handlers (requires `JobsHandle` registered as service)
- Start runner: `let jobs = modo_jobs::start(&db, &config.jobs, services).await?;`
- `start()` takes `ServiceRegistry` as third arg for DI in job handlers
- Cancel: `queue.cancel(&job_id).await?` — sets state to `cancelled` (distinct from `dead`)
- Entity: `modo_jobs` table with `is_framework: true` — auto-created by `sync_and_migrate`
- Job states: `pending`, `running`, `completed`, `dead`, `cancelled` (no `failed` state)
- Retry backoff: `5s * 2^(attempt-1)`, capped at 1h (uses saturating arithmetic, safe for any attempt count)
- Stale reaper: resets stuck `running` jobs older than `stale_threshold_secs` back to `pending`, decrements attempts
- Cleanup: auto-purges `completed`/`dead`/`cancelled` jobs older than `retention_secs`
- Shutdown: `jobs.shutdown().await` — signals cancel + waits up to `drain_timeout_secs` for in-flight jobs
- Error persistence: `last_error` column stores failure/timeout messages on retry and dead jobs
- Config validation: `start()` validates config (rejects zero poll_interval, concurrency, stale_threshold, cleanup interval, empty queues)
- Payload size limit: optional `max_payload_bytes` in `JobsConfig` (default: None = unlimited)
- Cron jobs: handler runs inline (no concurrent firings of same cron job); if execution exceeds interval, next tick is skipped
- Cron failures: consecutive failure counter warns after 5 failures in a row
- Design doc: `docs/plans/2026-03-07-modo-jobs-design.md`

## Key Decisions

- "Full magic" — proc macros for everything, auto-discovery, zero runtime cost
- Multi-DB — SQLite (default, WAL mode) + Postgres via modo-db feature flags
- Cron jobs: in-memory only (tokio timers), errors logged via tracing
- Multi-tenancy: shared-DB strategy (Phase 3); per-DB deferred to Phase 5
- Auth: layered traits with swappable defaults
- Cookie-based flash (not session) — no DB dependency
- CSRF via double-submit signed cookie — ~130 lines, no external crate
- `axum-extra` SignedCookieJar for all cookie ops
- Use official documentation only when researching dependencies
- Session IDs: ULID (no UUID anywhere)
- Session cookies: PrivateCookieJar (AES-encrypted), store token (not session ID); token is rotatable
- `SessionToken` newtype for cookie tokens (mirrors `SessionId`); use `SessionToken::generate()` not free functions
- Session fingerprint: SHA256(user_agent + accept_language + accept_encoding), configurable validation
- Session touch: only updates last_active_at when touch_interval elapses (default 5min)
- Session fingerprint uses `\x00` separator between hash inputs to prevent ambiguity
- `SessionStore` and `SessionStoreDyn` must have identical method sets (10 methods each)
- `cleanup_expired` lives on concrete store types, not in the trait

## Gotchas

- Feature flags: optional deps use `dep:name` syntax; gate fields with `#[cfg(feature = "...")]` in struct, Default, and from_env()
- Proc macros can't check `cfg` flags — emit both `#[cfg(feature = "x")]` / `#[cfg(not(feature = "x"))]` branches in generated code
- `SignedCookieJar` needs explicit `Key` type: `SignedCookieJar::<Key>::from_request_parts(...)`
- `cookie` crate needs `key-expansion` feature for `Key::derive_from()`
- Always run `just fmt` before `just check` — format diffs fail the check early
- When adding fields to `AppState`, update `modo/tests/integration.rs` (constructs AppState directly)
- `-D warnings` means dead code is a build error — remove unused code, don't just make it `pub(crate)`
- Clippy enforces `collapsible_if` — collapse nested `if`/`if let` with `&&`
- In handler macro: `func_name` must be cloned (`func.sig.ident.clone()`) before mutating `func` — otherwise borrow checker blocks `&mut func`
- Re-exports in `modo/src/lib.rs` must be alphabetically sorted (`cargo fmt` enforces this)
- `modo-jobs` entity module is named `job` (from `struct Job`); use `use modo_jobs::entity::job as jobs_entity;` in tests to avoid shadowing
- `inventory` registration from library crates may not link in tests — force with `use modo_jobs::entity::job as _;`
- `#[job]` macro validates: must be async, only one payload parameter allowed
- SeaORM's `ExprTrait` conflicts with `Ord::max`/`Ord::min` — disambiguate with `Ord::max(a, b)` syntax
- `JobQueue` extractor looks up `JobsHandle` in services (not `JobQueue` directly) — register `JobsHandle` as service
