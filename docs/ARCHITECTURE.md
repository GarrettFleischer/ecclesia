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

A leaf is a function in `src/leaf`. It takes values — a viewer, a church, a flag that an email is taken, an id and a timestamp the SDK already minted — and returns `Result<Effect, DomainError>`.

Leaves do not:

- talk to SQLite
- read the clock
- generate UUIDs
- look at cookies

If a test can construct the inputs in memory, the household rule is testable.

Stories live in `src/leaf/stories.rs`. Visibility math lives in `src/leaf/rules.rs`. Field limits live in `src/leaf/validate.rs`.

## SDK skin

`src/sdk` maps the outside world onto those values and applies the effect.

| Skin | External | Role |
| --- | --- | --- |
| `clock` | OS time, UUID | ids and timestamps leaves receive |
| `session` | HMAC cookies | who is seated, CSRF |
| `memory` | process RAM | apply effects in tests |
| `db` | SQLite | apply effects for the running app |
| `http` | Axum | load values, call a leaf, apply, render |

HTTP is not allowed to decide who may join, see a need, or wear an endorsement. It may only refuse a bad CSRF token or a missing session, then call the leaf.

## Tests

- **Unit** — leaf functions. Names start with the user-story id (`us_mem_01_…`).
- **SDK** — `MemoryWorld::apply` after a leaf (`src/sdk/memory.rs`).
- **Integration** — Axum + SQLite (`tests/flows.rs`), including CSRF.

See [SPEC.md](SPEC.md) for the story list and [SECURITY.md](SECURITY.md) for the skin’s duties.
