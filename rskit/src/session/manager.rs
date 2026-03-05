use crate::app::AppState;
use crate::error::RskitError;
use crate::session::meta::SessionMeta;
use crate::session::store::SessionStoreDyn;
use crate::session::types::{SessionData, SessionId};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) enum SessionAction {
    None,
    Set(SessionId),
    Remove,
}

#[derive(Clone)]
pub(crate) struct SessionManagerState {
    pub action: Arc<Mutex<SessionAction>>,
    pub meta: SessionMeta,
    pub store: Arc<dyn SessionStoreDyn>,
    pub current_session: Option<SessionData>,
}

/// High-level session API for handlers.
///
/// Handles login, logout, and session access without exposing cookies or IDs.
/// Requires the session middleware to be applied.
///
/// # Example
/// ```rust,ignore
/// #[handler(POST, "/login")]
/// async fn login(session: SessionManager, db: Db) -> Result<Redirect, RskitError> {
///     let user = verify_credentials(&db, &form).await?;
///     session.authenticate(&user.id).await?;
///     Ok(Redirect::to("/dashboard"))
/// }
/// ```
pub struct SessionManager {
    state: SessionManagerState,
}

impl SessionManager {
    /// Create a session for the given user.
    ///
    /// Destroys any existing session first (fixation prevention).
    /// The session cookie is set automatically on the response.
    pub async fn authenticate(&self, user_id: &str) -> Result<(), RskitError> {
        if let Some(ref session) = self.state.current_session {
            let _ = self.state.store.destroy(&session.id).await;
        }

        let session_id = self.state.store.create(user_id, &self.state.meta).await?;
        *self.state.action.lock().unwrap() = SessionAction::Set(session_id);
        Ok(())
    }

    /// Create a session with custom data attached.
    ///
    /// Same as [`authenticate()`](Self::authenticate) but stores additional JSON data.
    pub async fn authenticate_with(
        &self,
        user_id: &str,
        data: serde_json::Value,
    ) -> Result<(), RskitError> {
        if let Some(ref session) = self.state.current_session {
            let _ = self.state.store.destroy(&session.id).await;
        }

        let session_id = self
            .state
            .store
            .create_with(user_id, &self.state.meta, data)
            .await?;
        *self.state.action.lock().unwrap() = SessionAction::Set(session_id);
        Ok(())
    }

    /// Destroy the current session.
    ///
    /// The session cookie is removed automatically on the response.
    pub async fn logout(&self) -> Result<(), RskitError> {
        if let Some(ref session) = self.state.current_session {
            self.state.store.destroy(&session.id).await?;
        }
        *self.state.action.lock().unwrap() = SessionAction::Remove;
        Ok(())
    }

    /// Destroy ALL sessions for the current user.
    ///
    /// The session cookie is removed automatically on the response.
    pub async fn logout_all(&self) -> Result<(), RskitError> {
        if let Some(ref session) = self.state.current_session {
            self.state
                .store
                .destroy_all_for_user(&session.user_id)
                .await?;
        }
        *self.state.action.lock().unwrap() = SessionAction::Remove;
        Ok(())
    }

    /// Access the current session data (if authenticated).
    pub fn current(&self) -> Option<&SessionData> {
        self.state.current_session.as_ref()
    }
}

impl FromRequestParts<AppState> for SessionManager {
    type Rejection = RskitError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let state = parts
            .extensions
            .get::<SessionManagerState>()
            .cloned()
            .ok_or_else(|| RskitError::internal("SessionManager requires session middleware"))?;

        Ok(Self { state })
    }
}
