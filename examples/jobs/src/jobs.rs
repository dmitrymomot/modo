use modo::HandlerResult;

use crate::payloads::{GreetingPayload, ReminderPayload};

#[modo_jobs::job(queue = "default")]
async fn say_hello(payload: GreetingPayload) -> HandlerResult<()> {
    tracing::info!(name = %payload.name, "Hello, {}!", payload.name);
    Ok(())
}

#[modo_jobs::job(queue = "default")]
async fn remind(payload: ReminderPayload) -> HandlerResult<()> {
    tracing::info!(reminder_message = %payload.message, "Reminder: {}", payload.message);
    Ok(())
}

#[modo_jobs::job(cron = "0 */1 * * * *", timeout = "30s")]
async fn heartbeat() -> HandlerResult<()> {
    tracing::info!("heartbeat tick");
    Ok(())
}
