# SDK

## Overview

The only crate the App may call. Stories load rows, call Domain, and commit one transaction. Store, session, cache, mail, push, and the word gate live here.

## Key files

| File | Owns |
|---|---|
| `src/story.rs` | One function per user story |
| `src/identity.rs` | Register, sign in, magic link, reset, invite mail |
| `src/session.rs` | Cookie `v2.{session_id}.{csrf}.{hmac}` |
| `src/db/schema.rs` | Tables for both dialects |
| `src/db/apply.rs` | Effect plus `StoryExtras` in one transaction |
| `src/outbox.rs` | Worker delivery, including Resend |
| `src/host.rs` | Public host env checks |

## Conventions

- Domain never sees a raw password, a hash string, a cookie, or Resend.
- zxcvbn and argon2id stay in this crate. Domain receives `Strength` only.
- Session rows live on the primary. Upstash holds `catalog`, `church:{id}`, `directory`, and `rate:{kind}:{who}`.
- Seed writes the gift catalog only.
- Guest cookie has an empty session id. A missing or idle row is a guest.

## Gotchas

- Local mail with no `RESEND_API_KEY` is marked done and not sent.
- A public host refuses to boot without the mail env vars.
- sqlx `query!` cannot share one SQL string across SQLite and Postgres. Use runtime `?` text.

## Agent skills

- [sqlx](../../.agents/skills/sqlx/): `melonask/sqlx-skills`, pools and both Postgres and SQLite
- [neon-postgres](../../.agents/skills/neon-postgres/): `neondatabase/agent-skills`, Neon connections and branches
- [upstash](../../.agents/skills/upstash/): `upstash/skills`, Upstash product map (JavaScript client)
- [redis-core](../../.agents/skills/redis-core/): `redis/agent-skills`, Redis commands for any host
- [rust-auth](../../.agents/skills/rust-auth/): `huiali/rust-skills`, passwords and session patterns
- [resend](../../.agents/skills/resend/): `resend/resend-skills`, send, receive, webhooks
- [resend-cli](../../.agents/skills/resend-cli/): `resend/resend-skills`, Resend from the terminal
- [email-best-practices](../../.agents/skills/email-best-practices/): `resend/resend-skills`, authentication and compliance
- [react-email](../../.agents/skills/react-email/): `resend/resend-skills`, HTML email templates
- [agent-email-inbox](../../.agents/skills/agent-email-inbox/): `resend/resend-skills`, untrusted inbound mail

MCP servers: Neon (recommended), Upstash Redis (recommended)

## Related specs

- [0001](../../docs/specs/0001-scale-persistence/index.md)
- [0002](../../docs/specs/0002-identity-sessions/index.md)

_Drafted by /jsm-audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
