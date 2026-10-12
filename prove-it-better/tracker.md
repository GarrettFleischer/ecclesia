# Tracker: landing page below the hero

The first challenger was rejected by the owner. Its automated checks passed and
the tuning judge preferred it 16–0, but the page read like an implementation
tour written for a child. The rubric rewarded topic coverage, short sentences,
and feature enumeration while missing the quality of the prose. No fresh-case
round was run.

That result is recorded as a failed evaluation, not as evidence that the
challenger was better. The replacement gate is owner approval of the prose,
followed by the existing factual, accessibility, test, and weight checks.

Judge used for the rejected round: GPT-5.6 (OpenAI), Cursor subagent, blind,
both orders, fresh call per order, text plus unlabeled phone and desktop
screenshots. Author for the rejected copy: Claude (Anthropic). Cases were
synthetic reader profiles.

| # | Unit | Current | Challenger | Eval (cases, judge, hard checks) | Result | Status |
|---|------|---------|------------|----------------------------------|--------|--------|
| 1 | Below-hero sections (copy and layout) | Four-paragraph essay plus the Needs and Gifts panel | Intro, How it works, phone sequence, four cards, scene verse, closing band (the guide landing, later removed) | 16 tuning cases, GPT-5.6 pairwise, hard checks below | Automated judge preferred challenger 16–0; owner rejected its prose | ❌ rejected |
| 2 | Footer | None | Contact, Privacy, Terms, ESV notice | Links resolve, notice matches Crossway text | | building |
| 3 | Privacy and Terms pages | None | New pages drafted from the data inventory | Owner review, every claim traced to code | | todo |
| 4 | Homepage search preview | `Ecclesia` plus a concrete three-sentence feature summary | Bold mission title plus a 142-character description grounded in needs, practical help, prayer, and nearby churches | 10 synthetic search situations, Gemini 3.1 Pro blind pairwise comparison in both orders, length and claim checks, app flow and style suites | Owner selected the title/description pairing; 55 flow tests and 3 style tests pass | ready to deploy |

## Hard checks

Both columns come from the same headless capture (`shoot.js`, Chrome, 390x844 at 2x and 1280x800, light and dark, reduced motion). Current is the live site. Challenger is the local build.

| Check | Current (live, 2026-10-07) | Challenger (local) | Gate | Result |
|---|---|---|---|---|
| `cargo test` (flows, chaos, style) | Passing | Passing | Pass | Pass |
| Topics covered (steps, QR or invite, prayer, nearby, gifts, contact, free) | 0 fully, 2 partly | 7 of 7 | 7 of 7 | Pass |
| Wrong claims | 1 (need "does not stay hidden within a single congregation") | 0 | 0 | Pass |
| Scripture verbatim against esv.org | No quotes | 6 of 6 (three are partial verses ending at an ESV comma or colon) | Every quote | Pass |
| Banned words / "not" contrasts outside Scripture | 1 / 4 | 0 / 0 | 0 / 0 | Pass |
| Flesch-Kincaid grade, by sentence / by line | 7.4 / 7.4 | 7.2 / 6.7 | 8 or lower, no worse | Pass |
| Lowest text contrast anywhere below the hero, solid or over a photo | 3.5:1 (desktop "Needs and Gifts" over linen) | 5.2:1 (light, "Acts 2:44-45" cite) | At least 4.5:1, both modes | Pass |
| Horizontal overflow at 390px | None | None | None | Pass |
| Smallest tap target | 52px | 44px ("Sign in" text link) | At least 44px | Pass |
| Page weight, phone, text gzip -9 on both sides | 302 KB | 329 KB (23 KB is the steeple photo) | Under 500 KB, below-hero images lazy | Pass |
| Links load | 3 of 3 | 5 of 5 | All | Pass |

Page weight: Fly's proxy sends Brotli, and the local server sends raw bytes. The first capture read 312 KB live and 430 KB local for that reason. The tuning round's automated check line carried those two numbers. The fresh round uses the gzip estimate above for both sides.

The baseline FK of 6.0 in the research brief used a different text extraction. The row above re-measures both pages with one script. The over-photo contrast takes the 5th and 95th percentile pixel behind each text box with the text hidden. The "Create an account" button reads as a false 1.0:1 in that pass on both pages, because the script compares against the wrong color; screenshots show dark text on a cream pill.

Status values: todo · building · evaluating · iterating · ✅ shipped · ❌ kept current (with reason) · out of scope.

## Why the challenger failed

The challenger explains QR codes, invite links, pastor approval, visibility
settings, location use, installation, and checkbox behavior in places where the
reader can already understand the product from the screen. Its examples exist
to cover features instead of sounding like people. The sentence-level rules
also flatten public writing into short, equally weighted statements.

The replacement uses one Linda Harper handrail story across fixed mobile
frames. It shows a before photo, two useful replies, and a completion reply
with after photos. Supporting copy describes the purpose of Ecclesia and leaves
ordinary interface mechanics to the app.

The replacement copy is written by GPT-5.6 and reviewed directly by the owner.
Grok 4.7 implements the product and presentation without rewriting approved
copy.
