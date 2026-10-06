# 0004 rationale

## Context

Create account requires a church id. The SDK copies that church's city and region onto the user and files a join in the same transaction. The engineer wants the church step after the account exists.

An address belongs on the church building, not on the person. The building stores a multiline postal address and WGS84 latitude and longitude so the app can suggest the closest church. There are no live users or churches to preserve, so person city and region, and church city and region, are dropped. After register, `/churches/join` suggests that closest church only when the browser sends a granted device location. Otherwise the person searches by name or scans a QR code. The profile is the person's name, what they write about themselves, and each church they have joined or are waiting on.

The engineer also asked for one validation definition on the browser and the server, and asked whether Zod fits. This crate is Rust on the server and Maud HTML in the browser. Zod does not run in Rust, and there is no binding that keeps one Zod schema as the domain check.

## Options considered

### Option 1: Keep city and region on the person

Drop the church picker and ask city and region on the account form.

**Pros**:
- Home's place line keeps working.

**Cons**:
- The engineer said place is not a person field.

### Option 2: Person has a name only

Register collects first name, last name, email, and password. `users` loses `name`, `city`, and `region`. Profile shows memberships and does not edit them. Form limits match the domain limits. The domain still decides.

**Pros**:
- Matches the corrected place rule.
- Register does not need a church to exist.
- No second schema language to keep in sync.

**Cons**:
- Call sites that read city and region on a person or a church must change.
- Every church needs an address and coordinates so closest can be computed.

### Option 3: Add Zod (or a shared schema) for the form and the server

Generate or bind one schema so the browser and Rust reject the same payloads.

**Pros**:
- One artifact describes the fields.

**Cons**:
- Zod does not execute in this Rust domain. A binding would be a new toolchain for four fields.
- The domain would stop being the only place that knows whether a name is acceptable.

## Rationale

Option 2 matches the decision: no address on the person, first and last name, church location on the church, closest suggestion only with permission. Option 1 keeps place on the person. Option 3 does not fit the stack. Zod has no Rust bindings. HTML constraints mirror the domain maxima, and the server still runs the domain check.

## References

**Project sources**:
- `docs/scope/scope.md` feature 4
- `crates/domain/src/auth.rs` `register`
- `crates/sdk/src/identity.rs` `register`
- `crates/app/src/views/landing.rs` create account form
- `crates/app/src/views/home.rs` no church empty state
- spec 0002 identity and sessions
- spec 0003 `GET /api/me`
