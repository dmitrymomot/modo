# HTMX Integration Design

**Date:** 2026-03-11
**Status:** Approved
**Scope:** Extend the `modo` crate (within the `templates` feature) with first-class HTMX support

## Overview

modo already has basic HTMX support: dual-template rendering via `#[view("page.html", htmx = "partial.html")]`, `hx-request` detection in the render layer, non-200 pass-through, and status forcing. This design extends that foundation with a type-erased HTMX response type, response composition (including OOB), a smart redirect, a request extractor, and a response builder.

**Key principle:** OOB swap targeting (`hx-swap-oob`, `id` attributes) lives entirely in HTML templates. The server-side framework handles response composition, HTMX headers, and type-safe polymorphic returns — not OOB markup generation.

## Components

### 1. `#[modo::view]` Macro — No OOB Changes

The existing `#[view]` macro is unchanged. OOB fragments are just regular views whose templates contain `hx-swap-oob` attributes.

```rust
// Full page + HTMX partial (existing, unchanged)
#[modo::view("pages/items.html", htmx = "partials/items.html")]
struct ItemList { items: Vec<Item> }

// OOB fragment — just a regular view, OOB targeting is in the template
#[modo::view("partials/toast_success.html")]
struct ToastSuccess { message: String, ttl: u32 }

#[modo::view("partials/toast_error.html")]
struct ToastError { message: String, ttl: u32 }
```

The OOB behavior is defined in the template itself:
```html
<!-- partials/toast_success.html -->
<div id="notifications" hx-swap-oob="innerHTML">
  <div class="toast toast-success" data-ttl="{{ ttl }}">{{ message }}</div>
</div>
```

```html
<!-- partials/row.html (append to table body) -->
<tr hx-swap-oob="beforeend:#items-table tbody">
  <td>{{ name }}</td>
  <td>{{ value }}</td>
</tr>
```

**Macro behavior changes:**
- `#[view]` now generates `Into<Htmx>` implementation (for use in `HtmxResult`)
- `#[view]` now generates `.with_oob()` method (for composing responses)

### 2. `Htmx` Type — Type-Erased HTMX Response

A type-erased response container that any `#[view]` struct can convert into via `.into()`. Enables polymorphic handler returns.

```rust
pub struct Htmx { /* internal: boxed view + OOB fragments + HTMX headers */ }
```

**Conversions:**
- Any `#[view]` struct converts via `.into()`
- `modo::Redirect` converts via `.into()`
- `modo::htmx::response()` builder converts via `.into()`
- Implements `IntoResponse`

### 3. `HtmxResult` Type Alias

```rust
pub type HtmxResult<E = Error> = Result<Htmx, E>;
```

Note: unlike `HandlerResult<T, E>` and `JsonResult<T, E>`, the error type is the first (and only) generic parameter since `Htmx` is already type-erased.

### 4. `.with_oob()` Method

Available on any `#[view]` struct. Attaches one or more OOB fragments to a response. Chainable. The OOB fragment is just another `#[view]` struct — its template is rendered and appended to the response body. HTMX processes the `hx-swap-oob` attribute from the rendered HTML.

```rust
// Single OOB
Ok(ItemList { items }
    .with_oob(ToastSuccess { message: "Created!".into(), ttl: 3 })
    .into())

// Multiple OOB
Ok(ItemList { items }
    .with_oob(ToastSuccess { message: "Created!".into(), ttl: 3 })
    .with_oob(CartBadge { count: 5 })
    .into())
```

**Implementation:** The `#[view]` macro generates a `.with_oob()` method that wraps `self` into an intermediate type holding the main view + a `Vec` of OOB fragments. This intermediate type implements `Into<Htmx>`.

**Context merging:** OOB fragments go through the same `TemplateContext` merging as the main view, so `{{ csrf_token }}`, `{{ t("key") }}`, and other context values work in OOB templates.

### 5. `HtmxRequest` Extractor

Parses HTMX-specific request headers. Implements axum's `FromRequestParts`.

**Infallible:** always succeeds, even for non-HTMX requests. `is_htmx()` returns `false` when the `HX-Request` header is absent.

```rust
pub struct HtmxRequest { /* parsed from headers */ }

impl HtmxRequest {
    /// Whether this is an HTMX request (HX-Request header present)
    pub fn is_htmx(&self) -> bool;

    /// Whether the request is via hx-boost (HX-Boosted)
    pub fn is_boosted(&self) -> bool;

    /// Whether this is a history restoration request (HX-History-Restore-Request)
    pub fn is_history_restore(&self) -> bool;

    /// The id of the target element (HX-Target)
    pub fn target(&self) -> Option<&str>;

    /// The id of the triggered element (HX-Trigger)
    pub fn trigger(&self) -> Option<&str>;

    /// The name of the triggered element (HX-Trigger-Name)
    pub fn trigger_name(&self) -> Option<&str>;

    /// The user response to hx-prompt (HX-Prompt)
    pub fn prompt(&self) -> Option<&str>;

    /// The current URL of the browser (HX-Current-URL)
    pub fn current_url(&self) -> Option<&str>;
}
```

**Usage in handlers:**
```rust
#[modo::handler(POST, "/items")]
async fn create_item(hx: HtmxRequest, form: Form<CreateItem>) -> HtmxResult {
    if hx.is_boosted() { /* ... */ }
    // ...
}
```

### 6. `modo::redirect()` — Smart Redirect

A single function that produces the correct redirect for both HTMX and non-HTMX requests.

```rust
pub fn redirect(url: impl Into<String>) -> Redirect;
```

`modo::Redirect` is a **new custom type** (not axum's `axum::response::Redirect`). It stashes the target URL in response extensions. The render layer detects it and emits:

- Normal request: standard HTTP 302 redirect
- HTMX request: `HX-Redirect` header + 200 status

**Return type compatibility:**
```rust
// Direct return
async fn logout() -> Redirect { modo::redirect("/login") }

// With error handling
async fn create() -> HandlerResult<Redirect> { Ok(modo::redirect("/items")) }

// In HTMX polymorphic handler
async fn create() -> HtmxResult { Ok(modo::redirect("/items")) }
```

### 7. HTMX Response Builder

For advanced use cases requiring multiple HTMX response headers or combining headers with rendered views.

```rust
modo::htmx::response()
    // Navigation
    .redirect("/path")                     // HX-Redirect
    .location("/path")                     // HX-Location (simple string)
    .location_with(HxLocation {            // HX-Location (JSON object)
        path: "/path".into(),
        target: Some("#content".into()),
        swap: Some("innerHTML".into()),
        select: None,
        values: None,
        headers: None,
    })
    .push_url("/path")                     // HX-Push-Url
    .replace_url("/path")                  // HX-Replace-Url
    .refresh()                             // HX-Refresh: true

    // Swap control
    .reswap("outerHTML")                   // HX-Reswap
    .retarget("#sidebar")                  // HX-Retarget
    .reselect("#content")                  // HX-Reselect

    // Events
    .trigger("eventName")                  // HX-Trigger (simple)
    .trigger_with("showMessage", json!({   // HX-Trigger (with data)
        "level": "info",
        "message": "Done"
    }))
    .trigger_after_swap("highlight")       // HX-Trigger-After-Swap
    .trigger_after_settle("fadeIn")        // HX-Trigger-After-Settle

    // Content
    .render(ItemList { items })            // Attach rendered view
    .oob(ToastSuccess { ... })             // Attach OOB fragment

    // Convert
    .into()                                // -> Htmx
```

**`HxLocation` struct:**
```rust
#[derive(Serialize)]
pub struct HxLocation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
}
```

**Builder rules:**
- `.redirect()`, `.location()`, `.refresh()` are mutually exclusive with `.render()` — redirect responses have no body. Conflicting calls panic at runtime on `.into()`.
- `.push_url()` and `.replace_url()` are mutually exclusive. Conflicting calls panic at runtime on `.into()`.
- Multiple `.trigger()` / `.trigger_with()` calls accumulate (serialized as JSON object in the header)
- `.oob()` is chainable for multiple OOB fragments
- `.render()` takes any `#[view]` struct
- `.oob()` takes any `#[view]` struct (its template provides the OOB markup)
- The whole builder converts `.into()` `Htmx`

## Response Types Summary

| Pattern | Return type | When to use |
|---|---|---|
| Direct | `MyView` | No errors possible |
| `HandlerResult<T>` | `Result<T, Error>` | Single response type, needs `?` |
| `JsonResult` | `Result<Json<Value>, Error>` | Ad-hoc JSON |
| `JsonResult<T>` | `Result<Json<T>, Error>` | Typed JSON |
| `HtmxResult` | `Result<Htmx, Error>` | Multiple HTMX types or OOB |

## Handler Examples

### Simple view — no change
```rust
#[modo::view("pages/home.html", htmx = "partials/clock.html")]
struct HomePage { time: String, date: String }

#[modo::handler(GET, "/")]
async fn home() -> HomePage {
    let now = chrono::Local::now();
    HomePage {
        time: now.format("%H:%M:%S").to_string(),
        date: now.format("%A, %B %d, %Y").to_string(),
    }
}
```

### Single type with error handling
```rust
#[modo::handler(GET, "/items")]
async fn list_items(Db(db): Db) -> HandlerResult<ItemList> {
    let items = Item::find().all(&*db).await?;
    let item = items.first().ok_or(modo::HttpError::NotFound)?;
    Ok(ItemList { items })
}
```

### Form handling — validation, expected error, success
```rust
#[modo::view("partials/form.html")]
struct CreateItemForm { values: CreateItem, errors: ValidationErrors }

#[modo::view("partials/toast_success.html")]
struct ToastSuccess { message: String, ttl: u32 }

#[modo::view("partials/toast_error.html")]
struct ToastError { message: String, ttl: u32 }

#[modo::handler(POST, "/items")]
async fn create_item(Db(db): Db, form: Form<CreateItem>) -> HtmxResult {
    // Validation error -> re-render form with inline errors
    if let Err(errors) = form.validate() {
        return Ok(CreateItemForm { values: form.into_inner(), errors }.into());
    }

    // Unexpected error -> propagate via ?
    let result = Item::insert(form.into_active_model())
        .exec(&*db).await;

    match result {
        // Expected error -> error toast
        Err(e) => Ok(ToastError { message: e.to_string(), ttl: 5 }.into()),
        // Success -> success toast
        Ok(_) => Ok(ToastSuccess { message: "Created!".into(), ttl: 3 }.into()),
    }
}
```

### Main view + OOB toast
```rust
#[modo::handler(POST, "/items")]
async fn create_item(Db(db): Db, form: Form<CreateItem>) -> HtmxResult {
    Item::insert(form.into_active_model()).exec(&*db).await?;
    let items = Item::find().all(&*db).await?;

    Ok(ItemList { items }
        .with_oob(ToastSuccess { message: "Created!".into(), ttl: 3 })
        .into())
}
```

### Smart redirect
```rust
#[modo::handler(POST, "/logout")]
async fn logout(session: SessionManager) -> HandlerResult<Redirect> {
    session.logout().await?;
    Ok(modo::redirect("/login"))
}
```

### Response builder — advanced
```rust
#[modo::handler(POST, "/items")]
async fn create_item(Db(db): Db, form: Form<CreateItem>) -> HtmxResult {
    Item::insert(form.into_active_model()).exec(&*db).await?;
    let items = Item::find().all(&*db).await?;

    Ok(modo::htmx::response()
        .push_url("/items")
        .trigger("itemCreated")
        .render(ItemList { items })
        .oob(ToastSuccess { message: "Created!".into(), ttl: 3 })
        .into())
}
```

### OOB-only response (toast without main view)
```rust
#[modo::handler(DELETE, "/items/{id}")]
async fn delete_item(Db(db): Db, id: String) -> HtmxResult {
    let item = Item::find_by_id(&id).one(&*db).await?
        .ok_or(modo::HttpError::NotFound)?;
    item.delete(&*db).await?;

    Ok(ToastSuccess { message: "Deleted".into(), ttl: 3 }.into())
}
```

## Integration with Existing Systems

### Render Layer Changes

The existing `RenderLayer` in `modo/src/templates/render.rs` needs to handle three extension types, checked in this order:

1. **`Htmx`** — render main view (if present) + all OOB fragments, apply HTMX response headers, force 200 status
2. **`View`** — existing behavior (render template, HTMX partial selection, status forcing)
3. **`Redirect`** — check `hx-request` header, emit `HX-Redirect` + 200 or standard 302

Additional render layer responsibilities:
- OOB fragments go through the same `TemplateContext` merging as the main view
- Add `Vary: HX-Request` header when response content differs based on HTMX detection
- When `Htmx` contains a view with `htmx = "..."`, the dual-template selection (full page vs HTMX partial) still applies based on `hx-request` header

### View Macro Changes

The `#[view]` macro in `modo-macros/src/view.rs` needs to:

1. Generate `Into<Htmx>` implementation for all `#[view]` structs
2. Generate `.with_oob()` method for all `#[view]` structs

No `oob` parameter parsing — OOB markup lives in templates.

### No New Crates or Feature Flags

All changes live in:
- `modo-macros/` — macro extensions (`Into<Htmx>`, `.with_oob()`)
- `modo/src/templates/` — render layer, `Htmx` type, builder
- `modo/src/` — `HtmxResult` alias, `Redirect` type, `redirect()`, `HtmxRequest` extractor

Everything is gated behind the existing `templates` feature.

## Non-Goals

- No client-side HTMX JS bundling or serving (users include htmx.js themselves)
- No HTMX extensions support (users add extensions via HTML)
- No WebSocket/SSE integration (separate concern)
- No HTMX-specific error handler (existing `ErrorContext::is_htmx()` suffices)
- No server-side OOB markup generation — `hx-swap-oob` attributes belong in templates
