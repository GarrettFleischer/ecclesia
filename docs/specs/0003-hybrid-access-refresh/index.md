# 0003. Hybrid access and refresh tokens

**Date**: 2026-09-30
**Status**: Accepted

## Summary

Ecclesia keeps cookie sessions for the HTML site and the Capacitor WebView. JSON clients can sign in, hold a short lived access bearer, and rotate with a refresh secret stored as a hash on the same `sessions` row. Revoke, sign out everywhere, password change, and idle timeout delete that row, so API credentials die the same way a stolen cookie dies. The first slice proves the auth path with `GET /api/me` as JSON.

## Requirements

**User stories**:
- As a client author, I want to sign in with email and password over JSON and call APIs with a bearer so I do not depend on browser cookies.
- As a member, I want API devices on Me to revoke like today’s cookie devices so a lost phone loses access.
- As an operator, I want two App processes to agree on API sessions the same way they agree on cookie sessions.

**Acceptance criteria**:
- **AC-1**: `sessions` gains `transport` (`cookie` or `api`) and nullable `refresh_token_hash`. Existing rows behave as `cookie`. Cookie mint paths stay byte for byte compatible with spec 0002.
- **AC-2**: `POST /api/session` accepts JSON email and password. On success it returns `access_token`, `refresh_token`, `expires_in` (seconds), and `token_type` `Bearer`. On miss it returns `401` with a generic body (no email enumeration). Rate limits reuse `RateKind::Session` on the client key.
- **AC-3**: Access tokens use wire form `v3.at.{session_id}.{exp_unix}.{hmac}`. HMAC-SHA256 hex covers `at.{session_id}.{exp_unix}` only (no `v3.` in the MAC). `exp_unix` is UTC unix seconds from the SDK clock, exactly `now + 900`. Bearer load rejects dotted `session_id`, checks MAC, expiry, idle (`last_seen_at` plus thirty days), and `transport = api`. `resolve_session` for cookies treats `transport = api` as dead (no cookie bind on API rows). Failure is `401`.
- **AC-4**: `POST /api/session/refresh` accepts JSON `refresh_token`. Wire refresh is `rt.` plus thirty two random bytes as base64url no pad; hash is SHA-256 hex of that full string. On success it returns the same JSON shape as sign in (including `token_type` `Bearer`). Rotation uses `UPDATE` with `WHERE refresh_token_hash = :old`; concurrent refresh with the same token leaves one winner and `401` for the loser. Stale refresh after rotation returns `401`. Refresh checks idle before rotate; daily `last_seen_at` touch matches HTML.
- **AC-5**: `GET /api/me` with a valid access bearer returns JSON with the signed in person’s id, name, email, city, region, and bio (same fields as the profile form cares about). Guest or bad bearer is `401`.
- **AC-6**: `POST /api/session/logout` with a valid access bearer deletes that session row. `POST /session/logout`, `POST /session/logout-all`, `POST /session/{id}/revoke`, and password change continue to delete the affected session rows, including `transport = api` rows, per 0002 rules.
- **AC-7**: HTML routes, forms, CSRF, and `ecclesia_sid` behavior are unchanged. `/api/*` does not read or write `ecclesia_sid`; cookies do not authorize `/api/*`. JSON responses use `Cache-Control: no-store`. Integration tests cover at least one cookie flow and one API flow on the same database; `POST /api/session` does not set `Set-Cookie`.
- **AC-8**: API session rows set `user_agent` and `ip` from the request like cookie mint. Daily `last_seen_at` touch applies on authenticated API reads the same way as signed in HTML.

## Decision

**Chosen option**: Option 2 from `rationale.md`: short signed access plus opaque refresh on the same session row, cookie path untouched.

**Implementation skills**: `rust-auth` (`huiali/rust-skills`, `.agents/skills/rust-auth/`)

## Rationale

See [rationale.md](rationale.md).

## Feature design

**Data model sketch**:

`sessions` (alter existing):

| Column | Type | Notes |
|--------|------|--------|
| `transport` | TEXT NOT NULL DEFAULT `cookie` | `cookie` or `api` |
| `refresh_token_hash` | TEXT NULL | SHA-256 hex of full wire refresh string; NULL for cookie rows |

Indexes: `idx_sessions_user` on `user_id`; unique partial index on `refresh_token_hash` where not null.

Access token payload is not stored; refresh secret is never stored raw.

**State transitions**:

Session row: minted (register or sign in, cookie or api) → live (daily touch on use) → deleted (logout, revoke, idle on load, password change, logout all). Refresh: minted with row → rotated on each refresh → invalid when row deleted or hash replaced.

**API surface**:

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/session` | POST | JSON `email`, `password` | `200` snake_case: `access_token`, `refresh_token`, `expires_in` (900), `token_type` `Bearer` | none | 401 `{"error":"miss"}`, 429 `{"error":"rate"}`, 400 `{"error":"missing"}` |
| `/api/session/refresh` | POST | JSON `refresh_token` | same `200` shape as sign in | none | 401 stale or idle, 429 rate |
| `/api/session/logout` | POST | `Authorization: Bearer` access | `204` empty | access bearer | 401 |
| `/api/me` | GET | | `200` `{ id, name, email, city, region, bio }` strings | access bearer | 401 |

`/api/*` returns JSON errors only (never HTML form error pages). Sign in and refresh share `RateKind::Session` on `ClientKey` (10 per 60s). No CORS this slice (native clients; browsers stay on cookies).

HTML and cookie routes from 0002 are untouched.

**Value sourcing**:

| Action | Value produced / displayed | Source |
|---|---|---|
| API sign in | `user_id` on new row | Domain `normalize_email`, `user_by_email`, SDK password verify (same as `POST /session`) |
| API sign in | `session_id` | SDK `new_id` |
| API sign in | `refresh_token` wire | `rt.` + 32 random bytes base64url no pad |
| API sign in | `refresh_token_hash` | SHA-256 hex of full wire refresh string |
| API sign in | `access_token` | wire `v3.at.{session_id}.{exp_unix}.{hmac}`; MAC input `at.{session_id}.{exp_unix}` |
| API sign in | `exp_unix` | `chrono::Utc::now().timestamp() + 900` (SDK clock family) |
| API sign in | `expires_in` | 900 |
| API sign in | `sessions.csrf` on api row | `fresh_csrf()` unused for API; satisfies NOT NULL |
| Refresh | new refresh raw and hash | CAS update on old hash; daily touch when calendar day changed |
| Bearer load | `user_id` | `sessions.user_id` for `session_id` from access token after HMAC and idle |
| Me JSON | profile fields | `users` row for bearer `user_id` |
| Rate limit | refuse | `Cache::incr_rate` with `RateKind::Session` on `ClientKey` |
| Client IP | `sessions.ip` | `Fly-Client-IP` or socket peer |
| User agent | `sessions.user_agent` | `User-Agent` header truncated |

**Key invariants**:

- Domain never sees bearer strings, refresh strings, or hashes.
- Cargo edges stay App → SDK → Domain. API handlers call SDK stories only.
- Sessions stay on the primary; not in Upstash.
- Cookie rows always have `transport = cookie` and `refresh_token_hash` NULL.
- API rows use `transport = api`, carry refresh hash, and never receive `ecclesia_sid`.
- CSRF is not required on `/api/*` in this slice; bearer is the write gate for logout.
- Constant time compare for refresh lookup (hash compare).

**Security model**:

Only the owner of a session row may use its access or refresh. Revoke and Me follow 0002 ownership rules. API sign in uses the same quiet miss as HTML sign in. Refresh and sign in are rate limited per client key.

**Configuration required**:

No new env vars. Reuse `ECCLESIA_SECRET` for access HMAC.

**Critical test scenarios**:

- Happy path: API sign in, `GET /api/me`, refresh, `GET /api/me` again (AC-2, AC-3, AC-4, AC-5)
- Happy path: two `Sdk` handles, same refresh on both until rotation (AC-8, multi machine)
- Failure: wrong password `401` (AC-2)
- Failure: stale refresh after rotation `401` (AC-4)
- Failure: parallel refresh with same token, one `401` (AC-4)
- Failure: refresh after thirty day idle `401` (AC-3, AC-4)
- Auth: revoke API session from Me via existing revoke route, bearer fails (AC-6)
- Auth: cookie sign in still works; CSRF form post still works (AC-7)

## Build plan

Tracer Bullet order: schema and SDK token mint first, then routes, then proof read.

1. **Schema and SDK core** (AC-1, AC-3, AC-4): migrate `sessions` columns; implement access encode/decode, refresh mint/hash/rotate, `resolve_bearer`, api session mint on password sign in; wire idle and daily touch.
2. **API auth routes** (AC-2, AC-4, AC-6): Axum JSON handlers for `/api/session`, `/api/session/refresh`, `/api/session/logout`; JSON error bodies; rate limits.
3. **Proof endpoint** (AC-5, AC-8): `GET /api/me` JSON; bearer extractor shared with logout.
4. **Revocation parity** (AC-6): confirm logout all, revoke, password change delete api rows (extend tests, not new routes).
5. **Tests** (AC-7): extend `flows.rs` or sibling tests for API path plus one cookie regression.

## Migration plan

**Strategy**: strangler, additive columns and new routes only.

**Phases**:
1. Deploy migration with defaults so existing rows are `cookie`.
2. Deploy API routes; no change to HTML handlers.
3. Clients opt in by calling `/api/session`.

**Rollback**: remove routes; leave nullable columns harmless.

**Risks**: clients that log refresh tokens need clear docs not to log production secrets. Mitigation: prefix tokens for support identification only, never store raw refresh in logs on server.

## Consequences

**Positive**:
- One device table for cookie and API.
- Revocation semantics stay familiar.

**Negative / tradeoffs**:
- DB read on each API request (accepted in 0002 for HTML).
- Clients must store refresh securely (Keychain, Keystore).

## Follow-up

- **Magic link token exchange**: `POST /api/session/link` to trade one time mail token for access and refresh (native deep links).
- **JSON resource API**: needs, inbox, and writes behind bearer (separate features).
- **Refresh reuse detection**: optional family id and revoke all on reuse (OAuth best practice).
