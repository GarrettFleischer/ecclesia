# Security

Church rules live in Domain. The App and SDK still have duties they must not skip.

## Sessions

- Cookie name: `ecclesia_sid`
- Value: `v2.{session_id}.{csrf}.{hmac-sha256}`. Guest `session_id` is empty. Signed-in `session_id` is the primary key of a row in `sessions`, not the user id.
- HMAC key: `ECCLESIA_SECRET`. If unset, the process mints a 32-byte key and sessions die on restart. The published string `dev-only-change-me` is accepted only when you set it, and is logged as unsafe.
- Session ids and user ids in the cookie may only be `A-Z a-z 0-9 _ -` (max 80). A dotted id cannot hide extra fields in the payload.
- Each request loads `sessions` by `session_id`. A missing row, csrf mismatch, or `last_seen_at` older than 30 days becomes a guest with a fresh csrf. `v1` cookies decode as guests.
- A valid cookie is not rewritten on every request.
- Flags: `HttpOnly`, `SameSite=Lax`, `Path=/`, 30-day max age. Set `ECCLESIA_SECURE=1` when TLS terminates in front so the cookie also has `Secure`.
- Logout, revoke one device, sign out everywhere, and password change delete the affected session rows. See spec [0002](../specs/0002-identity-sessions/index.md).
- The old unsigned `ecclesia_uid` cookie is gone.

## Passwords

- Passwords are stored as Argon2 hashes on `users.password_hash`. Register and reset enforce Domain strength (zxcvbn score 2).
- Sign-in, magic link, and reset miss quietly so email cannot be probed. Rate limits apply to session and register paths.

## CSRF

Every HTML form posts a hidden `csrf` field. The token is 32 hex characters stored in the signed session. The comparison is length-checked and constant-time. A missing or wrong token redirects with `?err=csrf`. Redirect paths must be same-origin (`/`…, never `//`).

## Authorization

HTTP does not decide membership, need visibility, or endorsements. It loads values and calls Domain. A pastor-looking request that fails Domain is a `403`-shaped flash, not a write.

Push endpoints are scoped to the signed-in person. Unsubscribe cannot delete someone else's subscription. Subscribe cannot steal an endpoint or device token that already belongs to another member. Endpoints must be `https://`.

## Input

Domain trims and caps length (`validate.rs`). Emails must have a local part, `@`, and a dotted domain. Rewrite text is capped at 2,000 characters before it reaches a model. Maud escapes text into HTML.

Flash query codes (`?ok=` / `?err=`) are an allow-list. Unknown codes become `Done.` or `That didn't work.` They are never echoed back as the attacker's sentence.

An invite by email that names no one on Ecclesia still returns `invited`, so the address book cannot be probed.

## Public store

On a public host (`FLY_APP_NAME` or `ECCLESIA_PUBLIC=1`) boot needs a `postgres` or `postgresql` `DATABASE_URL` (the Neon pooler URL), `UPSTASH_REDIS_URL`, `RESEND_API_KEY`, `RESEND_FROM`, and `ECCLESIA_PUBLIC_URL`. Use the session pooler, not the transaction pooler, and keep runtime `query` strings. sqlx `query!` does not share one text across SQLite and Postgres.

In the Neon dashboard, leave point in time recovery on and keep a daily snapshot. Region is `us-east-1`. Fly stays in `iad`.

## Rate limits

Per client, per minute, shared in Upstash (`rate:{kind}:{who}`). Local work without Upstash uses an in process fallback:

- Sign-in: 10
- Register: 5
- Rewrite: 20
- Redeem an invite code: 8
- Push subscribe / unsubscribe / device: 30

Invite codes for new churches use an 8-character nonce when a church is planted.

## Headers

`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `Permissions-Policy` with camera / microphone / geolocation closed, `X-Permitted-Cross-Domain-Policies: none`, and a CSP that allows this origin plus Google Fonts.

## Local work

Local SQLite seeds the gift catalog only. Register a member and plant a church to walk flows. Sit-as demo (`ECCLESIA_DEMO`, persona grid) was removed in spec 0002.

```bash
ECCLESIA_SECRET=$(openssl rand -hex 32) cargo run
```

Do not run a shared host with weak secrets.

## What we are not claiming

- No audit log
- SQLite file permissions are the operator's job
- The word gate folds leetspeak and spaced letters, and it can still miss a clever insult
