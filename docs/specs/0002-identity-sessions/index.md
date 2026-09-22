# 0002. Identity and sessions

**Date**: 2026-09-22
**Status**: Accepted

## Summary

A person registers with an email and a password, then signs in on any App process. The cookie points at a session row both machines can load. Logout, idle time, and a password change kill that row, so a stolen cookie is dead. Magic link, password reset, and church invite mail go through the outbox and Resend. Sit-as demo and seed people are removed. Tests register, then plant a church and post needs.

## Structure

- [0002-passwords.md](0002-passwords.md): register, sign in, change password, zxcvbn, argon2id, gift-only seed. Supports a public host without sit-as demo.
- [0002-sessions.md](0002-sessions.md): session table, `v2` cookie, idle 30 days, Me device list, revoke one, sign out everywhere. Supports two Fly machines that agree who is in.
- [0002-mail.md](0002-mail.md): magic links, password resets, invite mail, Resend on the worker. Supports coming back without a password on the request.

## Shared contract

These rules bind every child. A child must not weaken them.

1. Cargo edges stay App to SDK to Domain. Domain never sees a raw password, a hash string, a cookie, or Resend. The SDK scores the password (zxcvbn), hashes it (argon2id), and loads session rows.
2. A write path is word gate when the story posts words, then one SDK story, then `apply(effect, StoryExtras)` in one transaction. `StoryExtras` may carry `password_hash`, a new `sessions` row, a magic or reset row, and a `mail` outbox payload. None of those belong on Domain `Effect`. The App never calls store apply.
3. The cookie `ecclesia_sid` is `v2.{session_id}.{csrf}.{hmac}` signed with `ECCLESIA_SECRET`. Guest has an empty `session_id` and no row. CSRF always lives in the cookie. A signed in row stores a copy of that csrf. A valid HMAC with a missing or idle row is a guest with a new csrf.
4. Email is the only sign in key. Every lookup runs Domain `normalize_email` before `user_by_email`. Unknown email, wrong password, missing hash, bad token, and mail rate share one quiet flash (`?err=miss` is `Try again.`; `?err=mail` is `Check your email.`).
5. At most one unconsumed magic link and one unconsumed password reset per person. A new request sets `consumed_at` on the unused live row, then inserts. A unique index on `user_id WHERE consumed_at IS NULL` (both dialects) refuses a second live row.
6. Church data stays on one primary. Do not store sessions in Upstash. Upstash stays fragments and rate keys (spec 0001).
7. Public host (`FLY_APP_NAME` or `ECCLESIA_PUBLIC=1`) requires `DATABASE_URL`, `UPSTASH_REDIS_URL`, `RESEND_API_KEY`, `RESEND_FROM`, and `ECCLESIA_PUBLIC_URL`. Local work and tests omit Resend. The worker skips the vendor and marks mail `done`.
8. Sit-as demo (`may_impersonate`, persona grid, `ECCLESIA_DEMO`) is removed. Seed writes the gift catalog only.

## Requirements

**User stories**:
- As a person, I want to register with a password and sign in later so I do not need a demo grid.
- As a person, I want a magic link or a reset mail so I can get in when I forget the password.
- As an operator, I want two App processes to agree who is signed in, and I want logout to kill that cookie on every machine.

**Acceptance criteria**:
- **AC-1**: Register requires a password of at most 128 characters. The SDK runs zxcvbn with the name and email as user inputs. A score of 0 or 1, or a password longer than 128, is `DomainError::WeakPassword`. A score of 2 or higher is stored as argon2id (`Argon2::default()`). Domain receives a `Strength` enum only. (basis: [0002-passwords.md](0002-passwords.md), [docs.rs argon2](https://docs.rs/argon2/latest/argon2/))
- **AC-2**: Sign in is email plus password after `normalize_email`. A hit mints a `sessions` row and a `v2` cookie that carries that row id and a new csrf. Two App processes load that row from the primary and treat the person as signed in. (basis: [0002-sessions.md](0002-sessions.md))
- **AC-3**: Logout deletes that session row. Sign out everywhere and a successful password change delete every session for that person, then change password mints a new row and rewrites the cookie. Cookie `Max-Age` is 30 days from mint and is not rewritten on load. Idle expiry is `last_seen_at` plus 30 days so a copied cookie dies after they stop loading pages. The SDK touches `last_seen_at` at most once per UTC calendar day. (basis: [0002-sessions.md](0002-sessions.md), [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html))
- **AC-4**: Magic link lives 60 minutes. Password reset lives 24 hours. Both are one use. `t` is 32 random bytes as base64url. `token_hash` is SHA-256 hex of that mailed string. Unknown email, miss, and mail rate all show the same quiet flash. (basis: [0002-mail.md](0002-mail.md), [OWASP Forgot Password](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html))
- **AC-5**: Magic and reset write a `mail` outbox row in the same `apply` as the token. Invite writes mail only when `user_by_email` finds them (unknown email stays today's quiet `StoryOk`, no outbox). The SDK story sets `to`, `subject`, `text` (the full `href` is inside `text`). Worker POSTs `from`, `to`, `subject`, `text` to `https://api.resend.com/emails` with `Idempotency-Key: mail/{outbox_id}`. Dead after 8 tries. (basis: [0002-mail.md](0002-mail.md), [0001-outbox-worker.md](../0001-scale-persistence/0001-outbox-worker.md), [Resend send email](https://resend.com/docs/api-reference/emails/send-email))
- **AC-6**: Seed inserts gifts only. Flow and chaos tests register with a password, then plant a church and post needs through the real stories. The persona grid is gone. (basis: [0002-passwords.md](0002-passwords.md))
- **AC-7**: Public boot without `RESEND_API_KEY`, `RESEND_FROM`, or `ECCLESIA_PUBLIC_URL` refuses to start. (basis: [0002-mail.md](0002-mail.md), spec 0001 public host checks)
- **AC-8**: Guest GET `/session/link/{id}?t=` consumes a live magic link, mints a session, and sends them to Home. Guest GET `/session/reset/{id}?t=` paints the form when the token is live. Guest POST sets a new password, deletes every session, mints one, and sends them to Home. A signed in GET on either path is 303 `/home` and does not consume. (basis: [0002-mail.md](0002-mail.md))

## Decision

**Chosen option**: Own passwords on the primary, session rows both machines share, Resend on the worker.

Keep Rust, Axum, Maud, Neon or SQLite, and the HMAC cookie name. Put argon2id and zxcvbn in the SDK. Put session, magic link, and reset rows on the same primary as church data. Send mail from `ecclesia-worker` the way push already leaves the request.

**Implementation skills**: `resend` (`resend/resend-skills`, `.agents/skills/resend/`) · `rust-auth` (`huiali/rust-skills`, `.agents/skills/rust-auth/`)

## Feature design

**Data model sketch**:

`users` keeps `id`, `name`, `email` (unique), `city`, `region`, `bio`, `created_at`. Add `password_hash` (nullable text). Empty hash means they use reset or magic link.

`sessions`: `id`, `user_id` (FK `users.id`), `csrf` (copy of the cookie csrf), `created_at`, `last_seen_at`, `user_agent`, `ip`. Many per user. Index `user_id`. Guest has no row.

`magic_links` and `password_resets`: `id`, `user_id` (FK), `token_hash`, `expires_at`, `consumed_at` (nullable), `created_at`. Unique index `user_id WHERE consumed_at IS NULL` on each table. Index `user_id`.

Outbox `kind = mail` is reserved in [0001-outbox-worker](../0001-scale-persistence/0001-outbox-worker.md). This spec defines payload and Resend delivery.

**State transitions**:

Session: minted → live (each HTML load, daily touch) → deleted (logout, revoke, idle on load, password change, sign out everywhere).

Token: minted → consumed or expired. A new request retires an unused live row.

**API surface**:

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/` | GET | | Landing: register, sign in, Email me a link, Forgot password | guest; signed in redirects `/home` | |
| `/register` | POST | csrf, name, email, city, region, bio, password | `v2` cookie, 303 `/home` | guest | weak password, email taken, csrf, rate |
| `/session` | POST | csrf, email, password | `v2` cookie, 303 `/home` | guest | quiet miss, csrf, rate |
| `/session/link` | POST | csrf, email | 303 landing, quiet flash | guest | csrf, rate (email and client) |
| `/session/link/{id}` | GET | `t` query | `v2` cookie, 303 `/home` | guest consumes; signed in 303 `/home` no consume | quiet miss |
| `/session/reset` | POST | csrf, email | 303 landing, `?err=mail` | guest | csrf, rate |
| `/session/reset/{id}` | GET | `t` query | Set a new password form | guest; signed in 303 `/home` no consume | quiet miss |
| `/session/reset/{id}` | POST | csrf, `t`, password | `v2` cookie, 303 `/home` | guest | quiet miss, weak password |
| `/session/password` | POST | csrf, current, new | new `v2` cookie, 303 `/me` | live session | quiet miss on current, weak new |
| `/session/logout` | POST | csrf | clear cookie, 303 `/` | live session | csrf |
| `/session/logout-all` | POST | csrf | delete all rows, clear cookie, 303 `/` | live session | csrf |
| `/session/{id}/revoke` | POST | csrf | 303 `/me` | live session, same `user_id` | csrf, not yours |
| `/me` | GET | | profile plus device list | live session | guest landing |
| `/churches/{id}/invite` | POST | csrf, email | notice plus mail when the person exists; quiet `StoryOk` when they do not | governor | existing Domain errors for found people |

**Value sourcing**:

| Action | Value produced / displayed | Source |
|---|---|---|
| Register / change / reset | `Strength` | SDK `zxcvbn` crate, score from password plus name and email inputs. Length over 128 is `TooGuessable` without scoring. 0 and 1 are too guessable. 2 to 4 are acceptable. (basis: [zxcvbn Score](https://docs.rs/zxcvbn/latest/zxcvbn/enum.Score.html)) |
| Register / change / reset | `password_hash` | SDK `Argon2::default()` PHC string. Domain never sees it. |
| Sign in / magic / reset lookup | email | Domain `normalize_email`, then `user_by_email` |
| Sign in | Hash check | SDK `PasswordVerifier` on `users.password_hash`. Missing hash is a miss. |
| Apply extras | hash, session, token, mail | `StoryExtras` on `apply`, not Domain `Write` |
| Mint session | `sessions.id` | SDK `new_id` |
| Guest or mint csrf | `csrf` | SDK `fresh_csrf` (32 hex) in the cookie. Copied onto the row when signed in. |
| Mint session | `user_agent` | `User-Agent` request header, truncated |
| Mint session | `ip` | `Fly-Client-IP` when set, else the socket peer |
| Cookie | `ecclesia_sid` | `v2.{session_id}.{csrf}.{hmac}` with `ECCLESIA_SECRET`. Guest `session_id` is empty. |
| Load viewer | `user_id` | `sessions.user_id` for that cookie id, if the row is live |
| Idle | live or dead | `last_seen_at` plus 30 days, compared to SDK `now_iso` |
| Daily touch | `last_seen_at` | Write when the calendar date of `last_seen_at` is before today (UTC) |
| Me devices | last seen, short agent | `sessions.last_seen_at`, `sessions.user_agent`. IP stays off the page. |
| Magic / reset secret | raw `t` | 32 random bytes encoded base64url (no pad). Mail that string. `token_hash` is SHA-256 hex of that same string. |
| Token row | `expires_at` | magic: now plus 60 minutes. reset: now plus 24 hours. |
| Retire unused token | `consumed_at` | Set to `now_iso` on the prior live row, then insert. Unique index blocks two live rows. |
| Mail From | | `RESEND_FROM` |
| Mail link host | | Public: `ECCLESIA_PUBLIC_URL`. Local and tests: App passes the listen origin into the mail story (`http://127.0.0.1:{port}`). |
| Invite mail | `subject`, `text` | SDK template at invite time. `text` names the church, the `invite_code`, and `{origin}/churches`. Unknown email: no mail. |
| Magic / reset mail | `subject`, `text` | SDK template at request time. `text` includes the full `{origin}/session/link/{id}?t=` or reset URL. |
| Mail rate | refuse or allow | `Cache::incr_rate_window("rate:mail:{normalized_email}", 3600)` max 5, plus existing `RateKind::Session` on the client key (60s window, max 10). |
| Quiet flash | copy | `?err=miss` → `Try again.` (sign in and consume). `?err=mail` → `Check your email.` (request). Both added to the flash allow list. |
| Weak password flash | copy | Domain `WeakPassword` → `?err=password` → `Pick a stronger password.` |
| Outbox payload | JSON | `{to,subject,text}` only. No separate href field. |
| Resend idempotency | header | `mail/{outbox_id}` (basis: Resend skill, keys expire in 24 hours) |

**Key invariants**:
- Domain stories take `Strength`, never `&str` password.
- Raw password is zeroized after hash or verify.
- Logs and traces never print password, `t`, or `token_hash`.
- CSRF on every POST. GET magic has no CSRF (one use token).
- Register still refuses `TearsDown` on bio. Password is not word gated.

**Security model**:
Guest may register, sign in, request link, request reset, open link, open reset. Signed in guests hitting those POSTs go to Home. Change password, device list, revoke, logout, logout all need a live session for that person. Revoke one only when `session.user_id` is the viewer. Invite stays a governor of that church. Email and password are PII and secrets. No per account lockout this spec. Rate limits on register, session, and mail.

**Configuration required**:
- `RESEND_API_KEY`: worker send. Required on a public host.
- `RESEND_FROM`: verified From, like `Ecclesia <hello@example.com>`. Required on a public host.
- `ECCLESIA_PUBLIC_URL`: origin for links, no trailing slash. Required on a public host.
- `ECCLESIA_SECRET`: existing HMAC key for `v2` cookies.
- `ECCLESIA_SECURE`: existing Secure cookie flag.

**Critical test scenarios**:
- Happy path: register with a score 2 password, open Home on a second process with the same cookie and store. Verifies **AC-1**, **AC-2**
- Happy path: logout, then the old cookie loads as guest on both processes. Verifies **AC-3**
- Happy path: request reset for a known email, open the link, set a new password, land on Home, old devices are guests. Verifies **AC-4**, **AC-8**
- Failure: unknown email on sign in, magic, and reset all paint `Try again.` or `Check your email.` with no row that names the email as missing. Verifies **AC-4**
- Failure: public boot without `RESEND_FROM` exits. Verifies **AC-7**
- Auth: revoke another person's session id is a miss, their row stays. Verifies **AC-3**

## Build plan

Shipped (tracer bullet through Domain, SDK, and App):

1. Widen `users` with nullable `password_hash`. Add `sessions`. Cookie is `v2`; load the viewer from the row. A missing row is a guest. **AC-2**, **AC-3** (load side).
2. Domain `register` takes `Strength`. SDK scores and hashes. Landing register requires a password. Sit-as demo removed; seed is gifts only. Flow and chaos tests register first. **AC-1**, **AC-6**
3. `POST /session` email and password. Logout deletes the row. Daily `last_seen_at`. Idle 30 days on load. **AC-2**, **AC-3**
4. Me device list, revoke one, sign out everywhere, change password (kills other rows). **AC-3**
5. `magic_links` and `password_resets`. Request and consume stories. Quiet flashes. Mail outbox in the same transaction. Worker Resend with `reqwest` and `mail/{outbox_id}`. Public env checks. Invite adds a mail row. **AC-4**, **AC-5**, **AC-7**, **AC-8**

## Consequences

**Positive**:
- A person can come back without a demo grid.
- Two machines share one idea of who is signed in.
- Logout and a password change kill a stolen cookie.

**Negative / tradeoffs**:
- We run passwords ourselves. Hosted auth would have taken the hash and mail work. It would also sit outside Domain and the one transaction rule.
- GET magic consume can be burned by a mail scanner that fetches the link. The person then sees a quiet miss and can request again.
- zxcvbn 2 is a low bar. Short phrases can pass.
- Empty local directory until someone registers and plants a church.
- `v1` cookies become guests on deploy.

**Neutral**:
- [x] `docs/SECURITY.md` describes `v2` and the session table.
- Gift seed stays. Seed people and the Cedar Falls demo body are removed.
- Extra Resend skills (`react-email`, `resend-cli`, `agent-email-inbox`) installed with the pack are unused this spec.

## Follow-up

- [x] `/jsm-audit` should write root `AGENTS.md` and record `resend` plus `rust-auth` (area: identity and mail, not root prose).
- [ ] Per account lockout if guessing from many client keys becomes a problem.
- [ ] Confirm magic link (POST) if scanner burn shows up in logs.
- MFA and passkeys: declined. Not a later build.
- [ ] Resend hosted MCP if a later build wants live send from the agent.

## Rationale

Reasoning and options: see [rationale.md](rationale.md).
