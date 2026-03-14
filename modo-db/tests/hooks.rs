use modo_db::sea_orm::{ConnectionTrait, Database, DatabaseConnection};

// Force inventory registration of test entities
#[allow(unused_imports)]
use hook_item as _;
#[allow(unused_imports)]
use no_hook_item as _;

// -- Entity with a custom before_save hook ------------------------------------

#[modo_db::entity(table = "hook_items")]
#[entity(timestamps)]
pub struct HookItem {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    pub name: String,
}

// Inherent method — takes priority over DefaultHooks blanket impl
impl HookItem {
    pub fn before_save(&mut self) -> Result<(), modo::Error> {
        self.name = self.name.to_uppercase();
        Ok(())
    }
}

// -- Entity WITHOUT a custom hook (default no-op) -----------------------------

#[modo_db::entity(table = "nohook_items")]
#[entity(timestamps)]
pub struct NoHookItem {
    #[entity(primary_key, auto = "ulid")]
    pub id: String,
    pub name: String,
}

// -- Setup helper -------------------------------------------------------------

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS hook_items (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS nohook_items (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .unwrap();
    db
}

// -- Tests --------------------------------------------------------------------

#[tokio::test]
async fn test_hook_fires_on_insert() {
    let db = setup_db().await;

    let item = HookItem {
        name: "hello".to_string(),
        ..Default::default()
    };
    let inserted = item.insert(&db).await.unwrap();

    assert_eq!(
        inserted.name, "HELLO",
        "before_save hook should uppercase the name on insert"
    );

    // Verify the stored value is also uppercased
    let found = HookItem::find_by_id(&inserted.id, &db).await.unwrap();
    assert_eq!(found.name, "HELLO");
}

#[tokio::test]
async fn test_hook_fires_on_update() {
    let db = setup_db().await;

    let item = HookItem {
        name: "initial".to_string(),
        ..Default::default()
    };
    let mut item = item.insert(&db).await.unwrap();
    let id = item.id.clone();

    item.name = "world".to_string();
    item.update(&db).await.unwrap();

    assert_eq!(
        item.name, "WORLD",
        "before_save hook should uppercase the name on update"
    );

    let found = HookItem::find_by_id(&id, &db).await.unwrap();
    assert_eq!(found.name, "WORLD");
}

#[tokio::test]
async fn test_no_hook_uses_default_noop() {
    let db = setup_db().await;

    let item = NoHookItem {
        name: "hello".to_string(),
        ..Default::default()
    };
    let inserted = item.insert(&db).await.unwrap();

    assert_eq!(
        inserted.name, "hello",
        "entity without custom hook should leave name unchanged"
    );

    let found = NoHookItem::find_by_id(&inserted.id, &db).await.unwrap();
    assert_eq!(found.name, "hello");
}
