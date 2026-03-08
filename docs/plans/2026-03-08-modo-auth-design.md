# Design: modo-auth

> Thin authentication layer — `UserProvider` trait + `Auth<U>` / `OptionalAuth<U>` extractors.

## Status

Accepted (2026-03-08)

## Context

modo-session provides session management (create/destroy/rotate sessions, read user_id). What's missing is the "load the actual user from a session" step. Apps need extractors that say "give me the authenticated user or reject" (`Auth<U>`) and "give me the user if logged in" (`OptionalAuth<U>`).

## Decision

A standalone crate with no DB dependency, no password hashing, no session mutation. It composes `SessionManager` (read-only) with a user-defined `UserProvider` trait to resolve `user_id → User`.

## Dependencies

```toml
[dependencies]
modo = { path = "../modo" }
modo-session = { path = "../modo-session" }
async-trait = "0.1"
```

No `modo-db` — the `UserProvider` trait is DB-agnostic. The app's implementation brings its own storage.

## Public API

### UserProvider Trait

```rust
#[async_trait]
pub trait UserProvider: Send + Sync + 'static {
    type User: Clone + Send + Sync + 'static;

    async fn find_by_id(&self, id: &str) -> Result<Option<Self::User>, modo::Error>;
}
```

### UserProviderService\<U\>

Type-erased wrapper that stores a `UserProvider` impl, keyed by user type `U` for stable `TypeId` lookup in the service registry.

```rust
pub struct UserProviderService<U: Clone + Send + Sync + 'static> {
    inner: Arc<dyn UserProviderDyn<User = U>>,
}

impl<U: Clone + Send + Sync + 'static> UserProviderService<U> {
    pub fn new<P: UserProvider<User = U>>(provider: P) -> Self { ... }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<U>, modo::Error> {
        self.inner.find_by_id(id).await
    }
}
```

### Auth\<U\> Extractor

Requires an authenticated user. Returns 401 if not authenticated or user not found.

```rust
pub struct Auth<U>(pub U);
```

Extraction flow:
1. Get `SessionManager` from request extensions
2. Call `session.user_id()` — `None` → 401
3. Get `Service<UserProviderService<U>>` from service registry
4. Call `provider.find_by_id(user_id)` — `None` → 401, `Err` → 500
5. Return `Auth(user)`

### OptionalAuth\<U\> Extractor

Never rejects. Returns `None` if not authenticated or user not found.

```rust
pub struct OptionalAuth<U>(pub Option<U>);
```

Same flow as `Auth<U>` but returns `Ok(OptionalAuth(None))` instead of 401. Provider errors (500) still propagate.

## Error Handling

| Scenario | `Auth<U>` | `OptionalAuth<U>` |
|---|---|---|
| No session middleware | 500 Internal | 500 Internal |
| No session / not authenticated | 401 Unauthorized | `Ok(None)` |
| `UserProvider` not registered | 500 Internal | 500 Internal |
| `find_by_id` returns `None` | 401 Unauthorized | `Ok(None)` |
| `find_by_id` returns `Err` | 500 Internal | 500 Internal |

## Registration

```rust
use modo_auth::{UserProvider, UserProviderService};

struct UserRepo { db: DbPool }

#[async_trait]
impl UserProvider for UserRepo {
    type User = User;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, modo::Error> {
        // query DB
    }
}

// In main:
let provider = UserProviderService::new(UserRepo::new(db.clone()));
app.service(provider)
   .layer(modo_session::layer(&config.session))
```

## Usage in Handlers

```rust
#[modo::handler(GET, "/dashboard")]
async fn dashboard(Auth(user): Auth<User>) -> Result<String> {
    Ok(format!("Hello, {}", user.name))
}

#[modo::handler(GET, "/")]
async fn home(OptionalAuth(user): OptionalAuth<User>) -> Result<String> {
    match user {
        Some(u) => Ok(format!("Welcome back, {}", u.name)),
        None => Ok("Welcome, guest".into()),
    }
}
```

## Design Decisions

### No auto-destroy of stale sessions

If `find_by_id` returns `None` (user deleted), `Auth<U>` returns 401 without destroying the session. The app is responsible for revoking sessions on user deletion via `SessionManager::logout_all()`. Rationale: extractors should be read-only; distinguishing "user not found" from "transient DB error" inside an extractor is error-prone.

### Trait, not closure

`UserProvider` is a trait, not a closure/function pointer. This is idiomatic Rust, works naturally with dependency injection (the impl struct holds a `DbPool`), and integrates with the `Service<T>` extractor pattern.

### No password hashing

modo-auth does not include password hashing utilities. Apps bring their own (argon2, bcrypt, etc.) and handle login/signup in their own handlers. Keeps the crate thin and dependency-free.

### No DB dependency

The crate depends on `modo` + `modo-session` only. The `UserProvider` trait takes `&str` IDs and returns domain objects — how the app loads users is its own concern.

### Type-erased provider via UserProviderService\<U\>

`Auth<U>` needs to look up the provider from the service registry by `TypeId`. Since the registry stores by concrete type, `UserProviderService<U>` provides a stable `TypeId` keyed by the user type `U`, wrapping any `UserProvider<User = U>` impl behind `Arc<dyn ...>`.

## Crate Structure

```
modo-auth/
├── Cargo.toml
└── src/
    ├── lib.rs          # Re-exports
    ├── provider.rs     # UserProvider trait + UserProviderDyn + UserProviderService<U>
    └── extractor.rs    # Auth<U> + OptionalAuth<U> (FromRequestParts<AppState>)
```

## Consequences

**Positive:**
- Minimal surface area — 3 source files, ~150 lines
- No DB coupling — works with any storage backend
- Follows established extractor patterns from modo-session
- Clean separation: session = "is someone logged in?", auth = "who is logged in?"

**Negative:**
- Requires `UserProviderService::new()` wrapper at registration — slight DX friction
- `FromRequestParts<AppState>` (not generic over `S`) — less flexible than SessionManager, but necessary for service registry access
