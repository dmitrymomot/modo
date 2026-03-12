//! Transactional email for the modo framework.
//!
//! `modo-email` provides Markdown-based email templates, responsive HTML rendering,
//! plain-text fallback generation, and pluggable delivery transports (SMTP and Resend).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use modo_email::{mailer, EmailConfig, SendEmail};
//!
//! # async fn example() -> Result<(), modo::Error> {
//! let config = EmailConfig::default(); // load from YAML in practice
//! let m = mailer(&config)?;
//!
//! m.send(
//!     &SendEmail::new("welcome", "user@example.com")
//!         .var("name", "Alice"),
//! ).await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod mailer;
mod message;
pub mod template;
pub mod transport;

pub use config::{EmailConfig, TransportBackend};
pub use mailer::Mailer;
pub use message::{MailMessage, SendEmail, SendEmailPayload, SenderProfile};
pub use template::{EmailTemplate, TemplateProvider};
pub use transport::MailTransport;

#[cfg(feature = "resend")]
pub use config::ResendConfig;
#[cfg(feature = "smtp")]
pub use config::SmtpConfig;

pub use template::filesystem::FilesystemProvider;
pub use template::layout::LayoutEngine;

use std::sync::Arc;

/// Create a [`Mailer`] using [`FilesystemProvider`] and the transport configured in `config`.
///
/// This is the standard entry point. Templates are loaded from `config.templates_path`.
pub fn mailer(config: &EmailConfig) -> Result<Mailer, modo::Error> {
    let provider = Arc::new(FilesystemProvider::new(&config.templates_path));
    mailer_with(config, provider)
}

/// Create a [`Mailer`] with a custom [`TemplateProvider`].
///
/// Use this when you want to load templates from a database, cache, or any
/// source other than the filesystem.
pub fn mailer_with(
    config: &EmailConfig,
    provider: Arc<dyn TemplateProvider>,
) -> Result<Mailer, modo::Error> {
    let transport = transport::transport(config)?;
    let layout = Arc::new(LayoutEngine::new(&config.templates_path));
    let sender = SenderProfile {
        from_name: config.default_from_name.clone(),
        from_email: config.default_from_email.clone(),
        reply_to: config.default_reply_to.clone(),
    };
    Ok(Mailer::new(transport, provider, sender, layout))
}
