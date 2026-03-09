# modo Feature Roadmap

Prioritized feature list for the modo framework, grouped by impact for micro-SaaS use cases.

## Tier 1 — High Impact

### `modo-mail` — Email Sending

Email is table stakes for SaaS — signup confirmation, password reset, notifications.

- `MailTransport` trait with pluggable backends
- SMTP backend (lettre)
- HTTP API backends (Resend, AWS SES)
- `MailMessage` builder: to, from, subject, body (text + HTML)
- Integration with `modo-templates` for HTML email rendering
- Integration with `modo-i18n` for localized emails
- `Mail` extractor for handlers
- Async sending (direct or via `modo-jobs` queue)

### `modo-cache` — Caching Layer

In-memory and optional Redis support for performance-critical paths.

- `CacheStore` trait with pluggable backends
- In-memory backend (moka — async, TTL, max size)
- Redis backend (optional feature flag)
- `Cache<T>` extractor for handlers
- Key-value API: `get`, `set`, `delete`, `exists`, `get_or_set`
- TTL per entry
- Cache invalidation helpers
- Use cases: session caching, rate limit state, query result caching

## Tier 2 — Strong Value-Add

### `modo-sse` — Server-Sent Events

Real-time server-to-client streaming. Simpler than WebSockets, works with HTMX.

- `SseStream` response type for handlers
- Named events, retry intervals, last-event-id
- Channel-based broadcasting (one-to-many)
- Integration with HTMX `hx-ext="sse"`
- Automatic client reconnection support
- Use cases: job progress, live notifications, dashboard updates

### OAuth2 Provider — in `modo-auth`

Social login and external identity providers. Table stakes for consumer-facing SaaS.

- `OAuthProvider` trait with pluggable providers (Google, GitHub, Apple, etc.)
- `OAuthConfig` — YAML-deserializable provider configuration (client ID, secret, scopes, endpoints)
- Authorization URL generation + PKCE support
- Callback handler: exchange code → tokens
- `OAuthTokenSet` storage: access token, refresh token, expiry, scopes
- Automatic token refresh when access token expired (transparent to application code)
- `OAuthUserProvider` wrapper — implements `UserProvider`, handles token lifecycle
- Token storage in session data (JSON) or pluggable `TokenStore` trait
- Account linking: connect OAuth identity to existing user
- Multi-provider support per user
- Integration with `modo-session` for login flow state (CSRF via `state` param)

### API Key Authentication — in `modo-auth`

Many SaaS products expose APIs to customers.

- `ApiKey` extractor for handlers
- Key generation with prefix (e.g., `modo_live_xxx`, `modo_test_xxx`)
- Hashed storage (SHA256, never store plaintext)
- Key scoping (read, write, admin)
- Per-key rate limiting integration
- Key rotation (create new before revoking old)
- `ApiKeyProvider` trait (similar to `UserProvider`)

### Audit Logging — `modo-audit`

Who did what when. Compliance requirement for B2B SaaS.

- `AuditLog::record(actor, action, resource, details)`
- Database-backed (auto-registered entity)
- Actor: user ID, API key, system
- Action: create, update, delete, login, etc.
- Resource: entity type + ID
- Optional diff/details JSON
- `AuditQuery` for filtering/searching logs
- Middleware option for automatic CRUD logging

## Tier 3 — Nice to Have

### WebSocket Support — `modo-ws`

Full-duplex communication for interactive features.

- `WebSocket` extractor upgrade
- Message types: text, binary, ping/pong
- Room/channel abstraction
- Authentication on upgrade
- Use cases: chat, collaborative editing, live cursors

### CLI Scaffolding Tool

Developer experience improvement for project bootstrapping.

- `modo new <project>` — scaffold new project
- `modo generate handler <name>` — create handler file
- `modo generate entity <name>` — create entity with migration
- `modo generate job <name>` — create background job
- Template-based generation

### Metrics & Observability — `modo-metrics`

Production monitoring and alerting.

- Prometheus metrics endpoint (`/metrics`)
- Request metrics: latency, status codes, in-flight count
- Database metrics: query count, connection pool stats
- Job metrics: queue depth, processing time, failure rate
- Custom application metrics API
- Optional Grafana dashboard templates

### Webhook Delivery — `modo-webhooks`

Outbound webhooks for SaaS integrations.

- Webhook endpoint registration (URL, events, secret)
- HMAC-SHA256 request signing
- Delivery with retry (exponential backoff via `modo-jobs`)
- Delivery log with status tracking
- Event filtering per endpoint
- Payload serialization (JSON)

### RBAC / Permissions

Role-based access control for multi-user apps.

- `#[middleware(require_role("admin"))]`
- `#[middleware(require_permission("posts:write"))]`
- `Permissions` extractor
- Role → permissions mapping
- `RoleProvider` trait (similar to `UserProvider`)
- Could be app-level initially, framework provides primitives
