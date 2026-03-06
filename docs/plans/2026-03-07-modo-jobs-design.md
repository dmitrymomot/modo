# modo-jobs Design

> Background job queue and cron scheduler for modo.

## Status

Accepted (2026-03-07)

## Overview

`modo-jobs` is a standalone crate providing a DB-backed job queue with per-queue concurrency isolation, priority ordering, exponential retry backoff, and in-memory cron scheduling. It uses `modo-db` for storage and `modo` core for services/errors.

No custom storage abstraction — if you need a different backend, swap the crate.

## Crate Structure

```
modo-jobs/
  Cargo.toml
  src/
    lib.rs          # Public API, re-exports
    entity.rs       # SeaORM entity (modo_jobs table)
    queue.rs        # JobQueue extractor + enqueue API
    runner.rs       # Poll loop, concurrency, claim, retry, stale reaper, cleanup
    cron.rs         # In-memory cron scheduler (tokio timers)
    config.rs       # JobsConfig, QueueConfig, CleanupConfig
    types.rs        # JobId, JobState
modo-jobs-macros/
  Cargo.toml
  src/
    lib.rs          # #[job(...)] proc macro
```

## Dependencies

- `modo` — ServiceRegistry, Error
- `modo-db` — DbPool, entity macro, chrono, sea-orm
- `inventory` — job handler auto-registration
- `cron` — cron expression parsing
- `serde` / `serde_json` — payload serialization
- `tokio` — timers, semaphore, spawn, cancellation
- `tracing` — structured logging
- `ulid` — worker ID generation

One-way dependency: `modo-jobs -> modo` (core never imports modo-jobs).

## Schema

```rust
#[modo_db::entity(table = "modo_jobs")]
#[entity(timestamps)]
pub struct Job {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    pub name: String,
    pub queue: String,
    pub payload: String,           // JSON
    pub state: String,             // pending|running|completed|failed|dead
    pub priority: i32,
    pub attempts: i32,
    pub max_retries: i32,
    pub run_at: DateTime<Utc>,
    pub timeout_secs: i32,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    // created_at, updated_at — auto from #[entity(timestamps)]
}
```

No `last_error` column — failures are logged via `tracing` with `job_id` field.
No `tenant_id` — add when tenancy support exists.
No `dedupe_key` — handle deduplication in application logic if needed.

## Configuration

YAML-based config loaded via `modo::config::load_or_default()`, matching the pattern used by modo core and modo-db.

```yaml
jobs:
  poll_interval_secs: 1
  stale_threshold_secs: 600
  drain_timeout_secs: 30
  queues:
    - name: default
      concurrency: 4
    - name: emails
      concurrency: 2
    - name: reports
      concurrency: 2
  cleanup:
    interval_secs: 3600
    retention_secs: 86400
    statuses: [completed, dead]
```

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct JobsConfig {
    pub poll_interval_secs: u64,       // default: 1
    pub stale_threshold_secs: u64,     // default: 600 (10 min)
    pub drain_timeout_secs: u64,       // default: 30
    pub queues: Vec<QueueConfig>,      // default: [{ name: "default", concurrency: 4 }]
    pub cleanup: CleanupConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    pub name: String,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CleanupConfig {
    pub interval_secs: u64,            // default: 3600 (1 hour)
    pub retention_secs: u64,           // default: 86400 (24 hours)
    pub statuses: Vec<String>,         // default: ["completed", "dead"]
}
```

If no queues are configured, a single `default` queue with concurrency 4 is used.

### Startup Validation

At startup, all `JobRegistration` queue names are cross-referenced against the config. If a job references a queue not in the config, the app **panics** with a clear error message. This catches typos immediately.

## User-Facing API

### Defining Jobs

```rust
// Queued job with payload
#[modo_jobs::job(queue = "emails", priority = 10, max_retries = 3, timeout = "30s")]
async fn send_welcome_email(
    payload: WelcomePayload,
    mailer: Service<Mailer>,
) -> Result<(), modo::Error> {
    mailer.send(&payload.email, "Welcome!").await
}

// Queued job with DB access
#[modo_jobs::job(queue = "reports", priority = 1, timeout = "5m")]
async fn generate_report(payload: ReportPayload, Db(db): Db) -> Result<(), modo::Error> {
    // heavy work
}

// Simple job (default queue, default priority)
#[modo_jobs::job(max_retries = 3, timeout = "30s")]
async fn simple_task(payload: Payload) -> Result<(), modo::Error> {
    // ...
}

// Cron job (in-memory, no queue)
#[modo_jobs::job(cron = "0 0 * * * *", timeout = "5m")]
async fn hourly_cleanup(Db(db): Db) -> Result<(), modo::Error> {
    // runs every hour
}
```

### Macro Attributes

**Queued jobs:**
- `queue` — queue name (default: `"default"`)
- `priority` — integer, higher = runs first (default: `0`)
- `max_retries` — retry count before dead (default: `3`)
- `timeout` — execution timeout, e.g. `"30s"`, `"5m"`, `"1h"` (default: `"5m"`)

**Cron jobs:**
- `cron` — cron expression (6-field: sec min hour day month weekday)
- `timeout` — execution timeout

**Mutual exclusion:** `cron` and `queue`/`priority`/`max_retries` cannot be combined. A job is either cron-scheduled or queue-enqueued. Compile error if both are set.

### Macro-Generated Code

For `send_welcome_email`, the macro generates:

```rust
pub struct SendWelcomeEmailJob;

impl modo_jobs::JobHandler for SendWelcomeEmailJob {
    async fn run(&self, ctx: modo_jobs::JobContext) -> Result<(), modo::Error> {
        let payload: WelcomePayload = ctx.payload()?;
        let mailer = Service(ctx.service::<Mailer>()?);
        __job_send_welcome_email_impl(payload, mailer).await
    }
}

impl SendWelcomeEmailJob {
    pub async fn enqueue(
        queue: &modo_jobs::JobQueue,
        payload: &WelcomePayload,
    ) -> Result<modo_jobs::JobId, modo::Error> { ... }

    pub async fn enqueue_at(
        queue: &modo_jobs::JobQueue,
        payload: &WelcomePayload,
        run_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<modo_jobs::JobId, modo::Error> { ... }
}

inventory::submit! { modo_jobs::JobRegistration { ... } }
```

### Enqueuing Jobs

```rust
// In a handler — JobQueue is an axum extractor
#[modo::handler(POST, "/users")]
async fn create_user(queue: JobQueue, Db(db): Db) -> Result<Json<User>, modo::Error> {
    let user = create_user_in_db(&db).await?;
    SendWelcomeEmailJob::enqueue(&queue, &WelcomePayload { email: user.email.clone() }).await?;
    Ok(Json(user))
}

// Delayed enqueue
SendWelcomeEmailJob::enqueue_at(&queue, &payload, Utc::now() + Duration::hours(1)).await?;

// Cancel a job
queue.cancel(&job_id).await?;
```

### App Setup

```rust
#[derive(Default, Deserialize)]
struct AppConfig {
    #[serde(flatten)]
    server: modo::config::ServerConfig,
    database: DatabaseConfig,
    jobs: modo_jobs::JobsConfig,
}

#[modo::main]
async fn main(app: modo::app::AppBuilder) -> Result<(), Box<dyn std::error::Error>> {
    let config: AppConfig = modo::config::load_or_default()?;
    let db = modo_db::connect(&config.database).await?;
    modo_db::sync_and_migrate(&db).await?;

    let jobs = modo_jobs::start(&db, &config.jobs).await?;

    app.server_config(config.server)
       .service(db)
       .service(jobs)
       .run()
       .await
}
```

`modo_jobs::start()` creates the jobs table, validates queue config, and spawns background tasks. Returns `JobsHandle` which is registered as a service and provides the `JobQueue` extractor.

Users decide whether to pass the same DB or a separate one for jobs:

```rust
// Separate DB — zero write contention with app tables
let jobs_db = modo_db::connect(&config.jobs_database).await?;
let jobs = modo_jobs::start(&jobs_db, &config.jobs).await?;
```

## Runner Internals

### Architecture

`modo_jobs::start()` spawns background tasks:

1. **Poll loop (one per queue)** — each queue has its own semaphore and claim loop
2. **Stale reaper** — single task, checks all queues for stuck running jobs
3. **Cleanup** — single task, purges old jobs by configured statuses and retention

All tasks share a `CancellationToken` for graceful shutdown.

### Atomic Claim (SQLite-optimized)

Single-statement claim minimizes write lock duration:

```sql
UPDATE modo_jobs
SET state = 'running',
    locked_by = ?,
    locked_at = ?,
    attempts = attempts + 1,
    updated_at = ?
WHERE id = (
    SELECT id FROM modo_jobs
    WHERE state = 'pending'
      AND queue = ?
      AND run_at <= ?
    ORDER BY priority DESC, run_at ASC
    LIMIT 1
)
RETURNING *;
```

Write lock held for microseconds — no transactions, no SELECT+UPDATE round trip.

For Postgres, runtime detection via `db.get_database_backend()` can use `FOR UPDATE SKIP LOCKED` in the subquery for better concurrent worker performance.

### Multiple Workers

Multiple app instances can poll the same database safely. The atomic claim ensures no double-processing. `locked_by` identifies which worker holds each job.

### Retry with Exponential Backoff

```
backoff = 5s * 2^(attempt-1), capped at 1h
```

On failure: `state = 'pending'`, `run_at = now + backoff`.
On exhausted retries: `state = 'dead'`.

### Failure Logging

All failures logged via structured tracing:

```rust
error!(
    job_id = %id,
    job_name = %name,
    queue = %queue,
    attempt = attempt,
    max_retries = max_retries,
    error = %err,
    "Job failed"
);
```

### Timeout

Each job execution wrapped in `tokio::time::timeout()`. Timeout counts as a failure (triggers retry or dead).

### Job Lifecycle

```
pending --[claimed]--> running --[success]--> completed
                          |
                          +--[failure, retries left]--> pending (run_at += backoff)
                          +--[failure, no retries]----> dead
                          +--[timeout]----------------> (same as failure)
                          +--[stale reap]-------------> pending (re-queued)
```

### Stale Reaper

Periodic task (every 60s) that resets stuck `running` jobs older than `stale_threshold_secs` back to `pending`. Handles crashed workers.

### Auto-Cleanup

Periodic task that deletes jobs matching configured statuses older than `retention_secs`. Default: purge `completed` and `dead` jobs older than 24 hours, every hour.

### Graceful Shutdown

1. `CancellationToken` signals all tasks to stop
2. Poll loops stop claiming new jobs
3. Wait up to `drain_timeout_secs` for in-flight jobs to complete
4. Stale reaper and cleanup tasks abort

## Cron Jobs

In-memory only — tokio timers, no DB persistence.

- Each cron job gets its own `tokio::spawn` task with a sleep loop
- Cron expression validated at startup (panic on invalid)
- Failures logged via `tracing`, no retry (next run happens on schedule)
- Cancelled via the shared `CancellationToken`
- Mutually exclusive with queue attributes

## Public API

```rust
// Types
pub struct JobId(String);
pub enum JobState { Pending, Running, Completed, Failed, Dead }

// Config
pub struct JobsConfig { ... }
pub struct QueueConfig { ... }
pub struct CleanupConfig { ... }

// Queue (axum extractor)
pub struct JobQueue { ... }
impl JobQueue {
    pub async fn enqueue<T: Serialize>(&self, name: &str, payload: &T) -> Result<JobId, Error>;
    pub async fn enqueue_at<T: Serialize>(&self, name: &str, payload: &T, run_at: DateTime<Utc>) -> Result<JobId, Error>;
    pub async fn cancel(&self, id: &JobId) -> Result<(), Error>;
}

// Startup
pub async fn start(db: &DbPool, config: &JobsConfig) -> Result<JobsHandle, Error>;

// Handle (registered as service, derefs to JobQueue)
pub struct JobsHandle { ... }

// Handler trait + registration (used by macro internals)
pub trait JobHandler: Send + Sync + 'static { ... }
pub trait JobHandlerDyn: Send + Sync + 'static { ... }
pub struct JobRegistration { ... }
pub struct JobContext { ... }

// Macro re-export
pub use modo_jobs_macros::job;
```

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Storage | Direct DB, no trait abstraction | Swap the crate, not the backend |
| DB connection | Receives `DbPool`, no opinion on same/separate DB | User decides topology |
| Claim strategy | Single-statement `UPDATE...RETURNING` | Minimal SQLite write lock duration |
| Concurrency | Per-queue semaphores | Resource isolation prevents starvation |
| Priority | Per-job integer in claim `ORDER BY` | Simple, effective, no extra config |
| Queue validation | Panic at startup on unknown queue | Catches typos immediately |
| Error storage | `tracing` logs only, no `last_error` column | Less DB writes, logs have full context |
| Cron | In-memory tokio timers | No DB persistence needed for schedules |
| Cron vs queue | Mutually exclusive | Different execution models, no overlap |
| Cleanup | Auto with configurable retention + statuses | Jobs table is a work queue, not audit log |
| Config pattern | `#[serde(default)]` YAML config | Matches modo core and modo-db conventions |
| Macro crate | Separate `modo-jobs-macros` | Core independence, modo-jobs re-exports |
| DI in jobs | Macro supports payload + `Service<T>` + `Db` | Matches handler pattern, convenient |
| Enqueue API | `JobQueue` extractor, no global singleton | Testable, explicit |
