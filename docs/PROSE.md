# Prose

How to write anything a person reads in Ecclesia: headings, buttons, labels,
placeholders, flashes, errors, notifications, empty states, seed content, and
the README.

This is drawn from sites whose copy reads like a person wrote it — Basecamp,
Are.na, Buttondown, Planning Center — and from the GOV.UK and Apollo writing
guides. The common thread: say what is here, say what to do next, and stop.

## The rule

> If you find yourself explaining how the interface works, something has gone
> wrong. Fix the interface. — GOV.UK

Copy names things and actions. It does not describe the product, justify the
product, or tell the reader what the product is not.

## Say it like this

**Name what is on the page.** The heading is the noun. `Open needs`.
`Churches nearby`. `Inbox`. Not a greeting, not a slogan.

**Name what will happen.** The button is the verb plus its object.
`Post need`. `Accept invite`. `Publish`. `Send invite`. Never `Submit`, `OK`,
or `Confirm`.

**One idea per sentence.** Short sentences. Plain words. `Join a church first.`

**Be concrete.** Dinners, a ramp, a ride, Thursday at 6. A specific example
does more than an adjective. `Dinners for the Okonkwos this week`, not
`A meaningful need`.

**Write the way the reader would say it.** Contractions are fine. `You're in.`
`We couldn't find that church.`

**Errors: what happened, then the next step.** No blame, no apology, no code.
`The form expired. Try again.` `Only members of this church can apply.`

**Flashes are one to four words.** `Posted.` `Approved.` `Sent.` `You're in.`
The page already shows the result; the flash confirms it.

**Empty states name the next action or the condition.** `No open needs.`
`Once you're in a church, its needs show up here.`

**Notifications: who did what, then where to act.** Title: `Elena Vasquez
endorsed you for Counseling`. Body: `Publish it from your inbox, or decline.`

**Seed content sounds like the person typing into the form.** A pastor writes
`Pastor at Grace Covenant since 2014. Two kids, one very old dog.` She does not
write a mission statement.

## Never

- **Explain what it isn't.** `A people, not a campus.` `Not a marketplace.`
  `A life, not a compliment.` Say what it is or say nothing.
- **Moralize or reassure.** `That is allowed.` `That is theirs to decide.`
  `Go be the hands.` `They have not learned to ask.` The reader did not ask
  for your opinion of them.
- **Describe the product on its own pages.** No lede that explains membership,
  visibility, or approval. The controls show it.
- **Use metaphor as a noun.** Household, table, seat, door, hands, wear, valley.
  Use the plain word: church, member, pastor, need, offer, gift, profile.
- **Pad.** `just`, `simply`, `actually`, `please`, `kindly`, `in order to`,
  `obviously`, `easy`, `quickly`, `Oops`, `Unfortunately`, `Looks like`.
- **Sell.** `seamless`, `empower`, `leverage`, `journey`, `elevate`,
  `we're excited`, `powerful`, `delightful`.
- **Lean on the em dash.** One clause, one sentence. Use a period.
- **Ask a rhetorical question in a heading.** `What does the body need?` is
  a heading that is trying to be clever. `Post a need` is a heading.
- **Write placeholders as instructions.** A placeholder is an example value.
  `Hospitality`, not `Type a skill here`.

## Before and after, from this codebase

| Before | After |
| --- | --- |
| `Peace, Miriam.` | `Open needs` |
| `The ecclesia is a people, not a campus.` | `Ask for help. Offer yours.` |
| `You have a place at the table.` | `Account created.` |
| `Your request is with the pastor. They will let you in.` | `Request sent. The pastor will approve or decline.` |
| `No needs posted. Either they are between crises, or they have not learned to ask.` | `No open needs.` |
| `You let it go. That is allowed.` | `Declined.` |
| `Something gave way` | `Sorry` |
| `Be specific. A gift is a life, not a compliment.` | `Something you saw them do.` |
| `Accept onto my profile` | `Publish` |

## Checklist

Read the line out loud. Then:

1. Does it name a thing or an action? If it describes or persuades, cut it.
2. Is there a `not`, `isn't`, or `rather than` contrasting with something the
   reader never mentioned? Cut the contrast.
3. Is there a second sentence that comments on the first? Cut it.
4. Any word from the pad or sell lists? Cut it.
5. Any metaphor standing in for a plain noun? Replace it.
6. Would a person say this to a friend across a table? If not, rewrite.

`us_prose_01` in `src/style.rs` fails the build when user-facing source
contains a word from the pad or sell lists or a `, not a` contrast.
