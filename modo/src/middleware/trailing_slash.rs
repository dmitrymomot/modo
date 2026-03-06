use crate::app::AppState;
use crate::config::TrailingSlash;
use axum::extract::State;
use axum::http::{Request, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn trailing_slash_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match state.server_config.http.trailing_slash {
        TrailingSlash::None => next.run(request).await,
        TrailingSlash::Strip => {
            let path = request.uri().path();
            if path != "/" && path.ends_with('/') {
                return redirect_to(request.uri(), path.trim_end_matches('/'));
            }
            next.run(request).await
        }
        TrailingSlash::Add => {
            let path = request.uri().path();
            if path != "/" && !path.ends_with('/') {
                let new_path = format!("{path}/");
                return redirect_to(request.uri(), &new_path);
            }
            next.run(request).await
        }
    }
}

fn redirect_to(original: &Uri, new_path: &str) -> Response {
    let new_uri = if let Some(query) = original.query() {
        format!("{new_path}?{query}")
    } else {
        new_path.to_string()
    };

    (
        StatusCode::MOVED_PERMANENTLY,
        [(axum::http::header::LOCATION, new_uri)],
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_preserves_query() {
        let uri: Uri = "/foo/?bar=baz".parse().unwrap();
        let response = redirect_to(&uri, "/foo");
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(response.headers().get("location").unwrap(), "/foo?bar=baz");
    }

    #[test]
    fn test_redirect_no_query() {
        let uri: Uri = "/foo/".parse().unwrap();
        let response = redirect_to(&uri, "/foo");
        assert_eq!(response.headers().get("location").unwrap(), "/foo");
    }
}
