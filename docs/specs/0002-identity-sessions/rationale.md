# 0002. Identity and sessions: rationale

## Context

> Premise note: Rolling our own passwords is a known failure pattern (hashes, reset tokens, enumeration, fixation). A hosted auth product would own those edges. Ecclesia still owns them because Domain must stay values in and `Effect` out, and spec 0001 already kept the session on our secret. Hosted auth would become a second source of truth beside Neon. This spec keeps auth on the primary and names the edges so the build cannot invent them. GET consume on the magic link can be burned by a prefetch. zxcvbn 2 is the bar the engineer set, not a high one. The topic is three decisions (passwords, session rows, mail), so this is an umbrella.

**Before this spec**, spec 0001 ran two Fly machines, Neon, Upstash, and an HMAC cookie that carried `user_id`. A stolen cookie worked until max age. Passwords, reset, and shared revoke were missing. Local demo used sit-as seats and a seeded Cedar Falls body.

**Shipped:** email and password, session rows and `v2` cookies, magic link and reset through Resend, invite mail on the same outbox path, no sit-as demo, gift-only seed. Tests register like anyone else. Churches and needs start empty until those stories run.

Forces: Domain cannot hash or call Resend. The request must not wait on mail (same as push). Two App processes must agree. Quiet replies on mail and sign in so an inbox cannot be probed. Fly sits in front, so the client IP is `Fly-Client-IP`, not a spoofable forwarded list.

If we do not decide this, the public host stays a one way register with a cookie both machines trust but neither can revoke.

## Options considered

### Option 1: HMAC cookie only, add passwords

Keep `v1.{user_id}.{csrf}.{hmac}`. Add `password_hash`. Sign in still writes no row.

**Pros**:
- Smallest change. No new table for the common path.

**Cons**:
- Logout cannot kill a stolen cookie on the other machine. The engineer required that.

### Option 2: Session rows on the primary (chosen)

Cookie is a signed pointer to `sessions.id`. The row is the seat. Logout deletes it. Password change deletes every row for that person.

**Pros**:
- Two processes agree. Revoke is a delete. Same primary as church data, same transaction as the story.

**Cons**:
- Every signed in HTML load is a session lookup. Daily touch is a write.

### Option 3: Hosted auth (Clerk, Auth0, Neon Auth)

Hand passwords and mail to a vendor. Ecclesia trusts a JWT or a webhook.

**Pros**:
- Reset, lockout, and bot scoring become their problem.

**Cons**:
- A second user store beside `users`. Domain stories would wait on a vendor. Conflicts with spec 0001 crate line and one transaction.

### Option 4: Sessions in Upstash

Put live sessions in Redis. Cookie holds the key.

**Pros**:
- Fast revoke. TTL is native.

**Cons**:
- Spec 0001 said cache is not the source of truth. A cache miss would sign everyone out or require a fallback row anyway.

## Rationale

The engineer asked for two machines to agree who is signed in, and for logout to kill the cookie. That is a row you can delete, not only an HMAC. The primary already holds `users` and the outbox. Putting `sessions` there keeps one transaction and one operator story. Upstash stays rate keys and fragments.

Passwords stay in process with argon2id and zxcvbn because Domain can take a `Strength` the way it already takes `Posture`. A hosted IdP would break that line. `rust-auth` shows `Argon2::default()` and HttpOnly cookies. We do not take its JWT default. This is an HTML app with CSRF in the session, not a bearer API.

Mail follows push: write the outbox in the story, let the worker talk to Resend with `reqwest` and an idempotency key so a retry does not double send. Magic link is sign in for a known email only. A new email gets the same `Check your email.` Reset is 24 hours so a delayed inbox still works. Invite reuses `invite_code` and the existing quiet miss when the email is new. CSRF stays in the cookie so guests can post. Mail rate uses a 3600s window because today's `incr_rate` expires in 60s.

Demo seats and seed people hide the product we are trying to ship. Tests that register, plant, and post prove the new door. The gift catalog can still seed so the need form has names.

## References

**Project sources**:
- spec 0001 crate line, outbox, public host
- [docs/SECURITY.md](../../SECURITY.md) sessions and passwords
- `docs/DOMAIN.md` values in, `Effect` out
- `docs/PROSE.md` flashes
- `.agents/skills/resend` idempotency keys
- `.agents/skills/rust-auth` Argon2::default and HttpOnly cookies

**Practices & standards**:
- Server side session, idle timeout, logout invalidates the server row
- Same reply for known and unknown email on recovery
- One use hashed tokens
- Argon2id for passwords, SHA-256 for high entropy tokens
- Rate limit public doors

**Links** (verified 2026-09-22):
- Argon2 crate: https://docs.rs/argon2/latest/argon2/
- Argon2 PasswordHasher trait: https://docs.rs/argon2/latest/argon2/trait.PasswordHasher.html
- Argon2 Params: https://docs.rs/argon2/latest/argon2/struct.Params.html
- zxcvbn crate: https://docs.rs/zxcvbn/latest/zxcvbn/
- zxcvbn Score: https://docs.rs/zxcvbn/latest/zxcvbn/enum.Score.html
- Resend send email: https://resend.com/docs/api-reference/emails/send-email
- OWASP Session Management: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- OWASP Forgot Password: https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html
- OWASP Authentication: https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
