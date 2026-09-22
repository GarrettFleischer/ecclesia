# Ecclesia

The body of Christ. Members care for one another.

A church posts a need. People who can help say so. Nearby churches see those needs too. Pastors approve who joins. Anyone can endorse a skill they have seen, even one the person never claimed. The note stays private until they accept it.

## Run it

Nightly Rust (edition 2024). `rustup toolchain install nightly`.

```bash
cargo test
cargo run
```

Open [http://127.0.0.1:43781](http://127.0.0.1:43781). On a phone, add it to the home screen, then turn on alerts under You. Store builds live in `mobile/`. See [docs/MOBILE.md](docs/MOBILE.md).

For a stable local secret:

```bash
PORT=43781 \
DATABASE_URL=sqlite://ecclesia.db \
ECCLESIA_SECRET=$(openssl rand -hex 32) \
cargo run
```

Delete `ecclesia.db` (and `-wal` / `-shm`) to reset the store. An empty store seeds the gift catalog only.

For a shared host: set a long random `ECCLESIA_SECRET`, set `ECCLESIA_SECURE=1` behind TLS, and use your own VAPID keys. See [docs/SECURITY.md](docs/SECURITY.md) and [docs/MOBILE.md](docs/MOBILE.md).

To weigh words with Jev, set `ECCLESIA_JEV_KEY`. For hosted rewrite and
classify while testing, set `ECCLESIA_OPENROUTER_KEY` (Free Models Router,
`openrouter/free`). On a 4080 Super, set `ECCLESIA_LLM_URL` instead. See
[docs/VOICE.md](docs/VOICE.md). Without those, a word gate still stops
obvious attacks. First submit shows the rewrite. Then you publish.

## Try it

1. Register on the landing page.
2. Plant a church or redeem an invite.
3. Post a need, apply, or endorse from another registered member.

## Docs

- How to write copy: [docs/PROSE.md](docs/PROSE.md)
- Classification and rewrite: [docs/VOICE.md](docs/VOICE.md)
- User stories: [docs/SPEC.md](docs/SPEC.md)
- Domain and honest functions: [docs/DOMAIN.md](docs/DOMAIN.md)
- Domain and SDK: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Sessions and CSRF: [docs/SECURITY.md](docs/SECURITY.md)
- iOS and Android: [docs/MOBILE.md](docs/MOBILE.md)
