use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct GreetingPayload {
    pub(crate) name: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ReminderPayload {
    pub(crate) message: String,
}
