# rskit

Rust web framework for micro-SaaS. Single binary, SQLite-only, maximum compile-time magic.

## Stack

- axum 0.8 (HTTP)
- SeaORM v2 RC (database) — use v2 only, not v1.x
- Askama (templates, Phase 2)
- inventory (auto-discovery, not linkme)
- tokio (async runtime)

## Architecture

- `rskit/` — main library crate
- `rskit-macros/` — proc macro crate
- Design doc: `docs/plans/2026-03-04-rskit-architecture-design.md`
- Phase 1 plan: `docs/plans/2026-03-04-phase1-foundation.md`

## Commands

- `cargo check` — type check
- `cargo test` — run all tests
- `cargo build --example hello` — build example
- `cargo run --example hello` — run example server

## Conventions

- Handlers: `#[rskit::handler(METHOD, "/path")]`
- Entry point: `#[rskit::main]`
- Routes auto-discovered via `inventory` crate
- DB extractor: `Db(db): Db`
- Service extractor: `Service<MyType>`
- Errors: `Result<T, RskitError>`
- Middleware: plain async functions, attached via `#[middleware(fn_name(params))]`
- Services: manually constructed, registered via `.service(instance)`

## Key Decisions

- "Full magic" — proc macros for everything, auto-discovery, zero runtime cost
- SQLite only — WAL mode, no Postgres/Redis
- Cron jobs: in-memory only (tokio timers), errors logged via tracing
- Multi-tenancy: both per-DB and shared-DB strategies supported
- Auth: layered traits with swappable defaults
- Use official documentation only when researching dependencies
