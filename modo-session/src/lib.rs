pub mod config;
pub mod device;
pub mod entity;
pub mod fingerprint;
pub mod manager;
pub mod meta;
pub mod middleware;
pub mod store;
pub mod types;

#[cfg(feature = "cleanup-job")]
pub mod cleanup;

// Public API
pub use config::SessionConfig;
pub use manager::SessionManager;
pub use meta::SessionMeta;
pub use middleware::{layer, user_id_from_extensions};
pub use store::SessionStore;
pub use types::{SessionData, SessionId, SessionToken};

// Re-exports for macro-generated code
pub use chrono;
pub use modo;
pub use modo_db;
pub use serde;
pub use serde_json;
