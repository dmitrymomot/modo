//! Static file serving backends: filesystem (dev) and embedded (prod).

/// Configuration for static file serving.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct StaticConfig {
    /// Filesystem directory to serve from (used by `static-fs` backend).
    pub dir: String,
    /// URL prefix where static files are mounted.
    pub prefix: String,
    /// Cache-Control header value. `None` = backend-specific default.
    pub cache_control: Option<String>,
}

impl Default for StaticConfig {
    fn default() -> Self {
        Self {
            dir: "static".to_string(),
            prefix: "/static".to_string(),
            cache_control: None,
        }
    }
}

/// Build a filesystem-backed static file router (dev-friendly, 1h cache).
#[cfg(feature = "static-fs")]
pub fn build_fs_service(config: &StaticConfig) -> axum::Router<()> {
    use axum::http::header;
    use tower_http::services::ServeDir;

    let cache_control = config
        .cache_control
        .clone()
        .unwrap_or_else(|| "max-age=3600".to_string());

    axum::Router::new()
        .fallback_service(ServeDir::new(&config.dir))
        .layer(axum::middleware::map_response(
            move |response: axum::response::Response| {
                let cache_control = cache_control.clone();
                async move {
                    if response.status().is_success() {
                        let (mut parts, body) = response.into_parts();
                        parts.headers.insert(
                            header::CACHE_CONTROL,
                            cache_control.parse().expect("invalid cache_control value"),
                        );
                        axum::response::Response::from_parts(parts, body)
                    } else {
                        response
                    }
                }
            },
        ))
}

/// Build an embedded static file router (prod-friendly, immutable cache).
#[cfg(feature = "static-embed")]
pub fn build_embed_service<E: rust_embed::Embed + 'static>(
    config: &StaticConfig,
) -> axum::Router<()> {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;

    let cache_control = config
        .cache_control
        .clone()
        .unwrap_or_else(|| "max-age=31536000, immutable".to_string());

    axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let cache_control = cache_control.clone();
        async move {
            let path = uri.path().trim_start_matches('/');
            match E::get(path) {
                Some(file) => {
                    let mime = mime_guess::from_path(path).first_or_octet_stream();
                    let hash: String = file
                        .metadata
                        .sha256_hash()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    let etag = format!("\"{hash}\"");

                    (
                        [
                            (header::CONTENT_TYPE, mime.as_ref().to_string()),
                            (header::ETAG, etag),
                            (header::CACHE_CONTROL, cache_control),
                        ],
                        file.data.to_vec(),
                    )
                        .into_response()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = StaticConfig::default();
        assert_eq!(config.dir, "static");
        assert_eq!(config.prefix, "/static");
        assert!(config.cache_control.is_none());
    }

    #[test]
    fn config_deserialize_defaults() {
        let config: StaticConfig = serde_yaml_ng::from_str("{}").unwrap();
        assert_eq!(config.dir, "static");
        assert_eq!(config.prefix, "/static");
        assert!(config.cache_control.is_none());
    }

    #[test]
    fn config_deserialize_custom() {
        let yaml = r#"
dir: "assets"
prefix: "/assets"
cache_control: "max-age=86400"
"#;
        let config: StaticConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.dir, "assets");
        assert_eq!(config.prefix, "/assets");
        assert_eq!(config.cache_control.as_deref(), Some("max-age=86400"));
    }
}

#[cfg(all(test, feature = "static-fs"))]
mod fs_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use tower::ServiceExt;

    #[tokio::test]
    async fn serve_existing_file() {
        let dir = std::env::temp_dir().join("modo_static_fs_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.css"), "body { color: red; }").unwrap();

        let config = StaticConfig {
            dir: dir.to_str().unwrap().to_string(),
            ..Default::default()
        };
        let router = build_fs_service(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/hello.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "max-age=3600"
        );
        assert!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("css")
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, "body { color: red; }");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn missing_file_returns_404() {
        let dir = std::env::temp_dir().join("modo_static_fs_404_test");
        std::fs::create_dir_all(&dir).unwrap();

        let config = StaticConfig {
            dir: dir.to_str().unwrap().to_string(),
            ..Default::default()
        };
        let router = build_fs_service(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/nope.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().get(header::CACHE_CONTROL).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn custom_cache_control() {
        let dir = std::env::temp_dir().join("modo_static_fs_cc_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.js"), "console.log('hi')").unwrap();

        let config = StaticConfig {
            dir: dir.to_str().unwrap().to_string(),
            cache_control: Some("no-cache".to_string()),
            ..Default::default()
        };
        let router = build_fs_service(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(all(test, feature = "static-embed"))]
mod embed_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use tower::ServiceExt;

    #[derive(rust_embed::Embed)]
    #[folder = "tests/static_fixtures/"]
    struct TestAssets;

    #[tokio::test]
    async fn serve_embedded_file() {
        let config = StaticConfig::default();
        let router = build_embed_service::<TestAssets>(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/hello.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "max-age=31536000, immutable"
        );
        assert!(response.headers().get(header::ETAG).is_some());
        assert!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("text/plain")
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, "hello world\n");
    }

    #[tokio::test]
    async fn missing_embedded_file_returns_404() {
        let config = StaticConfig::default();
        let router = build_embed_service::<TestAssets>(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/nope.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn custom_cache_control() {
        let config = StaticConfig {
            cache_control: Some("public, max-age=86400".to_string()),
            ..Default::default()
        };
        let router = build_embed_service::<TestAssets>(&config);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/hello.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=86400"
        );
    }
}
