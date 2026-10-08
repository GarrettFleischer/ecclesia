# 0005. Media conversations

**Date**: 2026-10-07
**Status**: Accepted

## Summary

A need is a conversation. Its author may include up to five photos, members may
reply with text and up to five photos, and the author closes the need with a
final reply that says what happened. Profiles may have one photo. New replies
arrive without a page reload, while every form still works through an ordinary
redirect when JavaScript is unavailable.

Media stays private. Ecclesia normalizes every upload, strips embedded
metadata, stores it outside the public static tree, and checks the viewer
before serving it.

## Requirements

**User stories**:
- As a member, I want to show the condition of a need so others can understand
  it before offering help.
- As a member, I want to include photos in a reply and see new replies while
  the conversation is open.
- As the author, I want to close my need with a final reply and photos of the
  result.
- As a member, I want a profile photo, with initials when I do not add one.
- As a viewer, I expect a private photo to follow the same audience rules as
  the conversation around it.

**Acceptance criteria**:
- **AC-1**: A need accepts zero through five attached photos. Each reply,
  including the completion reply, accepts zero through five. An attachment may
  have a description of at most 300 characters. A sixth attachment is refused
  by Domain and no partial attachment write commits.
- **AC-2**: A reply has kind `message` or `completion`. Only the need author may
  add a completion reply. The need must be open. One transaction inserts that
  reply, attaches its media, changes the need to met, and stores the closing
  reply id. Failure leaves both the reply and need unchanged.
- **AC-3**: The author may remove a photo from their own need or reply. A person
  may replace or remove their own profile photo. No one may remove another
  person's media. Removing an attachment does not delete a still-referenced
  asset.
- **AC-4**: Existing praise text remains readable as the completion fallback
  for needs that predate this feature. It is not copied into public replies.
  New completions use a typed completion reply.
- **AC-5**: Uploads accept static JPEG, PNG, and WebP images up to 12 MiB and 40
  megapixels. The server decodes and re-encodes each accepted image as WebP,
  strips metadata, creates a full image no larger than 2400 pixels on its
  longest edge, and creates a thumbnail no larger than 640 pixels. Unsupported,
  malformed, animated, oversized, or over-dimensioned files are refused before
  attachment.
- **AC-6**: A staged upload belongs to the signed-in uploader. Only that person
  may preview or attach it. Staged assets become attached in the same database
  transaction as the need, reply, or avatar change. Unattached staging objects
  expire after 24 hours.
- **AC-7**: Production objects live in a private Cloudflare R2 bucket. A media
  request first checks authorization in Ecclesia, then returns a short-lived
  object URL. Local development uses a private filesystem directory and
  streams the file only after the same authorization check.
- **AC-8**: Need and reply photos inherit the need's audience. A member who
  cannot open the need cannot open its media or receive its reply events.
  Profile photos follow profile visibility. Staged media is visible only to
  its owner.
- **AC-9**: After a reply commits, the SDK publishes its need id and reply id
  to one global Redis Stream. The need's SSE endpoint filters events by need
  and rechecks access. Local development uses an in-process broadcast hub.
  Redis failure does not roll back an already committed reply.
- **AC-10**: SSE reconnects from `Last-Event-ID`. A client deduplicates by reply
  id. If the requested stream history is no longer available, the server asks
  the client to refresh the conversation from Postgres, which remains the
  source of truth.
- **AC-11**: With JavaScript, a member can preview, describe, and remove staged
  photos before posting, and new replies appear in place. Without JavaScript,
  multipart forms post the same text and photos and redirect back to the need.
  Both paths call the same SDK stories and Domain rules.
- **AC-12**: One photo may render large. Several render as thumbnails. Opening
  either uses an accessible full-screen viewer. Every image with a description
  uses it as alternative text; an undescribed image has empty alternative text
  when surrounding text already supplies the context.
- **AC-13**: Avatars use the profile photo when available and the member's
  initials otherwise. Need pages and the public example use the same avatar,
  gallery, and conversation renderers.

## Decision

**Chosen option**: private normalized media with staged attachment, an
author-written completion reply, and SSE notifications backed by a global
Redis Stream.

## Feature design

### Data model

`media_assets`:

| Column | Notes |
|---|---|
| `id` | Stable application id |
| `owner_id` | User who uploaded the image |
| `state` | `staged`, `attached`, or `deleting` |
| `staging_full_key` | Temporary full-image object key |
| `staging_thumb_key` | Temporary thumbnail object key |
| `full_key` | Attached full-image key, nullable until promotion |
| `thumb_key` | Attached thumbnail key, nullable until promotion |
| `width` / `height` | Normalized full-image dimensions |
| `full_bytes` / `thumb_bytes` | Normalized object sizes |
| `created_at` | Used for stale staging cleanup |
| `attached_at` | Set by the attachment transaction |
| `deleted_at` | Set before asynchronous object deletion |

`need_media`:

| Column | Notes |
|---|---|
| `need_id` / `media_id` | Attachment identity |
| `position` | Zero-based display order, unique within the need |
| `description` | Optional, trimmed, at most 300 characters |

`reply_media` has the same shape with `reply_id`.

`users.avatar_media_id` is nullable. `need_replies.kind` is required and
defaults to `message`. `needs.closing_reply_id` is nullable. All changes are
additive. Existing `needs.praise` stays for legacy reads.

### State transitions

Media: uploaded and normalized → staged → attached → promoted. A removed or
replaced asset with no remaining reference becomes deleting → objects removed
→ row marked deleted. Staged media older than 24 hours follows the same delete
path.

Need: open → completion reply and met in one transaction. Reopening a need
clears `closing_reply_id`; the old completion remains in the conversation as
history.

### HTTP surface

| Endpoint | Method | Result | Authorization |
|---|---|---|---|
| `/media/stage` | POST multipart | Staged id, preview URLs, dimensions | signed in + CSRF |
| `/media/{id}/{variant}` | GET | Authorized redirect or local stream | reference audience or stage owner |
| `/media/{id}` | DELETE | Detach or discard owned media | signed in owner + CSRF |
| `/needs` | POST multipart | Need with optional photos, then redirect | signed in + CSRF |
| `/needs/{id}/replies` | POST multipart | Message with optional photos | need audience + CSRF |
| `/needs/{id}/complete` | POST multipart | Completion reply and closed need | need author + CSRF |
| `/needs/{id}/events` | GET | `text/event-stream` reply notices | need audience |
| `/needs/{id}/conversation` | GET | Current rendered conversation fragment | need audience |
| `/me/avatar` | POST multipart | Replaced profile photo | signed in owner + CSRF |
| `/me/avatar` | DELETE | Initials fallback restored | signed in owner + CSRF |

JavaScript may stage files before the parent form is posted. The no-JavaScript
multipart handlers perform the same staging work before calling the same story.

### Storage lifecycle

The upload path reads with a hard byte limit, identifies the decoded format
rather than trusting the filename or declared MIME type, checks dimensions
before allocating a full raster where the decoder permits it, fixes
orientation, and emits new WebP bytes. Original bytes are never retained.

The store writes normalized variants under staging keys. The attachment
transaction marks their row attached and writes a promotion outbox item.
Until promotion finishes, authorized reads may use the staging keys. The
worker copies both variants to stable keys, updates the row, and removes the
staging objects. Delete and stale-stage work are idempotent outbox jobs.

### Live updates

The Redis Stream carries small notices, not rendered HTML or private message
text. A notice contains the need id, reply id, and committed timestamp. The
authorized app process loads the reply from Postgres and renders it with the
shared conversation renderer.

The browser stores the last stream id supplied by SSE and deduplicates DOM
nodes by reply id. A normal page load and a reconnect both reconcile against
the current Postgres conversation, so a missed Redis publish cannot hide a
reply.

## Key invariants

- Domain receives attachment ids and descriptions, never bytes, object keys,
  MIME types, R2 clients, clocks, or filesystem paths.
- A sixth attachment, foreign staged id, or second completion cannot partially
  commit.
- A completion reply and the met status are one effect applied in one
  transaction.
- Publishing a live event happens only after commit.
- Object possession does not grant access. Every media read begins with an
  application authorization check.
- Original image bytes and metadata are not retained.
- The App loads values, calls one SDK story, and paints HTML. Audience and
  ownership decisions stay in Domain and SDK stories.

## Security and privacy

R2 has no public bucket URL. Signed object URLs expire after five minutes.
Responses prevent caching of staged media and allow private caching of attached
variants only for the signed URL lifetime. Object keys are random and are not
treated as secrets.

Descriptions are escaped as text. Image decoding runs with byte and pixel
limits. Upload, post, completion, and avatar routes use the existing CSRF and
rate-limit conventions. SSE sends no event before the current session and need
audience have been checked.

## Configuration

Local development uses `ECCLESIA_MEDIA_DIR`, defaulting to
`var/ecclesia-media`.

A public host requires `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`,
`R2_SECRET_ACCESS_KEY`, and `R2_BUCKET`. The bucket and secrets are provisioned
only with owner approval.

## Critical test scenarios

- Attach zero, one, and five photos; refuse six without a partial write.
- Refuse a staged id owned by another member.
- Decode a JPEG carrying metadata and prove the served WebP does not retain it.
- Refuse an oversized byte stream, excessive dimensions, malformed content,
  animation, and a declared type that does not match decoded content.
- Replace an avatar, keep the replacement readable, and clean the unreferenced
  old asset.
- Complete an open need with text and photos; prove one transaction creates the
  completion and closes the need.
- Fail an attachment write during completion; prove the need remains open and
  no completion reply exists.
- Read a legacy met need with `praise` and no closing reply.
- Deny media and SSE to a signed-in member outside the need audience.
- Reconnect SSE from the last id, deduplicate a repeated reply, and refresh
  from Postgres after trimmed history.
- Fail Redis after commit; prove the reply remains and appears on conversation
  reconciliation.
- Fail R2 during staging, promotion, and deletion; prove no broken attachment
  commits and cleanup is safe to retry.
- Post a need, message, and completion with JavaScript disabled.
- Render a profile photo when present and initials when absent.

## Rollout and rollback

Schema changes are additive. Deploy the tables and columns before enabling the
new forms. Existing pages continue to read old needs and praise text.

The application can stop accepting uploads and SSE connections without
dropping media rows. Attached objects remain private and readable through the
authorized route. Removing the new UI leaves the old text conversation and
legacy praise path intact.

## Consequences

**Positive**:
- The conversation can show the work from request through completion.
- Privacy follows the need instead of depending on unguessable image URLs.
- The same components render real conversations and the public example.

**Negative / tradeoffs**:
- Uploads now cross object storage and the database, so promotion and deletion
  need retryable background work.
- SSE adds a long-lived connection and catch-up behavior to each app process.
- Image decoding increases CPU and memory pressure and therefore requires hard
  limits.

**Neutral**:
- Redis carries invalidation-style notices. Postgres remains authoritative.
- The public landing uses generated static media and does not expose private
  application assets.
