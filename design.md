# Design

Decisions for the painted pages. Tokens live in `crates/app/static/app.css`.

## Tokens

| Token | Job |
|---|---|
| `--ink` | Body text, headings, field values |
| `--ink-soft` | Labels, secondary lines, ledes |
| `--gold-ink` | Small eyebrows only |
| `--paper`, `--paper-deep`, `--card` | Page and card surfaces |
| `--olive`, `--olive-deep` | Primary buttons, focus rings, links |
| `--olive-mist` | Chips |
| `--gold`, `--gold-soft` | Ornament: the rule under an h1, the mark glow. Not body text |
| `--clay` | Link hover |
| `--line` | Borders, quiet button outline |
| `--ok`, `--err` | Status text |
| `--shadow`, `--shadow-lift` | Cards at rest, cards on hover |

Text uses `--ink`, `--ink-soft`, `--gold-ink`, `--ok`, or `--err`. Gold and olive fills are for marks, rules, and buttons, not for a sentence.

## Type and structure

One h1 per page, from `page_lead`. The gold rule sits under that h1 and nowhere else on the same page.

The document title is the h1, then ` · Ecclesia`. The landing title is `Ecclesia` alone.

Human words for roles, offers, and gifts come from `crates/app/src/views/words.rs`. Stored values are not printed.

## Motion

`.page-rise` fades the page in once. Fields inside `.stack` do not rise again.

Hover lift on cards, buttons, and the dock is inside `@media (hover: hover)`.

## Chrome seats

| Seat | Nav | What you see |
|---|---|---|
| Landing | `Nav::None` | Essay, wide sheet, install bar. No dock |
| Account | `Nav::Account` | Mark home, one h1, narrow card, one form. No dock, no install bar |
| App | `Nav::Home` and the other member items | Top bar, dock, member pages |
| Sorry, guest | `Nav::None` | Guest shell, link home |
| Sorry, member | `Nav::Home` | App chrome stays |

The install bar is the landing seat only. Dismiss stays in `localStorage` under `ecclesia-install-dismissed`.

## Surface jobs

Account pages (`/register`, `/session/new`, `/session/link/new`, `/session/reset/new`) each do one job. The heading names it. The card holds the form. Links under the card switch to the other account pages. They are text, not a second primary button.

Inbox holds the decision. A card that already offers Accept and Decline does not also send the person away to decide.
