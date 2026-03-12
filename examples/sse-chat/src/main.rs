mod chat;
mod config;
mod entity;
mod handlers_auth;
mod types;
mod views;

use modo::sse::SseBroadcastManager;

#[modo::main]
async fn main(
    app: modo::app::AppBuilder,
    config: config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = modo_db::connect(&config.database).await?;
    modo_db::sync_and_migrate(&db).await?;

    let session_store = modo_session::SessionStore::new(
        &db,
        modo_session::SessionConfig::default(),
        config.core.cookies.clone(),
    );

    let bc: types::ChatBroadcaster = SseBroadcastManager::new(128);

    app.config(config.core)
        .managed_service(db)
        .service(session_store.clone())
        .service(bc)
        .layer(modo_session::layer(session_store))
        .run()
        .await
}
