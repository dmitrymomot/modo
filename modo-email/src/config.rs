use serde::Deserialize;

/// Which delivery backend to use for outgoing email.
///
/// Serialized as lowercase strings (`"smtp"`, `"resend"`) in YAML/JSON config.
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportBackend {
    /// Send via SMTP (default). Requires the `smtp` feature.
    #[default]
    Smtp,
    /// Send via the Resend HTTP API. Requires the `resend` feature.
    Resend,
}

/// Top-level email configuration loaded from YAML or environment.
///
/// All fields implement `Default`, so partial YAML is valid — only override
/// what differs from the defaults.
///
/// Feature-gated fields (`smtp`, `resend`) are only present when the
/// corresponding Cargo feature is enabled.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EmailConfig {
    /// Which transport backend to use. Defaults to `smtp`.
    pub transport: TransportBackend,
    /// Directory that contains `.md` template files. Defaults to `"emails"`.
    pub templates_path: String,
    /// Display name used in the `From` header when no per-email sender is set.
    pub default_from_name: String,
    /// Email address used in the `From` header when no per-email sender is set.
    pub default_from_email: String,
    /// Optional default `Reply-To` address.
    pub default_reply_to: Option<String>,

    /// SMTP connection settings. Requires the `smtp` feature.
    #[cfg(feature = "smtp")]
    pub smtp: SmtpConfig,

    /// Resend API settings. Requires the `resend` feature.
    #[cfg(feature = "resend")]
    pub resend: ResendConfig,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            transport: TransportBackend::default(),
            templates_path: "emails".to_string(),
            default_from_name: String::new(),
            default_from_email: String::new(),
            default_reply_to: None,
            #[cfg(feature = "smtp")]
            smtp: SmtpConfig::default(),
            #[cfg(feature = "resend")]
            resend: ResendConfig::default(),
        }
    }
}

/// SMTP connection settings. Requires the `smtp` feature.
#[cfg(feature = "smtp")]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SmtpConfig {
    /// SMTP server hostname. Defaults to `"localhost"`.
    pub host: String,
    /// SMTP server port. Defaults to `587`.
    pub port: u16,
    /// SMTP authentication username.
    pub username: String,
    /// SMTP authentication password.
    pub password: String,
    /// When `true`, uses STARTTLS (port 587). When `false`, no TLS at all.
    /// Implicit TLS / SMTPS (port 465) is not currently supported.
    pub tls: bool,
}

#[cfg(feature = "smtp")]
impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 587,
            username: String::new(),
            password: String::new(),
            tls: true,
        }
    }
}

/// Resend HTTP API settings. Requires the `resend` feature.
#[cfg(feature = "resend")]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ResendConfig {
    /// Resend API key (starts with `re_`).
    pub api_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = EmailConfig::default();
        assert_eq!(config.templates_path, "emails");
        assert_eq!(config.default_from_name, "");
        assert_eq!(config.default_from_email, "");
        assert!(config.default_reply_to.is_none());
        assert_eq!(config.transport, TransportBackend::Smtp);
    }

    #[test]
    fn partial_yaml_deserialization() {
        let yaml = r#"
templates_path: "mail"
default_from_name: "Acme"
default_from_email: "hi@acme.com"
"#;
        let config: EmailConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.templates_path, "mail");
        assert_eq!(config.default_from_name, "Acme");
        assert_eq!(config.default_from_email, "hi@acme.com");
        assert_eq!(config.transport, TransportBackend::Smtp);
    }

    #[test]
    fn transport_backend_deserialization() {
        let yaml = "transport: resend";
        let config: EmailConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.transport, TransportBackend::Resend);
    }
}
