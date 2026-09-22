# Domain

## Overview

Church rules live here and nowhere else. A story takes values a test can build and returns `Result<Effect, DomainError>`. The SDK applies the effect.

## Key files

| File | Owns |
|---|---|
| `src/lib.rs` | Public stories |
| `src/effect.rs` | `Effect`, `DomainError`, writes |
| `src/auth.rs` | Register and password strength |
| `src/style.rs` | `us_domain_01`, `us_clean_01`, `us_perf_01` |

## Conventions

- No SQLite, clock, UUID, cookies, env, sockets, or `ecclesia_sdk`.
- Names, arguments, and the return type say what happens.
- Use an enum or two functions. Never a `bool` argument.
- Return a specific `DomainError`. Do not reuse one variant for a different case.
- Read `docs/DOMAIN.md` before changing Rust here.

## Related specs

- [0001](../../docs/specs/0001-scale-persistence/index.md)
- [0002](../../docs/specs/0002-identity-sessions/index.md)

_Drafted by /jsm-audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
