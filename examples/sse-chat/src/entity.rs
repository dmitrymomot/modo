#[modo_db::entity(table = "messages")]
#[entity(timestamps)]
pub struct Message {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    #[entity(indexed)]
    pub room: String,
    pub username: String,
    pub text: String,
}
