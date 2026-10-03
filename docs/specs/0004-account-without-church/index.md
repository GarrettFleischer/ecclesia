# 0004. Create account without a church

**Date**: 2026-10-02
**Status**: In Progress

## Summary

Create account asks for a first name, a last name, an email, and a password. It does not ask for a church. A person has no address. The church building does: a multiline postal address plus latitude and longitude. Onboarding is two steps. You comes first. Church is next, unless they opened an invite link, in which case that church is already chosen and they only fill in who they are. After the account exists with no church, they can join one. If they allow location, the app suggests the closest church. Otherwise they search by name or scan a QR code.

## Requirements

**User stories**:
- As a new person, I want to create an account without picking a church so I can sign in first.
- As a signed in person, I want my profile to show my first name, last name, about you, and the church I have joined or am waiting on.
- As a builder, I want the form limits and the domain checks to agree so a bad field fails in the browser and again on the server.

**Acceptance criteria**:
- **AC-1**: `POST /register` accepts `csrf`, `first_name`, `last_name`, `email`, `password`, and an optional `code`. Without a code, success creates the user, stores the password hash, mints a cookie session, and redirects to `/churches/join` with flash `welcome`. It inserts no membership. With a code that matches a church, the same post joins that church and redirects to `/churches/{id}` with flash `redeemed`. An unknown code redisplays You and does not create a user.
- **AC-2**: The create account page is the You step: first name, last name, email, and password. It has no church search and no address. A known `code` shows that church, and the button joins it, with no later church step. Without a code, the button is Find your church and the next page is the church step. `GET /register/churches` is removed. A signed in `GET /register` with no code redirects to `/churches/join` when they have no church, and to `/home` when they do. A signed in open of `GET /join/{code}` joins that church.
- **AC-3**: Blank first name, blank last name, blank email, invalid email, and blank password are `InvalidInput` or `InvalidEmail` as today (`missing` or `bad_email`) and redirect to `/register` with that flash. Email taken redirects to `/register` with flash `email`. A bad CSRF token uses the existing CSRF failure on `/register`. A weak password (`password`) redisplays the form and keeps first name, last name, and email. Other domain errors redirect to `/register` with their flash and drop the draft. Rate limit is unchanged.
- **AC-4**: `users` drops `name`, `city`, and `region`. It gains `first_name` and `last_name`, each required trimmed text, max 80. There is no address column on `users`. No backfill. Local data may be wiped with the schema change because there are no live accounts.
- **AC-5**: A signed in person with no church can only join one. Home, You, the church list, Nearby, Inbox, and needs redirect to `/churches/join`. That page has no dock and no links into the rest of the app. Sign out stays. Signing in with no church lands there too. After they ask to join, or join with a code, the browser opens that church. Home does not show a city or region line. Once they have a church, pending memberships still render in the Waiting section.
- **AC-6**: Profile edits first name, last name, and about you (`bio`). It does not edit a city, a region, or an address. It shows the person's one church: the church name, plus "Waiting" when that membership is pending. No membership shows the same no church line as Home. The page links to `/churches/join` so they can pick a different church (a move, or a first church). Save of the name and about you redirects to `/me` with flash `saved`. The voice review path still redisplays the page before save. `GET /api/me` returns `first_name`, `last_name`, `email`, `bio`, and `church_id` (null when they have no membership). It does not return `city` or `region`.
- **AC-10**: A person has at most one membership row. The database enforces that with a unique constraint on `memberships.user_id`. Joining a church replaces that row. A QR join sets the new church active and drops the previous church in the same transaction. A name search sets the new church pending and drops the previous church in the same transaction. They are not a member of two churches at once, including while they wait.
- **AC-7**: Browser checks and domain checks use the same limits: first and last name required, maxlength 80; email required, type email, maxlength 120; password required. The domain remains the authority. Zod is not added. It has no Rust bindings, so it cannot be the check on this server.
- **AC-8**: `churches` drop `city` and `region`. They gain `address` (multiline text, required, max 400), `latitude` (degrees, required, -90 through 90), and `longitude` (degrees, required, -180 through 180). There are no live churches to preserve. Neighborhood matching uses these coordinates, not a city string.
- **AC-9**: `/churches/join` is signed in. If the browser sends coordinates from a granted Geolocation permission, the page shows one suggestion: the findable church with the smallest haversine distance to that point. If permission is missing, denied, or there is no church, the page does not invent a suggestion. The same page offers search by church name. A QR icon beside the search box opens the camera, and a code it reads joins that church. Name search and the scan do not require location.

## Decision

**Chosen option**: Option 2 in [rationale.md](rationale.md): name and password only, place fields leave the person.

**Implementation skills**: `axum` (`melonask/axum-skills`, `.agents/skills/axum/`) · `maud` (`uwuclxdy/agenticat`, `.agents/skills/maud/`) · `sqlx` (`melonask/sqlx-skills`, `.agents/skills/sqlx/`) · `rust-auth` (`huiali/rust-skills`, `.agents/skills/rust-auth/`)

## Rationale

See [rationale.md](rationale.md).

## Feature design

**Data model sketch**:

`users` (alter, destructive ok):

| Column | Notes |
|--------|--------|
| `first_name` | TEXT NOT NULL, trimmed, max 80 |
| `last_name` | TEXT NOT NULL, trimmed, max 80 |
| `name` | dropped |
| `city` | dropped |
| `region` | dropped |
| `address` | not added |

`email`, `bio`, and `created_at` stay.

`churches` (alter, destructive ok):

| Column | Notes |
|--------|--------|
| `address` | TEXT NOT NULL. Multiline postal address of the building. Max 400. |
| `latitude` | REAL NOT NULL. WGS84 degrees. |
| `longitude` | REAL NOT NULL. WGS84 degrees. |
| `city` | dropped |
| `region` | dropped |

A person never stores this address or these coordinates. The device location used for the suggestion is not written on the user.

**State transitions**:

Person: none → registered (user, password hash, session, zero memberships) → one church (active, or pending). A new join replaces that membership. There is no second row.

**API surface**:

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/register` | GET | optional `code` | HTML: You. A known code shows that church and skips the church step | guest | no church and no code → 303 `/churches/join`; in a church and no code → 303 `/home`; unknown code → flash `invite` |
| `/register` | POST | `csrf`, `first_name`, `last_name`, `email`, `password`, optional `code` | 303 `/churches/join` flash `welcome`, or `/churches/{id}` flash `redeemed` when `code` matches | guest + CSRF | `missing`, `bad_email`, `email`, `password` (redisplay), unknown code redisplays with `invite` and no user, `rate`, CSRF fail |
| `/join/{code}` | GET | invite code | guest 303 `/register?code=`; signed in 303 `/churches/{id}` | either | unknown code: guest sees You with `invite`; signed in sees `invite` on their home |
| `/churches/join` | GET | optional `lat` and `lng` query numbers | HTML: closest church, or name search and QR | signed in | missing permission: no suggestion; bad coordinates: `missing` |
| `/me` | POST | `csrf`, `first_name`, `last_name`, `bio`, voice pass | 303 `/me` flash `saved` | signed in + CSRF | `missing`; voice review redisplays |
| `/api/me` | GET | bearer | `id`, `first_name`, `last_name`, `email`, `bio` | access bearer | 401 |

`GET /register/churches` is deleted. No JSON register route.

**Value sourcing**:

| Action | Value produced / displayed | Source |
|---|---|---|
| Register | `first_name`, `last_name` | Form, domain trim, required, max 80 |
| Register | `email` | Form, `normalize_email` |
| Register | `bio` | Empty string |
| Register | `id`, `created_at` | SDK `new_id`, `now_iso` |
| Register | password hash | SDK score and hash, unchanged |
| Register | session cookie | Existing session extras |
| Register | membership | Not created |
| Home place line | none | Person has no place columns |
| Closest church | one church name | Browser Geolocation `latitude` and `longitude`, compared with `churches.latitude` and `churches.longitude` by haversine. Not stored on the user. |
| Church building place | address shown with the suggestion | `churches.address` |
| Name search and QR | the other ways to join | Typed name, or the church QR payload. Neither reads device location. |
| Home no church line | copy already on Home | `viewer` has no active membership and the pending list is empty |
| Profile church line | one church name, and Waiting when pending | The single membership row for this user, joined to `churches.name` |
| Profile change church | link to the join step | `/churches/join`, same closest, name, and QR choices |
| Join | the one membership | New church id from the suggestion, the name search, or the QR payload. Previous membership row is deleted in that same write. |
| Profile save | `first_name`, `last_name`, `bio` | Form. That save does not write a membership. |
| `/api/me` | `first_name`, `last_name`, `email`, `bio` | `users` columns |

**Key invariants**:
- A successful register inserts a user and a session and does not insert a membership.
- `users` has no city, region, address, latitude, or longitude.
- Every church has an address, a latitude, and a longitude. Closest means smallest haversine distance. A missing device location does not pick a church.
- First name and last name are non empty and at most 80 characters. The domain rejects violations. The form uses the same maxlength and required flags.
- `memberships.user_id` is unique. A person has one church or none.
- Replacing a church deletes the old membership and inserts the new one in one transaction.

**Security model**:
- Register stays guest only, CSRF checked, `RateKind::Register` unchanged.
- Profile and `/api/me` stay limited to the signed in person.
- Removing `GET /register/churches` stops a guest lookup of church names from the account form.

**Configuration required**: none.

**Critical test scenarios**:
- Happy path: register with first name, last name, email, and password, then stay on `/churches/join` with no dock, verifies **AC-1**, **AC-4**, **AC-5**
- Invite link: `GET /join/{code}` shows that church on You, and creating the account opens the church, verifies **AC-1**, **AC-2**
- Failure case: blank last name redirects with `missing` and creates no user; a weak password redisplays and keeps the names and email, verifies **AC-3**, **AC-7**
- Auth/permission: `GET /register/churches` is gone; a signed in person with no church opening `/register` goes to `/churches/join`, verifies **AC-2**
- Profile: save changes first name and about you, shows the one church, and links to `/churches/join`, verifies **AC-6**
- One church: joining a second church leaves one membership row, on the new church, verifies **AC-10**
- Closest church: granted coordinates show the nearest church by haversine; denied permission shows name search and QR only, verifies **AC-8**, **AC-9**

## Build plan

- [x] Replace `users.name`, `users.city`, and `users.region` with `first_name` and `last_name` in the shared schema, satisfies **AC-4**
- [x] Domain register and profile use first and last name and stop requiring a person place, satisfies **AC-1**, **AC-3**, **AC-4**, **AC-7**
- [x] SDK `register` drops the church load and the join call, satisfies **AC-1**
- [x] Create account form and handler drop church fields; delete `GET /register/churches`; match maxlength and required to the domain, satisfies **AC-2**, **AC-3**, **AC-7**
- [x] Home drops the place eyebrow; profile edits names and about you, shows the one church, and links to `/churches/join`; `/api/me` returns the new fields, satisfies **AC-5**, **AC-6**, **AC-10**
- [x] Church rows store address, latitude, and longitude, and drop city and region. `/churches/join` suggests the closest church only when the device sends coordinates, and otherwise offers name search and a QR code, satisfies **AC-8**, **AC-9**

## Consequences

**Positive**:
- A person can sign in before any church exists for them.
- Place data is not stored on the person.

**Negative / tradeoffs**:
- Anything that read `users.city` or `users.region` must stop. Church create no longer prefills from the person.
- Neighborhood matching that compared city strings now uses church coordinates.
- The browser check is a convenience. A client that skips it still hits the domain.

**Neutral**:
- Zod is a TypeScript library. It does not run in Rust. HTML `required`, `maxlength`, and `type="email"` mirror the domain limits. The domain stays the source of truth.
- Cookie session mint on register stays as in spec 0002.
- Spec 0003's `/api/me` body changes: `city` and `region` leave, `name` becomes `first_name` and `last_name`.

## Follow-up

- Join a church still owns how a name search waits for a pastor and how a QR code joins immediately. This spec decides the church location and the closest suggestion on `/churches/join`.
- Start a church still owns pastor review and the government ids. The building address it collects is the `churches.address` column defined here.
