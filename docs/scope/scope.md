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
| 3 | Hybrid access and refresh tokens | Slice 1 | done |
| 4 | Create account without a church | Slice 2 | in-progress |
| 5 | Join a church | Slice 2 | planned |
| 6 | Start a church | Slice 2 | planned |
| 7 | Prayer deck and public needs | Slice 3 | in-progress |

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

## Slice 1

### 3. Hybrid access and refresh tokens · done · GA
Short lived bearer access plus a revocable refresh credential for JSON clients. The Maud site and Capacitor WebView keep the `v2` cookie session until a client opts into tokens.
**Done when:** a client signs in over JSON, calls protected API routes with a short lived bearer, rotates with refresh, and logout or revoke matches today’s Me device semantics; cookie HTML auth is unchanged; stolen refresh dies on revoke, logout everywhere, and password change.
- [x] Design it (spec): `/jsm-architect hybrid access and refresh tokens`
- [x] Build it: `/jsm-develop hybrid access and refresh tokens`
   - [x] Schema and SDK tokens: transport, refresh hash, access and refresh mint (AC-1, AC-3, AC-4)
   - [x] JSON auth routes: sign in, refresh, logout (AC-2, AC-4, AC-6)
   - [x] Proof read: GET /api/me with bearer (AC-5, AC-8)
   - [x] Revocation parity with 0002 logout paths (AC-6)
   - [x] Integration tests: API path and cookie regression (AC-7)
- [x] Verify it: `/jsm-check verify hybrid access and refresh tokens`
- [x] Test it: `/jsm-test hybrid access and refresh tokens`
- [x] Review it (fresh model): `/jsm-check review hybrid access and refresh tokens`
- [x] Document it: `/jsm-document hybrid access and refresh tokens`
spec [0003](../specs/0003-hybrid-access-refresh/index.md)
code in crates/sdk, crates/app

## Slice 2

### 4. Create account without a church · in-progress
Create account asks for first name, last name, email, and password. Onboarding is You, then the church, unless an invite link already chose the church. The church building stores an address and coordinates. Until they have a church, Find your church is the only app page. They join the closest church if they allow location, or search by name or scan a QR code.
**Done when:** creating an account no longer asks for a church, a signed in person can join or switch church from profile, they have at most one church, and a granted device location suggests the nearest church.
- [x] Design it (spec): `/jsm-architect create account without a church`
- [x] Engineer it: `/jsm-engineer create account without a church`
- [x] Build it: `/jsm-develop create account without a church`
   - [x] Person name columns and register without a church (AC-1, AC-2, AC-3, AC-4, AC-7)
   - [x] Home, profile, and `/api/me` without a person place; profile can open join for the one church (AC-5, AC-6, AC-10)
   - [x] Church address and coordinates, and closest suggestion on `/churches/join` (AC-8, AC-9)
- [x] Verify it: `/jsm-check verify create account without a church`
- [x] Test it: `/jsm-test create account without a church`
spec [0004](../specs/0004-account-without-church/index.md)
code in crates/domain, crates/sdk, crates/app

### 5. Join a church · planned · needs a decision
After the account exists, `/churches/join` suggests the closest church when location is allowed (spec 0004). A church code makes them a member right away. A name search waits until a pastor accepts.
**Done when:** a signed in person can join from a church code immediately, or request to join from a name search sorted by device location, and only an accepted request makes them a member.
- [ ] Design it (spec): `/jsm-architect join a church`

### 6. Start a church · planned · needs a decision · GA
A signed in person can start a church in a short wizard. They give the United States employer identification number, the state charity or corporation number, and the facts that show they are the pastor. A reviewer checks that claim. Other people cannot find or join the church until the check passes.
**Done when:** a new church stays hidden until a reviewer accepts the pastor claim and both government numbers, and a refused claim never becomes a findable church.
- [ ] Design it (spec): `/jsm-architect start a church`

## Slice 3

### 7. Prayer deck and public needs · in-progress
Home stays the needs you can see from membership. Pray shows one open prayer at a time: your church first, then prayers within 40 km of a point the phone shares, each once per UTC day. Nearby lists open needs and open prayers in that same radius, including a church-scoped need when you are standing near that church. A prayer can carry your name or no name. An answered prayer leaves the deck and shows its praise report on the church page. A need is a public thread. Anyone who can see it can reply, and each name links to a profile.
**Done when:** the dock is Home, Pray, Nearby, Inbox, and You; Nearby is empty until a point is shared; a church-scoped need opens for a traveler within 40 km and stays hidden farther away; a marked prayer stays off the deck until the next UTC day; an unnamed prayer stores no author; and a reply is public on the need.
- [x] Title motion: dock and list headings rise, and a card title still morphs into its detail heading
- [x] Nearby feed from the shared point, with church pages and Your church left in place
- [x] Prayers, daily marks, unnamed manage token, praise report on the church page
- [x] Public need replies, with existing offer messages copied once
Stories are in [SPEC.md](../SPEC.md). This slice is separate from spec 0004.
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
