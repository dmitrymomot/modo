use sea_orm::DatabaseConnection;
use std::ops::Deref;

/// Newtype around `sea_orm::DatabaseConnection`.
///
/// Registered as a service via `app.service(db)` and extracted
/// in handlers via the `Db` extractor.
#[derive(Debug, Clone)]
pub struct DbPool(pub(crate) DatabaseConnection);

impl DbPool {
    /// Access the underlying SeaORM connection.
    pub fn connection(&self) -> &DatabaseConnection {
        &self.0
    }
}

impl Deref for DbPool {
    type Target = DatabaseConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
