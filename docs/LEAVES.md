# Leaves and honest functions

Ecclesia keeps church rules in one place: `src/leaf`. Everything else is skin.

Read this before writing Rust in this repo. `us_clean_01`, `us_perf_01`, and `us_leaf_01` in `src/style.rs` enforce the parts a compiler will not.

## A leaf

A leaf is a function in `src/leaf`. It takes values a test can build in memory — a `Viewer`, a `Church`, an `EmailAvailability`, an id and a timestamp the SDK already minted — and returns `Result<Effect, DomainError>`.

```
HTTP / Maud / cookies     ← SDK skin
        │
        ▼
   leaf stories           ← pure functions
        │
        ▼
   Effect { writes, notices }
        │
        ▼
 SQLite or MemoryWorld    ← skin applies data
```

Leaves do not:

- talk to SQLite (`sqlx::query`, `crate::db`)
- read the clock or mint UUIDs
- look at cookies, env, sockets, or HTTP
- import `crate::sdk`, `crate::http`, or `crate::views`

If a test can construct the inputs, the church rule is testable.

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
- Hide I/O inside a leaf (`now()`, `Uuid::new_v4()`, `sqlx`, `std::env`).
- Clone a `Vec` to iterate it (`to_vec`, `cloned().collect`, `Need::from_card`).
- Write a macro to hide repetition. Extract a function or an enum. Macros erase types and grep.
- Switch on a flag that changes what the function *means*.

Boolean *returns* are fine (`is_active`, `can_view_need`). Boolean *arguments* are not.

## Skin

`src/sdk`, `src/db`, `src/http`, and `src/views` map the outside world onto leaf values and apply the effect.

HTTP may refuse a bad CSRF token or a missing session. It may not decide who joins, who sees a need, or who accepts an endorsement. Load values, call a leaf, `commit` the effect, render.

Words are weighed in the skin (`JudgeHub`) and arrive as `Posture`. The leaf
calls `require_uplifting`. It does not talk to Jev or a local model. See
[VOICE.md](VOICE.md).

`Db::apply` takes `&Effect` and runs every write and notice on one SQLite transaction. `MemoryWorld::apply` takes the `Effect` by value and moves writes in. Push delivery happens after the transaction commits; a failed push does not roll back the write.

## Files

Keep leaf modules small and named after the story they serve. Shared records live in `household`, `person`, `need`, and `effect`. Named states that used to be booleans live in `flags`.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the module table and [SPEC.md](SPEC.md) for the story list.
