# 0001. Outbox worker

## Summary

The SDK writes outbox rows in the same transaction as the Effect. A second App binary, `ecclesia-worker`, claims those rows with `FOR UPDATE SKIP LOCKED` and talks to phones. A bad endpoint is retried, then marked dead after 8 attempts. Inbox notice rows stay in the request transaction.

## Requirements

**User stories**:
- As a member, I want the page to finish after I post, even if a phone is asleep.
- As an operator, I want a stuck push to show up as outbox lag, not as a hung request.

**Acceptance criteria**:
- **AC-O1**: Table `outbox` exists in both dialects with columns `id`, `kind`, `payload`, `attempts`, `available_at`, `status`, `dead_at`. Status is `pending`, `working`, `done`, or `dead`.
- **AC-O2**: Commit inserts one outbox row per notice that should reach a phone (same fan out set today's `PushHub::dispatch` uses). Notice rows in `notifications` are written in that same transaction. The request does not call Web Push.
- **AC-O3**: Kind `push` is implemented. Kind `mail` may be stored later. No kind runs a model or rewrite.
- **AC-O4**: `ecclesia-worker` is an App binary that calls the SDK poll loop. It does not open HTTP routes.
- **AC-O5**: On Postgres a claimed row uses `FOR UPDATE SKIP LOCKED`. On SQLite the worker opens `BEGIN IMMEDIATE` and selects `pending` rows with `available_at <= now` (one local worker only; no skip locked). Backoff grows with `attempts`. After 8 failures `status` is `dead` and `dead_at` is set. A row left `working` for 15 minutes returns to `pending` (lease). The worker continues with the next row.
- **AC-O6**: A failed push does not roll back the Effect. That is already true today.
- **AC-O7**: Tracing records claim, success, retry, and dead, plus a metric `ecclesia_outbox_lag_seconds` (age of the oldest `pending` or `working` row). Fly runs two processes in `fly.toml`: `ecclesia` and `ecclesia-worker`, region `iad`.

## Decision

Postgres (and SQLite locally) outbox. Same process must not send push on the request. Worker binary on Fly. Eight attempts, then dead.

**Row shape**:
- `id` TEXT primary key (SDK clock)
- `kind` TEXT (`push`, later `mail`)
- `payload` TEXT (JSON: `user_id`, `title`, `body`, `href`, notice id)
- `attempts` INTEGER default 0
- `available_at` TEXT ISO, when the worker may claim
- `status` TEXT
- `dead_at` TEXT null until dead

**Backoff** (decided here): 10s, 30s, 2m, 10m, 30m, 2h, 6h, then dead on the 8th failure. `available_at` is now plus that delay.

**Leadership**: on Postgres any worker replica may claim a row (`SKIP LOCKED`). On SQLite run one worker. A `working` row older than 15 minutes is treated as `pending` again. Delivery is at least once: a crash after send and before `done` may push twice. Payload includes the notice id so a later spec can dedupe.

## Feature design

**Data model sketch**:

| Entity | Fields | Notes |
|---|---|---|
| `outbox` | `id` PK, `kind` not null, `payload` not null, `attempts` not null, `available_at` not null, `status` not null, `dead_at` null | Index on `available_at` where status is `pending` |
| `notifications` | unchanged | Written in the request transaction |
| `push_subscriptions` / `push_devices` | unchanged | Worker reads them by `user_id` |

**State transitions**:
`pending` → `working` → `done`
`pending` → `working` → `pending` (retry, `available_at` in the future)
`pending` → `working` → `dead` (8th failure)

**API surface**:

| Action | Inputs | Outputs | Auth | Errors |
|---|---|---|---|---|
| SDK commit | Effect | void | internal | store error rolls back writes, notices, and outbox |
| SDK `claim_outbox` | now, batch 10 | rows | worker | none |
| SDK `finish_outbox` | id, success or fail | void | worker | none |
| Worker loop | poll 1s | void | Fly process | logs and continues |

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| Outbox `id` | New id | SDK clock `new_id` |
| Outbox `payload` | JSON | `NoticeDraft` fields plus the notice row id |
| Fan out targets | Endpoints and device tokens | `push_subscriptions` and `push_devices` for `NoticeDraft.user_id` |
| `attempts` | Integer | Column, increment on fail |
| Dead after | 8 | Decided in this spec |
| Lag | Age of oldest pending or working row | `now - min(available_at)` for those statuses, exported as `ecclesia_outbox_lag_seconds` |
| Fly topology | Two processes | `fly.toml` processes `app` (`ecclesia`) and `worker` (`ecclesia-worker`), region `iad` |
| SQLite claim | Locked rows | `BEGIN IMMEDIATE` then select `pending`. One worker on a laptop. |
| Word gate | Not on the worker | Request path only, [0001-lib-layers.md](0001-lib-layers.md) |

**Key invariants**:
- Inserting the Effect and the outbox rows is one transaction.
- Worker never writes Domain Effects.
- Worker never weighs words.

**Security model**:
Worker has the database URL, VAPID keys, and optional FCM key. It has no cookie jar. It must not expose an HTTP port. Payload holds notice text already stored in `notifications`.

**Configuration required**:
- `DATABASE_URL`: same as the App
- Existing VAPID / FCM env from [docs/MOBILE.md](../../MOBILE.md)
- Fly: `fly.toml` with processes `app` and `worker`, region `iad`

**Critical test scenarios**:
- Happy path: post a need, notice row exists, outbox row `pending`, request returns before any HTTP to a push endpoint. Verifies **AC-O2**, **AC-O6**
- Happy path: worker claims the row, send succeeds, status `done`. Verifies **AC-O4**, **AC-O5**
- Failure: send fails 8 times, status `dead`, next row still claimed. Verifies **AC-O5**
- Failure: commit fails after writes are staged, no outbox row remains (transaction). Verifies **AC-O2**

## Build plan

1. Add `outbox` to the SDK schema and insert rows inside apply. Remove push from `AppState::commit`. Satisfies **AC-O1**, **AC-O2**, **AC-O6**
2. Implement claim, backoff, and dead in the SDK. Satisfies **AC-O5**
3. Add `crates/app/src/bin/worker.rs` that loops `claim_outbox` and `PushHub`. Satisfies **AC-O3**, **AC-O4**
4. Emit lag and traces. Satisfies **AC-O7**
5. SQLite tests cover the state machine so CI does not need Neon. Satisfies **AC-O1**, **AC-O5**

## Consequences

**Positive**:
- Post stays fast when a phone vendor is slow.
- A poison endpoint does not block the queue.

**Negative / tradeoffs**:
- Two Fly processes to deploy and watch.
- Dead rows need a person to look. No admin UI in this spec.

**Neutral**:
- apalis was the runner up. Skip it until the table is hot. (basis: [apalis](https://docs.rs/apalis/latest/apalis/))
- Outbox `mail` is defined in [0002](../0002-identity-sessions/0002-mail.md). Push stays in this child.

## Rationale

The engineer asked for a transactional outbox and a worker binary, and later said the worker should not run LLM work. Push was the first outbound job; mail joined in spec 0002. Putting notice rows on the worker would make Inbox lag when the worker is down, so those rows stay in the request transaction. (basis: [crates/sdk/src/db/apply.rs](../../../crates/sdk/src/db/apply.rs), database backed queue first)
