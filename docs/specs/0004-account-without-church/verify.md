# Verify: create account without a church · spec 0004 · updated 2026-10-06
_Steps derived from spec 0004 acceptance criteria. `/jsm-check verify` runs these; `/jsm-test` locks the durable ones._

**Result (2026-10-06): pass.** Onboarding is You, then the church. An invite link shows that church on You and joins it when the account is created. A signed in person who opens the link joins immediately. With no church, Find your church is the only app page. Asking to join opens that church and keeps any church they already have. Leaving one church while another remains stays on `/me`. Leaving or closing the last church returns to Find your church. `GET /api/me` returns `memberships`.

## UI / manual

- [x] Open `/register` while signed out. The heading is You, the steps are You and Church, and the button is Find your church. The form asks for first name, last name, email, and password. It has no church search. First name and last name are required with maxlength 80. Email is required, type email, maxlength 120. Password is required. → AC-2, AC-7
- [x] Open `/join/{code}` while signed out. You shows that church and the button joins it. Creating the account opens the church. The same link while signed in, with no church, opens the church without the form. → AC-1, AC-2
- [x] Create an account with a strong password. The browser lands on `/churches/join` and the flash says Account created. The new user has no church. → AC-1, AC-4
- [x] Submit `/register` with a blank last name. The browser returns to `/register` with the missing flash, and no user is stored. → AC-3
- [x] Submit `/register` with the password `password` and the names Cara and Nguyen. The form stays up, the flash asks for a stronger password, and the names and email are still filled in. → AC-3, AC-7
- [x] Submit `/register` with an email that already has an account. The flash says an account with that email already exists. → AC-3
- [x] While signed in with no church, open `/register`. The browser goes to `/churches/join`. With a church, it goes to `/home`. → AC-2
- [x] Request `GET /register/churches`. The route is gone. → AC-2
- [x] With no church, open `/home`, `/me`, `/churches`, `/the-body`, and `/inbox`. Each one returns to `/churches/join`. That page has no dock. Sign out stays. → AC-5
- [x] Ask to join a church, then open `/home`. The Waiting section names that church. → AC-5
- [x] On `/me`, edit first name, last name, and about you. There is no city, region, or address field. Save lands on `/me` with the saved flash. → AC-6
- [x] On `/me` with no church, the browser returns to `/churches/join`. → AC-5, AC-6
- [x] On `/me` while a join is waiting, that church name shows with Waiting. Find another church goes to `/churches/join`. Other churches stay listed. → AC-6, AC-10
- [x] On `/churches/new`, enter an address and coordinates. The form has no city or region. After save, the church page shows the address. → AC-8
- [x] Open `/churches/join` with location allowed. One closest church is suggested, with its address. Deny location and the suggestion is absent. Name search remains, with a QR icon beside the search box. → AC-9
- [x] From `/churches/join`, ask to join by name. The browser opens that church and the page says the request has been sent. The link is pending and any previous church stays. Join with a code. The browser opens that church, the new church is active, and the previous church stays. → AC-5, AC-10

## Commands

- [x] `cargo test -p ecclesia --test flows us_auth_01_weak_password_stays_on_register` → the weak password form keeps first name, last name, and email → AC-3, AC-7
- [x] `cargo test -p ecclesia --test flows us_auth_01_join_search_finds_grace` → a signed in name search on `/churches/join` finds Grace Fellowship → AC-2, AC-9
- [x] `cargo test -p ecclesia --test flows us_api_01_json_sign_in_refresh_and_me` → `GET /api/me` returns first name, last name, email, and an empty `memberships` array → AC-6
- [x] `cargo test -p ecclesia --test flows us_mem_04_pastor_can_approve_a_join_request` → one pending join can be approved from the church page → AC-5, AC-10
- [x] `cargo test -p ecclesia-domain --lib` → register stores no church, and a second join keeps the first → AC-1, AC-4, AC-8, AC-10

## Value sourcing

- [x] Register with first name `  Ada` and last name `Lovelace  `. The stored names are `Ada` and `Lovelace`, each at most 80 characters. → Register names
- [x] Register with `Ada@Example.TEST`. The stored email is `ada@example.test`. → Register email
- [x] After register, bio is empty. → Register bio
- [x] After register, the user id and created at are set by the server, not the form. → Register id and created at
- [x] After register, the password column is a hash, not the typed password. → Register password hash
- [x] After register, the session cookie is present and `/churches/join` loads for that person. → Register session
- [x] After register, `memberships` is empty. → Register membership
- [x] `/home` does not print a city or a region. → Home place line
- [x] Open `/churches/join?lat=42.5349&lng=-92.4453`. The suggestion is the nearest church. Open the same page with no coordinates and there is no suggestion. The coordinates are not saved on the user. → Closest church
- [x] The suggestion card prints `churches.address`. → Church building place
- [x] Search `/churches/join?q=Grace` with no lat or lng. The QR icon is beside the search box, and there is no invite code field. → Name search and QR
- [x] A person with no church sees Find your church, and Home sends them there. → Home no church line
- [x] A pending link shows that church name and Waiting. An active link shows the church name without Waiting. Both can appear together. → Profile church line
- [x] Find another church on `/me` goes to `/churches/join`. → Profile add a church
- [x] Joining a second church keeps the first. A code sets the new row active. A name sets the new row pending. → Join
- [x] Saving `/me` updates first name, last name, and bio, and does not change the church link. → Profile save
- [x] `GET /api/me` returns first name, last name, email, and bio from the user row, and `memberships` from the live join. It does not return city, region, or a single church id. → `/api/me`

## Acceptance-criteria coverage

- AC-1 covered by create account landing on `/churches/join`, and by the domain register test
- AC-2 covered by the register form check, signed in `/register`, and the removed church lookup route
- AC-3 covered by blank last name, taken email, and the weak password command
- AC-4 covered by stored names and an empty church link after register
- AC-5 covered by the join-only gate, and by the church page after a request
- AC-6 covered by profile save, the no church line, and `GET /api/me`
- AC-7 covered by the form maxlength and the weak password redisplay
- AC-8 covered by the church form address and coordinates
- AC-9 covered by the join page with and without coordinates
- AC-10 covered by a second join keeping the first church, pending for a name and active for a code
