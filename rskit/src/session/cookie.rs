use crate::app::AppState;
use crate::config::Environment;
use crate::session::SessionId;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponseParts, ResponseParts};
use axum_extra::extract::cookie::{Key, PrivateCookieJar};
use cookie::Cookie;

/// Helper for setting and removing the encrypted session cookie.
///
/// Extract in handler, call `.set()` or `.remove()`, return as part of response tuple.
pub struct SessionCookie {
    jar: PrivateCookieJar,
    cookie_name: String,
    is_secure: bool,
    session_ttl: std::time::Duration,
}

impl SessionCookie {
    /// Set the session cookie with the given session ID.
    pub fn set(self, session_id: SessionId) -> Self {
        let mut cookie = Cookie::new(self.cookie_name.clone(), session_id.to_string());
        cookie.set_http_only(true);
        cookie.set_same_site(cookie::SameSite::Lax);
        cookie.set_path("/");
        cookie.set_secure(self.is_secure);
        cookie.set_max_age(cookie::time::Duration::seconds(
            self.session_ttl.as_secs() as i64
        ));
        Self {
            jar: self.jar.add(cookie),
            cookie_name: self.cookie_name,
            is_secure: self.is_secure,
            session_ttl: self.session_ttl,
        }
    }

    /// Remove the session cookie.
    pub fn remove(self) -> Self {
        Self {
            jar: self.jar.remove(Cookie::from(self.cookie_name.clone())),
            cookie_name: self.cookie_name,
            is_secure: self.is_secure,
            session_ttl: self.session_ttl,
        }
    }
}

impl FromRequestParts<AppState> for SessionCookie {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state)
            .await
            .expect("PrivateCookieJar is infallible");
        Ok(Self {
            jar,
            cookie_name: state.config.session_cookie_name.clone(),
            is_secure: state.config.environment == Environment::Production,
            session_ttl: state.config.session_ttl,
        })
    }
}

impl IntoResponseParts for SessionCookie {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        self.jar.into_response_parts(res)
    }
}
