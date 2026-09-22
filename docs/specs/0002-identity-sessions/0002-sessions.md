# 0002. Sessions

## Summary

A signed in cookie is a signed pointer to a session row. Both App processes load that row. Logout deletes it. Idle is 30 days from last seen, touched at most once a day. Me lists devices. They can sign out one device or all of them.

## Requirements

**User stories**:
- As a member, I want to see my devices and sign one out so a lost phone is not still in.
- As an operator, I want a stolen cookie to die when they log out.

**Acceptance criteria**:
- **AC-S1**: Cookie value is `v2.{session_id}.{csrf}.{hmac}`. Guest `session_id` is empty. Signed in `session_id` follows today's user id charset (1 to 80, `A-Z a-z 0-9 _ -`). CSRF is 32 hex in the cookie. `v1` decode is gone. Tamper is a guest.
- **AC-S2**: `from_jar` reads csrf from the cookie. When `session_id` is set it loads `sessions` by id. Missing, idle (`last_seen_at` plus 30 days before now), deleted, or row csrf that does not match the cookie is a guest plus a new csrf. Cookie `Max-Age` stays 30 days from mint and is not rewritten on a normal load. Signed in HTML that needs a person goes to landing with `?err=miss`.
- **AC-S3**: `POST /session/logout` deletes that row and sets a guest cookie. `POST /session/logout-all` deletes every row for that `user_id` and sets a guest cookie.
- **AC-S4**: `POST /session/{id}/revoke` deletes that row only when it belongs to the viewer. Revoking the current row is the same as logout.
- **AC-S5**: Me lists each live session: last seen, a short `user_agent` line, and a mark on the current one. IP is stored and not painted.
- **AC-S6**: On a signed in HTML load, if `last_seen_at` is a previous UTC date, the SDK writes today. Static files do not touch.

## Decision

Session rows on Neon or SQLite, not Upstash. Cookie holds the id, not the user id.

## Feature design

**Data model sketch**:
`sessions(id, user_id, csrf, created_at, last_seen_at, user_agent, ip)`. Index `user_id`. Forms post the csrf from the cookie. The row stores a copy written at mint.

**State transitions**:
Minted at register, sign in, magic consume, reset complete, change password. Deleted on logout, revoke, logout all, password change (others), idle when loaded.

**API surface**:

| Endpoint | Method | Auth | Key errors |
|---|---|---|---|
| `/session/logout` | POST | live session | csrf |
| `/session/logout-all` | POST | live session | csrf |
| `/session/{id}/revoke` | POST | live session, owner | csrf, not yours |
| `/me` | GET | live session | guest |

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| Cookie mac | hex HMAC-SHA256 | `ECCLESIA_SECRET` plus payload `v2.{session_id}.{csrf}` |
| Idle check | dead or live | `last_seen_at` + 30 days vs `now_iso` |
| Device line | short agent | first 80 chars of `User-Agent`, no raw IP |
| Client IP | `sessions.ip` | `Fly-Client-IP` or socket peer |

**Key invariants**:
- CSRF still 32 hex, compared constant time.
- A valid cookie is not rewritten on every request (today's rule). Only daily last seen writes.
- Secure flag still follows `ECCLESIA_SECURE`.

**Security model**:
Only the owner lists or revokes their rows. Guest with a dead cookie is not told why.

**Configuration required**:
`ECCLESIA_SECRET`, `ECCLESIA_SECURE` as today.

**Critical test scenarios**:
- Happy path: two `Sdk` handles on one SQLite file, same cookie, both load the user. Verifies **AC-S1**, **AC-S2**
- Happy path: logout, second handle sees a guest. Verifies **AC-S3**
- Failure: revoke a session id for another user leaves it. Verifies **AC-S4**
- Auth: Me as guest redirects to landing. Verifies **AC-S2**

## Build plan

1. Table, `v2` encode/decode, load by row. Satisfies **AC-S1**, **AC-S2**
2. Logout, logout all, revoke. Satisfies **AC-S3**, **AC-S4**
3. Me list and daily touch. Satisfies **AC-S5**, **AC-S6**

## Consequences

**Positive**:
- Revoke is a delete both machines see.

**Negative / tradeoffs**:
- Extra lookup on every signed in page.
- `v1` cookies drop on deploy.

**Neutral**:
- [x] [docs/SECURITY.md](../../SECURITY.md) Sessions describe `v2` and the session table.

## Rationale

OWASP wants the server to invalidate on logout. A signed user id cannot do that across two machines. A row can. Upstash would make cache miss a mass sign out. The primary already has to be up for Home.
