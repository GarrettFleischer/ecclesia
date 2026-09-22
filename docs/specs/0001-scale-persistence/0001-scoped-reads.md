# 0001. Scoped reads

## Summary

Home and the body stop loading every open need and every church. The SDK loads at most 20 rows the viewer might see. Domain still decides visibility. Lists paginate on `(created_at, id)`.

## Requirements

**User stories**:
- As a member, I want Home to show needs from my churches and neighbors without waiting on the whole body.
- As a pastor, I want the church page to list that church's needs, not every need.

**Acceptance criteria**:
- **AC-R1**: `home_needs` does not call today's `open_need_cards`. SQL uses the three clauses below, `ORDER BY created_at DESC, id DESC`, `LIMIT 40`. Domain then keeps at most 20 visible cards.
- **AC-R2**: `church_directory` does not `SELECT * FROM churches`. It returns at most 20 churches `ORDER BY city, name, id` after the cursor, with counts for those ids only. `group_churches_by_place` runs on that page of churches (a city may split across pages).
- **AC-R3**: `church_show` pages that church's needs and that church's members at 20 each, same cursor rules as Home for needs, `(name, id)` for members.
- **AC-R4**: Need lists use query `after` as `{created_at}|{id}` (URL encoded). Church lists use `{city}|{name}|{id}`. No offset. No base64.
- **AC-R5**: Domain `visible_need_cards` receives the viewer's churches plus the `Church` rows for every `church_id` on the SQL page (at most 40). A row Domain rejects is omitted. `next_cursor` is the last **visible** card when 20 visible cards were kept.
- **AC-R6**: A viewer with no active membership gets an empty Home need list from the SDK (no global open scan). Domain `NotInTheBody` still applies when they open a body scoped need by id.

## Decision

Scoped SQL in the SDK. Visible page size 20. SQL fetches up to 40 so Domain hide cannot empty the page. Domain visibility stays the authority.

Home SQL is a union of three clauses, all `status = 'open'`:
1. Needs on the viewer's active churches, any scope.
2. Needs on neighbor churches (same city or same region, not the same id, case insensitive) whose scope is `neighboring` or `body`. Not `church`.
3. Needs with scope `body` on any church.

That is the same rule as Domain `can_view_need` / `churches_are_neighbors` (basis: [crates/domain/src/rules.rs](../../../crates/domain/src/rules.rs)). Clause 1 plus 2 plus 3 replace the old fetch-all open-need query.

## Feature design

**Data model sketch**:
No new tables. Reads use indexes from [0001-postgres-skin.md](0001-postgres-skin.md).

**State transitions**: none.

**API surface**:

| Story | Inputs | Outputs | Auth | Errors |
|---|---|---|---|---|
| `home_needs` | viewer, optional `after` | up to 20 visible `NeedCard`, optional next cursor | session | empty page if not in a church |
| `church_directory` | optional `after` | up to 20 churches with counts, optional next cursor | session | none |
| `church_show` | church id, optional need `after`, optional member `after` | church, up to 20 members, up to 20 need cards | session | `NotFound` |
| `need_show` | need id | one card, offers Domain `visible_offers` already allows | session | Domain hide errors |

**Value sourcing**:

| Action | Value produced | Source |
|---|---|---|
| `home_needs` | Viewer church ids | `memberships` where `user_id` is the session user and status is active |
| `home_needs` | Neighbor church ids | `churches` whose `city` or `region` matches an active church of the viewer |
| `home_needs` | Need rows | Union of the three clauses above, after cursor, limit 40 |
| `home_needs` | Churches for visibility | Viewer's churches union `Church` rows for each `church_id` on that SQL page |
| `home_needs` | Visible cards | Domain `visible_need_cards` on the 40, keep 20 |
| `home_needs` | `next_cursor` | Last visible card's `created_at` and `id`, as `{created_at}|{id}`, when 20 visible cards were kept |
| `church_directory` | Church rows | `ORDER BY city, name, id`, after `{city}|{name}|{id}`, limit 20 |
| `church_directory` | Counts | `counts_for_churches` restricted to those 20 ids |
| `church_show` members | Member rows | That `church_id`, `ORDER BY name, id`, limit 20 |
| Page size | 20 visible | Decided in this spec. SQL over fetch is 40 on Home only. |

**Key invariants**:
- SQL may over fetch a row Domain will hide. SQL must not under fetch a row Domain would show on that page, except when the page is full.
- Body scoped needs (`NeedScope::Body`) appear on Home only if the viewer is active somewhere. The query includes them when the viewer's id set is non empty (they sit on some church_id). A body need on a far church still appears if status is open and the viewer is in the body. **That last case needs a second clause**: open needs with `scope = 'body'` are included for any active viewer, still limited to 20 in the same ordering.

Home query is the three clauses in Decision. Do not also load every open need on a neighbor church (that was the first draft of AC-R1 and would fill the 40 with hidden `church` scoped cards).

**Security model**:
Pending members are not active. They must not appear in the viewer's church id set. Offer message text still uses Domain `visible_offers`.

**Configuration required**:
None.

**Critical test scenarios**:
- Happy path: two churches in one city, a neighbor need, the member sees it on Home without a global scan. Verifies **AC-R1**, **AC-R5**
- Happy path: 21 visible open needs on one church, Home shows 20 and `after` of the 20th; the next page shows the 21st. Verifies **AC-R4**, **AC-R5**
- Failure: a pending member's Home need list is empty. Verifies **AC-R6**
- Auth: a far church scoped need is not in the SQL id set and Domain would hide it anyway. Verifies **AC-R1**, **AC-R5**

## Build plan

1. Replace `open_need_cards` callers with `home_needs` and the union above. Satisfies **AC-R1**, **AC-R5**, **AC-R6**
2. Page `church_directory` (city, name, id) and `church_show` needs and members. Satisfies **AC-R2**, **AC-R3**, **AC-R4**
3. Add HTTP query `after` with the encodings above. Satisfies **AC-R4**
4. Keep Domain tests for `visible_need_cards`. Add an SDK test that the SQL does not `SELECT` all open needs. Satisfies **AC-R1**

## Consequences

**Positive**:
- Home cost tracks the viewer's neighborhood.

**Negative / tradeoffs**:
- A body scoped need far away still enters clause 3. That set can grow. Revisit only when that list is hot. Do not bring back fetch all.
- SQL and Domain can disagree on a row at the page boundary. Domain wins. The next page may skip a hidden row. That is acceptable.

**Neutral**:
- Inbox and `/me` stay keyed by `user_id` and are already bounded by one person.

## Rationale

Fetch all plus a Domain filter was honest and small, and it was the first wall. SQL now matches Domain scope so a wide region cannot fill the page with hidden church scoped cards. A read model written on every Effect was the heavier runner up. (basis: [crates/sdk/src/db/needs.rs](../../../crates/sdk/src/db/needs.rs), [crates/domain/src/rules.rs](../../../crates/domain/src/rules.rs))
