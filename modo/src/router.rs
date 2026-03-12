use axum::Router;
use axum::routing::MethodRouter;

/// HTTP method variants used in route registrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    HEAD,
    OPTIONS,
}

/// Function type for per-handler middleware applied to a `MethodRouter`.
pub type MiddlewareFn =
    fn(MethodRouter<crate::app::AppState>) -> MethodRouter<crate::app::AppState>;

/// Function type for per-module middleware applied to a `Router`.
pub type RouterMiddlewareFn = fn(Router<crate::app::AppState>) -> Router<crate::app::AppState>;

/// `inventory` registration entry for a single route, created by `#[modo::handler]`.
pub struct RouteRegistration {
    pub method: Method,
    pub path: &'static str,
    pub handler: fn() -> MethodRouter<crate::app::AppState>,
    pub middleware: Vec<MiddlewareFn>,
    pub module: Option<&'static str>,
}

inventory::collect!(RouteRegistration);

/// `inventory` registration entry for a module, created by `#[modo::module]`.
pub struct ModuleRegistration {
    pub name: &'static str,
    pub prefix: &'static str,
    pub middleware: Vec<RouterMiddlewareFn>,
}

inventory::collect!(ModuleRegistration);
