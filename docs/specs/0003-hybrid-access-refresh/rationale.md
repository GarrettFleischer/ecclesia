# 0003. Hybrid access and refresh tokens (rationale)

## Context

Ecclesia already signs people in with a `v2` cookie that points at a `sessions` row on the primary (spec 0002). HTML forms, magic links, and Capacitor in a WebView all use that path. A future native or JSON client cannot rely on HttpOnly cookies and CSRF hidden fields the same way. The product still needs one revocation story: logout, revoke one device, sign out everywhere, password change, and idle expiry must kill API credentials as fast as they kill a stolen cookie.

This spec adds a parallel credential path. It does not replace the cookie for the Maud site or the installable web shell.

## Options considered

### Option 1: Long lived bearer only (session id in Authorization)

Reuse `sessions.id` as the bearer string with no refresh rotation.

**Pros**:
- Smallest schema change.
- Me and revoke already map to session rows.

**Cons**:
- A leaked bearer lives until idle or revoke. Mobile apps store tokens in OS keychains; long lived secrets raise impact.
- No standard rotation story for clients.

### Option 2: Short signed access plus opaque refresh on the same session row (recommended)

Keep one `sessions` row per device. Cookie rows stay as today. API rows store a refresh hash. Access is a short lived signed bearer tied to `session_id`. Refresh rotates the hash and mints new access.

**Pros**:
- One device list on Me for cookie and API clients.
- Revoke deletes the row; access dies on the next lookup even inside the short window.
- No sessions in Redis (matches 0002).
- Strangler: HTML unchanged.

**Cons**:
- Every API request checks the row (same cost as today’s signed in HTML).
- Clients must implement refresh rotation.

### Option 3: JWT access only, no server refresh table

Self contained JWT for access, refresh JWT with longer exp.

**Pros**:
- Fewer DB reads if you skip session lookup.

**Cons**:
- Revoke and “sign out everywhere” need deny lists or short TTL plus refresh rows anyway.
- Duplicates the session model 0002 already paid for.

### Option 4: Hosted auth provider (Auth0, Clerk, etc.)

Move API tokens to a vendor.

**Pros**:
- Less custom security code.

**Cons**:
- Breaks the 0002 contract (passwords and sessions on the primary, Domain never sees vendor tokens).
- New cost and migration risk for a small surface.

## Rationale

Option 2 fits the existing architecture. Sessions already represent devices, sync across Fly machines through Neon or SQLite, and support revoke. Short access limits blast radius if a bearer leaks from logs or a crash dump. Rotating refresh detects reuse when you add that check later. Cookie auth stays the default for the shipped web product; JSON is opt in for clients that need it.

The staged interview for this run leaned on scope feature 3, prior auth discussion in chat, and the engineer’s direction toward a robust production shape. Load bearing picks: opaque refresh with hash at rest, fifteen minute access, strangler migration, first slice ends at JSON sign in, refresh, logout, and `GET /api/me`.

## References

**Project sources**
- [0002 identity and sessions](../0002-identity-sessions/index.md) shared contract
- [0002 sessions](../0002-identity-sessions/0002-sessions.md) idle and device list
- `crates/sdk/src/session.rs`, `crates/sdk/src/identity.rs`

**Practices**
- OWASP Session Management Cheat Sheet (idle timeout, logout server side)
- OWASP JWT Cheat Sheet (prefer short access and server side revocation when sessions exist)
