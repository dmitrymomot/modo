use crate::handler::{JobContext, JobRegistration};
use crate::types::JobId;
use modo::app::ServiceRegistry;
use modo_db::pool::DbPool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Spawn in-memory cron job tasks for all registered cron jobs.
pub(crate) async fn start_cron_jobs(
    cancel: CancellationToken,
    services: ServiceRegistry,
    db_pool: Option<Arc<DbPool>>,
) {
    for reg in inventory::iter::<JobRegistration> {
        let Some(cron_expr) = reg.cron else {
            continue;
        };

        let schedule: cron::Schedule = cron_expr.parse().unwrap_or_else(|e| {
            panic!(
                "Invalid cron expression '{}' for job '{}': {e}",
                cron_expr, reg.name
            )
        });

        let cancel = cancel.clone();
        let services = services.clone();
        let db_pool = db_pool.clone();
        let name = reg.name;
        let timeout_secs = reg.timeout_secs;
        let handler_factory = reg.handler_factory;

        tokio::spawn(async move {
            run_cron_loop(
                cancel,
                services,
                db_pool,
                name,
                timeout_secs,
                handler_factory,
                schedule,
            )
            .await;
        });

        info!(job = reg.name, cron = cron_expr, "Scheduled cron job");
    }
}

async fn run_cron_loop(
    cancel: CancellationToken,
    services: ServiceRegistry,
    db_pool: Option<Arc<DbPool>>,
    name: &'static str,
    timeout_secs: u64,
    handler_factory: fn() -> Box<dyn crate::handler::JobHandlerDyn>,
    schedule: cron::Schedule,
) {
    loop {
        // Calculate time until next fire
        let now = chrono::Utc::now();
        let next = match schedule.upcoming(chrono::Utc).next() {
            Some(t) => t,
            None => {
                info!(job = name, "Cron schedule exhausted, stopping");
                break;
            }
        };

        let duration = (next - now).to_std().unwrap_or(std::time::Duration::ZERO);

        tokio::select! {
            _ = cancel.cancelled() => {
                info!(job = name, "Cron job shutting down");
                break;
            }
            _ = tokio::time::sleep(duration) => {
                let handler = handler_factory();
                let ctx = JobContext {
                    job_id: JobId::new(),
                    name: name.to_string(),
                    queue: "cron".to_string(),
                    attempt: 1,
                    services: services.clone(),
                    db: db_pool.as_ref().map(|p| (**p).clone()),
                    payload_json: "null".to_string(),
                };

                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    handler.run_dyn(ctx),
                )
                .await;

                match result {
                    Ok(Ok(())) => {
                        info!(job = name, "Cron job completed");
                    }
                    Ok(Err(e)) => {
                        error!(job = name, error = %e, "Cron job failed");
                    }
                    Err(_) => {
                        error!(job = name, "Cron job timed out");
                    }
                }
            }
        }
    }
}
