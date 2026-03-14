use modo_db::sea_orm::{ConnectionTrait, Database, Schema};

pub async fn setup_db() -> modo_db::sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect");

    let schema = Schema::new(db.get_database_backend());
    let mut builder = schema.builder();
    let reg = inventory::iter::<modo_db::EntityRegistration>()
        .find(|r| r.table_name == "modo_jobs")
        .unwrap();
    builder = (reg.register_fn)(builder);
    builder.sync(&db).await.expect("Schema sync failed");
    for sql in reg.extra_sql {
        db.execute_unprepared(sql).await.expect("Extra SQL failed");
    }
    db
}
