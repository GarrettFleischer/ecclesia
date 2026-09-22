# Domain and honest functions

Ecclesia keeps church rules in one place: `crates/domain`. Everything else is skin.

Read this before writing Rust in this repo. `us_clean_01`, `us_perf_01`, and `us_domain_01` in `crates/domain/src/style.rs` enforce the parts a compiler will not.

## A domain story

A domain story is a function in `crates/domain`. It takes values a test can build in memory, a `Viewer`, a `Church`, an `EmailAvailability`, an id and a timestamp the SDK already minted, and returns `Result<Effect, DomainError>`.

```
HTTP / Maud / cookies     ← App crate
        │
        ▼
   SDK stories            ← load, gate, call Domain, commit
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

Domain does not:

- talk to SQLite or Postgres (`sqlx::query`, `crate::db`)
- read the clock or mint UUIDs
- look at cookies, env, sockets, or HTTP
- import `ecclesia_sdk` or the App crate

If a test can construct the inputs, the church rule is testable. `crates/app/tests/domain_props.rs` (Domain property tests) throws random strings and states at every story. A story may return `Err`; it may not panic, and an `Ok` write must look like the person typed.

## Honest functions

A function is honest when its name, arguments, and return type say exactly what it does. Nothing useful happens off to the side.

Do:

- Return a typed `Effect` (writes + notice drafts). The caller applies it.
- Name the situation with an enum (`CatalogPresence::Listed`, `PriorOffer::Fresh`) or split the function (`approve_membership` / `decline_membership`).
- Return a specific `DomainError` (`NotRecipient`, `NotInvitee`, `NotSteward`). Do not reuse `NotGovernor` for a different person.
- Take slices, iterators, or a borrowed view (`NeedSight`) instead of cloning a collection to walk it again.
- Put a loop in its own named function. Page bodies compose those functions.

Do not:

- Pass `bool` as a function argument. The type system cannot name `true`.
- Hide I/O inside Domain (`now()`, `Uuid::new_v4()`, `sqlx`, `std::env`).
- Clone a `Vec` to iterate it (`to_vec`, `cloned().collect`, `Need::from_card`).
- Write a macro to hide repetition. Extract a function or an enum. Macros erase types and grep.
- Switch on a flag that changes what the function *means*.

Boolean *returns* are fine (`is_active`, `can_view_need`). Boolean *arguments* are not.

## Skin

`crates/sdk` maps the outside world onto Domain values and applies the effect. `crates/app` paints HTML and runs the Fly binaries.

HTTP may refuse a bad CSRF token or a missing session. It may not decide who joins, who sees a need, or who accepts an endorsement. Load values, call one SDK story, render.

Words are weighed in the SDK (`JudgeHub`) and arrive as `Posture`. The first submit is `VoicePass::Review`; the second is `Publish`. Domain calls `require_uplifting`. It does not talk to Jev or OpenRouter. See [VOICE.md](VOICE.md).

`Db::apply` takes `&Effect` and runs every write, notice, and outbox row in one transaction. `MemoryWorld::apply` takes the `Effect` by value and moves writes in. The request does not talk to phones. `ecclesia-worker` claims outbox rows. A failed push does not roll back the write.

## Files

Keep domain modules small and named after the story they serve. Shared records live in `household`, `person`, `need`, and `effect`. Named states that used to be booleans live in `flags`.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the crate table and [SPEC.md](SPEC.md) for the story list.

Random domain inputs live in `crates/app/tests/domain_props.rs`. Random clicks through the HTTP skin live in `crates/app/tests/chaos.rs`.
