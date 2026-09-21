# Feature spec and user stories

Each story has a leaf (or a documented skin exception), a unit test, and where the world changes, an SDK or HTTP test.

## Identity

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-AUTH-01 | I can create an account with name, email, city, region, and how I serve. A used email is refused. | `register` | `us_auth_01_*` |
| US-AUTH-02 | In demo mode I may sign in as a seeded person. When demo is off, that sign-in is closed. | `may_impersonate` | `us_auth_02_*` |
| US-PROF-01 | I can change how I am known. | `update_profile` | leaf + `/me` |

## Membership

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-MEM-01 | I can ask to join a church. Pastors and stewards are notified. I cannot ask twice. | `request_join` | `us_mem_01_*` |
| US-MEM-02 | A pastor or steward can invite someone already in Ecclesia by email. A member cannot. | `invite_member` | `us_mem_02_*` |
| US-MEM-03 | I can redeem an invite code. The pastor already chose; I still confirm. | `redeem_invite` | leaf |
| US-MEM-04 | A pastor or steward can approve or decline a pending request. A member cannot. The person is notified. | `decide_membership` | `us_mem_04_*`, HTTP |
| US-MEM-05 | Only the invited person can accept an invite. | `accept_invite` | `us_mem_05_*` |
| US-CH-01 | I can plant a church and become its owner. | `plant_church` | `us_ch_01_*` |

## Needs

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-NEED-01 | An approved member can post a need for their church, scoped to the church, churches nearby, or everyone. | `post_need` | `us_need_01_*` |
| US-NEED-02 | I can apply to a need I am allowed to see. The author is notified. I cannot apply to my own. | `apply_to_need` | `us_need_02_*`, HTTP |
| US-NEED-03 | The author or a governor can close a need. | `close_need` | leaf |
| US-NEED-04 | The author or a governor can receive or decline an offer. | `decide_application` | leaf |
| US-NEED-05 | Church / neighboring / body visibility. Pending members are not yet in the body. | `can_view_need`, `visible_need_cards`, `require_need_view` | `us_need_05_*`, HTTP |
| US-NEED-06 | Offer messages are visible to the author or pastor, and to the person who wrote the offer. | `visible_offers` | `us_need_06_*` |

## Gifts and endorsements

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-GIFT-01 | I can name or remove a gift I practice. | `add_gift`, `remove_gift` | leaf |
| US-END-01 | I can endorse someone else for any skill, including ones they have not claimed and ones not in the catalog. They are notified. I cannot endorse myself. | `endorse` | `us_end_01_*` |
| US-END-02 | I can accept or decline an endorsement. The message is not public until I accept. A declined note stays visible to me and the endorser, grayed out, and I can accept it later. Accepting a catalog skill I had not claimed adds that gift. It does not overwrite a gift I already named. | `accept_endorsement`, `decline_endorsement` | `us_end_02_*`, HTTP |

## The body

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-BODY-01 | Churches in the same city or region are neighbors. | `churches_are_neighbors` | `us_body_01_*` |

## App shell

| ID | Story | Where | Tests |
| --- | --- | --- | --- |
| US-APP-01 | The site is installable on a phone: standalone display, icons, offline shell, share, alerts. | manifest, service worker, `/me` | `us_app_01_*` |
| US-APP-02 | A Capacitor shell embeds the same server on iOS and Android with status bar, splash, keyboard, share, haptics, back, and `ecclesia://` links. | `mobile/` | docs |
| US-PUSH-01 | A signed-in person can subscribe for Web Push or register a native device token. Notices fan out to stored subscriptions. | `sdk/push`, `/push/*` | `us_push_01_*` |

## Validation, prose, and security skins

| ID | Story | Where | Tests |
| --- | --- | --- | --- |
| US-VAL-01 | Empty, overlong, and malformed email values are refused. | `validate` | `us_val_01_*` |
| US-SEC-01 | A session cookie is HMAC-signed. Tampering is rejected. | `sdk::session` | `us_sec_01_*` |
| US-SEC-02 | Every POST carries a CSRF token bound to the session. | session + HTTP | `us_sec_02_*` |
| US-PROSE-01 | User-facing copy follows [PROSE.md](PROSE.md). | `style` | `us_prose_01` |
| US-LEAF-01 | Leaves do no I/O. | `style` | `us_leaf_01` |
| US-TONE-01 | Posted words must lift people up. The skin weighs them (Jev, OpenRouter, a local model, or the word gate) and the leaf refuses `TearsDown`. | `require_uplifting`, `JudgeHub` | `us_tone_01_*` |
| US-REFINE-01 | I can ask to rewrite a bio, need, offer, endorsement, gift note, or church description. | `RefineHub`, `/refine` | `us_refine_01_*` |
| US-REFINE-02 | When I submit, I see the rewritten words, I can edit them, then I publish. | `VoicePass`, writing forms | `us_refine_02_*` |
