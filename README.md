# Ecclesia

A church is a household. The ecclesia is the households, together.

Ecclesia is a **phone-first web app** (installable as a standalone page) for churches that refuse to be islands. Each church is a group. People **request** or receive an **invite**; a pastor or steward **approves**. Members name **gifts**. Needs can stay in the household, open to the **valley**, or go to the **whole body**. People **apply** to help. Endorsements are **proposals** — the named person is notified and must accept.

The Cedar Falls / Waterloo seed is still the way to walk it.

## Honest structure

Rules are **leaves**: pure functions that take values and return an `Effect`. The **SDK skin** loads those values from SQLite and cookies, then writes the effect. HTTP does not invent a second set of household laws.

```
src/leaf/    auth, membership, needs, gifts, rules, flags
src/sdk/     clock, HMAC session, memory world
src/db/      schema, seed, reads, apply(effect)
src/http/    Axum skin (auth, churches, needs, people)
src/views/   Maud pages; loops live in cards.rs
```

- Specs and user stories: [docs/SPEC.md](docs/SPEC.md)
- Leaf / SDK split: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Sessions, CSRF, demo seats: [docs/SECURITY.md](docs/SECURITY.md)

## Run it

Rust 1.83+.

```bash
cargo test
cargo run
```

Open [http://127.0.0.1:43781](http://127.0.0.1:43781).

```bash
PORT=43781 \
DATABASE_URL=sqlite://ecclesia.db \
ECCLESIA_SECRET=dev-only-change-me \
ECCLESIA_DEMO=1 \
cargo run
```

Delete `ecclesia.db` (and `-wal` / `-shm`) to reset the valley.

For anything shared: `ECCLESIA_DEMO=0` and a long random `ECCLESIA_SECRET`. Demo seats let anyone become Miriam.

## Walk the valley

1. **Peter Lang** — still in the doorway at Grace Covenant.
2. **Miriam Cole** — approve Peter. See what she opened to neighbors.
3. **Ruth Alvarez** — Inbox: James endorsed her hospitality.
4. **Elena Vasquez** — New Mercy. She can see Grace’s neighboring interpreter need and apply.
5. **James Whitaker** — the ramp need is open to the valley.

Invite codes: `grace-k2m9`, `luke-p4r1`, `mercy-n8q2`.
