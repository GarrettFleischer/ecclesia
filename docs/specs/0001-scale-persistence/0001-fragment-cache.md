# 0001. Fragment cache

## Summary

Upstash holds a few shared JSON fragments and rate limit counts. It never holds landing HTML, one person's Home, or inbox. On commit the SDK deletes the keys that write touched. A cache miss is a store read.

## Requirements

**User stories**:
- As a visitor, I want the public landing to paint with a fresh CSRF token every time.
- As an operator, I want sign in and register limits to be shared across machines.

**Acceptance criteria**:
- **AC-C1**: Cache keys are `catalog`, `church:{id}`, `directory`, and `rate:{kind}:{who}`. No `landing`, `home:{user}`, or `inbox:{user}` key exists in code.
- **AC-C2**: `gift_catalog`, a church card by id, and the first directory page (20 churches, no cursor) may be read from Upstash as JSON of Domain structs (`serde_json`). Home, inbox, and landing always render in the App. Landing always gets a fresh CSRF.
- **AC-C3**: After a successful commit, the SDK deletes the keys in the map below for each `Write` in the Effect. It does not wait on TTL for those keys. Every fragment also has a 1 hour TTL as a safety net.
- **AC-C4**: Rate kinds from [crates/sdk/src/limit.rs](../../../crates/sdk/src/limit.rs) (`session`, `register`, `refine`, `redeem`, `push`) use Redis `INCR` and `EXPIRE 60` on first increment. Two App processes share the count. This is a fixed 60s window from first hit, not an in-process `Instant` bucket.
- **AC-C5**: If Upstash is unset and this is not a public host, the SDK uses an in process fallback. A public host (`FLY_APP_NAME` or `ECCLESIA_PUBLIC=1`) refuses to boot without `UPSTASH_REDIS_URL`.
- **AC-C6**: A cache error on a fragment read is a miss, then a store read, then a log. It is not a 500.

## Decision

Upstash Redis protocol. Explicit key delete on commit. In process fallback only for local work.

**Invalidation map** (`Write` → keys to delete):

| Write | Keys |
|---|---|
| `InsertUser` | none (no public fragment) |
| `UpdateUser` | none |
| `InsertChurch` | `directory`, `church:{id}` |
| `InsertMembership` | `church:{church_id}`, `directory` |
| `SetMembershipStatus` | `church:{church_id}`, `directory` |
| `InsertNeed` | `church:{church_id}`, `directory` |
| `SetNeedStatus` | `church:{church_id}`, `directory` |
| `InsertApplication` | none |
| `SetApplicationStatus` | none |
| `InsertEndorsement` | none |
| `SetEndorsementStatus` | none |
| `UpsertMemberGift` | none (catalog is the gift list, not member gifts) |
| `RemoveMemberGift` | none |

`SetMembershipStatus` and `SetNeedStatus` carry only `{id, status}` in Domain. SDK commit, in the same transaction, reads `church_id` from `memberships` or `needs` by that id before it deletes keys. Do not add fields to Domain `Write` in this spec.

`catalog` deletes when a later story mutates the `gifts` table. Seed stays a boot path, not a `Write`.

Landing is not cached. The essay already lives in App source. App paints it with a new CSRF each request.

## Feature design

**Data model sketch**:
No Postgres tables. Upstash values:
- `catalog`: JSON array of `Gift`
- `church:{id}`: JSON church card (id, name, city, region, member count, need count)
- `directory`: JSON array of at most 20 `Church` plus counts, first page only (`ORDER BY city, name, id`). Cursor pages always hit the store.
- `rate:{kind}:{who}`: integer via `INCR`, `EXPIRE 60` on first increment

**State transitions**: none.

**API surface**:
SDK `cache.get`, `cache.set`, `cache.del`. Stories call them. App does not.

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| Landing HTML | Markup | App Maud each request. Fresh CSRF from the session. Not cached. |
| Church card | Name, city, region, counts | Upstash JSON `church:{id}` or store `church` plus counts |
| Directory page 1 | 20 churches plus counts | Upstash JSON `directory` or store |
| Catalog | Gift list | Upstash JSON `catalog` or store `gifts` |
| Rate decision | Allow or refuse | `INCR` vs the caps in `RateKind::max_hits` |
| Who | Rate key suffix | Client key from the request (IP or session), see [crates/app/src/http/context.rs](../../../crates/app/src/http/context.rs) |

**Key invariants**:
- Cache is ephemeral. The store is the truth.
- Personalized HTML never enters Upstash.

**Security model**:
Upstash holds public church names and the landing essay. Do not put session tokens or offer messages in the cache. The Upstash URL is a secret. TLS on.

**Configuration required**:
- `UPSTASH_REDIS_URL`: Redis URL with TLS. Required when `FLY_APP_NAME` is set or `ECCLESIA_PUBLIC=1`. Optional locally.
- `ECCLESIA_PUBLIC`: `1` marks a public host when not on Fly.

**Critical test scenarios**:
- Happy path: two `register` hits from the same who increment one shared count in a fake Redis. Verifies **AC-C4**
- Happy path: `InsertChurch` deletes `directory` and `church:{id}`. Verifies **AC-C3**
- Failure: Upstash down during `gift_catalog` still returns the store list. Verifies **AC-C6**
- Auth: no code path writes `landing`, Home HTML, or inbox HTML to a key. A style or unit test greps for forbidden key prefixes. Verifies **AC-C1**, **AC-C2**

## Build plan

1. Add an SDK cache trait with Upstash and in process backends. Satisfies **AC-C5**, **AC-C6**
2. Move `RateGate` to the cache. Satisfies **AC-C4**
3. Cache catalog, church card, first directory page as JSON. Do not cache landing. Satisfies **AC-C2**
4. Delete keys from the map inside commit. Satisfies **AC-C3**
5. Forbid Home and inbox keys with a style test. Satisfies **AC-C1**

## Consequences

**Positive**:
- Two machines share limits.
- The landing does not hit Postgres on every anonymous visit.

**Negative / tradeoffs**:
- A missed key in the map means a stale church card until TTL. Add the key beside the `Write`, not later.
- Upstash is another vendor.

**Neutral**:
- Valkey on the same private network was the runner up. Pick that if Upstash cost or region pairing hurts.

## Rationale

The engineer asked for performant HTML caching and then agreed Home is per person. Landing HTML embeds CSRF, so it cannot live in a shared key. Shared JSON fragments plus rate keys are the set that cannot leak a neighbor need or a CSRF token. (basis: [crates/app/src/views/landing.rs](../../../crates/app/src/views/landing.rs), Domain `can_view_need` in [crates/domain/src/rules.rs](../../../crates/domain/src/rules.rs))
