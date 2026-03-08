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
