# Design

Decisions for the painted pages. Tokens live in `crates/app/static/app.css`.

## Theme

Light is the default. Dark follows the device through `prefers-color-scheme`. One `@media` block redefines the same token names, so rules never branch on the scheme. Each scheme gets its own `theme-color` meta.

## Tokens

| Token | Job |
|---|---|
| `--bg` | Page ground under the wallpaper |
| `--ink`, `--ink-2`, `--ink-3` | Headings and body, ledes and secondary lines, labels and eyebrows |
| `--accent`, `--accent-hover`, `--on-accent` | Primary button fill and its text. Evergreen in light, cream in dark |
| `--accent-ink`, `--accent-soft` | Links, the current dock item, status pills |
| `--gold`, `--gold-soft`, `--gold-line` | Ornament: the mark, meta icons, the empty mark, gold chips, Rewrite. Not body text |
| `--gold-ink` | Text on gold fills, gift categories, link hover |
| `--lake-ink`, `--lake-soft` | The nearby-churches scope mark |
| `--ok`, `--err`, `--err-soft` | Status text and the error banner |
| `--focus`, `--focus-ring` | Focus outline and the field ring |
| `--surface`, `--surface-2`, `--surface-3` | Card, raised control, near-solid menu |
| `--sunk`, `--field` | Rows nested in a panel, chips, empty states; form fields |
| `--line`, `--line-2`, `--line-3` | Hairlines, control outlines, link underlines |
| `--bar`, `--glass`, `--toast`, `--pill` | Top bar and dock, auth card, toast and install bar, the dock pill |
| `--glow-a`, `--glow-b` | The two lights drifting in the backdrop |
| `--veil-*`, `--scene-*` | Scrims over the wallpaper and the scene photos |
| `--shadow-1`, `--shadow-2`, `--shadow-3` | Card at rest, card on hover, the landing panel |
| `--r-sm` to `--r-xl`, `--sheet` | Radii, and the 44rem reading column |
| `--ease-out`, `--spring`, `--t-fast`, `--t-med`, `--t-slow` | Motion curves and durations |

Text uses `--ink`, `--ink-2`, `--ink-3`, `--accent-ink`, `--gold-ink`, `--ok`, or `--err`. Check contrast in both schemes when a token changes.

## Type and structure

Instrument Serif for h1 to h3, the landing guide, and its verses. Inter for everything else. Both are self-hosted in `static/fonts/`, split into latin and latin-ext, and the two latin files are preloaded.

One h1 per page, from `page_lead`. The document title is the h1, then ` · Ecclesia`. The landing title is `Ecclesia` alone.

An eyebrow above the h1 gives context when the h1 needs it: the city on Home, the scope and church on a need. People and churches get a monogram from `monogram(seed, name, shape)`. The seed is the id, so a person keeps one color everywhere.

Icons come from `icon(Icon)` in `layout.rs`: 24px, 1.8 stroke, `currentColor`. A new icon is a new `Icon` variant. No icon fonts.

Human words for roles, offers, and gifts come from `crates/app/src/views/words.rs`. Stored values are not printed.

## Imagery and glass

Scenes hold no people: dawn light, mist, water, and fields. They live in `static/img/` as WebP. `dawn-tall` and `dawn-wide` are the landing hero and the account backdrop. `wallpaper-*` are the same dawn scene pre-blurred for member pages.

Cards are translucent over the pre-blurred wallpaper and carry no `backdrop-filter`. Live blur is for chrome that floats over moving content: top bar, dock, toast, install bar, `.btn-glass`, and the auth card. Reduced transparency makes `--surface`, `--bar`, `--glass`, and `--toast` solid and drops the live blur.

## Motion

`.page-rise` staggers the page in once. A tab heading starts large and shrinks to its size. The same motion runs on every tab. A detail heading stays put because it is the shared element.

Cross-document view transitions (`@view-transition`). Opening a card moves the `page-title` name onto that card's h3 (`hookTitleMorph` in `app.js`), so the card title becomes the next page's h1. The dock pill glides to the new tab on `--spring`.

Scroll-driven, inside `@supports (animation-timeline: view())`: cards, people rows, and the landing guide reveal as they enter, the top bar frosts after the first 4rem, and the hero drifts and fades. Browsers without it get the settled state.

Hover lift on card links and buttons is inside `@media (hover: hover)`. `prefers-reduced-motion: reduce` turns off view transitions and cuts every animation and transition to nothing. Forced colors outline each surface in `CanvasText` and mark the current tab and the checked choice with `Highlight`.

## Chrome seats

| Seat | Nav | What you see |
|---|---|---|
| Landing | `Nav::Landing` | Scene hero, the guide below it, install bar. No top bar, no dock |
| Account | `Nav::Account` | Dawn backdrop, glass card with the mark, one h1, one form. No dock, no install bar |
| App | `Nav::Home` and the other member items | Top bar, dock, member pages over the wallpaper |
| Sorry, guest | `Nav::None` | Guest shell, link home |
| Sorry, member | `Nav::Home` | App chrome stays |

The public landing runs in this order: hero (lockup, title, account actions), Acts 2:44-45, the church-takes-care narrative with the Sarah, David, and handrail phones, the prayer phones, the church map and one body, the account actions, footer. `footer.site-footer` is on the landing and on the privacy and terms pages. Privacy and terms use `main.sheet-legal`.

The install bar is the landing seat only. Dismiss stays in `localStorage` under `ecclesia-install-dismissed`.

## Surface jobs

Account pages (`/register`, `/session/new`, `/session/link/new`, `/session/reset/new`) each do one job. The heading names it. The card holds the form. Links under the card switch to the other account pages. They are text, not a second primary button.

Inbox holds the decision. A card that already offers Accept and Decline does not also send the person away to decide.
