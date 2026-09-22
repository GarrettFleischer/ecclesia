# 0002. Passwords

## Summary

Register asks for a password. Sign in is email and password. The SDK scores with zxcvbn and stores argon2id. Domain only sees a `Strength`. Sit-as demo and seed people are removed. Tests register, then plant and post.

## Requirements

**User stories**:
- As a guest, I want to set a password when I register so I can come back.
- As a member, I want to change that password on my page.

**Acceptance criteria**:
- **AC-P1**: `register` takes `Strength`. `TooGuessable` is `DomainError::WeakPassword` (`?err=password`, `Pick a stronger password.`). `Acceptable` continues today's person field checks. `InsertUser` still has no hash field.
- **AC-P2**: The SDK story refuses length over 128 as `TooGuessable`. Otherwise it runs zxcvbn on the raw password with name and email as user inputs. Score 0 or 1 is `TooGuessable`. Score 2, 3, or 4 is `Acceptable`. Then `Argon2::default()` hashes. The raw secret is zeroized. `apply(effect, StoryExtras { password_hash, session, .. })` writes hash and session in the same transaction as `InsertUser`.
- **AC-P3**: Landing register has a password field. `POST /register` sends it. `POST /session` takes `email` and `password`, not `user_id`. Email is `normalize_email` first. Wrong password, unknown email, and null hash are `?err=miss`.
- **AC-P4**: `POST /session/password` needs the current password and a new one that passes zxcvbn. Success writes a new hash, deletes every session for that person, mints a new row, and rewrites the cookie.
- **AC-P5**: `may_impersonate`, `DemoSeat`, the persona grid, and `ECCLESIA_DEMO` are removed from the product.
- **AC-P6**: `seed_if_empty` inserts the gift catalog only. No users, churches, memberships, or needs. Flow and chaos tests call `register` with a password, then the church and need stories.

## Decision

Score in the SDK, hash in the SDK, `Strength` in Domain. No password string in `crates/domain`.

## Feature design

**Data model sketch**:
`users.password_hash` nullable text. PHC string from argon2. Null or empty is a miss at sign in.

**State transitions**:
No hash → must use magic or reset. Hash set on register, reset, or change.

**API surface**:

| Story | Inputs App passes | SDK does | App receives |
|---|---|---|---|
| `register` | name, email, city, region, bio, password | Score, hash, Domain register, apply user plus hash, mint session | `StoryOk.user_id` |
| `sign_in` | email, password | Lookup by email, verify hash, mint session | `StoryOk.user_id` or quiet miss |
| `change_password` | user id, current, new | Verify current, score new, hash, apply, delete other sessions, mint | `StoryOk` or quiet miss / weak |

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| Score | 0 to 4 | zxcvbn crate `.score()` |
| Hash | PHC text | `Argon2::default().hash_password` |
| Sign in miss | same flash | SDK maps verify fail, missing user, and missing hash to one result |
| New `DomainError` | `WeakPassword` | flash code `password` |

**Key invariants**:
- Password is never word gated.
- Bio still goes through the word gate and `require_uplifting`.
- Email taken on register still uses `EmailTaken` (they typed that email on purpose).

**Security model**:
Guest register and sign in. Change password is the signed in person only.

**Configuration required**:
None beyond the umbrella.

**Critical test scenarios**:
- Happy path: register score 2, sign in on a fresh cookie. Verifies **AC-P2**, **AC-P3**
- Failure: password that scores 1 never writes a user. Verifies **AC-P1**
- Failure: sit as `user_id` is gone; that form field is not read. Verifies **AC-P5**
- Auth: change password with a wrong current is `Try again.` and the hash stays. Verifies **AC-P4**

## Build plan

1. Add `Strength` and `WeakPassword`. Change `register`. Satisfies **AC-P1**
2. SDK score, hash, apply column. Landing field. Satisfies **AC-P2**, **AC-P3**
3. `sign_in` and `change_password` stories. Satisfies **AC-P3**, **AC-P4**
4. Delete demo path and shrink seed. Rewrite tests. Satisfies **AC-P5**, **AC-P6**

## Consequences

**Positive**:
- A public host has a real door back in.

**Negative / tradeoffs**:
- Score 2 will accept some common phrases.
- Tests lose the seeded body. Each run pays register plus plant.

**Neutral**:
- `us_auth_02_*` tests cover shared `v2` sessions, not sit-as demo.

## Rationale

zxcvbn in the SDK matches the word gate: HTTP does not decide, Domain does not take a crate. `Argon2::default()` is what `rust-auth` shows and what docs.rs documents as Argon2id v19.
