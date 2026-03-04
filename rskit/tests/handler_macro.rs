use rskit::router::RouteRegistration;

#[rskit::handler(GET, "/hello")]
async fn hello() -> &'static str {
    "Hello rskit"
}

#[rskit::handler(POST, "/echo")]
async fn echo(body: String) -> String {
    body
}

#[test]
fn test_handler_macro_registers_routes() {
    let routes: Vec<&RouteRegistration> = inventory::iter::<RouteRegistration>().collect();
    let paths: Vec<&str> = routes.iter().map(|r| r.path).collect();
    assert!(paths.contains(&"/hello"), "GET /hello not registered");
    assert!(paths.contains(&"/echo"), "POST /echo not registered");
}

#[test]
fn test_handler_macro_correct_methods() {
    let routes: Vec<&RouteRegistration> = inventory::iter::<RouteRegistration>().collect();
    let hello = routes.iter().find(|r| r.path == "/hello").unwrap();
    assert_eq!(hello.method, rskit::router::Method::GET);

    let echo = routes.iter().find(|r| r.path == "/echo").unwrap();
    assert_eq!(echo.method, rskit::router::Method::POST);
}
