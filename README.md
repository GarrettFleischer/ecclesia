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

Delete `ecclesia.db` (and `-wal` / `-shm`) to reset the store. A new store seeds the gift catalog and **Grace Fellowship** so create-account search works. Each boot also ensures a dev governor at `admin@seed.test` with password `password` for join approvals on Grace Fellowship. `owner@seed.test` uses the same password. Restarting the app resets both seed passwords to `password`.

To weigh words with Jev, set `ECCLESIA_JEV_KEY`. For hosted rewrite and
classify while testing, set `ECCLESIA_OPENROUTER_KEY` (Free Models Router,
`openrouter/free`). On a 4080 Super, set `ECCLESIA_LLM_URL` instead. See
[docs/VOICE.md](docs/VOICE.md). Without those, a word gate still stops
obvious attacks. First submit shows the rewrite. Then you publish.

## Host it

Production is **Fly.io** (`iad`) for HTTP and a background worker, **Neon Postgres** (`us-east-1`) for the store, **Upstash Redis** for shared cache and rate limits, and **Resend** for mail from the outbox. `fly.toml` defines two processes: `ecclesia` (port 8080) and `ecclesia-worker` (outbox mail and Web Push).

Fly sets `FLY_APP_NAME`, which turns on public-host checks. You can mimic that locally with `ECCLESIA_PUBLIC=1` and the same env vars.

### 1. Neon (primary database)

1. Create a Neon project in **us-east-1** (same coast as Fly `iad`).
2. Turn on **point-in-time recovery** and keep a daily snapshot.
3. Copy the **pooled** connection string whose host includes `-pooler`. Use the **session** pooler, not the transaction pooler. Scheme must be `postgres` or `postgresql`.
4. Set it as `DATABASE_URL` on Fly (secret, below).

The app and worker run schema migration on boot. An empty database seeds the gift catalog. Grace Fellowship and `admin@seed.test` / `owner@seed.test` are inserted only on **local SQLite**, never on Postgres (including Neon production), even if `DATABASE_URL` points at a remote database from your laptop.

### 2. Upstash (Redis)

1. Create an Upstash Redis database.
2. Copy the `rediss://` URL.
3. Set `UPSTASH_REDIS_URL` on Fly.

Without Redis on a public host the app refuses to boot.

### 3. Resend (mail)

1. Add and verify your sending domain in Resend.
2. Create an API key.
3. Pick a From address on that domain, for example `Ecclesia <mail@yourdomain.com>`.

Set `RESEND_API_KEY`, `RESEND_FROM`, and `ECCLESIA_PUBLIC_URL` (your public origin, no trailing slash, for example `https://ecclesia.fly.dev`). Magic links, password reset, and invite mail use that origin.

### 4. Secrets and push keys

Generate a session signing key:

```bash
openssl rand -hex 32
```

Set `ECCLESIA_SECRET` to that value on Fly. Do not use `dev-only-change-me` in production.

TLS terminates at Fly. Set `ECCLESIA_SECURE=1` so session cookies get the `Secure` flag.

For Web Push on a shared host, use your own VAPID key (do not rely on the built-in dev key):

```bash
openssl ecparam -genkey -name prime256v1 -noout -out ecclesia-vapid.pem
```

Set `ECCLESIA_VAPID_PEM` to the PEM file path in a VM, or paste the PEM into the Fly secret (newlines as `\n` work). Optional: set `ECCLESIA_VAPID_PUBLIC` if you already have the uncompressed public point. Store builds that send through Firebase need `ECCLESIA_FCM_KEY`; see [docs/MOBILE.md](docs/MOBILE.md).

### 5. Fly deploy

Install the [Fly CLI](https://fly.io/docs/hands-on/install-flyctl/) and sign in.

The repo root is a Cargo **workspace** only (no `[package]`). Fly must use the root `Dockerfile` in `fly.toml`; do not rely on `fly launch` Rust autodetect.

Create the app once (name must match `app =` in `fly.toml`, or edit `fly.toml` first):

```bash
fly apps create ecclesia
```

Set secrets (replace placeholders). The worker process reads the same secrets as the app:

```bash
fly secrets set \
  DATABASE_URL='postgres://USER:PASSWORD@HOST-pooler.region.aws.neon.tech/DB?sslmode=require' \
  UPSTASH_REDIS_URL='rediss://default:TOKEN@HOST.upstash.io:6379' \
  RESEND_API_KEY='re_xxxxxxxx' \
  RESEND_FROM='Ecclesia <mail@yourdomain.com>' \
  ECCLESIA_PUBLIC_URL='https://ecclesia.fly.dev' \
  ECCLESIA_SECRET='paste_openssl_rand_hex_32' \
  ECCLESIA_SECURE=1 \
  ECCLESIA_VAPID_PEM='-----BEGIN EC PRIVATE KEY-----
...
-----END EC PRIVATE KEY-----'
```

Deploy (from the repo root, after secrets are set):

```bash
fly deploy
```

**Auto deploy:** Pushes to `main` run [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml): `cargo test --workspace`, then `flyctl deploy`. Add a GitHub repo secret **`FLY_API_TOKEN`** ([Fly personal access token](https://fly.io/user/personal_access_tokens)) with access to deploy app `ecclesia`. Fly’s remote builder reuses Docker layer cache by default; the root `Dockerfile` uses [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) so dependency crates stay cached when only source under `crates/` changes.

Skip `fly launch` if `fly.toml` already exists; it tries to parse the root `Cargo.toml` as a single crate and fails.

Confirm both processes are up:

```bash
fly status
fly logs -a ecclesia
```

`fly.toml` keeps at least one app machine running and runs the worker on its own VM. Scale or resize there if you need more headroom.

### 6. Custom domain (optional)

```bash
fly certs add yourdomain.com
```

Point DNS at Fly, then set `ECCLESIA_PUBLIC_URL` to `https://yourdomain.com` and redeploy or update the secret.

### 7. Required env on a public host

| Variable | Role |
| --- | --- |
| `DATABASE_URL` | Neon pooled Postgres URL |
| `UPSTASH_REDIS_URL` | Upstash `rediss://` URL |
| `RESEND_API_KEY` | Resend API key |
| `RESEND_FROM` | Verified From header |
| `ECCLESIA_PUBLIC_URL` | Origin for mail links |
| `ECCLESIA_SECRET` | HMAC key for `v2` session cookies |
| `ECCLESIA_SECURE` | `1` when the site is HTTPS |
| `ECCLESIA_VAPID_PEM` | Web Push signing key (strongly recommended) |

`PORT` is set by Fly to match `internal_port` (8080). More detail: [docs/SECURITY.md](docs/SECURITY.md), spec [0001](docs/specs/0001-scale-persistence/index.md), spec [0002](docs/specs/0002-identity-sessions/index.md), spec [0003](docs/specs/0003-hybrid-access-refresh/index.md) (JSON `POST /api/session`, refresh, and `GET /api/me` for native clients; HTML cookies unchanged). Copy and comments for all vars: [.env.example](.env.example).

### 8. Smoke test after deploy

1. Open `ECCLESIA_PUBLIC_URL` and create an account.
2. Request a magic link or reset and confirm mail arrives (Resend dashboard if it does not).
3. Post a need, approve a join as governor, turn on alerts on a phone (Add to Home Screen first on iOS).

Run the worker locally against production only if you know what you are doing; normally Fly runs `ecclesia-worker` for you:

```bash
cargo run -p ecclesia --bin ecclesia-worker
```

## Try it

1. Create an account. Search **Grace Fellowship**, pick it, submit.
2. Open **Sign in**, use `admin@seed.test` and `password`, then open Grace Fellowship to approve the join.
3. Plant another church or redeem an invite if you want a second household.
4. Post a need, apply, or endorse from another registered member.

## Docs

- How to write copy: [docs/PROSE.md](docs/PROSE.md)
- Classification and rewrite: [docs/VOICE.md](docs/VOICE.md)
- User stories: [docs/SPEC.md](docs/SPEC.md)
- Domain and honest functions: [docs/DOMAIN.md](docs/DOMAIN.md)
- Domain and SDK: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Sessions and CSRF: [docs/SECURITY.md](docs/SECURITY.md)
- iOS and Android: [docs/MOBILE.md](docs/MOBILE.md)
