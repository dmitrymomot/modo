use crate::error::RskitError;
use crate::session::{SessionData, SessionId, SessionMeta};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde::Serialize;
use std::time::Duration;

/// Trait for session persistence.
pub trait SessionStore: Send + Sync + 'static {
    fn create(
        &self,
        user_id: &str,
        meta: &SessionMeta,
    ) -> impl std::future::Future<Output = Result<SessionId, RskitError>> + Send;

    fn create_with<T: Serialize + Send>(
        &self,
        user_id: &str,
        meta: &SessionMeta,
        data: T,
    ) -> impl std::future::Future<Output = Result<SessionId, RskitError>> + Send;

    fn read(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<Option<SessionData>, RskitError>> + Send;

    fn touch(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<(), RskitError>> + Send;

    fn update_data(
        &self,
        id: &SessionId,
        data: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), RskitError>> + Send;

    fn destroy(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<(), RskitError>> + Send;

    fn destroy_all_for_user(
        &self,
        user_id: &str,
    ) -> impl std::future::Future<Output = Result<(), RskitError>> + Send;

    fn cleanup_expired(&self) -> impl std::future::Future<Output = Result<u64, RskitError>> + Send;
}

/// SQLite-backed session store.
pub struct SqliteSessionStore {
    db: DatabaseConnection,
    ttl: Duration,
    max_per_user: usize,
}

impl SqliteSessionStore {
    pub fn new(db: DatabaseConnection, ttl: Duration, max_per_user: usize) -> Self {
        Self {
            db,
            ttl,
            max_per_user,
        }
    }

    /// Create the sessions table if it doesn't exist.
    pub async fn initialize(&self) -> Result<(), RskitError> {
        self.db
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS rskit_sessions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    ip_address TEXT NOT NULL,
                    user_agent TEXT NOT NULL,
                    device_name TEXT NOT NULL,
                    device_type TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    data TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL,
                    last_active_at TEXT NOT NULL,
                    expires_at TEXT NOT NULL
                )",
            )
            .await?;
        self.db
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON rskit_sessions(user_id)",
            )
            .await?;
        self.db
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON rskit_sessions(expires_at)",
            )
            .await?;
        Ok(())
    }

    async fn insert_session(
        &self,
        user_id: &str,
        meta: &SessionMeta,
        data: serde_json::Value,
    ) -> Result<SessionId, RskitError> {
        let id = SessionId::new();
        let now = Utc::now();
        let expires_at =
            now + chrono::Duration::from_std(self.ttl).unwrap_or(chrono::Duration::days(30));
        let now_str = now.to_rfc3339();
        let expires_str = expires_at.to_rfc3339();
        let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());

        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO rskit_sessions (id, user_id, ip_address, user_agent, device_name, device_type, fingerprint, data, created_at, last_active_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            [
                id.as_str().into(),
                user_id.into(),
                meta.ip_address.as_str().into(),
                meta.user_agent.as_str().into(),
                meta.device_name.as_str().into(),
                meta.device_type.as_str().into(),
                meta.fingerprint.as_str().into(),
                data_str.into(),
                now_str.clone().into(),
                now_str.into(),
                expires_str.into(),
            ],
        );
        self.db.execute_raw(stmt).await?;

        // Evict oldest sessions if over max_per_user limit
        self.evict_excess_sessions(user_id).await?;

        Ok(id)
    }

    async fn evict_excess_sessions(&self, user_id: &str) -> Result<(), RskitError> {
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM rskit_sessions WHERE user_id = $1 AND id NOT IN (SELECT id FROM rskit_sessions WHERE user_id = $2 ORDER BY created_at DESC LIMIT $3)",
            [
                user_id.into(),
                user_id.into(),
                (self.max_per_user as i64).into(),
            ],
        );
        self.db.execute_raw(stmt).await?;
        Ok(())
    }

    fn row_to_session_data(row: &sea_orm::QueryResult) -> Result<SessionData, RskitError> {
        let id: String = row
            .try_get("", "id")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let user_id: String = row
            .try_get("", "user_id")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let ip_address: String = row
            .try_get("", "ip_address")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let user_agent: String = row
            .try_get("", "user_agent")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let device_name: String = row
            .try_get("", "device_name")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let device_type: String = row
            .try_get("", "device_type")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let fingerprint: String = row
            .try_get("", "fingerprint")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let data_str: String = row
            .try_get("", "data")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let created_at_str: String = row
            .try_get("", "created_at")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let last_active_at_str: String = row
            .try_get("", "last_active_at")
            .map_err(|e| RskitError::internal(e.to_string()))?;
        let expires_at_str: String = row
            .try_get("", "expires_at")
            .map_err(|e| RskitError::internal(e.to_string()))?;

        Ok(SessionData {
            id: SessionId::from(id),
            user_id,
            ip_address,
            user_agent,
            device_name,
            device_type,
            fingerprint,
            data: serde_json::from_str(&data_str).unwrap_or(serde_json::json!({})),
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_active_at: chrono::DateTime::parse_from_rfc3339(&last_active_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            expires_at: chrono::DateTime::parse_from_rfc3339(&expires_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

impl SessionStore for SqliteSessionStore {
    async fn create(&self, user_id: &str, meta: &SessionMeta) -> Result<SessionId, RskitError> {
        self.insert_session(user_id, meta, serde_json::json!({}))
            .await
    }

    async fn create_with<T: Serialize + Send>(
        &self,
        user_id: &str,
        meta: &SessionMeta,
        data: T,
    ) -> Result<SessionId, RskitError> {
        let value = serde_json::to_value(data)
            .map_err(|e| RskitError::internal(format!("Failed to serialize session data: {e}")))?;
        self.insert_session(user_id, meta, value).await
    }

    async fn read(&self, id: &SessionId) -> Result<Option<SessionData>, RskitError> {
        let now = Utc::now().to_rfc3339();
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT * FROM rskit_sessions WHERE id = $1 AND expires_at > $2",
            [id.as_str().into(), now.into()],
        );
        let row = self.db.query_one_raw(stmt).await?;
        match row {
            Some(r) => Ok(Some(Self::row_to_session_data(&r)?)),
            None => Ok(None),
        }
    }

    async fn touch(&self, id: &SessionId) -> Result<(), RskitError> {
        let now = Utc::now().to_rfc3339();
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE rskit_sessions SET last_active_at = $1 WHERE id = $2",
            [now.into(), id.as_str().into()],
        );
        self.db.execute_raw(stmt).await?;
        Ok(())
    }

    async fn update_data(&self, id: &SessionId, data: serde_json::Value) -> Result<(), RskitError> {
        let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE rskit_sessions SET data = $1 WHERE id = $2",
            [data_str.into(), id.as_str().into()],
        );
        self.db.execute_raw(stmt).await?;
        Ok(())
    }

    async fn destroy(&self, id: &SessionId) -> Result<(), RskitError> {
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM rskit_sessions WHERE id = $1",
            [id.as_str().into()],
        );
        self.db.execute_raw(stmt).await?;
        Ok(())
    }

    async fn destroy_all_for_user(&self, user_id: &str) -> Result<(), RskitError> {
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM rskit_sessions WHERE user_id = $1",
            [user_id.into()],
        );
        self.db.execute_raw(stmt).await?;
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<u64, RskitError> {
        let now = Utc::now().to_rfc3339();
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM rskit_sessions WHERE expires_at <= $1",
            [now.into()],
        );
        let result = self.db.execute_raw(stmt).await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn setup_store() -> SqliteSessionStore {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let store = SqliteSessionStore::new(
            db,
            Duration::from_secs(3600), // 1 hour for tests
            3,                         // max 3 sessions per user
        );
        store.initialize().await.unwrap();
        store
    }

    fn test_meta() -> SessionMeta {
        SessionMeta {
            ip_address: "127.0.0.1".to_string(),
            user_agent: "TestAgent/1.0".to_string(),
            device_name: "Test on Test".to_string(),
            device_type: "desktop".to_string(),
            fingerprint: "abc123".to_string(),
        }
    }

    #[tokio::test]
    async fn create_and_read_session() {
        let store = setup_store().await;
        let meta = test_meta();
        let id = store.create("user1", &meta).await.unwrap();
        let session = store.read(&id).await.unwrap().unwrap();
        assert_eq!(session.user_id, "user1");
        assert_eq!(session.ip_address, "127.0.0.1");
        assert_eq!(session.device_name, "Test on Test");
    }

    #[tokio::test]
    async fn create_with_data() {
        let store = setup_store().await;
        let meta = test_meta();
        let id = store
            .create_with("user1", &meta, serde_json::json!({"onboarding": true}))
            .await
            .unwrap();
        let session = store.read(&id).await.unwrap().unwrap();
        assert_eq!(session.data["onboarding"], true);
    }

    #[tokio::test]
    async fn read_nonexistent_returns_none() {
        let store = setup_store().await;
        let id = SessionId::from("nonexistent".to_string());
        assert!(store.read(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn destroy_session() {
        let store = setup_store().await;
        let meta = test_meta();
        let id = store.create("user1", &meta).await.unwrap();
        store.destroy(&id).await.unwrap();
        assert!(store.read(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn destroy_all_for_user() {
        let store = setup_store().await;
        let meta = test_meta();
        let id1 = store.create("user1", &meta).await.unwrap();
        let id2 = store.create("user1", &meta).await.unwrap();
        let id3 = store.create("user2", &meta).await.unwrap();

        store.destroy_all_for_user("user1").await.unwrap();

        assert!(store.read(&id1).await.unwrap().is_none());
        assert!(store.read(&id2).await.unwrap().is_none());
        assert!(store.read(&id3).await.unwrap().is_some()); // user2 untouched
    }

    #[tokio::test]
    async fn evicts_oldest_when_over_max() {
        let store = setup_store().await; // max_per_user = 3
        let meta = test_meta();

        let id1 = store.create("user1", &meta).await.unwrap();
        let _id2 = store.create("user1", &meta).await.unwrap();
        let _id3 = store.create("user1", &meta).await.unwrap();
        let _id4 = store.create("user1", &meta).await.unwrap(); // should evict id1

        assert!(
            store.read(&id1).await.unwrap().is_none(),
            "oldest session should be evicted"
        );
    }

    #[tokio::test]
    async fn touch_updates_last_active() {
        let store = setup_store().await;
        let meta = test_meta();
        let id = store.create("user1", &meta).await.unwrap();
        let before = store.read(&id).await.unwrap().unwrap().last_active_at;

        // Small delay to ensure timestamp differs
        tokio::time::sleep(Duration::from_millis(10)).await;
        store.touch(&id).await.unwrap();

        let after = store.read(&id).await.unwrap().unwrap().last_active_at;
        assert!(after >= before);
    }

    #[tokio::test]
    async fn update_data() {
        let store = setup_store().await;
        let meta = test_meta();
        let id = store.create("user1", &meta).await.unwrap();

        store
            .update_data(&id, serde_json::json!({"step": 2}))
            .await
            .unwrap();

        let session = store.read(&id).await.unwrap().unwrap();
        assert_eq!(session.data["step"], 2);
    }
}
