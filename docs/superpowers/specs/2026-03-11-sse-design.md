# SSE (Server-Sent Events) Feature Design

**Date:** 2026-03-11
**Status:** Approved
**Location:** `modo/` core crate, feature = `"sse"`

## Overview

Add Server-Sent Events support to the `modo` core crate as an opt-in feature flag. The module provides a clean streaming primitive for real-time event delivery over HTTP, with ergonomic helpers for broadcasting to multiple clients.

## Use Cases

- **Support chat:** Per-conversation channels, multiple participants (user, agent, AI bot), participants can change mid-conversation
- **Uptime monitoring dashboard:** Per-tenant broadcast, one data source to many viewers
- **Real-time notifications:** Per-user channels, multiple tabs receive the same events

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Module location | `modo/` core crate, feature = `"sse"` | SSE is an HTTP response type, belongs with `Json`, `ViewResponse` |
| Room/channel management | Not included | Domain logic — varies per use case |
| Format selection | Per-handler | Each endpoint explicitly chooses JSON, HTML, or text |
| Template rendering in SSE | Handler responsibility | SSE module is format-agnostic; handler uses `ViewRenderer::render_to_string()` |
| Broadcast model | All subscribers of a key receive all messages | Filter on consumer side via stream combinators |
| Global broadcast (`send_all`) | Not included | Rare in multi-tenant SaaS; YAGNI |
| Send to specific subscriber | Not included | No compelling use case within a keyed channel |

## Dependencies

No new external crates required:

- `axum::response::sse` — built-in SSE response types
- `tokio::sync::broadcast` — multi-producer, multi-consumer channels
- `tokio::sync::RwLock` — concurrent access to channel registry
- `futures-util` — `Stream` trait and combinators (already a dependency)

## Types

### `SseEvent`

Builder for a single SSE event. Wraps `axum::response::sse::Event`.

```rust
SseEvent::new()
    .event("message")                    // named event type (optional)
    .data("plain text")                  // string payload
    .json(&my_struct)?                   // JSON-serialized payload (mutually exclusive with .data/.html)
    .html("<div>fragment</div>")         // HTML fragment payload (mutually exclusive with .data/.json)
    .id("evt-123")                       // last-event-id for reconnection (optional)
    .retry(Duration::from_secs(5))       // client reconnect hint (optional)
```

- `.data()`, `.json()`, `.html()` are mutually exclusive — each sets the data payload
- `.html()` is semantically identical to `.data()` but communicates intent; may gain escaping/wrapping behavior later
- `.json()` is fallible (returns `Result`) because serialization can fail

### `SseResponse`

Handler return type. Wraps `axum::response::sse::Sse<S>` with automatic keep-alive configured from `SseConfig`.

Implements `IntoResponse` so handlers can return it directly.

### `SseConfig`

Optional YAML-deserializable configuration. Auto-registered in `AppBuilder::run()`.

```yaml
sse:
  keep_alive_interval: 15s   # default: 15 seconds
```

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SseConfig {
    #[serde(default = "default_keep_alive_interval")]
    pub keep_alive_interval: Duration,  // default: 15s
}
```

### `SseSender`

Channel sender for imperative message production within a `modo::sse::channel()` closure.

```rust
impl SseSender {
    async fn send(&self, event: SseEvent) -> Result<(), Error>;
}
```

Returns an error if the client has disconnected — no sending into the void.

### `SseBroadcastManager<K, T>`

Registry of keyed broadcast channels. One manager per domain concept, registered as a service.

```rust
impl<K, T> SseBroadcastManager<K, T>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    T: Into<SseEvent> + Clone + Send + Sync + 'static,
{
    /// Create a new manager with the given per-channel buffer size.
    fn new(buffer: usize) -> Self;

    /// Subscribe to a keyed channel.
    /// Creates the channel lazily on first subscription.
    fn subscribe(&self, key: &K) -> SseStream<T>;

    /// Send an event to all subscribers of a keyed channel.
    /// Returns the number of receivers that got the message.
    /// Returns Ok(0) if no subscribers exist for the key.
    fn send(&self, key: &K, event: T) -> Result<usize, Error>;

    /// Number of active subscribers for a key.
    fn subscriber_count(&self, key: &K) -> usize;

    /// Manually remove a channel. Typically not needed — channels
    /// auto-cleanup when the last subscriber drops.
    fn remove(&self, key: &K);
}
```

**Internals:** `Arc<RwLock<HashMap<K, broadcast::Sender<T>>>>`. Channels created lazily on first `subscribe()`. Auto-removed when the last subscriber's `SseStream` is dropped (detected via `broadcast::Sender::receiver_count() == 0` on next operation, or a background cleanup).

### `SseStream<T>`

Wraps `broadcast::Receiver<T>`, implements `Stream<Item = Result<SseEvent, Error>>`. Converts `T` to `SseEvent` via `Into<SseEvent>` on each poll.

Handles `RecvError::Lagged` gracefully — logs a warning and continues (slow consumers skip missed messages rather than disconnecting).

## Entry Points

### `modo::sse::from_stream`

```rust
pub fn from_stream<S, E>(stream: S) -> SseResponse
where
    S: Stream<Item = Result<SseEvent, E>> + Send + 'static,
    E: Into<Error>,
```

Main entry point. Wraps any stream as an SSE response with auto keep-alive.

### `modo::sse::channel`

```rust
pub fn channel<F, Fut>(f: F) -> SseResponse
where
    F: FnOnce(SseSender) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Error>> + Send,
```

Spawns the closure as a tokio task, returns an `SseResponse` backed by the receiver end. When the closure returns or the client disconnects, everything cleans up.

## Stream Ergonomics

### `SseStreamExt` trait

Extension trait on `Stream` for ergonomic event mapping.

```rust
pub trait SseStreamExt: Stream {
    /// Set event name on each item (item must impl Into<SseEvent>).
    fn sse_event(self, name: &'static str) -> impl Stream<Item = Result<SseEvent, Error>>;

    /// Serialize each item as JSON data.
    fn sse_json(self) -> impl Stream<Item = Result<SseEvent, Error>>;

    /// Map each item to an SseEvent with a custom closure.
    fn sse_map<F>(self, f: F) -> impl Stream<Item = Result<SseEvent, Error>>
    where
        F: FnMut(Self::Item) -> Result<SseEvent, Error>;
}
```

## Integration with `modo` Core

### `AppBuilder::run()` changes

Within `#[cfg(feature = "sse")]`:

1. Load `SseConfig` from app config (with defaults)
2. Register `SseConfig` as a service

No middleware or layers needed — SSE responses are self-contained.

### `lib.rs` re-exports

```rust
#[cfg(feature = "sse")]
pub mod sse;
```

Public items from `modo::sse`:
- `SseEvent`, `SseResponse`, `SseConfig`
- `SseSender`
- `SseBroadcastManager`, `SseStream`
- `SseStreamExt` (trait)
- `from_stream`, `channel`

### `Cargo.toml` feature

```toml
[features]
sse = []  # no extra deps — uses axum's built-in SSE + tokio broadcast
```

No new dependencies. Everything needed is already in `axum` and `tokio`.

## File Structure

```
modo/src/sse/
├── mod.rs              # Public re-exports, module docs, entry point functions
├── event.rs            # SseEvent builder
├── response.rs         # SseResponse wrapper
├── config.rs           # SseConfig
├── sender.rs           # SseSender for channel()
├── broadcast.rs        # SseBroadcastManager<K, T>, SseStream<T>
└── stream_ext.rs       # SseStreamExt trait
```

## Documentation Requirements

Every public type, method, and function must have:

- **Module-level doc** (`mod.rs`): Overview of the SSE feature, when to use it, quick-start example covering all three patterns (from_stream, channel, broadcast manager)
- **Type-level docs**: Purpose, when to use, complete example
- **Method-level docs**: What it does, parameters, return value, error conditions, example where non-obvious
- **`# Examples`** sections: Compilable doc examples for all primary APIs
- **`# Panics`** / **`# Errors`** sections: Where applicable
- **Cross-references**: Link related types (e.g., `SseEvent` docs reference `SseBroadcastManager`)

## Examples

### Chat (HTML via HTMX)

```rust
struct ChatMessage { sender: String, text: String }

impl From<ChatMessage> for SseEvent {
    fn from(msg: ChatMessage) -> Self {
        SseEvent::new()
            .event("message")
            .data(format!("{}: {}", msg.sender, msg.text))
    }
}

#[modo::handler(GET, "/chat/{id}/events")]
async fn chat_stream(
    id: String,
    auth: Auth<User>,
    view: ViewRenderer,
    Service(chat): Service<SseBroadcastManager<String, ChatMessage>>,
) -> SseResponse {
    let user_id = auth.user.id.clone();
    let stream = chat.subscribe(&id)
        .filter(move |msg| msg.sender != user_id)
        .map(move |msg| {
            let html = view.render_to_string(ChatBubbleView::from(&msg))?;
            Ok(SseEvent::new().event("message").html(html))
        });
    modo::sse::from_stream(stream)
}

#[modo::handler(POST, "/chat/{id}/send")]
async fn chat_send(
    id: String,
    Json(msg): Json<SendMessage>,
    Service(chat): Service<SseBroadcastManager<String, ChatMessage>>,
) -> HandlerResult<()> {
    chat.send(&id, ChatMessage::from(msg))?;
    Ok(())
}
```

### Dashboard (JSON)

```rust
struct UptimeCheck { service: String, status: String, latency_ms: u64 }

impl From<UptimeCheck> for SseEvent {
    fn from(check: UptimeCheck) -> Self {
        SseEvent::new()
            .event("check")
            .json(&check)
            .unwrap()
    }
}

#[modo::handler(GET, "/dashboard/events")]
async fn dashboard(
    tenant: Tenant<MyTenant>,
    Service(uptime): Service<SseBroadcastManager<TenantId, UptimeCheck>>,
) -> SseResponse {
    modo::sse::from_stream(uptime.subscribe(&tenant.id))
}
```

### Notifications (per-user, multiple tabs)

```rust
#[modo::handler(GET, "/notifications/events")]
async fn notifications(
    auth: Auth<User>,
    Service(notif): Service<SseBroadcastManager<UserId, Notification>>,
) -> SseResponse {
    modo::sse::from_stream(notif.subscribe(&auth.user.id))
}
```

### Imperative channel (job progress)

```rust
#[modo::handler(GET, "/jobs/{id}/progress")]
async fn job_progress(
    id: String,
    Service(jobs): Service<JobService>,
) -> SseResponse {
    modo::sse::channel(|tx| async move {
        while let Some(status) = jobs.poll_status(&id).await {
            tx.send(SseEvent::new().event("progress").json(&status)?).await?;
            if status.is_done() { break; }
        }
        Ok(())
    })
}
```

## What's NOT Included

- **Room/membership management** — domain logic, not framework concern
- **Global broadcast (`send_all`)** — rare in multi-tenant SaaS
- **Send to specific subscriber** — no use case within keyed channels
- **Message persistence** — application concern
- **WebSocket support** — separate feature if ever needed
- **Authentication/authorization** — use existing `Auth<User>` / `Tenant<T>` extractors
- **Client-side library** — HTMX `hx-ext="sse"` or native `EventSource` API
