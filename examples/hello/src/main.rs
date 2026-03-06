use modo::error::HttpError;
use modo::extractors::ValidatedForm;

#[derive(serde::Deserialize, modo::Validate)]
struct ContactForm {
    #[validate(required, email)]
    email: String,

    #[validate(required, min_length = 5, max_length = 1000)]
    message: String,
}

#[modo::handler(GET, "/")]
async fn index(request_id: modo::RequestId) -> String {
    format!("Hello modo! (request: {request_id})")
}

#[modo::handler(GET, "/health")]
async fn health() -> &'static str {
    "ok"
}

#[modo::handler(GET, "/error")]
async fn error_example() -> Result<&'static str, HttpError> {
    Err(HttpError::NotFound)
}

#[modo::handler(POST, "/contact")]
async fn contact(ValidatedForm(_form): ValidatedForm<ContactForm>) -> &'static str {
    "Thanks for your message!"
}

#[modo::main]
async fn main(app: modo::app::AppBuilder) -> Result<(), Box<dyn std::error::Error>> {
    app.run().await
}
