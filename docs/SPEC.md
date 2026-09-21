# Feature spec and user stories

Each story has a leaf (or a documented skin exception), a unit test, and where the world changes, an SDK or HTTP test.

## Identity

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-AUTH-01 | I can take a seat with name, email, city, region, and how I serve. A used email is refused. | `register` | `us_auth_01_*` |
| US-AUTH-02 | In demo mode I may sit as a seeded person. When demo is off, that door is closed. | `may_impersonate` | `us_auth_02_*` |
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
| US-NEED-01 | An approved member can post a need for their church, scoped to the household, the valley, or the body. | `post_need` | `us_need_01_*` |
| US-NEED-02 | I can apply to a need I am allowed to see. The author is notified. I cannot apply to my own. | `apply_to_need` | `us_need_02_*`, HTTP |
| US-NEED-03 | The author or a governor can close a need. | `close_need` | leaf |
| US-NEED-04 | The author or a governor can receive or decline an offer. | `decide_application` | leaf |
| US-NEED-05 | Church / neighboring / body visibility. Pending members are not yet in the body. | `can_view_need`, `visible_need_cards` | `us_need_05_*`, HTTP |

## Gifts and endorsements

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-GIFT-01 | I can name or remove a gift I practice. | `add_gift`, `remove_gift` | leaf |
| US-END-01 | I can endorse someone else for any skill, including ones they have not claimed and ones not in the catalog. They are notified. I cannot endorse myself. | `endorse` | `us_end_01_*` |
| US-END-02 | I can publish or decline an endorsement. The message is not public until I publish. Publishing a catalog skill they had not claimed adds that gift. It does not overwrite a gift they already named. | `accept_endorsement`, `decline_endorsement` | `us_end_02_*`, HTTP |

## The body

| ID | Story | Leaf | Tests |
| --- | --- | --- | --- |
| US-BODY-01 | Churches in the same city or region are neighbors. | `churches_are_neighbors` | `us_body_01_*` |

## Validation and security skins

| ID | Story | Where | Tests |
| --- | --- | --- | --- |
| US-VAL-01 | Empty, overlong, and malformed email values are refused. | `validate` | `us_val_01_*` |
| US-SEC-01 | A session cookie is HMAC-signed. Tampering is rejected. | `sdk::session` | `us_sec_01_*` |
| US-SEC-02 | Every POST carries a CSRF token bound to the session. | session + HTTP | `us_sec_02_*` |
