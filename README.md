# Ecclesia

A church is a household. The ecclesia is the households, together.

Ecclesia is a **mobile-responsive web app** for churches that do not want to live as islands. Each church is a group. People **request** to join or receive an **invite**; a pastor or steward **approves**. Members name their **gifts**. Anyone in the right circle can **post a need**. People who can help **apply**. Members **endorse** one another for gifts; the person who was named is **notified** and can accept or decline.

The first slice is a Cedar Falls / Waterloo valley: three churches, real needs, a pending member, and an endorsement waiting in someone's inbox.

## What we chose, and why

This is a product with rules, not a brochure. Membership, visibility, and endorsements are easy to get subtly wrong. Those rules live in **Rust**, where they can be tested without a browser.

| Layer | Choice | Why |
| --- | --- | --- |
| Language | **Rust** | You asked for it, and the domain (who may see a need, who may approve a person) belongs in a typed core. |
| HTTP | **Axum** | One binary. Forms, cookies, and a JSON-shaped domain can grow into a native mobile client later without rewriting the household. |
| Data | **SQLite** via SQLx | Zero extra services for a church that wants to try this on a laptop. WAL mode. Postgres is a later move, not a first one. |
| UI | **Server-rendered HTML** (Maud) + CSS | Phones are the real device. A single origin, no Node runtime, no SPA auth maze. Native iOS/Android can wait until the body is real. |
| Auth (v1) | **Cookie session, no passwords** | Enough to walk every role in the demo. Real credentials come after the product is true. |
| Nearby | **Same city or region** | Good enough to prove churches are not islands. GPS can wait. |

What we did **not** do yet: OAuth, push notifications, a separate React app, or a workspace of unused crates. The same Axum API shape is the path to a later iOS/Android client (or a Dioxus/Tauri shell) without abandoning Rust.

## Run it

You need Rust 1.83+ (the version this repo was built with).

```bash
cargo test
cargo run
```

Then open [http://127.0.0.1:43781](http://127.0.0.1:43781).

Optional:

```bash
PORT=43781 DATABASE_URL=sqlite://ecclesia.db cargo run
```

The database file is created and seeded on first boot. Delete `ecclesia.db` (and `-wal` / `-shm`) to reset the valley.

## Walk the story

From the landing page, sit in someone else's seat:

1. **Peter Lang** — still in the doorway. He requested Grace Covenant.
2. **Miriam Cole** — pastor at Grace. Approve Peter. See the meal train and the needs she opened to neighbors.
3. **Ruth Alvarez** — open **Inbox**. James (from St. Luke's) endorsed her hospitality. Accept or decline.
4. **Elena Vasquez** — New Mercy in Waterloo. She can see Grace's *neighboring* interpreter need and apply.
5. **James Whitaker** — carpenter at St. Luke's. Daniel's ramp need is open to the valley.

Plant a church, invite by email, or redeem `grace-k2m9` / `luke-p4r1` / `mercy-n8q2`.

## Domain rules worth knowing

- **Request**: member asks; owner/steward approves.
- **Invite**: owner/steward invites (or shares a code); the person accepts.
- A need scoped to **this church** stays inside approved members.
- **Neighboring** is visible to approved members of churches in the same city or region.
- **The whole body** is visible to anyone already received into some household.
- An endorsement is a proposal. It is not on a profile until the receiver says yes. Accepting can add the gift if they had not named it yet.

`src/domain.rs` is the contract. The HTTP layer is not allowed to invent a second set of rules.
