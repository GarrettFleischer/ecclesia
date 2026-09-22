# 0001. Scale persistence foundation (rationale)

## Context

> Premise note: The engineer asked for hosted Postgres, a cache, a queue, and Fly before any page has been timed. The known failure mode is extra vendors with no measured stall. The load bearing work is the crate line and scoped reads. Neon, Upstash, and Fly are in this spec because the engineer asked for that skin now, not because Home is proven slow. Identity (passwords, session rows, mail) shipped in [0002](../0002-identity-sessions/index.md).

**Before this spec (2026-09-21)** the repo was one crate. Church rules lived under `src/leaf`. Home loaded every open need and every church, then filtered in Domain. The rate gate was in-process. SDK code imported the store directly. A clock shim mixed Domain and machine time.

A million members on a Sunday morning is a real target the engineer set. Neighbor needs join on city and region. Email is unique across the body. Those two facts fight a shard by church. The first wall was `SELECT` all open needs, not the SQLite file itself.

Not deciding would have left HTTP free to import SQL, Home free to grow with every need in the body, and a second Fly machine with a private rate gate.

## Options considered

### Option 1: One crate, rename modules

Keep one package. Rename `leaf` to `domain` and move `db` under `sdk`. Fastest edit.

**Pros**:
- Smallest diff. Tests keep working.

**Cons**:
- `use crate::db` from HTTP still compiles. The hell returns on the next feature. (basis: crate boundaries for layered monoliths)

### Option 2: Layered workspace, one relational primary (chosen)

Three crates. App to SDK to Domain. Neon in public, SQLite locally. Scoped pages. Upstash fragments. Postgres outbox.

**Pros**:
- Cargo enforces the line. One primary matches neighbor joins and a global email. A million members fit if Home stops fetching the world. (basis: [docs/DOMAIN.md](../../DOMAIN.md), relational default)

**Cons**:
- Two SQL dialects. Three `Cargo.toml` files. Vendor bills start now.

### Option 3: Shard by church from day one

A database or schema per church, or a shard key on `church_id`.

**Pros**:
- Matches a later world with many huge churches.

**Cons**:
- Neighbor needs and invite by email need a second directory on day one. The product is not that large. (basis: church neighbor rule in [crates/domain/src/rules.rs](../../../crates/domain/src/rules.rs))

### Option 4: Topcoat plus Toasty

Rewrite HTML onto Topcoat. Let components fetch. Take Toasty as the ORM.

**Pros**:
- Newer reactivity. Tokio org backing. (basis: [Topcoat announcement](https://tokio.rs/blog/2026-07-22-announcing-topcoat))

**Cons**:
- Components that query the store fight Domain. Jobs, auth, and rate limits are still on the Topcoat roadmap. You would still build Neon, cache, and the worker yourself. (basis: [docs.rs topcoat roadmap](https://docs.rs/topcoat/latest/topcoat/))

## Rationale

The engineer chose the crate line so data flows through libraries and HTTP cannot reach SQL. Option 1 cannot promise that. Option 3 makes neighbor visibility a distributed query before any church is large. Option 4 throws away Maud and the Domain contract for a framework that does not yet ship the jobs the engineer wanted.

Option 2 keeps the rules where they are and moves the skin behind an SDK the App must use. Neon is the public primary because the engineer asked to stand that host up now, and because branching and point in time restore fit a small team (basis: [Neon branching](https://neon.com/docs/introduction/branching), [Neon pooling](https://neon.com/docs/connect/connection-pooling)). Crunchy Bridge was the runner up: ordinary managed Postgres, no copy on write branches for CI. sqlx stays on runtime SQL because this repo already writes `?` strings; a tiny rewrite to `$n` beats `sqlx::Any` and beats two compile time `query!` worlds (basis: [sqlx any](https://docs.rs/sqlx/latest/sqlx/any/index.html)).

Upstash holds only shared fragments, not landing HTML or one person's Home. Caching one person's Home would leak a neighbor need or go stale when any nearby church posts. The in process rate gate is already wrong the moment Fly runs two machines, so rate keys belong in that same cache.

The outbox lives in Postgres so commit and "please push" are one transaction. A second `ecclesia-worker` binary keeps the request off the phone network. Word weighing stays on the request. The engineer asked to drop LLM judge and rewrite from this spec and to keep a blacklist (the word gate already in `JudgeHub`) before any write.

## Landscape notes

Checked once during design. Links are for humans. Do not fetch them again.

**Managed Postgres**: Neon (branching, pooler, instant restore) and Crunchy Bridge (pooling and backups in the base price) led. Fly unmanaged Postgres is a cluster you operate. Transaction poolers fight sqlx prepared statements. This repo uses runtime SQL, so a pooled Neon URL is the fit.

**Cache**: Valkey is the Redis compatible open fork. Upstash hosts that protocol with TLS.

**Jobs**: `FOR UPDATE SKIP LOCKED` on a Postgres table is enough until that table is hot. apalis 0.7 docs lean Redis. Do not add NATS.

**sqlx**: docs.rs current crate at check time was 0.9. This repo is on 0.8.2. Stay on 0.8 until a migrate forces a bump. Dual `query!` across SQLite and Postgres is still painful.

## References

**Project sources**:
- [docs/DOMAIN.md](../../DOMAIN.md) and [docs/ARCHITECTURE.md](../../ARCHITECTURE.md): Domain does no I/O; HTTP must not decide visibility.
- [docs/SECURITY.md](../../SECURITY.md): sessions, CSRF, rate keys in Upstash.
- [docs/VOICE.md](../../VOICE.md) and [crates/sdk/src/judge.rs](../../../crates/sdk/src/judge.rs): word gate when no judge key is set.
- [crates/sdk/src/db/needs.rs](../../../crates/sdk/src/db/needs.rs) scoped Home queries and [crates/app/src/http/auth.rs](../../../crates/app/src/http/auth.rs) `member_home`.
- [crates/sdk/src/push.rs](../../../crates/sdk/src/push.rs): push delivery from the worker.
- [docs/SPEC.md](../../SPEC.md): user stories the SDK story list must cover.

**Practices & standards**:
- Layered monolith with crate boundaries, not modules.
- Relational primary first. Shard only when one primary is hot.
- Database backed queue first.
- Cache is ephemeral. Never the source of truth.
- Word gate or classifier on the request, before commit.

**Links**:
- Neon pooling: https://neon.com/docs/connect/connection-pooling
- Neon branching: https://neon.com/docs/introduction/branching
- Crunchy Bridge: https://www.crunchydata.com/products/crunchy-bridge
- Fly Postgres: https://fly.io/docs/postgres/
- Valkey: https://valkey.io/about/
- Upstash Redis: https://upstash.com/docs/redis/overall/getstarted
- sqlx: https://docs.rs/sqlx/latest/sqlx/
- sqlx any: https://docs.rs/sqlx/latest/sqlx/any/index.html
- apalis: https://docs.rs/apalis/latest/apalis/
- Topcoat announcement: https://tokio.rs/blog/2026-07-22-announcing-topcoat
- Topcoat docs and roadmap: https://docs.rs/topcoat/latest/topcoat/
