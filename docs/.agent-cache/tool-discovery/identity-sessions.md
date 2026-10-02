# Identity / sessions tool discovery
**Freshness:** 2026-09-22  
**CLI:** `npx skills find` / `npx skills add … --list` (skills npm 1.7.0 via npx; `--list` clones repo to temp, no install)

## Candidates (verified via skills.sh / --list where noted)
| Tech | Skill | Notes |
|------|-------|-------|
| Argon2 Rust | none dedicated | tangled `argon2-25` is **Python argon2-cffi** |
| zxcvbn Rust | none | |
| Resend | resend/resend-skills@resend (+ send-email, agent-email-inbox, email-best-practices) | official; 5 skills in repo |
| Rust auth/sessions | huiali/rust-skills@rust-auth | Argon2 + Axum middleware patterns |
| Rust auth | claude-dev-suite/claude-dev-suite@cryptography, @rust-security | generic crypto / Rust web security |
| Password anti-patterns | igbuend/grimbard@weak-password-hashing-anti-pattern | |
| sqlx/Neon/Axum auth | skip general DB/framework skills | neon-auth = Neon Auth product (JS), not Rust sessions |

## MCP
| Name | Access |
|------|--------|
| Resend hosted | Full Resend API (send, domains, webhooks, …) — https://mcp.resend.com/mcp OAuth or Bearer RESEND_API_KEY |
| resend-mcp (npm) | Same; local stdio/HTTP — `npx resend-mcp` |
| Smithery @resend/mcp-send-email | Resend send (API key config) |
| argon2 / zxcvbn / Rust session | none dedicated |

## Declines
none recorded (no root AGENTS.md)
