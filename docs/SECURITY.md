# Security

The household rules are in the leaves. The skin still has duties it must not skip.

## Sessions

- Cookie name: `ecclesia_sid`
- Value: `v1.{user_id}.{csrf}.{hmac-sha256}`
- HMAC key: `ECCLESIA_SECRET`. If unset, the process mints a 32-byte key and sessions die on restart. The published string `dev-only-change-me` is accepted only when you set it, and is logged as unsafe.
- User ids in the cookie may only be `A-Z a-z 0-9 _ -` (max 80). A dotted id cannot hide extra fields in the payload.
- A valid cookie is not rewritten on every request, so a stolen cookie does not keep gaining 30 days.
- Flags: `HttpOnly`, `SameSite=Lax`, `Path=/`, 30-day max age. Set `ECCLESIA_SECURE=1` when TLS terminates in front so the cookie also has `Secure`.
- The old unsigned `ecclesia_uid` cookie is gone.

## CSRF

Every HTML form posts a hidden `csrf` field. The token is 32 hex characters stored in the signed session. The comparison is length-checked and constant-time. A missing or wrong token redirects with `?err=csrf`. Redirect paths must be same-origin (`/`…, never `//`).

## Authorization

HTTP does not decide membership, need visibility, or endorsements. It loads values and calls a leaf. A steward-looking request that fails the leaf is a `403`-shaped flash, not a write.

Push endpoints are scoped to the signed-in person. Unsubscribe cannot delete someone else's subscription. Subscribe cannot steal an endpoint or device token that already belongs to another seat. Endpoints must be `https://`.

## Input

Leaves trim and cap length (`validate.rs`). Emails must have a local part, `@`, and a dotted domain. Rewrite text is capped at 2,000 characters before it reaches a model. Maud escapes text into HTML.

Flash query codes (`?ok=` / `?err=`) are an allow-list. Unknown codes become `Done.` or `That didn't work.` They are never echoed back as the attacker's sentence.

An invite by email that names no one on Ecclesia still returns `invited`, so the address book cannot be probed.

## Rate limits

Per client, per minute, in process:

- Sign-in: 10
- Register: 5
- Rewrite: 20
- Redeem an invite code: 8
- Push subscribe / unsubscribe / device: 30

Invite codes for new churches use an 8-character nonce (`gracecov-k2m9p4r1`). Seeded demo churches keep their short codes.

## Headers

`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `Permissions-Policy` with camera / microphone / geolocation closed, `X-Permitted-Cross-Domain-Policies: none`, and a CSP that allows this origin plus Google Fonts.

## Demo seats

Sitting as a seeded person is `may_impersonate`. It is **off** unless you set `ECCLESIA_DEMO=1`. There are still no passwords. Do not put this process on the public internet with demo seats enabled.

```bash
ECCLESIA_DEMO=1 ECCLESIA_SECRET=$(openssl rand -hex 32) cargo run
```

## What we are not claiming

- The in-process rate gate is per replica, not a shared store
- No audit log
- SQLite file permissions are the operator's job
- The word gate is a last resort when Jev or a chat model is down; it folds leetspeak and spaced letters, and it can still miss a clever insult
