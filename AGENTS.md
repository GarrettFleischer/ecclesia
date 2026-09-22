# Ecclesia

A church posts a need. People who can help say so. Nearby churches see those needs too.

Church rules live in Domain. The SDK talks to the store, cache, mail, and sessions. The App paints HTML.

## Stack

Rust nightly, edition 2024. Crates `ecclesia-domain`, `ecclesia-sdk`, `ecclesia`. Axum 0.8 and Maud. Neon Postgres in public, SQLite locally. sqlx 0.8 runtime SQL, `?` rewritten to `$n` on Postgres. Upstash Redis for fragments and rate limits. HMAC `v2` session cookies backed by `sessions` on the primary. Resend for outbox mail. Word gate on the request. Postgres outbox and `ecclesia-worker`. Fly.io, region `iad`.

Source: `docs/specs/0001-scale-persistence/index.md` (persistence) and `docs/specs/0002-identity-sessions/index.md` (auth and mail).

## Build approach

Tracer Bullet (thin end to end slices through Domain, SDK, and App).

## Git

- integration: off

## Commands

```bash
cargo test
cargo test style
cargo run -p ecclesia
cargo run -p ecclesia --bin ecclesia-worker
```

## Specs

Stored in `docs/specs/`. Format: `docs/specs/NNNN-title/` with `index.md`.

## Rules

- App depends on SDK only. Domain never does I/O.
- A story takes values in and returns `Result<Effect, DomainError>`.
- Do not pass `bool` as a function argument.
- HTTP loads values, calls one SDK story, paints HTML.
- Copy follows `docs/PROSE.md`. Run `cargo test style` after copy or Rust changes.
- A public host (`FLY_APP_NAME` or `ECCLESIA_PUBLIC=1`) requires `DATABASE_URL`, `UPSTASH_REDIS_URL`, `RESEND_API_KEY`, `RESEND_FROM`, and `ECCLESIA_PUBLIC_URL`.

## Agent skills

Installed under `.agents/skills/`. Per crate lists live in the nested `AGENTS.md` files below.

MCP servers: Neon (recommended), Upstash Redis (recommended)

## Context files

- [crates/domain/AGENTS.md](crates/domain/AGENTS.md): pure church rules
- [crates/sdk/AGENTS.md](crates/sdk/AGENTS.md): store, sessions, mail, and cache
- [crates/app/AGENTS.md](crates/app/AGENTS.md): HTTP and HTML

_Drafted by /jsm-audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
