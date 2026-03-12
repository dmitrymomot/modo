use modo::sse::SseBroadcastManager;

#[derive(Debug, Clone)]
pub(crate) struct ChatEvent {
    pub(crate) username: String,
    pub(crate) text: String,
    pub(crate) created_at: String,
}

pub(crate) type ChatBroadcaster = SseBroadcastManager<String, ChatEvent>;

pub(crate) const ROOMS: &[&str] = &["general", "random", "support", "dev"];
