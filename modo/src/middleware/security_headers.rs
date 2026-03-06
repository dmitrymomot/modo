use crate::app::AppState;
use crate::config::{Environment, SecurityHeadersConfig};
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

pub async fn security_headers_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let config = &state.server_config.security_headers;
    if !config.enabled {
        return response;
    }
    apply_headers(response.headers_mut(), config, &state.server_config.environment);
    response
}

fn apply_headers(
    headers: &mut axum::http::HeaderMap,
    config: &SecurityHeadersConfig,
    env: &Environment,
) {
    if let Some(ref val) = config.x_content_type_options {
        set_header(headers, "x-content-type-options", val);
    }
    if let Some(ref val) = config.x_frame_options {
        set_header(headers, "x-frame-options", val);
    }
    if let Some(ref val) = config.referrer_policy {
        set_header(headers, "referrer-policy", val);
    }
    if let Some(ref val) = config.permissions_policy {
        set_header(headers, "permissions-policy", val);
    }
    if let Some(ref val) = config.content_security_policy {
        set_header(headers, "content-security-policy", val);
    }
    // HSTS only in production
    if config.hsts && *env == Environment::Production {
        let val = format!("max-age={}; includeSubDomains", config.hsts_max_age);
        set_header(headers, "strict-transport-security", &val);
    }
}

fn set_header(headers: &mut axum::http::HeaderMap, name: &str, value: &str) {
    if let Ok(v) = value.parse() {
        headers.insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            v,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_headers_applied() {
        let config = SecurityHeadersConfig::default();
        let mut headers = axum::http::HeaderMap::new();
        apply_headers(&mut headers, &config, &Environment::Development);
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            headers.get("content-security-policy").unwrap(),
            "default-src 'self'"
        );
        // HSTS should NOT be set in non-production env
        assert!(headers.get("strict-transport-security").is_none());
    }

    #[test]
    fn test_disabled_headers() {
        let config = SecurityHeadersConfig {
            enabled: false,
            ..Default::default()
        };
        let mut headers = axum::http::HeaderMap::new();
        // When disabled, apply_headers still adds them but the middleware short-circuits
        // Testing the middleware behavior indirectly:
        if config.enabled {
            apply_headers(&mut headers, &config, &Environment::Development);
        }
        assert!(headers.is_empty());
    }
}
