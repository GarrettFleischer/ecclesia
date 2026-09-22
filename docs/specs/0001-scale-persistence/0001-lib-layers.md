# 0001. Library layers

## Summary

Shipped the migration from one crate to three. Domain is `crates/domain`. The SDK is the only crate that calls Domain and the machine. The App crate holds Axum, Maud, and the Fly binaries. HTTP extracts a form, calls one SDK story, and paints HTML.

## Requirements

**User stories**:
- As a builder, I want a crate line Cargo can enforce so a new route cannot import SQL.
- As a builder, I want one SDK function per user story so HTTP does not load rows, call Domain, and apply an Effect by hand.

**Acceptance criteria**:
- **AC-L1**: Workspace members are `crates/domain` (`ecclesia-domain`), `crates/sdk` (`ecclesia-sdk`), and `crates/app` (`ecclesia`). Root `Cargo.toml` is the workspace.
- **AC-L2**: App `Cargo.toml` depends on `ecclesia-sdk` only. It does not list `ecclesia-domain` or `sqlx`.
- **AC-L3**: Domain `Cargo.toml` lists no `sqlx`, `reqwest`, `axum`, `cookie`, or `ecclesia-sdk`. `us_domain_01` fails the build if Domain source grows I/O or an SDK/App import.
- **AC-L4**: `ecclesia_sdk::prelude` re-exports the Domain types App paints (`User`, `Church`, `Viewer`, `NeedCard`, `DomainError`, `Posture`, `VoiceKind`, visibility helpers). App modules import that prelude, not `ecclesia_domain`.
- **AC-L5**: Each story in [docs/SPEC.md](../../SPEC.md) that changes the world has one SDK function with the same name as the Domain entry point. HTTP does not call store apply. Each write story returns `StoryOk` (`user_id`, `church_id`, `need_id`, `membership_id` as `Option<String>`) for the redirect the App already does. App does not receive `Effect`.
- **AC-L7**: Static files live in `crates/app/static`. `ServeDir` uses that crate's `CARGO_MANIFEST_DIR`.
- **AC-L8**: `us_domain_01` lives in `ecclesia-domain` and walks that crate's `src/`. Prose style tests live in the App crate.
- **AC-L9**: `JudgeHub::load` is the word gate. It ignores `ECCLESIA_JEV_KEY` and OpenRouter keys until a later spec. `RefineHub::load` is silent.
- **AC-L6**: Domain rules live in `docs/DOMAIN.md`. `.cursor/rules/leaves.mdc` points at Domain. `us_domain_01` guards the crate.

## Decision

Three workspace crates, not three modules. App talks to the SDK only.

**SDK write stories** (load, word gate when the story posts words, call Domain, commit):
`register`, `update_profile`, `request_join`, `invite_member`, `redeem_invite`, `approve_membership`, `decline_membership`, `accept_invite`, `plant_church`, `post_need`, `apply_to_need`, `close_need`, `accept_application`, `decline_application`, `add_gift`, `remove_gift`, `endorse`, `accept_endorsement`, `decline_endorsement`, plus push subscribe, unsubscribe, and device register. Sign in, sessions, and mail are in [0002](../0002-identity-sessions/index.md).

**SDK read stories**:
`viewer`, `home_needs` (page of 20), `church_directory` (page of 20), `church_show`, `need_show`, `inbox`, `member_show`, `me`, `gift_catalog`, `landing`.

Ids and timestamps the Domain story needs come from the SDK clock, never from Domain.

## Feature design

**Data model sketch**:
No new tables. Records stay the Domain structs already in `household`, `person`, `need`, and `effect`.

**API surface**:

| Story | Inputs App passes | SDK does | App receives |
|---|---|---|---|
| Write story | Form fields, session user id, CSRF already checked in App | Gate words if the story posts text. Load rows. Call Domain. Commit Effect. | `Result<StoryOk, DomainError>` |
| Read story | Session user id, optional `after` | Scoped query. Re-export Domain filter. | Page of cards plus `next_cursor` |
| `landing` | None | Nothing cached | App paints Maud with session CSRF |

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| Any write | New row id | SDK clock `new_id` |
| Any write | `created_at` | SDK clock `now_iso` |
| `register` | `EmailAvailability` | SDK store `user_by_email` |
| `post_need` | `CatalogPresence` | SDK store `gift` lookup |
| `home_needs` | Page of `NeedCard` | SDK query of open needs for the viewer's churches and neighbors, then Domain `visible_need_cards` |
| `home_needs` | `next_cursor` | Last row `(created_at, id)` or none |
| Redirect flash | `DomainError` code | Domain `flash_code`, in [crates/domain/src/effect.rs](../../../crates/domain/src/effect.rs) |
| Redirect after post | `church_id` / `need_id` / `user_id` | `StoryOk` optional fields from the Domain Effect ids the SDK already reads (`inserted_church_id`, and so on) |
| Static CSS / JS / PWA | File bytes | `crates/app/static` via `CARGO_MANIFEST_DIR` |

**Key invariants**:
- Domain never imports SDK or App.
- App never imports Domain or `sqlx`.
- HTTP may refuse a bad CSRF token or a missing session. It may not decide membership, visibility, or endorsements.

**Security model**:
[docs/SECURITY.md](../../SECURITY.md). The SDK session module signs `v2` cookies (session rows: [0002](../0002-identity-sessions/index.md)). The App still runs `signed_in` / `signed_form` before a write story.

**Configuration required**:
- Workspace only. No new env vars in this child.

**Critical test scenarios**:
- Happy path: `cargo test -p ecclesia-domain` passes Domain unit tests. Verifies **AC-L3**
- Happy path: an HTTP flow test still posts a need through the SDK story. Verifies **AC-L5**
- Failure: adding `use ecclesia_domain` in App does not compile (or a style test fails the workspace). Verifies **AC-L2**
- Failure: a Domain file that calls `std::env` fails `us_domain_01`. Verifies **AC-L3**

## Build plan

Assume end to end slices (no approach on record).

1. Create the workspace and move leaf code to `crates/domain` as `ecclesia-domain`. Rename modules and `us_domain_01`. Satisfies **AC-L1**, **AC-L3**, **AC-L6**
2. Move SDK and store code into `crates/sdk`. Add `prelude`. Push talks to the store module inside the SDK. Satisfies **AC-L4**
3. Move HTTP, views, `main`, and `static/` into `crates/app`. Add `src/bin/worker.rs` for the outbox child. Satisfies **AC-L2**, **AC-L7**
4. Wrap each SPEC write in one SDK story that returns `StoryOk`. Switch HTTP to call it. Force word gate and silent refine in `load`. Satisfies **AC-L5**, **AC-L9**
5. Put `us_domain_01` in the Domain crate. Rewrite [docs/ARCHITECTURE.md](../../ARCHITECTURE.md) and the Cursor Domain rule. Satisfies **AC-L6**, **AC-L8**

## Consequences

**Positive**:
- A new route cannot reach SQLite without changing `Cargo.toml`.

**Negative / tradeoffs**:
- Three packages slow a cold `cargo test` a bit.
- Views must take types from the prelude. A missed re-export is a compile error.

**Neutral**:
- Integration tests stay in the App package (`tests/flows.rs`).
- Domain property tests stay in the Domain package.

## Rationale

Modules in one crate were the runner up. They do not stop HTTP from importing the store. Three crates are the smallest line Cargo will enforce. (basis: layered monolith crate boundaries)
