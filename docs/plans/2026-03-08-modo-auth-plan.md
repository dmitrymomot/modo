# modo-auth Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a thin auth crate with `UserProvider` trait + `Auth<U>`/`OptionalAuth<U>` extractors that resolve session user IDs to typed user objects.

**Architecture:** `modo-auth` depends on `modo` (for `Error`, `AppState`, `Service<T>`) and `modo-session` (for `SessionManager`). A `UserProvider` trait defines how to load users. A type-erased `UserProviderService<U>` wrapper enables service registry lookup by user type. Two extractors compose `SessionManager` + `UserProviderService<U>`.

**Tech Stack:** Rust, axum extractors, async-trait, modo service registry

---

### Task 1: Scaffold the crate

**Files:**
- Create: `modo-auth/Cargo.toml`
- Create: `modo-auth/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Create Cargo.toml**

```toml
[package]
name = "modo-auth"
version = "0.1.0"
edition = "2024"
license.workspace = true

[dependencies]
modo = { path = "../modo" }
modo-session = { path = "../modo-session" }
async-trait = "0.1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

**Step 2: Create empty lib.rs**

```rust
pub mod extractor;
pub mod provider;
```

**Step 3: Add to workspace members**

In root `Cargo.toml`, add `"modo-auth"` to the `members` array.

**Step 4: Create empty module files**

Create `modo-auth/src/provider.rs` and `modo-auth/src/extractor.rs` as empty files.

**Step 5: Verify it compiles**

Run: `cargo check -p modo-auth`
Expected: PASS (empty modules compile fine)

**Step 6: Commit**

```bash
git add modo-auth/ Cargo.toml
git commit -m "feat(modo-auth): scaffold crate with empty modules"
```

---

### Task 2: Implement UserProvider trait and UserProviderService

**Files:**
- Create: `modo-auth/src/provider.rs`

**Step 1: Write the failing test**

Add to the bottom of `modo-auth/src/provider.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct TestUser {
        id: String,
        name: String,
    }

    struct TestProvider;

    #[async_trait]
    impl UserProvider for TestProvider {
        type User = TestUser;

        async fn find_by_id(&self, id: &str) -> Result<Option<Self::User>, modo::Error> {
            if id == "user-1" {
                Ok(Some(TestUser {
                    id: "user-1".to_string(),
                    name: "Alice".to_string(),
                }))
            } else {
                Ok(None)
            }
        }
    }

    #[tokio::test]
    async fn user_provider_service_finds_existing_user() {
        let svc = UserProviderService::new(TestProvider);
        let user = svc.find_by_id("user-1").await.unwrap();
        assert_eq!(
            user,
            Some(TestUser {
                id: "user-1".to_string(),
                name: "Alice".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn user_provider_service_returns_none_for_missing_user() {
        let svc = UserProviderService::new(TestProvider);
        let user = svc.find_by_id("nonexistent").await.unwrap();
        assert_eq!(user, None);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p modo-auth`
Expected: FAIL — `UserProvider` and `UserProviderService` not defined yet

**Step 3: Write the implementation**

In `modo-auth/src/provider.rs`, above the tests:

```rust
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for loading a user by their session-stored ID.
///
/// Implement this on your own type (e.g., a repository struct that holds a DB pool)
/// and register it via `UserProviderService::new(your_impl)` as a service.
#[async_trait]
pub trait UserProvider: Send + Sync + 'static {
    type User: Clone + Send + Sync + 'static;

    /// Look up a user by their ID (as stored in the session).
    /// Return `Ok(None)` if the user doesn't exist.
    /// Return `Err` only for infrastructure failures (DB errors, etc.).
    async fn find_by_id(&self, id: &str) -> Result<Option<Self::User>, modo::Error>;
}

/// Type-erased wrapper around a `UserProvider` implementation.
///
/// Stored in the service registry keyed by user type `U`, so that
/// `Auth<U>` can look up `Service<UserProviderService<U>>` by `TypeId`.
pub struct UserProviderService<U: Clone + Send + Sync + 'static> {
    inner: Arc<dyn UserProvider<User = U>>,
}

impl<U: Clone + Send + Sync + 'static> UserProviderService<U> {
    /// Wrap a `UserProvider` implementation for registration in the service registry.
    pub fn new<P: UserProvider<User = U>>(provider: P) -> Self {
        Self {
            inner: Arc::new(provider),
        }
    }

    /// Delegate to the wrapped provider.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<U>, modo::Error> {
        self.inner.find_by_id(id).await
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p modo-auth`
Expected: PASS — both tests green

**Step 5: Commit**

```bash
git add modo-auth/src/provider.rs
git commit -m "feat(modo-auth): add UserProvider trait and UserProviderService"
```

---

### Task 3: Implement Auth\<U\> extractor

**Files:**
- Create: `modo-auth/src/extractor.rs`

**Step 1: Write the implementation**

The `Auth<U>` extractor cannot be easily unit-tested without a full axum `AppState` + `SessionManager` in request extensions. Write the implementation directly, then validate via integration test in Task 5.

In `modo-auth/src/extractor.rs`:

```rust
use crate::provider::UserProviderService;
use modo::app::AppState;
use modo::axum::extract::FromRequestParts;
use modo::axum::http::request::Parts;
use modo::{Error, HttpError};
use modo_session::SessionManager;
use std::ops::Deref;

/// Extractor that requires an authenticated user.
///
/// Returns 401 if no session exists or the user is not found.
/// Returns 500 if the session middleware or `UserProviderService` is not registered.
///
/// # Usage
///
/// ```ignore
/// #[modo::handler(GET, "/dashboard")]
/// async fn dashboard(Auth(user): Auth<User>) -> Result<String> {
///     Ok(format!("Hello, {}", user.name))
/// }
/// ```
pub struct Auth<U: Clone + Send + Sync + 'static>(pub U);

impl<U: Clone + Send + Sync + 'static> Deref for Auth<U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<U: Clone + Send + Sync + 'static> FromRequestParts<AppState> for Auth<U> {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract SessionManager from request extensions
        let session = SessionManager::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::internal("Auth<U> requires session middleware"))?;

        // 2. Get user_id from session
        let user_id = session
            .user_id()
            .await
            .ok_or_else(|| Error::from(HttpError::Unauthorized))?;

        // 3. Look up UserProviderService<U> in service registry
        let provider = state
            .services
            .get::<UserProviderService<U>>()
            .ok_or_else(|| {
                Error::internal(format!(
                    "UserProviderService<{}> not registered",
                    std::any::type_name::<U>()
                ))
            })?;

        // 4. Load user — None means 401, Err means 500
        let user = provider
            .find_by_id(&user_id)
            .await?
            .ok_or_else(|| Error::from(HttpError::Unauthorized))?;

        Ok(Auth(user))
    }
}

/// Extractor that optionally loads the authenticated user.
///
/// Never rejects — returns `OptionalAuth(None)` if not authenticated or user not found.
/// Still returns 500 if session middleware or `UserProviderService` is missing,
/// or if the provider returns an error (infrastructure failure).
///
/// # Usage
///
/// ```ignore
/// #[modo::handler(GET, "/")]
/// async fn home(OptionalAuth(user): OptionalAuth<User>) -> Result<String> {
///     match user {
///         Some(u) => Ok(format!("Welcome back, {}", u.name)),
///         None => Ok("Welcome, guest".into()),
///     }
/// }
/// ```
pub struct OptionalAuth<U: Clone + Send + Sync + 'static>(pub Option<U>);

impl<U: Clone + Send + Sync + 'static> Deref for OptionalAuth<U> {
    type Target = Option<U>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<U: Clone + Send + Sync + 'static> FromRequestParts<AppState> for OptionalAuth<U> {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract SessionManager — missing middleware is still a 500
        let session = SessionManager::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::internal("OptionalAuth<U> requires session middleware"))?;

        // 2. Get user_id — no session means None (not an error)
        let user_id = match session.user_id().await {
            Some(id) => id,
            None => return Ok(OptionalAuth(None)),
        };

        // 3. Look up provider — missing provider is still a 500
        let provider = state
            .services
            .get::<UserProviderService<U>>()
            .ok_or_else(|| {
                Error::internal(format!(
                    "UserProviderService<{}> not registered",
                    std::any::type_name::<U>()
                ))
            })?;

        // 4. Load user — Err propagates as 500, None returns OptionalAuth(None)
        let user = provider.find_by_id(&user_id).await?;

        Ok(OptionalAuth(user))
    }
}
```

**Step 2: Update lib.rs with re-exports**

```rust
pub mod extractor;
pub mod provider;

pub use extractor::{Auth, OptionalAuth};
pub use provider::{UserProvider, UserProviderService};
```

**Step 3: Verify it compiles**

Run: `cargo check -p modo-auth`
Expected: PASS

**Step 4: Commit**

```bash
git add modo-auth/src/extractor.rs modo-auth/src/lib.rs
git commit -m "feat(modo-auth): add Auth<U> and OptionalAuth<U> extractors"
```

---

### Task 4: Lint and format

**Step 1: Format**

Run: `just fmt`

**Step 2: Lint**

Run: `just lint`
Expected: PASS — no warnings

**Step 3: Fix any issues found, then commit**

```bash
git add -A
git commit -m "style(modo-auth): fix formatting and lint"
```

---

### Task 5: Update CLAUDE.md and workspace docs

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add modo-auth conventions to CLAUDE.md**

Add a `## Auth (modo-auth)` section after the Sessions section with:
- Dependencies: `modo`, `modo-session`
- Trait: `UserProvider` with `find_by_id`
- Registration: `UserProviderService::new(impl)` + `app.service(provider)`
- Extractors: `Auth<U>` (401 if not authenticated), `OptionalAuth<U>` (never rejects)
- No password hashing, no session mutation, no DB dependency

**Step 2: Update Architecture section**

Mark `modo-auth` as **implemented** in the architecture list.

**Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add modo-auth conventions to CLAUDE.md"
```

---

### Task 6: Run full workspace check

**Step 1: Run full check**

Run: `just check`
Expected: PASS — all workspace targets compile, lint, and test

**Step 2: Fix any issues found across the workspace**

If any breakage, fix and commit.
