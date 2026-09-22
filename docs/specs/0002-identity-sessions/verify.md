# Verify: Identity and sessions · spec 0002 · 2026-09-22
_Steps derived from spec 0002 acceptance criteria. `/jsm-check verify` runs these; `/jsm-test` locks the durable ones._

## Commands
- [x] `rg "may_impersonate|DemoSeat|ECCLESIA_DEMO|persona_grid" crates/app crates/domain crates/sdk` shows no live sit as path → AC-6, AC-P5
- [x] `rg "password_hash" crates/domain` is empty → AC-1
- [x] `cargo test -p ecclesia-domain --lib` includes `WeakPassword` and no `us_auth_02` impersonation → AC-1, AC-P1
- [x] `cargo test -p ecclesia-sdk --lib` session `v2` round trip and tamper reject → AC-2, AC-S1
- [x] Two SDK handles on one SQLite file: register, second handle loads the same session cookie as signed in → AC-2
- [x] Logout, second handle is a guest → AC-3
- [x] zxcvbn score 1 register does not insert a user → AC-1
- [x] Unknown email on magic writes no `magic_links` row → AC-4, AC-M1
- [x] Apply register writes `mail` only when the story is invite or a mail request, never on a failed score → AC-5
- [x] Worker test: `mail` payload POSTs with `Idempotency-Key: mail/{id}` or the test hub records that header → AC-5, AC-M7
- [x] `ECCLESIA_PUBLIC=1` without `RESEND_API_KEY` (with DB and Upstash set) refuses boot → AC-7
- [x] Seed on empty SQLite: gifts exist, `users` count is 0 → AC-6

## UI / manual
- [x] Open `/` as a guest: register, sign in, Email me a link, Forgot password. No persona grid.
- [x] Register with a weak password: flash `Pick a stronger password.`, still on landing.
- [x] Register with a passing password: Home, cookie `ecclesia_sid` starts with `v2.` and has three dotted fields after `v2`.
- [x] Sign out: landing. Old cookie does not open Home.
- [x] Sign in with a wrong password: `?err=miss` and `Try again.`, no hint the email exists.
- [x] Me shows this device. Sign out everywhere, then Home is landing.
- [x] Change password: other device cookie is guest, this one stays.
- [x] Forgot password for a real inbox: `?err=mail` and `Check your email.`, outbox `mail` pending, worker sends, open link, set password, Home.
- [x] Magic link GET signs in. Second open of the same URL is `Try again.`
- [x] Invite a known member: inbox notice plus outbox mail with the invite code and a `/churches` link. Invite an unknown email: `Invite sent.`, no outbox.

## Value sources
- [x] Home `user_id` comes from `sessions.user_id`, not from a user id in the cookie.
- [x] zxcvbn inputs include name and email on register.
- [x] Session `ip` is `Fly-Client-IP` or the peer, never the first `X-Forwarded-For` hop.
- [x] Magic and reset URLs use `ECCLESIA_PUBLIC_URL` on a public host, and the App listen origin locally.
- [x] Resend `from` is `RESEND_FROM`.
- [x] Invite mail body uses `churches.invite_code` and `{origin}/churches`.
