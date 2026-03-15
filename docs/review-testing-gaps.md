# Testing Gaps

Findings from comprehensive framework review (2026-03-15).

## Critical Missing Tests

### TEST-01: Pagination completely untested

**Location:** `modo-db/src/pagination.rs`

Both `paginate` (offset-based) and `paginate_cursor` (cursor-based) functions have zero test coverage. The cursor-based pagination has complex boundary detection logic (`has_prev`/`has_next` with the extra-item trick) that is especially important to test.

---

### TEST-02: Cron system has zero tests

**Location:** `modo-jobs/src/cron.rs`

No tests at all for the cron scheduling system. Untested behaviors:

- Scheduling fires at the right time
- Cancellation works
- Consecutive-failure counter increments
- Handler timeout is applied
- Overlap prevention (serial execution)

---

### TEST-03: Stale reaper untested

**Location:** `modo-jobs/src/runner.rs:519-560`

Key behaviors not tested:

- Stale jobs are moved back to `pending`
- `attempts` is decremented
- `locked_by` is cleared
- Race condition with `execute_job` timeout path

---

### TEST-04: Cleanup loop untested

**Location:** `modo-jobs/src/runner.rs:562-602`

The cleanup task that removes old completed/dead jobs after `retention_secs` has no test coverage.

---

### TEST-05: Concurrent job claims untested

No test that fires two concurrent `claim_next` calls against the same job to verify only one worker wins. This is the core correctness property of the claim query.

---

### TEST-06: Postgres backend untested

All job tests use `sqlite::memory:`. The Postgres-specific `FOR UPDATE SKIP LOCKED` path in `claim_next` is completely untested.

---

### TEST-07: max_payload_bytes enforcement untested

**Location:** `modo-jobs/src/queue.rs:70-77`

The payload size check in `enqueue_at` has no test coverage.

---

### TEST-08: Session fingerprint mismatch untested

**Location:** `modo-session/src/middleware.rs:148-162`

The middleware's fingerprint rejection logic is not integration-tested. Tests only cover `SessionStore` directly.

---

### TEST-09: Cross-user session revocation untested

The `revoke` method has an ownership check (`target.user_id != session.user_id` -> 404). No test verifies that User A cannot revoke User B's session.

---

### TEST-10: max_sessions_per_user = 0 untested

**Location:** `modo-session/src/store.rs:247`

Edge case where `max_sessions_per_user = 0` causes all sessions to be immediately evicted. No test documents this broken behavior.

---

## Missing Test Infrastructure

### TEST-11: No compile-fail tests (trybuild)

No `trybuild` or `compile_fail` tests anywhere in the workspace. Cases that need compile-fail tests:

- `#[handler]` on a non-async function
- `#[handler]` with unsupported HTTP method
- `#[module]` without `prefix` argument
- `#[main]` on a non-`main` function
- `#[main]` without exactly 2 parameters
- `#[error_handler]` on an async function
- `#[view]` on a tuple struct
- `#[derive(Sanitize)]` on a generic struct
- `#[template_function(name = 5)]` (non-string name)
- `#[entity]` with invalid `column_type`

---

### TEST-12: No concurrent access tests

No multi-threaded/multi-task tests for:

- DB connection pool exhaustion
- Session store concurrent login (enforce_session_limit race)
- Job claim contention between workers
- SQLite WAL mode concurrent read/write

---

### TEST-13: Middleware stacking not integration-tested

Handlers with `#[middleware(...)]` attributes are not tested at the integration level. The middleware wrapper generation in `handler.rs:196-221` is untested.

---

## Per-Crate Test Gaps

### modo (core)

- No integration tests for full `AppBuilder::run()` pipeline
- No tests for `error_handler_middleware` (custom error handler interception path)
- No tests for `TemplateContext::merge_with` with non-map user contexts
- No tests for `ViewResponse::redirect` / `hx_redirect` with invalid URLs (panic case)
- No tests for maintenance mode + trailing slash interaction
- No tests for rate limiter with `by_header` using invalid header name
- No tests for `percent_decode` fallback for invalid UTF-8 in i18n
- No tests for `SseBroadcastManager` cleanup race conditions
- `TemplateEngine` tests rely on `debug_assertions` being set
- Config tests use `unsafe { std::env::set_var() }` (UB in parallel tests)

### modo-db

- No test for `paginate` and `paginate_cursor` (critical gap)
- No test for `update_many` or `delete_many` with soft-delete entities
- No test for duplicate migration versions detection
- No test for migration macro `group` argument
- No test for `restore` on composite PK entities
- No test for concurrent writes or connection pool exhaustion
- Tests bypass `sync_and_migrate` and create tables with raw SQL

### modo-session

- No test for concurrent `create` calls (race condition in enforce_session_limit)
- No test for fingerprint mismatch behavior in middleware
- No test for cookie prefix collision (e.g., `_session_old=abc; _session=valid`)
- No test for cross-user session revocation authorization
- No test for `max_sessions_per_user = 0`
- No test for `ResolvedUser` cache hit path (UserContextLayer already populated)

### modo-auth

- No extractor tests (`extractor.rs` and `context_layer.rs` have no tests)
- No test for `ResolvedUser` caching in extractor

### modo-jobs

- No test for `enqueue`/`enqueue_at` via `JobQueue` (full path including inventory lookup)
- No test for `max_payload_bytes` enforcement
- No test for stale reaper logic
- No test for cleanup loop
- No test for cron loop (zero tests in entire module)
- No test for concurrent claim (core correctness)
- No test for Postgres `FOR UPDATE SKIP LOCKED` path
- No test for `#[job]` proc macro output (no trybuild tests)
- `test_claim_index_created` asserts `result.is_ok()` but doesn't verify the index actually exists

### modo-email

- No test for `ResendTransport` (no mock HTTP server)
- No test for template with missing layout reference
- No test for `mailer()` factory function (only `mailer_with` tested)
- No test for `FilesystemProvider` with relative path and CWD change

### modo-upload

- No test for `OpendalStorage` (requires live S3)
- No test for `store_stream` with write failure (partial file cleanup)
- No test for `MultipartForm` extractor with `AppState` (only `T::from_multipart` tested)
- No test for invalid `max_file_size` string triggering fallback
- No test for `UploadedFile` with filename containing control characters
- No test for `BufferedUpload` validation attributes being silently ignored

### modo-tenant

- No test for `SubdomainResolver` with `www.acme.myapp.com` host
- No test for `SubdomainResolver` with IPv6 host
- No test for `HeaderResolver` with non-ASCII header value
- No test for `PathPrefixResolver` with URL-encoded segments
- No test for `TenantContextLayer` overwrite of pre-existing `"tenant"` key
- No test for layer + extractor cache sharing (resolver called once)

### modo-macros

- No compile-fail tests for any macro
- No test for wildcard path segments in `#[handler]`
- No test for `#[template_function]` / `#[template_filter]` inventory registration
- No test for `t!` macro
- No test for `#[view]` on generic struct or tuple struct
