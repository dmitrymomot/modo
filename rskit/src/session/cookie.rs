use crate::app::AppState;
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
}

impl SessionCookie {
    /// Set the session cookie with the given session ID.
    pub fn set(self, session_id: SessionId) -> Self {
        let mut cookie = Cookie::new(self.cookie_name.clone(), session_id.to_string());
        cookie.set_http_only(true);
        cookie.set_same_site(cookie::SameSite::Lax);
        cookie.set_path("/");
        Self {
            jar: self.jar.add(cookie),
            cookie_name: self.cookie_name,
        }
    }

    /// Remove the session cookie.
    pub fn remove(self) -> Self {
        Self {
            jar: self.jar.remove(Cookie::from(self.cookie_name.clone())),
            cookie_name: self.cookie_name,
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
        })
    }
}

impl IntoResponseParts for SessionCookie {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        self.jar.into_response_parts(res)
    }
}
