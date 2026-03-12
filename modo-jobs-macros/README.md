# modo-jobs-macros

Procedural macro companion for `modo-jobs`.

This crate provides the `#[job]` attribute macro. It is re-exported by
`modo-jobs` as `modo_jobs::job` — import it from there rather than depending
on this crate directly.

## `#[job]`

Annotates an async function as a background job handler.

```rust
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

The macro generates:

- A `SendEmailJob` unit struct implementing `JobHandler`
- `SendEmailJob::enqueue(&queue, &payload)` — enqueue for immediate execution
- `SendEmailJob::enqueue_at(&queue, &payload, run_at)` — schedule for later
- An `inventory` registration for automatic discovery at startup

### Parameters

| Parameter      | Type                     | Default     | Description                                                                          |
| -------------- | ------------------------ | ----------- | ------------------------------------------------------------------------------------ |
| `queue`        | string                   | `"default"` | Target queue name                                                                    |
| `priority`     | integer                  | `0`         | Higher values run first                                                              |
| `max_attempts` | integer                  | `3`         | Retries before the job is marked `dead`                                              |
| `timeout`      | `"Xs"` / `"Xm"` / `"Xh"` | `"5m"`      | Per-execution timeout                                                                |
| `cron`         | cron expression          | —           | Recurring in-memory job; mutually exclusive with `queue`, `priority`, `max_attempts` |

### Function Signature Rules

- Must be `async`.
- At most one plain parameter is the **payload** (deserialized from JSON).
- Additional dependencies: `Service<T>` for services, `Db` for the database pool.
- Return type: `Result<(), modo::Error>` or any compatible alias.

### Cron Jobs

```rust
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
