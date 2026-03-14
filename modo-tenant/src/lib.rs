//! Multi-tenant support for modo applications.
//!
//! Provides pluggable tenant resolution strategies and extractors:
//!
//! - **[`TenantResolver`]** — implement this trait to resolve a tenant from the request.
//! - **[`Tenant<T>`] / [`OptionalTenant<T>`]** — axum extractors that resolve the current tenant
//!   via the registered [`TenantResolverService<T>`].
//! - Built-in resolvers: [`SubdomainResolver`], [`HeaderResolver`], [`PathPrefixResolver`].
//!
//! An optional **`templates`** feature adds [`TenantContextLayer`], a Tower middleware that
//! injects the resolved tenant into the minijinja template context under the key `"tenant"`.

pub(crate) mod cache;
#[cfg(feature = "templates")]
pub mod context_layer;
pub mod extractor;
pub mod resolver;
pub mod resolvers;

#[cfg(feature = "templates")]
pub use context_layer::TenantContextLayer;
pub use extractor::{OptionalTenant, Tenant};
pub use resolver::{HasTenantId, TenantResolver, TenantResolverService};
pub use resolvers::{HeaderResolver, PathPrefixResolver, SubdomainResolver};
