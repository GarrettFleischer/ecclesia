# Ecclesia

Ask for help. Offer yours.

A church posts a need. People who can help say so. Nearby churches see those needs too. Pastors approve who joins. Anyone can endorse a skill they have seen, even one the person never claimed. The note stays private until they accept it.

## Run it

Rust 1.83+.

```bash
cargo test
cargo run
```

Open [http://127.0.0.1:43781](http://127.0.0.1:43781). On a phone, add it to the home screen, then turn on alerts under You. Store builds live in `mobile/`. See [docs/MOBILE.md](docs/MOBILE.md).

```bash
PORT=43781 \
DATABASE_URL=sqlite://ecclesia.db \
ECCLESIA_SECRET=dev-only-change-me \
ECCLESIA_DEMO=1 \
cargo run
```

Delete `ecclesia.db` (and `-wal` / `-shm`) to reset the demo.

For a shared host: `ECCLESIA_DEMO=0`, a long random `ECCLESIA_SECRET`, and your own VAPID keys. See [docs/MOBILE.md](docs/MOBILE.md).

## Walk the demo

1. **Peter Lang** — asked to join Grace Covenant.
2. **Miriam Cole** — approve Peter. See the needs she posted.
3. **Ruth Alvarez** — Inbox: James endorsed her hospitality. Accept it or decline. A declined note stays with the two of you.
4. **Daniel Okonkwo** — Inbox: Elena endorsed him for counseling. He never listed it.
5. **Elena Vasquez** — can see Grace's interpreter need and apply.
6. **James Whitaker** — the ramp need is open to nearby churches.

Invite codes: `grace-k2m9`, `luke-p4r1`, `mercy-n8q2`.

## Docs

- How to write copy: [docs/PROSE.md](docs/PROSE.md)
- User stories: [docs/SPEC.md](docs/SPEC.md)
- Leaves and honest functions: [docs/LEAVES.md](docs/LEAVES.md)
- Leaves and SDK: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Sessions and CSRF: [docs/SECURITY.md](docs/SECURITY.md)
- iOS and Android: [docs/MOBILE.md](docs/MOBILE.md)
