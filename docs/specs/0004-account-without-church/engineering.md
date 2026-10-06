# Engineering: create account without a church

Spec: [index.md](index.md). Product rules stay there. This file says how this repo implements them.

Nearby church needs use **40 km**. That cutoff is the number you picked for this plan. The join page still suggests only the single nearest church, with no cutoff.

## Inherits

- Domain decides. SDK loads rows and commits one transaction. App paints HTML and calls one story. (`AGENTS.md`)
- No `bool` arguments. Pending join and QR join are two functions.
- Schema changes live in `crates/sdk/src/db/schema.rs` `migrate`, the same place as `add_session_api_columns`.
- Forms stay Maud. Limits on the inputs match the domain maxima. No Zod.
- Flashes stay the allow list in `crates/app/src/views/flash.rs`.

## Decisions

**Person name.** `User` gains `first_name` and `last_name` and loses `name`, `city`, and `region`. One domain function `display_name(first, last)` joins them with a single space for notices and headings. Call sites do not format names themselves. That keeps one rule (maintainability).

**Church place.** `Church` gains `address`, `latitude`, and `longitude` (`f64`) and loses `city` and `region`. Domain rejects a blank address, an address over 400 characters, and coordinates outside the ranges in AC-8. Church create and seed write those three fields. There is no prefill from the person, because the person has no place.

**Distance.** Domain owns `nearby_km() -> f64` (returns 40) and `distance_km(a_lat, a_lng, b_lat, b_lng) -> f64` (haversine). The SDK does not reimplement the formula in a second language. The nearby needs query and the closest church query both interpolate `nearby_km()` only where a cutoff is required. Closest church is `ORDER BY` that same expression `LIMIT 1`. Computing in SQL keeps the database from shipping every church into the process (performance). The pure function is what tests assert (developer experience).

**Many churches, one row each.** Membership is `memberships (user_id, church_id)`, not columns on `users`. A person needs at least one live row to leave Find your church. They may hold several at once, including pending at one church and active at another. Register still inserts none.

`Write::UpsertMembership { user_id, church_id, status, role }` inserts or updates that pair and clears `deleted_at`. `Write::DeleteMembership { user_id, church_id }` sets `deleted_at` on that pair. Live reads use `memberships_live`.

`Viewer.churches` holds every church on those rows. Home, nearby needs, and the prayer deck treat every active membership as a home church.

Two domain functions, names kept:

- `replace_with_pending` for a name or suggestion join. Adds a pending member row. Other churches stay. Same church and already related returns `AlreadyMember`.
- `replace_with_code` for a QR or typed invite code. Adds an active member row. Other churches stay. Unknown code is `NotFound` at the SDK, before domain, because the code lookup is a row read.

Notices for a pending join stay on the domain effect, as `request_join` does today. A code join sends no join request notice.

**Join HTTP.** Spec table lists `GET /churches/join`. The write in AC-10 is two posts so the handler does not take a mode flag:

- `POST /churches/join` with `church_id` calls `replace_with_pending`.
- `POST /churches/join/code` with `invite_code` calls `replace_with_code`.

Both are signed in and CSRF checked. Success opens `/churches/{id}` for the church they joined. A failure stays on `/churches/join`.

**Device location.** A small script `crates/app/static/join.js` runs only on that page. If `lat` and `lng` are absent, it calls `navigator.geolocation` once. On grant it navigates to the same path with those query values. On deny it does nothing. The server never stores them. Bad numbers use flash `missing` and render the page with no suggestion.

**QR.** The QR payload is the existing `invite_code` string, or a `/join/{code}` link. On the finder, a QR icon beside the church name field opens the camera. `BarcodeDetector` reads it and the browser opens `/join/{code}`. No typed code field. No new crate.

**Schema reshape.** `CREATE TABLE IF NOT EXISTS` will not drop old columns. `migrate` keeps the new `CREATE` text for empty databases. `reshape_account_place` runs only when `users` still has `name`: it drops `memberships`, `users`, and `churches`, then recreates them, including `memberships`. Databases that already moved the link onto `users.church_id` keep their rows. `move_church_columns_to_memberships` copies those columns into `memberships`, then drops `church_id`, `church_status`, and `church_role`. Fresh databases skip both steps.

## Build plan

### 1. Schema columns (AC-4, and the church half of AC-8)

Files: `crates/sdk/src/db/schema.rs`, `crates/sdk/src/db/seed.rs`, `crates/sdk/src/db/rows.rs`, `crates/sdk/src/db/apply.rs`.

New `users` columns: `first_name`, `last_name`. Drop `name`, `city`, `region`, and any `church_id`, `church_status`, `church_role`. `memberships` is the join, primary key `(user_id, church_id)`. New `churches` columns: `address`, `latitude`, `longitude`. Drop `city`, `region`. Member lists and home needs read `memberships_live`.

Seed insert lists match the new columns. One seed church gets a fixed address and fixed coordinates in `seed.rs`.

### 2. Domain names and place (AC-1, AC-3, AC-4, AC-7, AC-8)

Files: `crates/domain/src/person.rs`, `crates/domain/src/auth.rs`, `crates/domain/src/gifts.rs` (`update_profile`), `crates/domain/src/validate.rs`, `crates/domain/src/household.rs` (`Church`), `crates/domain/src/effect.rs` (`UpdateUser`), `crates/domain/src/membership.rs`.

`register` takes `first_name` and `last_name`, not city or region. Bio stays empty. `update_profile` takes `first_name`, `last_name`, and `bio`. `church_fields` takes address and coordinates. `NAME_MAX` stays 80. Address max is 400.

`UpdateUser` fields become `first_name`, `last_name`, `bio`.

### 3. SDK register (AC-1)

File: `crates/sdk/src/identity.rs` `register`.

Drop `church_id`. Stop loading a church. Stop calling `request_join`. Still score and hash the password, still mint the session extras. Pass `first_name` and `last_name` into domain `register`.

### 4. Create account form (AC-2, AC-3, AC-7)

Files: `crates/app/src/views/landing.rs`, `crates/app/src/views/draft.rs`, `crates/app/src/http/forms.rs`, `crates/app/src/http/auth.rs`, `crates/app/src/http/mod.rs`.

`RegisterForm` and `RegisterDraft` use `first_name` and `last_name`, plus an optional `code`. Remove `church_id` and `church_query`. Delete the `GET /register/churches` route and `register_church_search`. Inputs: required, maxlength 80 on both names, email maxlength 120, password required. The You page shows steps You and Church. A known code shows that church and the button joins it. Weak password redisplays the draft, including the code. Without a code, success redirects to `/churches/join` with flash `welcome`. With a code, success joins that church and opens it with flash `redeemed`. `GET /join/{code}` sends a guest to You with that church, and a signed in person straight into the church.

### 5. Home, profile, API (AC-5, AC-6, AC-10)

Files: `crates/app/src/views/home.rs`, `crates/app/src/views/people.rs`, `crates/app/src/views/layout.rs`, `crates/app/src/http/people.rs`, `crates/app/src/http/api.rs`.

Remove the city region eyebrow on Home and profile. `first_name()` in `layout.rs` that splits a full string goes away. Use `user.first_name`.

Profile form: first name, last name, about you. List every live church. A pending or invited row shows Waiting. Leave, transfer, and close name that church. Find another church goes to `/churches/join`. Zero memberships never render this block, because the gate already sent them to join.

`GET /api/me` JSON: `id`, `first_name`, `last_name`, `email`, `bio`, `memberships`. Each item is `church_id`, `status`, `role`. Drop `name`, `city`, `region`, and a single `church_id`.

Anywhere a card printed `church.city` and `church.region` (`crates/app/src/views/cards.rs` and church drafts) prints `church.address` instead.

### 6. Closest church and join (AC-8, AC-9, AC-10)

Files: `crates/domain/src/rules.rs` or a new `crates/domain/src/geo.rs` (distance only), `crates/domain/src/membership.rs`, `crates/sdk/src/db/needs.rs`, `crates/sdk/src/story.rs`, `crates/app/src/http/churches.rs`, `crates/app/src/views/churches.rs`, `crates/app/static/join.js`.

Needs query: replace the city or region `EXISTS` with haversine distance `<= nearby_km()` between any of the viewer's active churches and the need's church. Keep the existing page limit. Posting a need or a prayer offers each active church.

`GET /churches/join`: signed in. Parse optional `lat` and `lng`. In range: load the one nearest church that is not already this person's church, and show its name and address with a form that posts `church_id`. Out of range: flash `missing`, no suggestion. Missing params: no suggestion. Always show the name search (existing `search_churches`) and a QR icon beside that search box.

Name results post `church_id` to `POST /churches/join`. The code form posts to `POST /churches/join/code`.

SDK stories load the church and the person's memberships, call the matching domain function, and commit one upsert or delete of that pair.

Church create form (`ChurchDraft`, `crates/app/src/http/churches.rs`) asks for address (textarea, maxlength 400) and latitude and longitude. It does not ask for city or region.

## Tests

- Domain: `distance_km` is smaller for the nearer pair. `replace_with_pending` and `replace_with_code` each emit one `UpsertMembership` and no `DeleteMembership`. A second church keeps the first.
- SDK or app flow: `POST /register` without a church id creates a user with an empty `memberships` list, and the next location is `/churches/join`.
- App: `GET /register/churches` is not routed.
- Domain or SDK: two churches 10 km and 100 km apart, the needs query keeps the near one only.

## Open questions

None.
