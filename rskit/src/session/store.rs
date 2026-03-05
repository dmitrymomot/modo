use crate::error::RskitError;
use crate::session::{SessionData, SessionId, SessionMeta};
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;

/// Trait for session persistence.
///
/// Implement this for your app's session backend (SQLite, Redis, etc.)
/// and register it via `app.session_store(my_store)`.
pub trait SessionStore: Send + Sync + 'static {
    fn create(
        &self,
        user_id: &str,
        meta: &SessionMeta,
    ) -> impl Future<Output = Result<SessionId, RskitError>> + Send;

    fn create_with<T: Serialize + Send>(
        &self,
        user_id: &str,
        meta: &SessionMeta,
        data: T,
    ) -> impl Future<Output = Result<SessionId, RskitError>> + Send;

    fn read(
        &self,
        id: &SessionId,
    ) -> impl Future<Output = Result<Option<SessionData>, RskitError>> + Send;

    fn touch(&self, id: &SessionId) -> impl Future<Output = Result<(), RskitError>> + Send;

    fn update_data(
        &self,
        id: &SessionId,
        data: serde_json::Value,
    ) -> impl Future<Output = Result<(), RskitError>> + Send;

    fn destroy(&self, id: &SessionId) -> impl Future<Output = Result<(), RskitError>> + Send;

    fn destroy_all_for_user(
        &self,
        user_id: &str,
    ) -> impl Future<Output = Result<(), RskitError>> + Send;

    fn cleanup_expired(&self) -> impl Future<Output = Result<u64, RskitError>> + Send;
}

/// Object-safe, type-erased version of [`SessionStore`].
///
/// This trait exists so we can store the session store as `Arc<dyn SessionStoreDyn>`
/// inside [`AppState`](crate::app::AppState). You should not need to implement this
/// directly; a blanket impl covers all `T: SessionStore`.
///
/// Only includes methods used by the session middleware and `SessionManager`.
/// For `update_data` and `cleanup_expired`, use the concrete store via `Service<MyStore>`.
pub trait SessionStoreDyn: Send + Sync + 'static {
    fn create<'a>(
        &'a self,
        user_id: &'a str,
        meta: &'a SessionMeta,
    ) -> Pin<Box<dyn Future<Output = Result<SessionId, RskitError>> + Send + 'a>>;

    fn create_with<'a>(
        &'a self,
        user_id: &'a str,
        meta: &'a SessionMeta,
        data: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<SessionId, RskitError>> + Send + 'a>>;

    fn read<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SessionData>, RskitError>> + Send + 'a>>;

    fn touch<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>>;

    fn destroy<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>>;

    fn destroy_all_for_user<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>>;
}

/// Blanket impl: any `SessionStore` automatically implements `SessionStoreDyn`.
impl<T: SessionStore> SessionStoreDyn for T {
    fn create<'a>(
        &'a self,
        user_id: &'a str,
        meta: &'a SessionMeta,
    ) -> Pin<Box<dyn Future<Output = Result<SessionId, RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::create(self, user_id, meta))
    }

    fn create_with<'a>(
        &'a self,
        user_id: &'a str,
        meta: &'a SessionMeta,
        data: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<SessionId, RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::create_with(self, user_id, meta, data))
    }

    fn read<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SessionData>, RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::read(self, id))
    }

    fn touch<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::touch(self, id))
    }

    fn destroy<'a>(
        &'a self,
        id: &'a SessionId,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::destroy(self, id))
    }

    fn destroy_all_for_user<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RskitError>> + Send + 'a>> {
        Box::pin(SessionStore::destroy_all_for_user(self, user_id))
    }
}
