# Architecture

Ecclesia is honest about where rules live.

```
HTTP / Maud / cookies     ← App crate
        │
        ▼
   SDK stories            ← load, gate, commit
        │
        ▼
   Domain stories         ← pure functions
        │
        ▼
   Effect { writes, notices }
        │
        ▼
 Neon, SQLite, or MemoryWorld    ← SDK applies data
```

## Domain

The full contract is in [DOMAIN.md](DOMAIN.md).

A domain story is a function in `crates/domain`. It takes values — a viewer, a church, an
`EmailAvailability`, an id and a timestamp the SDK already minted — and returns
`Result<Effect, DomainError>`.

Domain does not:

- talk to SQLite
- read the clock
- generate UUIDs
- look at cookies

If a test can construct the inputs in memory, the church rule is testable.

Boolean function arguments are not used. A caller names the situation
(`CatalogPresence::Listed`, `PriorOffer::Fresh`) or calls a dedicated
function (`approve_membership`, `decline_membership`) instead of switching
on a flag. A wrong person is a specific error (`NotRecipient`, `NotInvitee`,
`NotSteward`), not `NotGovernor`.

Do not clone a collection to walk it again. Directory grouping, need
visibility, and card stacks take slices or iterators. Visibility reads a
`NeedSight` (borrowed fields) instead of allocating a `Need` from a card.
`MemoryWorld::apply` takes the `Effect` by value and moves writes in.
`Db::apply` takes `&Effect` and commits writes, notices, and outbox rows in one
transaction. Notice titles are `Arc<str>` so a governor fan-out clones the
pointer, not the words. Status fields on `Write` and notice kind/body stay
`&'static str`. `us_perf_01` fails the build if source grows
`.cloned().collect()`, `.to_vec()`, or `Need::from_card`. `us_domain_01`
fails the build if a domain file grows I/O.

| Module | Responsibility |
| --- | --- |
| `domain/auth.rs` | `register` with `Strength`, profile fields |
| `domain/membership.rs` | join, invite, redeem, approve/decline, plant |
| `domain/needs.rs` | post, apply, close, receive/pass an offer |
| `domain/gifts.rs` | endorse any skill, accept/decline, name a gift, profile |
| `domain/rules.rs` | visibility, neighbors, invite codes |
| `domain/validate.rs` | field limits |
| `domain/directory.rs` | church counts and place grouping |
| `domain/flags.rs` | named states that used to be booleans |
| `domain/notice.rs` | notice drafts, including governor fan-out |
| `domain/household.rs` | church and membership records |
| `domain/person.rs` | people, gifts, endorsements, `Viewer` |
| `domain/need.rs` | needs, applications, borrowed `NeedSight` |
| `domain/effect.rs` | `Write`, `Effect`, `DomainError` |
| `domain/model.rs` | re-exports the record modules |

## Crates

Workspace members are `ecclesia-domain`, `ecclesia-sdk`, and `ecclesia` (the App).
The App talks to the SDK only. The public store is Neon Postgres. Local work and
tests keep SQLite. Upstash holds catalog, church card, first directory page, and
rate keys. `ecclesia-worker` claims outbox rows and delivers Web Push and Resend mail.

| Skin | Files | External | Role |
| --- | --- | --- | --- |
| `clock` | `sdk/clock.rs` | OS time, UUID | ids and timestamps Domain receives |
| `session` | `sdk/session.rs` | HMAC `v2` cookies | cookie shape, CSRF |
| `identity` | `sdk/identity.rs`, `sdk/password.rs` | primary `sessions` | sign in, resolve row, mail tokens |
| `cache` | `sdk/cache.rs` | Upstash or process RAM | fragments and rate keys |
| `push` | `sdk/push.rs`, `sdk/web_push.rs` | Web Push, optional FCM | worker delivers notices |
| `outbox` | `sdk/outbox.rs` | Resend | worker sends mail |
| `judge` | `sdk/judge.rs` | word gate | weigh words; return `Posture` |
| `refine` | `sdk/refine.rs` | silent | rewrite stays off |
| `memory` | `sdk/memory.rs` | process RAM | apply effects in tests |
| `db` | `sdk/db/*.rs` | Neon or SQLite | persist effects |
| `http` | `app/http/*.rs` | Axum | load, call a story, render |
| `views` | `app/views/*.rs` | Maud | HTML; loops live in `cards` |

HTTP is not allowed to decide who may join, see a need, or accept an endorsement.
It may only refuse a bad CSRF token or a missing session, then call the SDK story.
Session + CSRF live in `signed_in` / `signed_form`, not a macro.

Offer messages are filtered by `visible_offers` before a page sees them. Only
the need author, a governor, and the person who wrote the offer read the text.

Loops that walk a collection live in a named function (`insert_gift_rows`,
`notice_each_governor`, `apply_writes`). Page bodies compose those functions;
they do not embed `for` in the middle of a story.

## Tests

- **Unit** — domain functions. Names start with the user-story id (`us_mem_01_…`).
- **SDK** — `MemoryWorld::apply` after a story (`crates/sdk/src/memory.rs`).
- **Integration** — Axum + SQLite (`crates/app/tests/flows.rs`), including CSRF.

See [SPEC.md](SPEC.md) for the story list, [PROSE.md](PROSE.md) for copy,
[VOICE.md](VOICE.md) for classification and rewrite, [MOBILE.md](MOBILE.md)
for iOS and Android, and [SECURITY.md](SECURITY.md) for the skin’s duties.
