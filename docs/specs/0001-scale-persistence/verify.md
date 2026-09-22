# Verify: Scale persistence foundation · spec 0001 · updated 2026-09-21
_Steps derived from spec 0001 acceptance criteria. `/jsm-check verify` runs these; `/jsm-test` locks the durable ones._

## Commands
- [x] `cargo tree -p ecclesia --depth 1` shows `ecclesia-sdk` and does not list `ecclesia-domain` or `sqlx` as a direct edge → AC-1, AC-L2
- [x] `cargo tree -p ecclesia-domain --depth 1` shows neither `ecclesia-sdk` nor `ecclesia` → AC-1, AC-L3
- [x] `cargo test -p ecclesia-domain --lib` passes, including `us_domain_01` → AC-2, AC-L3, AC-L8
- [x] `DATABASE_URL=sqlite://ecclesia-verify.db cargo test --workspace` passes without a Postgres server → AC-4, AC-P1, AC-P5
- [x] `cargo test -p ecclesia-sdk --lib us_store_01` rewrites `?` to `$1` `$2` for Postgres → AC-P4
- [x] `cargo test -p ecclesia-sdk --lib us_store_02` refuses a missing or sqlite URL on a public host → AC-P1
- [x] `cargo test -p ecclesia-sdk --lib us_read_06` gives a pending member an empty Home page → AC-R6, AC-3
- [x] `cargo test -p ecclesia-sdk --lib us_cache_01` finds no `landing`, `home:`, or `inbox:` cache keys → AC-C1, AC-C2, AC-6
- [x] `cargo test -p ecclesia-sdk --lib us_cache_04` shares one register count across two RateGates → AC-C4, AC-5
- [x] `cargo test -p ecclesia-sdk --lib us_cache_03` deletes `directory` and `church:{id}` on InsertChurch → AC-C3
- [x] `cargo test -p ecclesia-sdk --lib us_outbox_02` writes a pending `push` row in the same apply as the notice → AC-O1, AC-O2, AC-7
- [x] `cargo test -p ecclesia-sdk --lib us_outbox_05` marks a row `dead` after 8 failures and still claims the next row → AC-O5
- [x] `rg "open_need_cards" crates` matches nothing in call sites → AC-R1
- [x] `rg "JudgeHub::load" crates/sdk/src/judge.rs` stays the word gate and ignores Jev / OpenRouter env → AC-8, AC-L9

## UI / manual
- [x] Sign in as an active member, open `/home`, see at most 20 need cards, and if more exist follow `More needs` with `?after={created_at}|{id}` to the next page → AC-3, AC-R1, AC-R4, AC-R5
- [x] Sign in as Peter (pending), open `/home`, see no open need list from a global scan → AC-R6
- [x] Open `/churches` and `/the-body`, each page shows at most 20 churches, `More churches` uses `{city}|{name}|{id}` → AC-3, AC-R2, AC-R4
- [x] Open a church page, needs page with `after`, members page with `members_after` at 20 each → AC-R3
- [x] Open `/` as a guest: landing paints, CSRF in the form is new each load, no store of landing HTML → AC-C2, AC-6
- [x] Post a need: the redirect returns before any phone vendor call; `notifications` has the row; `outbox` has a `pending` `push` row → AC-O2, AC-O6, AC-7
- [x] Post words the word gate refuses: tone flash, no store write → AC-8
- [x] With `ECCLESIA_PUBLIC=1` and no `DATABASE_URL`, `cargo run` refuses to boot → AC-P1
- [x] With `ECCLESIA_PUBLIC=1` and no `UPSTASH_REDIS_URL`, boot refuses → AC-C5
- [x] `ecclesia-worker` starts, claims a `pending` row, silent hub marks it `done`, no HTTP port → AC-O3, AC-O4, AC-O7

## Value sources
- [x] Home church ids come from `memberships` where the session user is `active` (pending ids stay out) → AC-R1, AC-R6
- [x] Neighbor needs come from churches that share city or region (case insensitive), scope `neighboring` or `body` only → AC-R1
- [x] Body scoped needs on any church appear only when the viewer is active somewhere → AC-R1
- [x] Domain `visible_need_cards` sees the viewer's churches plus Church rows for each `church_id` on the SQL page of at most 40 → AC-R5
- [x] `next_cursor` is the last **visible** card's `{created_at}|{id}` only when 20 visible cards were kept → AC-R4, AC-R5
- [x] Directory counts come from `counts_for_church_ids` for those 20 ids only → AC-R2
- [x] First directory page and `catalog` may come from Upstash JSON; a cursor page always hits the store → AC-C2
- [x] A cache error on `gift_catalog` still returns the store list → AC-C6
- [x] Rate `who` is the request client key (IP or session), key shape `rate:{kind}:{who}`, window 60s from first increment → AC-C4
- [x] Outbox payload is notice `user_id`, `title`, `body`, `href`, plus the notice row id → AC-O2
- [x] Driver comes from `DATABASE_URL` scheme; pool size 16 on Postgres, 8 on SQLite → AC-P1, AC-P2
- [x] Public host is `FLY_APP_NAME` set or `ECCLESIA_PUBLIC=1` → AC-P1, AC-C5

## Acceptance-criteria coverage
- AC-1 covered by cargo tree commands
- AC-2 covered by `us_domain_01`
- AC-3 covered by Home / churches / the-body UI steps and `us_read_06`
- AC-4 covered by sqlite workspace test and public host boot step
- AC-5 covered by `us_cache_04`
- AC-6 covered by `us_cache_01` and landing CSRF step
- AC-7 covered by `us_outbox_02` and post-a-need step
- AC-8 covered by word gate step and judge load command
- AC-R1 to AC-R6 covered by Home / directory / church page steps and value source checks
- AC-P1 to AC-P6 covered by dialect tests, sqlite test run, and SECURITY.md PITR note
- AC-C1 to AC-C6 covered by cache tests and landing step
- AC-O1 to AC-O7 covered by outbox tests, worker step, and `fly.toml`
- AC-L1 to AC-L9 covered by cargo tree, domain tests, and the crate split
