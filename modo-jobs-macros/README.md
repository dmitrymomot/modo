# modo-jobs-macros

[![docs.rs](https://img.shields.io/docsrs/modo-jobs-macros)](https://docs.rs/modo-jobs-macros)

Procedural macro companion for `modo-jobs`.

This crate provides the `#[job]` attribute macro. It is re-exported by
`modo-jobs` as `modo_jobs::job` — import it from there rather than depending
on this crate directly.

## Usage

Add `modo-jobs` to your `Cargo.toml`:

```toml
[dependencies]
modo-jobs = { version = "0.2" }
```

## `#[job]`

Annotates an async function as a background job handler.

```rust,ignore
use modo_jobs::job;
use modo::HandlerResult;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct EmailPayload {
    to: String,
}

#[job(queue = "emails", max_attempts = 3, timeout = "1m")]
async fn send_email(payload: EmailPayload) -> HandlerResult<()> {
    tracing::info!(to = %payload.to, "Sending email");
    Ok(())
}
```

### What the macro generates

For a function `send_email`, the macro emits:

- `SendEmailJob` — a unit struct implementing `modo_jobs::JobHandler`
- `SendEmailJob::JOB_NAME: &str` — the snake_case function name (`"send_email"`)
- `SendEmailJob::enqueue(&queue, &payload) -> Result<JobId, Error>` — enqueue for immediate execution
- `SendEmailJob::enqueue_at(&queue, &payload, run_at) -> Result<JobId, Error>` — schedule for a future time
- An `inventory` registration for automatic discovery at startup

Cron jobs omit `enqueue` and `enqueue_at`.

### Parameters

| Parameter      | Type                          | Default     | Description                                                                           |
|----------------|-------------------------------|-------------|---------------------------------------------------------------------------------------|
| `queue`        | string                        | `"default"` | Target queue name (must match a configured queue)                                     |
| `priority`     | integer                       | `0`         | Higher values run first within the same queue                                         |
| `max_attempts` | integer                       | `3`         | Retries before the job is marked `dead`                                               |
| `timeout`      | `"Xs"` / `"Xm"` / `"Xh"`    | `"5m"`      | Per-execution timeout                                                                 |
| `cron`         | cron expression               | —           | Recurring in-memory job; mutually exclusive with `queue`, `priority`, `max_attempts`  |

### Function Signature Rules

- Must be `async`.
- At most one plain parameter is the **payload** (deserialized from JSON via `serde`).
- Use `Service<T>` to inject a registered service and `Db` to inject the database pool.
- Return type must be `Result<(), modo::Error>` or a compatible alias such as `HandlerResult<()>`.

### Enqueueing from an HTTP Handler

```rust,ignore
use modo_jobs::JobQueue;
use modo::{Json, JsonResult};

#[modo::handler(POST, "/emails/send")]
async fn send_email_handler(
    queue: JobQueue,
    input: modo::extractor::JsonReq<EmailPayload>,
) -> JsonResult<serde_json::Value> {
    let job_id = SendEmailJob::enqueue(&queue, &input).await?;
    Ok(Json(serde_json::json!({ "job_id": job_id.to_string() })))
}
```

### Scheduling a Job for Later

```rust,ignore
let run_at = chrono::Utc::now() + chrono::Duration::seconds(60);
let job_id = SendEmailJob::enqueue_at(&queue, &payload, run_at).await?;
```

### Cron Jobs

```rust,ignore
use modo_jobs::job;
use modo::HandlerResult;

#[job(cron = "0 0 * * * *", timeout = "30s")]
async fn hourly_cleanup() -> HandlerResult<()> {
    tracing::info!("Running hourly cleanup");
    Ok(())
}
```

Cron jobs run in memory only, are not persisted to the database, and do not
generate `enqueue` / `enqueue_at` methods.

## Integration with modo

Start the job runner in `main` and register the `JobsHandle` as a managed service:

```rust,ignore
#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = modo_db::connect(&config.database).await?;
    modo_db::sync_and_migrate(&db).await?;

    let jobs = modo_jobs::new(&db, &config.jobs)
        .service(db.clone())
        .run()
        .await?;

    app.config(config.core)
        .managed_service(db)
        .managed_service(jobs)
        .run()
        .await
}
```

`JobsHandle` implements `modo::GracefulShutdown` and drains in-flight jobs on
shutdown. `JobQueue` is available as an axum extractor once `JobsHandle` is
registered as a service.

## Key Types (from `modo-jobs`)

| Type              | Description                                              |
|-------------------|----------------------------------------------------------|
| `JobQueue`        | Extractor and handle for enqueuing and cancelling jobs   |
| `JobsHandle`      | Returned by `JobsBuilder::run`; manages the runner lifecycle |
| `JobContext`      | Passed to each handler; provides payload, services, and db |
| `JobRegistration` | Static metadata collected by `inventory` (generated by `#[job]`) |
| `JobId`           | ULID-backed unique job identifier                        |
| `JobState`        | Enum: `Pending`, `Running`, `Completed`, `Dead`, `Cancelled` |
