pub mod cache;
#[cfg(feature = "templates")]
pub mod context_layer;
pub mod extractor;
pub mod guard;
pub mod member;
pub mod resolver;
pub mod resolvers;

#[cfg(feature = "templates")]
pub use context_layer::TenantContextLayer;
pub use extractor::{Member, OptionalTenant, Tenant, TenantContext};
pub use member::{MemberProvider, MemberProviderService};
pub use modo_tenant_macros::{allow_roles, deny_roles};
pub use resolver::{HasTenantId, TenantResolver, TenantResolverService};
pub use resolvers::{HeaderResolver, PathPrefixResolver, SubdomainResolver};
