pub mod config;
pub mod context;
pub mod engine;
pub mod error;
pub mod view;

pub use config::TemplateConfig;
pub use context::TemplateContext;
pub use engine::{TemplateEngine, engine};
pub use error::TemplateError;
pub use view::View;

// Re-export macro
pub use modo_templates_macros::view;

// Re-export minijinja essentials for macro-generated code
pub use minijinja;
pub use minijinja::context;
