mod common;

use common::setup_db;
use modo_db::sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};
use modo_jobs::entity::job as jobs_entity;
use modo_jobs::{JobId, JobState};

#[tokio::test]
async fn test_insert_and_query_job() {
    let db = setup_db().await;
    let now = chrono::Utc::now();
    let id = JobId::new();

    let model = jobs_entity::ActiveModel {
        id: ActiveValue::Set(id.as_str().to_string()),
        name: ActiveValue::Set("test_job".to_string()),
        queue: ActiveValue::Set("default".to_string()),
        payload: ActiveValue::Set("{}".to_string()),
        state: ActiveValue::Set(JobState::Pending.as_str().to_string()),
        priority: ActiveValue::Set(0),
        attempts: ActiveValue::Set(0),
        max_attempts: ActiveValue::Set(3),
        run_at: ActiveValue::Set(now),
        timeout_secs: ActiveValue::Set(300),
        locked_by: ActiveValue::Set(None),
        locked_at: ActiveValue::Set(None),
        last_error: ActiveValue::Set(None),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
    };

    model.insert(&db).await.expect("Insert failed");

    let found = jobs_entity::Entity::find_by_id(id.as_str())
        .one(&db)
        .await
        .expect("Query failed");

    let found = found.expect("Job not found");
    assert_eq!(found.name, "test_job");
    assert_eq!(found.queue, "default");
    assert_eq!(found.state, "pending");
    assert_eq!(found.attempts, 0);
    assert_eq!(found.max_attempts, 3);
}

#[tokio::test]
async fn test_cancel_pending_job() {
    use modo_db::sea_orm::{ColumnTrait, QueryFilter};

    let db = setup_db().await;
    let now = chrono::Utc::now();
    let id = JobId::new();

    let model = jobs_entity::ActiveModel {
        id: ActiveValue::Set(id.as_str().to_string()),
        name: ActiveValue::Set("cancel_test".to_string()),
        queue: ActiveValue::Set("default".to_string()),
        payload: ActiveValue::Set("{}".to_string()),
        state: ActiveValue::Set(JobState::Pending.as_str().to_string()),
        priority: ActiveValue::Set(0),
        attempts: ActiveValue::Set(0),
        max_attempts: ActiveValue::Set(3),
        run_at: ActiveValue::Set(now),
        timeout_secs: ActiveValue::Set(300),
        locked_by: ActiveValue::Set(None),
        locked_at: ActiveValue::Set(None),
        last_error: ActiveValue::Set(None),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
    };

    model.insert(&db).await.expect("Insert failed");

    // Cancel via direct UPDATE (mimicking JobQueue::cancel without needing registry)
    let result = jobs_entity::Entity::update_many()
        .filter(jobs_entity::Column::Id.eq(id.as_str()))
        .filter(jobs_entity::Column::State.eq(JobState::Pending.as_str()))
        .col_expr(
            jobs_entity::Column::State,
            modo_db::sea_orm::sea_query::Expr::value(JobState::Cancelled.as_str()),
        )
        .exec(&db)
        .await
        .expect("Cancel failed");

    assert_eq!(result.rows_affected, 1);

    let found = jobs_entity::Entity::find_by_id(id.as_str())
        .one(&db)
        .await
        .expect("Query failed")
        .expect("Job not found");

    assert_eq!(found.state, "cancelled");
}
