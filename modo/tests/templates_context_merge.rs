#![cfg(feature = "templates")]

use minijinja::Value;
use modo::templates::TemplateContext;

#[test]
fn merge_with_combines_request_and_user_context() {
    let mut ctx = TemplateContext::new();
    ctx.insert("request_key", Value::from("request_value"));
    ctx.insert("shared_key", Value::from("from_request"));

    let user_ctx = Value::from_serialize(&serde_json::json!({
        "user_key": "user_value",
        "shared_key": "from_user"
    }));

    let merged = ctx.merge_with(user_ctx);

    // Request-only key preserved
    assert_eq!(
        merged.get_attr("request_key").unwrap().to_string(),
        "request_value"
    );
    // User-only key present
    assert_eq!(
        merged.get_attr("user_key").unwrap().to_string(),
        "user_value"
    );
    // User context wins on collision
    assert_eq!(
        merged.get_attr("shared_key").unwrap().to_string(),
        "from_user"
    );
}

#[test]
fn merge_with_empty_user_context() {
    let mut ctx = TemplateContext::new();
    ctx.insert("key", Value::from("value"));

    let merged = ctx.merge_with(Value::UNDEFINED);

    assert_eq!(merged.get_attr("key").unwrap().to_string(), "value");
}
