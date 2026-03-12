# modo-tenant

Multi-tenancy support for modo applications: resolve, cache, and extract the current tenant from any HTTP request signal.

## Features

| Feature | Description |
|---------|-------------|
| `templates` | Enables `TenantContextLayer`, which injects the resolved tenant into the MiniJinja `TemplateContext` under the key `"tenant"`. Requires `modo/templates`. |

## Usage

### Define your tenant type

```rust
use modo_tenant::HasTenantId;

#[derive(Clone, serde::Serialize)]
pub struct Tenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

impl HasTenantId for Tenant {
    fn tenant_id(&self) -> &str {
        &self.id
    }
}
```

### Implement a resolver

```rust
use modo_tenant::{HasTenantId, TenantResolver};
use modo::axum::http::request::Parts;

pub struct DbTenantResolver {
    // e.g., a database connection pool
}

impl TenantResolver for DbTenantResolver {
    type Tenant = Tenant;

    async fn resolve(
        &self,
        parts: &Parts,
    ) -> Result<Option<Tenant>, modo::Error> {
        // Extract a signal from `parts` (subdomain, header, path, etc.)
        // and load the tenant from your database.
        // Return Ok(None) when no tenant matches.
        Ok(None)
    }
}
```

### Register with AppState

```rust
use modo_tenant::TenantResolverService;
use modo::app::{AppState, ServiceRegistry};

let services = ServiceRegistry::new()
    .with(TenantResolverService::new(DbTenantResolver { /* … */ }));
```

### Extract in handlers

```rust
use modo_tenant::{Tenant, OptionalTenant};

// Requires a tenant — returns HTTP 404 if none is resolved.
async fn dashboard(tenant: Tenant<MyTenant>) {
    println!("tenant: {}", tenant.name);
}

// Optional — never rejects; inner value is None when no tenant matches.
async fn home(tenant: OptionalTenant<MyTenant>) {
    if let Some(t) = &*tenant {
        println!("logged-in tenant: {}", t.name);
    }
}
```

### Built-in resolvers

#### Subdomain

```rust
use modo_tenant::{SubdomainResolver, TenantResolverService};

let resolver = SubdomainResolver::new("myapp.com", |slug| async move {
    // load tenant by slug from DB
    Ok(Some(my_tenant))
});
let svc = TenantResolverService::new(resolver);
```

`acme.myapp.com` → slug `"acme"`. The bare domain and `www` subdomain return `None`. Port suffixes are stripped automatically.

#### HTTP header

```rust
use modo_tenant::{HeaderResolver, TenantResolverService};

let resolver = HeaderResolver::new("x-tenant-id", |id| async move {
    // load tenant by id from DB
    Ok(Some(my_tenant))
});
let svc = TenantResolverService::new(resolver);
```

The header value is trimmed of whitespace. Missing or whitespace-only headers return `None`.

#### Path prefix

```rust
use modo_tenant::{PathPrefixResolver, TenantResolverService};

let resolver = PathPrefixResolver::new(|slug| async move {
    // load tenant by slug from DB
    Ok(Some(my_tenant))
});
let svc = TenantResolverService::new(resolver);
```

`/acme/dashboard` → slug `"acme"`. The root path `/` returns `None`.

### Template context injection (feature `templates`)

```rust
use modo_tenant::{TenantContextLayer, TenantResolverService};

let layer = TenantContextLayer::new(TenantResolverService::new(my_resolver));
// Apply to a router or globally via AppBuilder
let app = router.layer(layer);
```

Inside templates the tenant is accessible as `{{ tenant.name }}`. Resolution errors are logged and silently swallowed so the request always continues.

## Key Types

| Type | Description |
|------|-------------|
| `HasTenantId` | Trait a tenant type must implement to expose its ID. |
| `TenantResolver` | Trait for pluggable tenant resolution strategies. |
| `TenantResolverService<T>` | Type-erased, cloneable wrapper registered in `AppState`. |
| `Tenant<T>` | Extractor that requires a tenant; returns HTTP 404 when absent. |
| `OptionalTenant<T>` | Extractor that yields `Option<T>`; never rejects on missing tenant. |
| `SubdomainResolver<T, F>` | Resolves tenant from the subdomain of the `Host` header. |
| `HeaderResolver<T, F>` | Resolves tenant from a named HTTP header. |
| `PathPrefixResolver<T, F>` | Resolves tenant from the first URL path segment. |
| `TenantContextLayer<T>` | Tower layer that injects the tenant into `TemplateContext` (feature `templates`). |

## Caching

The resolved tenant is cached in request extensions after the first lookup. When `Tenant<T>` and `OptionalTenant<T>` are both declared as handler parameters — or when `TenantContextLayer` runs before an extractor — the underlying resolver is called only once per request.
