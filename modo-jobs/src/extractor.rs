use crate::queue::JobQueue;
use modo::app::AppState;
use modo::axum::extract::FromRequestParts;
use modo::axum::http::request::Parts;
use modo::error::Error;

impl FromRequestParts<AppState> for JobQueue {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        state
            .services
            .get::<JobQueue>()
            .map(|q| JobQueue {
                db: q.db.clone(),
            })
            .ok_or_else(|| {
                Error::internal(
                    "JobQueue not configured. Start the job runner and register JobsHandle as a service.",
                )
            })
    }
}
