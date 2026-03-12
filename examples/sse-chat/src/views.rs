use serde::Deserialize;

#[modo::view("pages/login.html", htmx = "partials/login_form.html")]
pub(crate) struct LoginPage {
    pub(crate) error: Option<String>,
}

#[modo::view("pages/rooms.html")]
pub(crate) struct RoomsPage {
    pub(crate) username: String,
    pub(crate) rooms: Vec<&'static str>,
}

#[modo::view("pages/chat.html")]
pub(crate) struct ChatPage {
    pub(crate) room: String,
    pub(crate) username: String,
    pub(crate) messages: Vec<String>,
}

#[modo::view("partials/message.html")]
pub(crate) struct MessagePartial {
    pub(crate) username: String,
    pub(crate) text: String,
    pub(crate) created_at: String,
    pub(crate) is_own: bool,
}

#[modo::view("partials/send_form.html")]
pub(crate) struct SendFormPartial {
    pub(crate) room: String,
}

#[derive(Deserialize)]
pub(crate) struct LoginForm {
    pub(crate) username: String,
}

#[derive(Deserialize)]
pub(crate) struct SendForm {
    pub(crate) text: String,
}
