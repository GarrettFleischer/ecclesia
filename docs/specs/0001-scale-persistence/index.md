# 0001. Scale persistence foundation

**Date**: 2026-09-21
**Status**: Accepted

Passwords, session rows, `v2` cookies, and outbox mail live in [0002](../0002-identity-sessions/index.md). This spec stays the persistence, cache, scoped reads, and push foundation.

## Summary

Ecclesia keeps church rules in a Domain crate (pure functions, no disk or clock). An SDK crate is the only code that talks to Domain and to the machine. The App crate (HTML, routes, Fly binaries) talks to the SDK only. The public store is one Neon Postgres (a hosted relational database). Local work and tests keep SQLite. Home loads a page of 20 needs the viewer may see, not every open need in the body.

## Structure

- [0001-lib-layers.md](0001-lib-layers.md): workspace crates, rename leaf to Domain, SDK prelude and story functions. Supports the one way library line.
- [0001-postgres-skin.md](0001-postgres-skin.md): Neon for the public host, SQLite for local and tests, indexes, dialect helper. Supports one relational primary under the SDK.
- [0001-scoped-reads.md](0001-scoped-reads.md): stop fetch all on Home and the body. Page size 20. Domain still decides visibility. Supports a million members on one primary.
- [0001-fragment-cache.md](0001-fragment-cache.md): Upstash holds shared fragments and rate limit keys, not landing HTML. Supports two Fly machines without an in process gate.
- [0001-outbox-worker.md](0001-outbox-worker.md): outbox rows in the same transaction as the Effect, `ecclesia-worker` on Fly, Web Push, dead after 8 tries. Supports request time that does not wait on phones.

## Shared contract

These rules bind every child. A child must not weaken them.

1. Cargo edges are App to SDK to Domain only. App does not take `ecclesia-domain` or `sqlx` as a direct dependency. Domain takes neither SDK nor App.
2. Domain stays values in, `Result<Effect, DomainError>` out. No SQL, clock, UUID, cookies, env, sockets, or HTTP.
3. A write path is word gate (SDK), then one SDK story (load, call Domain, commit). The App never calls store apply itself. The word gate runs before any write. `TearsDown` means rewrite and resubmit. Inbox notice rows persist in that same transaction. Outbox rows for push persist in that same transaction.
4. A read path is an SDK scoped query (at most 20 rows, cursor on `created_at` then `id`), then a Domain visibility helper re-exported by the SDK, then Maud. Home and the body must not `fetch_all` every open need or every church.
5. Cache may hold shared fragments (`catalog`, `church:{id}`, `directory`). Cache must not hold landing HTML (CSRF lives in the App paint), one person's Home, or inbox HTML.
6. Church data stays on one primary. `church_id` marks rows that belong to a church. Email stays unique across the body. Do not shard by church.
7. Jev, OpenRouter, and rewrite stay out. The public host uses the word gate unless a later spec turns a judge on.

## Requirements

**User stories**:
- As a builder, I want Domain, SDK, and App as separate crates so a route cannot import SQL or church rules by accident.
- As a member, I want Home to open on Sunday morning without loading every open need in the body.
- As an operator, I want one hosted Postgres, a shared cache, and a worker that talks to phones so two machines can serve the same church.

**Acceptance criteria**:
- **AC-1**: `cargo tree` for the App package shows `ecclesia-sdk` and does not show a direct `ecclesia-domain` or `sqlx` edge. The Domain package shows neither `ecclesia-sdk` nor the App package. (basis: [0001-lib-layers.md](0001-lib-layers.md))
- **AC-2**: Domain source has no `sqlx`, clock, UUID, cookie, env, or socket use. `us_domain_01` fails the build if it grows any of those. (basis: [docs/DOMAIN.md](../../DOMAIN.md))
- **AC-3**: Home requests at most 20 visible needs. The body requests at most 20 churches. Need cursors are `(created_at, id)`. Church list cursors are `(city, name, id)`. Domain `visible_need_cards` still filters the Home page. (basis: [0001-scoped-reads.md](0001-scoped-reads.md))
- **AC-4**: `DATABASE_URL` with a `postgres` scheme opens Neon on the public host. A `sqlite` scheme opens the local file. Tests that do not need HTTP keep MemoryWorld or SQLite. (basis: [0001-postgres-skin.md](0001-postgres-skin.md))
- **AC-5**: Two App processes share rate limit counts in Upstash. A stolen or missing cache is a store read, not a 500. (basis: [0001-fragment-cache.md](0001-fragment-cache.md))
- **AC-6**: Gift catalog, church card, and first directory page can be served from Upstash as JSON. Landing HTML, Home HTML, and inbox HTML are never stored there. (basis: [0001-fragment-cache.md](0001-fragment-cache.md))
- **AC-7**: A successful SDK story writes Effect writes, notice rows, and outbox rows in one transaction. `ecclesia-worker` delivers Web Push. A failed push does not roll back the write. After 8 failed attempts the row is dead. (basis: [0001-outbox-worker.md](0001-outbox-worker.md))
- **AC-8**: Posted words that fail the word gate never reach the store. `JudgeHub::load` is the word gate and ignores Jev and OpenRouter env until a later spec. The person sees the existing tone flash and must change the words. (basis: [crates/sdk/src/judge.rs](../../../crates/sdk/src/judge.rs), [crates/domain/src/flags.rs](../../../crates/domain/src/flags.rs))

## Decision

**Chosen option**: Layered workspace on one relational primary.

Keep Rust, Axum, and Maud. Rename leaf to Domain and split three crates. Put store, session, cache, word gate, and story functions in the SDK. Put HTML and Fly binaries in the App. Use Neon as the public primary, SQLite locally, Upstash for fragments and rate limits, and a Postgres outbox plus `ecclesia-worker` for push.

## Proposed stack

| Layer | Choice | Reason |
|---|---|---|
| Language | Rust nightly, edition 2024 | Already the crate. Domain stays honest functions. (basis: [Cargo.toml](../../../Cargo.toml)) |
| Crate line | `ecclesia-domain`, `ecclesia-sdk`, `ecclesia` | Cargo enforces App to SDK to Domain. Modules in one crate cannot. (basis: layered monolith) |
| Framework | Axum 0.8 plus Maud | Keep the HTML skin. Topcoat asks views to query the store and does not ship jobs yet. (basis: [docs.rs topcoat](https://docs.rs/topcoat/latest/topcoat/)) |
| Primary DB | Neon Postgres in public, SQLite in local and tests | One primary holds a million members if reads stay scoped. sqlx already speaks both. (basis: [Neon pooling](https://neon.com/docs/connect/connection-pooling)) |
| Query style | sqlx 0.8 runtime SQL, `?` rewritten to `$n` on Postgres | This repo already uses `query_as` strings. Compile time `query!` cannot share one text across two dialects. (basis: [sqlx docs](https://docs.rs/sqlx/latest/sqlx/)) |
| Cache | Upstash Redis protocol | Shared rate limits and short lived JSON fragments. Not landing HTML. Not the source of truth. (basis: [Upstash Redis](https://upstash.com/docs/redis/overall/getstarted)) |
| Auth | HMAC `v2` cookie plus `sessions` on the primary | Spec [0002](../0002-identity-sessions/index.md). Two machines load the same row. `ECCLESIA_SECRET` signs the cookie. (basis: [docs/SECURITY.md](../../SECURITY.md)) |
| Mail | Outbox `mail` rows, Resend in the worker | Same transaction as the write. Spec 0002. Public host needs `RESEND_API_KEY`, `RESEND_FROM`, `ECCLESIA_PUBLIC_URL`. (basis: [0002-mail.md](../0002-identity-sessions/0002-mail.md)) |
| Classifier | Word gate on the request, before commit | `JudgeHub::load` is the word gate until a later spec. Ignore Jev and OpenRouter env. `RefineHub` stays silent. Domain still refuses `TearsDown`. (basis: [docs/VOICE.md](../../VOICE.md)) |
| Background jobs | Postgres outbox, `FOR UPDATE SKIP LOCKED`, `ecclesia-worker` | No second bus until the table is hot. (basis: database backed queue first) |
| Hosting | Fly.io App process plus worker | Two processes in `fly.toml`. Region `iad`. Neon `us-east-1`. Public host is `FLY_APP_NAME` or `ECCLESIA_PUBLIC=1`. |
| Observability | tracing, query timing, cache hit rate, outbox lag | A Sunday stall is an old unsent row or a slow Home query, not a silent log. |

## Consequences

**Positive**:
- Church rules stay testable in memory.
- Two machines can share limits and a store.
- Home cost grows with the viewer's churches, not with the whole body.

**Negative / tradeoffs**:
- Three crates and two SQL dialects cost more compile and review time than today's one file.
- Neon, Upstash, and Fly are bills and runbooks before any measured stall.
- Session cookies still bind to a live row. A stolen cookie works until revoke, password change, sign out everywhere, or 30 days idle (spec 0002).

**Neutral**:
- Domain rules live in [docs/DOMAIN.md](../../DOMAIN.md). `.cursor/rules/leaves.mdc` points agents at that file.
- Push stays best effort. Transactional mail uses outbox `kind = mail` (spec 0002).

## Follow-up

- [x] Next foundational spec: identity and sessions (passwords, a shared session store if cookies stop being enough). See [0002](../0002-identity-sessions/index.md).
- [ ] Hosting thickness: TLS and Fly autosize. Region is `iad` with Neon `us-east-1` until that spec says otherwise.
- [x] `/jsm-audit` should write root `AGENTS.md` so later skills see the crate line.
- [ ] Do not enroll Topcoat until it ships jobs and the Domain line can survive fetch in a component.
- [x] Outbox `mail` kind and Resend delivery: [0002](../0002-identity-sessions/index.md).

## Rationale

Reasoning and options: see [rationale.md](rationale.md).
