# App

## Overview

Axum routes and Maud pages. This crate loads values, calls one SDK story, and paints HTML. It does not decide membership, visibility, or endorsements.

## Key files

| File | Owns |
|---|---|
| `src/http/mod.rs` | Router |
| `src/http/context.rs` | Session bind, CSRF, client address |
| `src/http/auth.rs` | Register, sign in, mail links, devices |
| `src/views/` | Maud pages |
| `src/bin/worker.rs` | Outbox worker, no HTTP |
| `static/` | CSS, service worker, icons |

## Commands

```bash
cargo dev
cargo run -p ecclesia
cargo test -p ecclesia --test flows
cargo test -p ecclesia --test chaos
```

`cargo dev` restarts on Rust changes. Static files reload the browser on their own.

## Conventions

- Depend on `ecclesia-sdk` only. Use `ecclesia_sdk::prelude` for domain types.
- `signed_in` and `signed_form` gate member pages. Domain still refuses `TearsDown`.
- Session `ip` is `Fly-Client-IP` or the peer socket.
- Flashes are allow listed codes in `src/views/flash.rs`.
- Guest account UI: `/` is the public story plus CTAs, and the only page with the install bar. Account pages (`/register`, `/session/new`, `/session/link/new`, `/session/reset/new`) use `Nav::Account`: one h1, a narrow card, one form. Unsigned `/home` is CTAs, not the full landing.
- Read `docs/PROSE.md` before changing any string a person reads.

## Agent skills

- [axum](../../.agents/skills/axum/): `melonask/axum-skills`, routing, state, and middleware
- [maud](../../.agents/skills/maud/): `uwuclxdy/agenticat`, the `html!` macro

## Related specs

- [0001](../../docs/specs/0001-scale-persistence/index.md)
- [0002](../../docs/specs/0002-identity-sessions/index.md)

_Drafted by /jsm-audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
