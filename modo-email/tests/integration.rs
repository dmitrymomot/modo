use modo_email::template::filesystem::FilesystemProvider;
use modo_email::template::layout::LayoutEngine;
use modo_email::{MailMessage, MailTransport, Mailer, SendEmail, SenderProfile};
use std::sync::{Arc, Mutex};

/// A transport that captures sent messages for assertions.
struct CapturingTransport {
    messages: Arc<Mutex<Vec<MailMessage>>>,
}

#[async_trait::async_trait]
impl MailTransport for CapturingTransport {
    async fn send(&self, message: &MailMessage) -> Result<(), modo::Error> {
        self.messages.lock().unwrap().push(MailMessage {
            from: message.from.clone(),
            reply_to: message.reply_to.clone(),
            to: message.to.clone(),
            subject: message.subject.clone(),
            html: message.html.clone(),
            text: message.text.clone(),
        });
        Ok(())
    }
}

#[tokio::test]
async fn end_to_end_filesystem_template() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    // Create a template with frontmatter, variable placeholders, and a button link.
    std::fs::write(
        path.join("welcome.md"),
        "---\nsubject: \"Welcome {{name}}!\"\n---\n\nHi **{{name}}**,\n\nGet started:\n\n[button|Launch Dashboard]({{url}})\n",
    )
    .unwrap();

    let messages = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(FilesystemProvider::new(path.to_str().unwrap()));
    let layout = LayoutEngine::new(path.to_str().unwrap());

    let mailer = Mailer::new(
        Box::new(CapturingTransport {
            messages: messages.clone(),
        }),
        provider,
        SenderProfile {
            from_name: "App".to_string(),
            from_email: "app@test.com".to_string(),
            reply_to: None,
        },
        layout,
    );

    mailer
        .send(
            SendEmail::new("welcome", "user@example.com")
                .var("name", "Alice")
                .var("url", "https://app.com/dashboard"),
        )
        .await
        .unwrap();

    let msgs = messages.lock().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].subject, "Welcome Alice!");
    assert!(msgs[0].html.contains("Alice"));
    assert!(msgs[0].html.contains("Launch Dashboard"));
    assert!(msgs[0].html.contains("https://app.com/dashboard"));
    assert!(msgs[0].html.contains("role=\"presentation\"")); // button rendered
    assert!(
        msgs[0]
            .text
            .contains("Launch Dashboard (https://app.com/dashboard)")
    );
}

#[tokio::test]
async fn end_to_end_locale_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    // Root template only — no locale-specific variant.
    std::fs::write(
        path.join("reset.md"),
        "---\nsubject: \"Reset password\"\n---\n\nClick below to reset.",
    )
    .unwrap();

    let messages = Arc::new(Mutex::new(Vec::new()));
    let mailer = Mailer::new(
        Box::new(CapturingTransport {
            messages: messages.clone(),
        }),
        Box::new(FilesystemProvider::new(path.to_str().unwrap())),
        SenderProfile {
            from_name: "App".to_string(),
            from_email: "app@test.com".to_string(),
            reply_to: None,
        },
        LayoutEngine::new(path.to_str().unwrap()),
    );

    // Request "fr" locale — should fall back to root template.
    mailer
        .send(SendEmail::new("reset", "user@example.com").locale("fr"))
        .await
        .unwrap();

    let msgs = messages.lock().unwrap();
    assert_eq!(msgs[0].subject, "Reset password");
}
