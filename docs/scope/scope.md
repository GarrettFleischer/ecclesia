# Scope: Ecclesia

A church posts a need. People who can help say so. Nearby churches see those needs too.

**Build approach:** Tracer Bullet (thin end to end slices through Domain, SDK, and App).
**Workflow:** Beta (`/jsm-check verify`, then `/jsm-test`).

_These are recommendations to keep your build orderly, not requirements._

## At a glance

| # | Feature | Phase | Status |
|---|---------|-------|--------|
| 1 | Scale persistence foundation | Foundation | done |
| 2 | Identity and sessions | Foundation | done |

## Foundations

### 1. Scale persistence foundation · done
Split Domain, SDK, and App so church rules stay pure. Scope Home and the body to a page of 20. Put the public store, cache, and push worker behind the SDK.
**Done when:** App depends on SDK only, Home never loads every open need, two machines share rate limits, and push leaves the request through the outbox.
- [x] Design it (spec): `/jsm-architect scale persistence foundation`
- [x] Build it: `/jsm-develop scale persistence foundation`
   - [x] Crate line: Domain, SDK, App, prelude, story functions (AC-1, AC-2, AC-L1 to AC-L9)
   - [x] Scoped reads and indexes on SQLite (AC-3, AC-R1 to AC-R6, AC-P3)
   - [x] Public store skin: dialect helper, Neon URL, public host checks (AC-4, AC-P1, AC-P2, AC-P4 to AC-P6)
   - [x] Fragment cache and shared rate keys (AC-5, AC-6, AC-C1 to AC-C6)
   - [x] Outbox, worker binary, push off the request (AC-7, AC-8, AC-O1 to AC-O7)
- [x] Verify it: `/jsm-check verify scale persistence foundation`
- [x] Test it: `/jsm-test scale persistence foundation`
spec [0001](../specs/0001-scale-persistence/index.md)
code in crates/domain, crates/sdk, crates/app

### 2. Identity and sessions · done · from spec 0001
Passwords and a shared session store so a stolen cookie is not a stolen seat once more than one machine is live.
**Done when:** a person can sign in without demo seats, and two App processes agree who is signed in.
- [x] Design it (spec): `/jsm-architect identity and sessions`
- [x] Build it: `/jsm-develop identity and sessions`
   - [x] Session rows and `v2` cookie (AC-2, AC-3)
   - [x] Passwords, register, sign in, remove demo and seed people (AC-1, AC-6)
   - [x] Devices, revoke, change password (AC-3)
   - [x] Mail tokens, Resend worker, invite mail, public env (AC-4, AC-5, AC-7, AC-8)
- [x] Verify it: `/jsm-check verify identity and sessions`
- [x] Test it: `/jsm-test identity and sessions`
spec [0002](../specs/0002-identity-sessions/index.md)
code in crates/domain, crates/sdk, crates/app

## Deferred

- **Hosting thickness**: TLS and Fly autosize. Region is already `iad` in spec 0001.
- **Topcoat**: wait until it ships jobs and Domain can survive fetch in a component.
- **Per account lockout**: spec 0002 rates by client key and by email. No lock field yet.
- **Confirm magic link**: switch GET consume to POST if mail scanners burn links.
- **Resend hosted MCP**: optional for the agent, not the app.
- **Passkeys and a second factor**: declined. Password, magic link, and reset stay the way in.

## Legend

**The decision box** is the sub task whose label ends with `(spec)`.
**Next step** is the first unticked box.
**Atomic build tasks** live in the spec child `## Build plan` files, not here.
