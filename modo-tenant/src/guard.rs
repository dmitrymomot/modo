use crate::HasTenantId;
use crate::cache::{ResolvedMember, ResolvedRole, ResolvedTenant};
use crate::member::MemberProviderService;
use crate::resolver::TenantResolverService;
use modo::{Error, HttpError};
use std::sync::Arc;

/// Check that the resolved role is in the allowed list.
pub fn check_allowed(role: &str, allowed: &[&str]) -> Result<(), Error> {
    if allowed.contains(&role) {
        Ok(())
    } else {
        Err(HttpError::Forbidden.into())
    }
}

/// Check that the resolved role is NOT in the denied list.
pub fn check_denied(role: &str, denied: &[&str]) -> Result<(), Error> {
    if denied.contains(&role) {
        Err(HttpError::Forbidden.into())
    } else {
        Ok(())
    }
}

/// Resolve the current user's role for the current tenant from request extensions.
///
/// Checks cached `ResolvedRole` first, then resolves tenant + member if needed.
/// Requires session middleware and both `TenantResolverService` and `MemberProviderService`
/// to be registered.
pub async fn resolve_role<T, M>(
    extensions: &mut http::Extensions,
    _tenant_svc: &TenantResolverService<T>,
    member_svc: &MemberProviderService<M, T>,
) -> Result<String, Error>
where
    T: Clone + Send + Sync + HasTenantId + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::Serialize + 'static,
{
    // Check cache first
    if let Some(cached) = extensions.get::<ResolvedRole>() {
        return Ok(cached.0.clone());
    }

    // Resolve tenant
    let tenant = if let Some(cached) = extensions.get::<ResolvedTenant<T>>() {
        (*cached.0).clone()
    } else {
        // Build temporary Parts for the resolver
        // We can't call resolve on extensions alone — need request parts.
        // This path should rarely hit since TenantContextLayer/extractors cache the tenant.
        return Err(Error::internal(
            "Role guard requires tenant to be resolved first (use Tenant<T> extractor or TenantContextLayer)",
        ));
    };

    // Get user_id from session
    let user_id = modo_session::user_id_from_extensions(extensions)
        .ok_or_else(|| Error::from(HttpError::Unauthorized))?;

    // Resolve member
    let member = member_svc
        .find_member(&user_id, tenant.tenant_id())
        .await?
        .ok_or_else(|| Error::from(HttpError::Forbidden))?;

    let role = member_svc.role(&member).to_string();

    // Cache
    extensions.insert(ResolvedMember(Arc::new(member)));
    extensions.insert(ResolvedRole(role.clone()));

    Ok(role)
}

/// Middleware function: allow only specified roles.
///
/// Reads `AppState` from request extensions (injected by modo's global middleware).
///
/// Usage:
/// ```ignore
/// #[allow_roles(MyTenant, MyMember, "admin", "owner")]
/// #[modo::handler(GET, "/admin")]
/// async fn admin_page() -> &'static str { "admin" }
/// ```
pub fn require_roles<T, M>(
    roles: &'static [&'static str],
) -> impl Fn(
    modo::axum::http::Request<modo::axum::body::Body>,
    modo::axum::middleware::Next,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = modo::axum::response::Response> + Send>,
> + Clone
+ Send
+ Sync
where
    T: Clone + Send + Sync + HasTenantId + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::Serialize + 'static,
{
    use modo::axum::response::IntoResponse;

    move |req: modo::axum::http::Request<modo::axum::body::Body>,
          next: modo::axum::middleware::Next| {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();

            let state = match parts.extensions.get::<modo::app::AppState>().cloned() {
                Some(s) => s,
                None => {
                    return Error::internal("Role guard: AppState not in extensions")
                        .into_response();
                }
            };

            let (tenant_svc, member_svc) = {
                let t = state.services.get::<TenantResolverService<T>>();
                let m = state.services.get::<MemberProviderService<M, T>>();
                match (t, m) {
                    (Some(t), Some(m)) => (t, m),
                    _ => {
                        return Error::internal("Role guard: services not registered")
                            .into_response();
                    }
                }
            };

            match resolve_role::<T, M>(&mut parts.extensions, &tenant_svc, &member_svc).await {
                Ok(role) => {
                    if let Err(e) = check_allowed(&role, roles) {
                        return e.into_response();
                    }
                }
                Err(e) => return e.into_response(),
            }

            let req = modo::axum::http::Request::from_parts(parts, body);
            next.run(req).await
        })
    }
}

/// Middleware function: deny specified roles.
///
/// Reads `AppState` from request extensions (injected by modo's global middleware).
///
/// Usage:
/// ```ignore
/// #[deny_roles(MyTenant, MyMember, "viewer")]
/// #[modo::handler(GET, "/admin")]
/// async fn admin_page() -> &'static str { "admin" }
/// ```
pub fn exclude_roles<T, M>(
    roles: &'static [&'static str],
) -> impl Fn(
    modo::axum::http::Request<modo::axum::body::Body>,
    modo::axum::middleware::Next,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = modo::axum::response::Response> + Send>,
> + Clone
+ Send
+ Sync
where
    T: Clone + Send + Sync + HasTenantId + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::Serialize + 'static,
{
    use modo::axum::response::IntoResponse;

    move |req: modo::axum::http::Request<modo::axum::body::Body>,
          next: modo::axum::middleware::Next| {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();

            let state = match parts.extensions.get::<modo::app::AppState>().cloned() {
                Some(s) => s,
                None => {
                    return Error::internal("Role guard: AppState not in extensions")
                        .into_response();
                }
            };

            let (tenant_svc, member_svc) = {
                let t = state.services.get::<TenantResolverService<T>>();
                let m = state.services.get::<MemberProviderService<M, T>>();
                match (t, m) {
                    (Some(t), Some(m)) => (t, m),
                    _ => {
                        return Error::internal("Role guard: services not registered")
                            .into_response();
                    }
                }
            };

            match resolve_role::<T, M>(&mut parts.extensions, &tenant_svc, &member_svc).await {
                Ok(role) => {
                    if let Err(e) = check_denied(&role, roles) {
                        return e.into_response();
                    }
                }
                Err(e) => return e.into_response(),
            }

            let req = modo::axum::http::Request::from_parts(parts, body);
            next.run(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TenantResolver;
    use crate::cache::ResolvedTenant;
    use crate::member::MemberProvider;
    use crate::resolver::TenantResolverService;
    use modo::app::{AppState, ServiceRegistry};
    use modo::axum::Router;
    use modo::axum::body::Body;
    use modo::axum::http::{Request, StatusCode};
    use modo::axum::routing::get;
    use tower::ServiceExt;

    #[test]
    fn allowed_passes_matching_role() {
        assert!(check_allowed("admin", &["admin", "owner"]).is_ok());
    }

    #[test]
    fn allowed_rejects_non_matching_role() {
        let err = check_allowed("viewer", &["admin", "owner"]).unwrap_err();
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn denied_blocks_matching_role() {
        let err = check_denied("viewer", &["viewer"]).unwrap_err();
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn denied_passes_non_matching_role() {
        assert!(check_denied("admin", &["viewer"]).is_ok());
    }

    // -- Integration test types --

    #[derive(Clone, Debug, serde::Serialize)]
    struct TestTenant {
        id: String,
    }

    impl crate::HasTenantId for TestTenant {
        fn tenant_id(&self) -> &str {
            &self.id
        }
    }

    #[derive(Clone, Debug, serde::Serialize)]
    struct TestMember {
        role: String,
    }

    struct TestResolver;

    impl TenantResolver for TestResolver {
        type Tenant = TestTenant;

        async fn resolve(
            &self,
            _parts: &modo::axum::http::request::Parts,
        ) -> Result<Option<Self::Tenant>, modo::Error> {
            Ok(None)
        }
    }

    struct TestMemberProvider;

    impl MemberProvider for TestMemberProvider {
        type Member = TestMember;
        type Tenant = TestTenant;

        async fn find_member(
            &self,
            _user_id: &str,
            _tenant_id: &str,
        ) -> Result<Option<Self::Member>, modo::Error> {
            Ok(None)
        }

        async fn list_tenants(&self, _user_id: &str) -> Result<Vec<Self::Tenant>, modo::Error> {
            Ok(vec![])
        }

        fn role<'a>(&'a self, member: &'a Self::Member) -> &'a str {
            &member.role
        }
    }

    fn test_state() -> AppState {
        let services = ServiceRegistry::new()
            .with(TenantResolverService::new(TestResolver))
            .with(MemberProviderService::new(TestMemberProvider));
        AppState {
            services,
            server_config: Default::default(),
            cookie_key: axum_extra::extract::cookie::Key::generate(),
        }
    }

    /// Build a request with AppState, ResolvedRole, and ResolvedTenant pre-populated.
    fn test_request(state: &AppState, role: &str) -> Request<Body> {
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(state.clone());
        req.extensions_mut().insert(ResolvedRole(role.to_string()));
        req.extensions_mut()
            .insert(ResolvedTenant(Arc::new(TestTenant {
                id: "t-1".to_string(),
            })));
        req
    }

    #[tokio::test]
    async fn require_roles_middleware_allows_matching_role() {
        let state = test_state();
        let app = Router::<AppState>::new()
            .route("/", get(|| async { "ok" }))
            .route_layer(modo::axum::middleware::from_fn(require_roles::<
                TestTenant,
                TestMember,
            >(&[
                "admin", "owner",
            ])))
            .with_state(state.clone());

        let resp = app.oneshot(test_request(&state, "admin")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_roles_middleware_rejects_non_matching_role() {
        let state = test_state();
        let app = Router::<AppState>::new()
            .route("/", get(|| async { "ok" }))
            .route_layer(modo::axum::middleware::from_fn(require_roles::<
                TestTenant,
                TestMember,
            >(&[
                "admin", "owner",
            ])))
            .with_state(state.clone());

        let resp = app.oneshot(test_request(&state, "viewer")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn exclude_roles_middleware_allows_non_matching_role() {
        let state = test_state();
        let app = Router::<AppState>::new()
            .route("/", get(|| async { "ok" }))
            .route_layer(modo::axum::middleware::from_fn(exclude_roles::<
                TestTenant,
                TestMember,
            >(&["viewer"])))
            .with_state(state.clone());

        let resp = app.oneshot(test_request(&state, "admin")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn exclude_roles_middleware_rejects_matching_role() {
        let state = test_state();
        let app = Router::<AppState>::new()
            .route("/", get(|| async { "ok" }))
            .route_layer(modo::axum::middleware::from_fn(exclude_roles::<
                TestTenant,
                TestMember,
            >(&["viewer"])))
            .with_state(state.clone());

        let resp = app.oneshot(test_request(&state, "viewer")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
