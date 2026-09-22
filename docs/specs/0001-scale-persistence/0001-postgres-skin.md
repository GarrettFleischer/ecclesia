# 0001. Postgres skin

## Summary

The public host uses Neon Postgres through a pooled URL. `cargo run` on a laptop and most tests keep SQLite. The SDK owns both. A small helper rewrites `?` to `$n` so one query text serves both dialects. New indexes make church and place lookups cheap.

## Requirements

**User stories**:
- As an operator, I want a hosted Postgres with point in time restore so a bad migrate can be walked back.
- As a builder, I want `cargo test` and `cargo run` to keep working without a Postgres server.

**Acceptance criteria**:
- **AC-P1**: On a public host (`FLY_APP_NAME` set or `ECCLESIA_PUBLIC=1`) boot requires `DATABASE_URL` with a `postgres` or `postgresql` scheme and refuses a missing URL. Otherwise `sqlite` (or the current default `sqlite://ecclesia.db`) opens SQLite WAL as today.
- **AC-P2**: Public App and worker use the Neon pooler URL. sqlx stays on runtime queries, not `query!`. (basis: [Neon pooling](https://neon.com/docs/connect/connection-pooling))
- **AC-P3**: Schema gains the indexes listed below. Existing uniques stay (`users.email`, `churches.invite_code`, `memberships(church_id, user_id)`, `applications(need_id, user_id)`).
- **AC-P4**: Shared query strings use `?`. The SDK rewrites placeholders to `$1`…`$n` when the driver is Postgres. No `sqlx::Any`.
- **AC-P5**: Domain tests and MemoryWorld tests do not start Postgres. HTTP flow tests may keep SQLite.
- **AC-P6**: Neon is configured for point in time recovery and a daily snapshot (operator checklist, not application code).

## Decision

Neon for the public primary. SQLite for local and CI. sqlx 0.8, runtime SQL, placeholder rewrite. Stay on 0.8 until a migrate forces 0.9.

**Indexes** (create in the SDK schema, both dialects):
- `needs(status, created_at)`
- `needs(church_id, status)`
- `churches(region, city)`
- `churches(city, name, id)`
- `memberships(user_id)`
- `memberships(church_id, status)`
- `notifications(user_id, created_at)`
- `outbox(available_at)` where the row is not `done` (see the outbox child)

**Types**: keep `TEXT` ids and ISO timestamps so SQLite and Postgres share the same rows. Do not switch to native UUID or `timestamptz` in this spec.

## Feature design

**Data model sketch**:
Existing tables in [crates/sdk/src/db/schema.rs](../../../crates/sdk/src/db/schema.rs), plus `outbox` from [0001-outbox-worker.md](0001-outbox-worker.md). No church shard key. `church_id` on church scoped tables stays a plain column with indexes.

**State transitions**: none beyond today's status strings.

**API surface**:
SDK store methods keep today's names (`user`, `user_by_email`, `church`, `open_need_cards` becomes the scoped read in the next child). `Db::connect` reads the URL scheme.

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| `Db::connect` | Driver | `DATABASE_URL` scheme |
| Query bind | Placeholder style | Driver: SQLite keeps `?`, Postgres gets `$n` from the helper |
| Pool size | App sqlx pool | Decided here: 16 on Postgres, 8 on SQLite (today's SQLite cap) |
| Neon region | Where data lives | `us-east-1`. Fly region is `iad`. |
| Public host | Require Postgres and Upstash | `FLY_APP_NAME` is set, or `ECCLESIA_PUBLIC=1` |

**Key invariants**:
- One primary. No replica reads in this spec.
- Email remains `UNIQUE` on `users`.
- Foreign keys stay on.

**Security model**:
TLS to Neon. `DATABASE_URL` is a secret. SQLite file permissions stay the operator's job, as [docs/SECURITY.md](../../SECURITY.md) already says.

**Configuration required**:
- `DATABASE_URL`: `sqlite://ecclesia.db` locally. Neon pooled URL in public. (existing, new schemes)
- Neon dashboard: PITR on, daily snapshot. Not an env var.

**Critical test scenarios**:
- Happy path: `DATABASE_URL=sqlite://… cargo test` passes. Verifies **AC-P1**, **AC-P5**
- Happy path: a SDK unit test runs the same `SELECT` text through the placeholder helper and gets `$1` for Postgres. Verifies **AC-P4**
- Failure: `ECCLESIA_PUBLIC=1` with no `DATABASE_URL` refuses to boot rather than inventing a file. Verifies **AC-P1**

## Build plan

1. Add the placeholder helper and a driver enum inside the SDK store. Satisfies **AC-P4**
2. Add the indexes to the SQLite schema so local Home can use them. Satisfies **AC-P3**
3. Enable the sqlx `postgres` feature on `ecclesia-sdk` and branch `Db::connect` on URL scheme. Satisfies **AC-P1**, **AC-P2**
4. Document Neon PITR, the pooled URL, and "do not use transaction pooler with `query!`" in `docs/SECURITY.md` or a short ops note. Satisfies **AC-P6**
5. Keep MemoryWorld and SQLite as the test stores. Satisfies **AC-P5**

## Consequences

**Positive**:
- Local work stays offline.
- A deleted church row can be walked back on Neon.

**Negative / tradeoffs**:
- Two dialects. A query that uses a SQLite only function will fail on Neon. Prefer standard SQL.
- Neon compute idle and cold start are vendor behavior. Pair Fly in the same region.

**Neutral**:
- Crunchy Bridge remains the runner up if Neon pooling fights a later sqlx change.
- Do not take Aurora or Cloud SQL unless the team already lives on that cloud.

## Rationale

The engineer asked to stand the public store up now and to keep SQLite for `cargo run` and tests. Neon won on branching and restore (basis: [Neon branching](https://neon.com/docs/introduction/branching)). Crunchy Bridge is the runner up. `sqlx::Any` drops compile checks and is marked caution in the docs (basis: [sqlx any](https://docs.rs/sqlx/latest/sqlx/any/index.html)). A `?` to `$n` helper matches the strings already in `crates/sdk/src/db`.
