# Verify: Hybrid access and refresh tokens · spec 0003 · 2026-09-30
_Steps derived from spec 0003 acceptance criteria. `/jsm-check verify` runs these; `/jsm-test` locks the durable ones._

## Commands
- [x] `cargo test -p ecclesia-sdk bearer::` access round trip and dotted session id reject → AC-3
- [x] `cargo test -p ecclesia-sdk us_api_` mint, cookie isolation, parallel refresh → AC-3, AC-4, AC-7
- [x] `cargo test -p ecclesia --test flows us_api_01` JSON sign in, me, refresh, stale refresh, logout all kills bearer, cookie cannot call /api/me → AC-2, AC-4, AC-5, AC-6, AC-7
- [x] `cargo test -p ecclesia --test flows us_auth_01_register_then_sign_in` cookie path unchanged → AC-7
- [x] `rg "transport" crates/sdk/src/db/schema.rs` shows migration and create table columns → AC-1
- [x] `rg "/api" crates/app/src/http` shows nested api router with four routes → AC-2, AC-4, AC-5, AC-6

## UI / manual
- [x] Not required this slice (JSON clients only; HTML unchanged).

## Acceptance-criteria coverage
- AC-1 schema · AC-2 sign in JSON · AC-3 bearer validate · AC-4 refresh rotate · AC-5 GET /api/me · AC-6 revoke parity · AC-7 cookie unchanged · AC-8 device meta and daily touch (SDK resolve_bearer + flows me)
