# Security

The household rules are in the leaves. The skin still has duties it must not skip.

## Sessions

- Cookie name: `ecclesia_sid`
- Value: `v1.{user_id}.{csrf}.{hmac-sha256}`
- HMAC key: `ECCLESIA_SECRET` (required in any real deployment; local default is `dev-only-change-me` and is logged)
- Flags: `HttpOnly`, `SameSite=Lax`, `Path=/`, 30-day max age
- The old unsigned `ecclesia_uid` cookie is gone. You cannot become Miriam by writing her id into a cookie.

## CSRF

Every HTML form posts a hidden `csrf` field. The token is 32 hex characters stored in the signed session. The comparison is length-checked and constant-time. A missing or wrong token redirects with `?err=csrf`.

## Authorization

HTTP does not decide membership, need visibility, or endorsements. It loads values and calls a leaf. A steward-looking request that fails the leaf is a `403`-shaped flash, not a write.

## Input

Leaves trim and cap length (`validate.rs`). Emails must have a local part, `@`, and a dotted domain. Maud escapes text into HTML.

## Headers

`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, and a CSP that allows this origin plus Google Fonts.

## Demo seats

Sitting as a seeded person is `may_impersonate`, not a household rule. It is on when `ECCLESIA_DEMO` is unset or not `0`. Turn it off for any shared host:

```bash
ECCLESIA_DEMO=0 ECCLESIA_SECRET=$(openssl rand -hex 32) cargo run
```

There are still no passwords. Production identity (magic links) is a later skin. Do not put this process on the public internet with demo seats enabled.

## What we are not claiming

- No rate limiter yet
- No Secure cookie flag unless you terminate TLS in front (set the proxy to add it)
- No audit log
- SQLite file permissions are the operator’s job
