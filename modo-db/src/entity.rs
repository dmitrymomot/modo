/// Registration info for a SeaORM entity, collected via `inventory`.
///
/// The `#[modo_db::entity]` macro generates an `inventory::submit!` block
/// for each entity. Framework entities (migrations, sessions)
/// register themselves identically with `is_framework: true`.
pub struct EntityRegistration {
    pub table_name: &'static str,
    pub create_table: fn(sea_orm::DbBackend) -> sea_orm::sea_query::TableCreateStatement,
    pub is_framework: bool,
    pub extra_sql: &'static [&'static str],
}

inventory::collect!(EntityRegistration);
