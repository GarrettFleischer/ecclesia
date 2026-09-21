# Architecture

Ecclesia is honest about where rules live.

```
HTTP / Maud / cookies     ← SDK skin (externals)
        │
        ▼
   leaf stories           ← pure functions
        │
        ▼
   Effect { writes, notices }
        │
        ▼
 SQLite or MemoryWorld    ← SDK applies data
```

## Leaves

A leaf is a function in `src/leaf`. It takes values — a viewer, a church, an
`EmailAvailability`, an id and a timestamp the SDK already minted — and returns
`Result<Effect, DomainError>`.

Leaves do not:

- talk to SQLite
- read the clock
- generate UUIDs
- look at cookies

If a test can construct the inputs in memory, the church rule is testable.

Boolean function arguments are not used. A caller names the situation
(`CatalogPresence::Listed`, `PriorOffer::Fresh`) or calls a dedicated
function (`approve_membership`, `decline_membership`) instead of switching
on a flag.

Do not clone a collection to walk it again. Directory grouping, need
visibility, and card stacks take slices or iterators. Visibility reads a
`NeedSight` (borrowed fields) instead of allocating a `Need` from a card.
`MemoryWorld::apply` takes the `Effect` by value and moves writes in.
Status fields on `Write` and notice kind/body stay `&'static str`.
`us_perf_01` fails the build if source grows `.cloned().collect()`,
`.to_vec()`, or `Need::from_card`.

| Module | Responsibility |
| --- | --- |
| `leaf/auth.rs` | register, demo-seat impersonation |
| `leaf/membership.rs` | join, invite, redeem, approve/decline, plant |
| `leaf/needs.rs` | post, apply, close, receive/pass an offer |
| `leaf/gifts.rs` | endorse any skill, publish/decline, name a gift, profile |
| `leaf/rules.rs` | visibility, neighbors, invite codes |
| `leaf/validate.rs` | field limits |
| `leaf/directory.rs` | church counts and place grouping |
| `leaf/flags.rs` | named states that used to be booleans |
| `leaf/notice.rs` | notice drafts, including governor fan-out |
| `leaf/household.rs` | church and membership records |
| `leaf/person.rs` | people, gifts, endorsements, `Viewer` |
| `leaf/need.rs` | needs, applications, borrowed `NeedSight` |
| `leaf/effect.rs` | `Write`, `Effect`, `DomainError` |
| `leaf/model.rs` | re-exports the record modules |

## SDK skin

`src/sdk` maps the outside world onto those values and applies the effect.

| Skin | Files | External | Role |
| --- | --- | --- | --- |
| `clock` | `sdk/clock.rs` | OS time, UUID | ids and timestamps leaves receive |
| `session` | `sdk/session.rs` | HMAC cookies | who is signed in, CSRF |
| `push` | `sdk/push.rs`, `sdk/web_push.rs` | Web Push, optional FCM | deliver notices to an installed phone |
| `memory` | `sdk/memory.rs` | process RAM | apply effects in tests |
| `db` | `db/{schema,seed,users,churches,needs,gifts,notices,push,apply}.rs` | SQLite | persist effects |
| `http` | `http/{auth,churches,needs,people,push,context,forms}.rs` | Axum | load, call a leaf, apply, render |
| `views` | `views/{layout,flash,cards,landing,home,churches,needs,people}.rs` | Maud | HTML; loops live in `cards` |

HTTP is not allowed to decide who may join, see a need, or publish an endorsement.
It may only refuse a bad CSRF token or a missing session, then call the leaf.

Loops that walk a collection live in a named function (`insert_gift_rows`,
`notice_each_governor`, `apply_writes`, `persona_forms`). Page bodies compose
those functions; they do not embed `for` in the middle of a story.

## Tests

- **Unit** — leaf functions. Names start with the user-story id (`us_mem_01_…`).
- **SDK** — `MemoryWorld::apply` after a leaf (`src/sdk/memory.rs`).
- **Integration** — Axum + SQLite (`tests/flows.rs`), including CSRF.

See [SPEC.md](SPEC.md) for the story list, [PROSE.md](PROSE.md) for copy,
[MOBILE.md](MOBILE.md) for iOS and Android, and [SECURITY.md](SECURITY.md) for
the skin’s duties.
