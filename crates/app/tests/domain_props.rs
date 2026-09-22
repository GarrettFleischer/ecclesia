//! Random inputs against Domain stories. A story must return Ok or DomainError.
//! It must not panic, and a successful write must look like the person typed.

use ecclesia_sdk::prelude::{
    Application, ApplicationStatus, CatalogPresence, Church, DomainError, Strength,
    EmailAvailability, Endorsement, EndorsementQueue, Gift, GiftOnProfile, Membership, Need,
    NeedScope, NeedStatus, Posture, PriorOffer, SkillSource, User, Viewer, Write,
    accept_application, accept_endorsement, accept_invite, add_gift, apply_to_need,
    approve_membership, can_apply, can_view_need, close_need, decline_application,
    decline_endorsement, decline_membership, declined_visible_to, endorse, https_endpoint,
    invite_code_for, invite_member, normalize_email, pair_memberships,
    plant_church, post_need, redeem_invite, register, remove_gift, request_join, require_text,
    unique_church_ids, update_profile, visible_need_cards,
};
use ecclesia_sdk::judge::word_gate;
use ecclesia_sdk::session::Session;
use ecclesia::views::flash_from;
use proptest::prelude::*;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 96,
        max_shrink_iters: 400,
        ..ProptestConfig::default()
    }
}

fn junk() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("   ".into()),
        Just("\t\n".into()),
        Just("<script>alert(1)</script>".into()),
        Just("you are worthless".into()),
        Just("😊🔥".into()),
        Just("!!!".into()),
        Just("Ada".into()),
        Just("Need five dinners".into()),
        Just("ada@example.test".into()),
        Just("ada@@nope.com".into()),
        Just("ada@nope.com@x.com".into()),
        Just("a@b".into()),
        (0..80usize).prop_map(|n| "x".repeat(n)),
        (0..2100usize).prop_map(|n| "y".repeat(n)),
        "\\PC{0,48}",
    ]
}

fn person() -> impl Strategy<Value = User> {
    "[A-Za-z]{1,12}".prop_map(|id| User {
        id: id.clone(),
        name: id.clone(),
        email: format!("{id}@ecclesia.test"),
        city: "Cedar Falls".into(),
        region: "Iowa".into(),
        bio: String::new(),
        created_at: "t0".into(),
    })
}

fn church_at(id: &str, city: &str, region: &str) -> Church {
    Church {
        id: id.into(),
        name: id.into(),
        city: city.into(),
        region: region.into(),
        country: "US".into(),
        description: String::new(),
        gathering: String::new(),
        owner_id: "owner".into(),
        invite_code: "code".into(),
        created_at: "t0".into(),
    }
}

fn membership_of(user_id: &str, church_id: &str, role: &str, status: &str) -> Membership {
    Membership {
        id: format!("{church_id}-{user_id}"),
        church_id: church_id.into(),
        user_id: user_id.into(),
        role: role.into(),
        status: status.into(),
        created_at: "t0".into(),
    }
}

fn viewer_of(user: User, memberships: Vec<Membership>, churches: Vec<Church>) -> Viewer {
    Viewer {
        user,
        memberships,
        churches,
        gift_ids: vec![],
    }
}

fn need_of(scope: NeedScope, status: NeedStatus, church_id: &str, author_id: &str) -> Need {
    Need {
        id: "need".into(),
        church_id: church_id.into(),
        author_id: author_id.into(),
        title: "Help".into(),
        body: "Please".into(),
        gift_id: None,
        scope: scope.as_str().into(),
        status: status.as_str().into(),
        created_at: "t0".into(),
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn us_prop_01_register_is_total(
        name in junk(),
        email in junk(),
        city in junk(),
        region in junk(),
        bio in junk(),
        taken in any::<bool>(),
        tears in any::<bool>(),
    ) {
        let availability = if taken {
            EmailAvailability::Taken
        } else {
            EmailAvailability::Free
        };
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let result = register(
            &name, &email, &city, &region, &bio, availability, posture, Strength::Acceptable, "u1".into(), "t1".into(),
        );
        match (posture, availability, result) {
            (Posture::TearsDown, _, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, EmailAvailability::Taken, Err(DomainError::EmailTaken)) => {}
            (Posture::Lifts, EmailAvailability::Free, Ok(effect)) => {
                let Write::InsertUser(user) = &effect.writes[0] else {
                    panic!("register wrote something else");
                };
                assert_eq!(user.id, "u1");
                assert_eq!(user.name, name.trim());
                assert_eq!(user.email, email.trim().to_lowercase());
                assert!(user.email.contains('@'));
                assert!(!user.email.contains("@@"));
                assert!(user.name.chars().count() <= 80);
                assert!(user.bio.chars().count() <= 800);
            }
            (Posture::Lifts, EmailAvailability::Free, Err(DomainError::InvalidInput | DomainError::InvalidEmail)) => {}
            other => panic!("unexpected register outcome: {other:?}"),
        }
    }

    #[test]
    fn us_prop_01_email_never_keeps_a_second_at(raw in junk()) {
        match normalize_email(&raw) {
            Ok(email) => {
                assert_eq!(email.chars().filter(|c| *c == '@').count(), 1);
                assert_eq!(email, email.to_lowercase());
                assert!(!email.contains(".."));
                assert!(!email.chars().any(char::is_whitespace));
            }
            Err(DomainError::InvalidEmail | DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_01_require_text_trims_or_refuses(raw in junk(), max in 1usize..80) {
        match require_text(&raw, max) {
            Ok(text) => {
                assert_eq!(text, raw.trim());
                assert!(!text.is_empty());
                assert!(text.chars().count() <= max);
                assert_eq!(require_text(&text, max), Ok(text));
            }
            Err(DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_03_plant_church_is_total(
        name in junk(),
        city in junk(),
        region in junk(),
        description in junk(),
        gathering in junk(),
        tears in any::<bool>(),
        nonce in "[a-f0-9]{0,16}",
    ) {
        let owner = User {
            id: "owner".into(),
            name: "Owner".into(),
            email: "owner@ecclesia.test".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let result = plant_church(
            &owner, &name, &city, &region, &description, &gathering, posture,
            "c1".into(), "m1".into(), &nonce, "t1".into(),
        );
        match (posture, result) {
            (Posture::TearsDown, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, Ok(effect)) => {
                assert_eq!(effect.writes.len(), 2);
                let Write::InsertChurch(church) = &effect.writes[0] else {
                    panic!("expected church");
                };
                assert!(church.invite_code.contains('-'));
                assert!(!church.invite_code.contains(' '));
                assert!(church.name.chars().any(|c| c.is_ascii_alphanumeric()));
            }
            (Posture::Lifts, Err(DomainError::InvalidInput)) => {}
            other => panic!("unexpected plant: {other:?}"),
        }
    }

    #[test]
    fn us_prop_03_invite_code_is_stable(name in junk(), nonce in "[A-Za-z0-9]{0,16}") {
        let a = invite_code_for(&name, &nonce);
        let b = invite_code_for(&name, &nonce);
        assert_eq!(a, b);
        assert!(!a.contains(' '));
        assert_eq!(a, a.to_ascii_lowercase());
    }

    #[test]
    fn us_prop_04_post_need_is_total(
        title in junk(),
        body in junk(),
        scope in junk(),
        church_id in "[a-z]{0,8}",
        tears in any::<bool>(),
        active in any::<bool>(),
    ) {
        let user = User {
            id: "miriam".into(),
            name: "Miriam".into(),
            email: "miriam@ecclesia.test".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        let status = if active { "active" } else { "pending_request" };
        let viewer = viewer_of(
            user,
            vec![membership_of("miriam", "grace", "owner", status)],
            vec![church_at("grace", "Cedar Falls", "Iowa")],
        );
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let result = post_need(
            &viewer, &church_id, &title, &body, None, CatalogPresence::Listed, &scope,
            posture, "n1".into(), "t1".into(),
        );
        match (posture, result) {
            (Posture::TearsDown, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, Ok(effect)) => {
                assert_eq!(church_id, "grace");
                assert!(active);
                let Write::InsertNeed(need) = &effect.writes[0] else {
                    panic!("expected need");
                };
                assert_eq!(need.author_id, "miriam");
                assert_eq!(need.title, title.trim());
            }
            (Posture::Lifts, Err(DomainError::InvalidInput)) => {
                assert_eq!(church_id, "grace");
                assert!(active);
            }
            (Posture::Lifts, Err(DomainError::NotInTheBody)) => {
                assert!(church_id != "grace" || !active);
            }
            other => panic!("unexpected post_need: {other:?}"),
        }
    }

    #[test]
    fn us_prop_05_apply_respects_sight(
        scope in prop_oneof![Just(NeedScope::Church), Just(NeedScope::Neighboring), Just(NeedScope::Body)],
        status in prop_oneof![Just(NeedStatus::Open), Just(NeedStatus::Closed)],
        same_person in any::<bool>(),
        message in junk(),
        tears in any::<bool>(),
    ) {
        let author_id = "miriam";
        let actor_id = if same_person { "miriam" } else { "elena" };
        let actor = viewer_of(
            User {
                id: actor_id.into(),
                name: actor_id.into(),
                email: format!("{actor_id}@ecclesia.test"),
                city: "Waterloo".into(),
                region: "Iowa".into(),
                bio: String::new(),
                created_at: "t0".into(),
            },
            vec![membership_of(actor_id, "mercy", "member", "active")],
            vec![church_at("mercy", "Waterloo", "Iowa")],
        );
        let need = need_of(scope, status, "grace", author_id);
        let church = church_at("grace", "Cedar Falls", "Iowa");
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let result = apply_to_need(
            &actor, &need, &church, PriorOffer::Fresh, &message, posture, "a1".into(), "t1".into(),
        );
        if let Ok(_) = can_apply(&actor, need.sight(), &church) {
            assert!(can_view_need(&actor, need.sight(), &church));
        }
        match (posture, result) {
            (Posture::TearsDown, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, Ok(_)) => {
                assert_eq!(can_apply(&actor, need.sight(), &church), Ok(()));
            }
            (Posture::Lifts, Err(DomainError::OwnNeed | DomainError::NeedClosed | DomainError::OutsideChurch | DomainError::OutsideNeighborhood | DomainError::NotInTheBody | DomainError::InvalidInput)) => {}
            other => panic!("unexpected apply: {other:?}"),
        }
    }

    #[test]
    fn us_prop_06_close_and_decide_offers(
        steward in any::<bool>(),
        open in any::<bool>(),
    ) {
        let actor_id = if steward { "miriam" } else { "peter" };
        let role = if steward { "owner" } else { "member" };
        let viewer = viewer_of(
            User {
                id: actor_id.into(),
                name: actor_id.into(),
                email: format!("{actor_id}@ecclesia.test"),
                city: "Cedar Falls".into(),
                region: "Iowa".into(),
                bio: String::new(),
                created_at: "t0".into(),
            },
            vec![membership_of(actor_id, "grace", role, "active")],
            vec![church_at("grace", "Cedar Falls", "Iowa")],
        );
        let status = if open { NeedStatus::Open } else { NeedStatus::Closed };
        let need = need_of(NeedScope::Church, status, "grace", "miriam");
        let close = close_need(&viewer, &need);
        match (steward || actor_id == "miriam", open, close) {
            (true, true, Ok(_)) => {}
            (true, false, Err(DomainError::NeedClosed)) => {}
            (false, _, Err(DomainError::NotSteward)) => {}
            other => panic!("unexpected close: {other:?}"),
        }
        let application = Application {
            id: "a1".into(),
            need_id: need.id.clone(),
            user_id: "elena".into(),
            message: "I can cook.".into(),
            status: ApplicationStatus::Pending.as_str().into(),
            created_at: "t1".into(),
        };
        let accept = accept_application(&viewer, &need, &application);
        let decline = decline_application(&viewer, &need, &application);
        if steward || actor_id == "miriam" {
            assert!(accept.is_ok());
            assert!(decline.is_ok());
        } else {
            assert_eq!(accept, Err(DomainError::NotSteward));
            assert_eq!(decline, Err(DomainError::NotSteward));
        }
    }

    #[test]
    fn us_prop_07_endorse_is_total(
        note in junk(),
        skill in junk(),
        same in any::<bool>(),
        waiting in any::<bool>(),
        tears in any::<bool>(),
    ) {
        let from = User {
            id: "james".into(),
            name: "James".into(),
            email: "james@ecclesia.test".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        let to_id = if same { "james" } else { "ruth" };
        let to = User {
            id: to_id.into(),
            name: to_id.into(),
            email: format!("{to_id}@ecclesia.test"),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        let catalog = [Gift {
            id: "gift_hospitality".into(),
            name: "Hospitality".into(),
            category: "spiritual".into(),
        }];
        let source = SkillSource::from_catalog(&catalog, &skill);
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let queue = if waiting {
            EndorsementQueue::Waiting
        } else {
            EndorsementQueue::Clear
        };
        match source {
            Err(DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
            Ok(skill_source) => {
                let result = endorse(&from, &to, skill_source, queue, &note, posture, "e1".into(), "t1".into());
                match (posture, same, waiting, result) {
                    (Posture::TearsDown, _, _, Err(DomainError::TearsDown)) => {}
                    (Posture::Lifts, true, _, Err(DomainError::SelfAction)) => {}
                    (Posture::Lifts, false, true, Err(DomainError::DuplicateEndorsement)) => {}
                    (Posture::Lifts, false, false, Ok(effect)) => {
                        let Write::InsertEndorsement(endorsement) = &effect.writes[0] else {
                            panic!("expected endorsement");
                        };
                        assert_eq!(endorsement.from_user_id, "james");
                        assert_eq!(endorsement.to_user_id, "ruth");
                    }
                    (Posture::Lifts, false, false, Err(DomainError::InvalidInput)) => {}
                    other => panic!("unexpected endorse: {other:?}"),
                }
            }
        }
    }

    #[test]
    fn us_prop_07_endorsement_decisions_stay_with_the_named_person(
        actor in person(),
        held_named in any::<bool>(),
    ) {
        let endorsement = Endorsement {
            id: "e1".into(),
            from_user_id: "james".into(),
            to_user_id: "ruth".into(),
            gift_id: "gift_hospitality".into(),
            skill: "Hospitality".into(),
            note: "She stayed.".into(),
            status: "pending".into(),
            created_at: "t0".into(),
        };
        let held = if held_named {
            GiftOnProfile::Named
        } else {
            GiftOnProfile::Absent
        };
        let accept = accept_endorsement(&actor, &endorsement, held);
        let decline = decline_endorsement(&actor, &endorsement);
        if actor.id == "ruth" {
            assert!(accept.is_ok());
            assert!(decline.is_ok());
        } else {
            assert_eq!(accept, Err(DomainError::NotRecipient));
            assert_eq!(decline, Err(DomainError::NotRecipient));
        }
    }

    #[test]
    fn us_prop_07_declined_cards_stay_with_the_pair(viewer in "[a-z]{1,8}") {
        let card = ecclesia_sdk::prelude::EndorsementCard {
            id: "e1".into(),
            from_user_id: "james".into(),
            from_user_name: "James".into(),
            to_user_id: "ruth".into(),
            to_user_name: "Ruth".into(),
            gift_id: "gift_hospitality".into(),
            gift_name: "Hospitality".into(),
            note: "She stayed.".into(),
            status: "declined".into(),
            created_at: "t0".into(),
        };
        let cards = [card];
        let visible: Vec<_> = declined_visible_to(&viewer, &cards).collect();
        if viewer == "james" || viewer == "ruth" {
            assert_eq!(visible.len(), 1);
        } else {
            assert!(visible.is_empty());
        }
    }

    #[test]
    fn us_prop_08_profile_and_gifts_are_total(
        name in junk(),
        city in junk(),
        region in junk(),
        bio in junk(),
        gift_id in junk(),
        note in junk(),
        listed in any::<bool>(),
        tears in any::<bool>(),
    ) {
        let posture = if tears { Posture::TearsDown } else { Posture::Lifts };
        let profile = update_profile("u1", &name, &city, &region, &bio, posture);
        match (posture, profile) {
            (Posture::TearsDown, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, Ok(effect)) => {
                let Write::UpdateUser { name: stored, .. } = &effect.writes[0] else {
                    panic!("expected update");
                };
                assert_eq!(stored, name.trim());
            }
            (Posture::Lifts, Err(DomainError::InvalidInput)) => {}
            other => panic!("unexpected profile: {other:?}"),
        }
        let presence = if listed {
            CatalogPresence::Listed
        } else {
            CatalogPresence::Unknown
        };
        let add = add_gift("u1", &gift_id, presence, &note, posture);
        match (posture, listed, add) {
            (Posture::TearsDown, _, Err(DomainError::TearsDown)) => {}
            (Posture::Lifts, false, Err(DomainError::UnknownGift)) => {}
            (Posture::Lifts, true, Ok(_)) => {}
            (Posture::Lifts, true, Err(DomainError::InvalidInput)) => {}
            other => panic!("unexpected add_gift: {other:?}"),
        }
        let remove = remove_gift("u1", &gift_id);
        match remove {
            Ok(_) | Err(DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_09_join_invite_redeem_are_total(
        already in any::<bool>(),
        governor in any::<bool>(),
    ) {
        let actor = User {
            id: "peter".into(),
            name: "Peter".into(),
            email: "peter@ecclesia.test".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        let church = church_at("grace", "Cedar Falls", "Iowa");
        let existing = if already {
            Some(membership_of("peter", "grace", "member", "pending_request"))
        } else {
            None
        };
        let join = request_join(&actor, &church, existing.as_ref(), &["miriam".into()], "m1".into(), "t1".into());
        match (already, join) {
            (true, Err(DomainError::AlreadyMember)) => {}
            (false, Ok(_)) => {}
            other => panic!("unexpected join: {other:?}"),
        }
        let role = if governor { "owner" } else { "member" };
        let pastor = viewer_of(
            User {
                id: "miriam".into(),
                name: "Miriam".into(),
                email: "miriam@ecclesia.test".into(),
                city: "Cedar Falls".into(),
                region: "Iowa".into(),
                bio: String::new(),
                created_at: "t0".into(),
            },
            vec![membership_of("miriam", "grace", role, "active")],
            vec![church.clone()],
        );
        let invite = invite_member(&pastor, &church, &actor, existing.as_ref(), "m2".into(), "t1".into());
        match (governor, already, invite) {
            (false, _, Err(DomainError::NotGovernor)) => {}
            (true, true, Err(DomainError::AlreadyMember)) => {}
            (true, false, Ok(_)) => {}
            other => panic!("unexpected invite: {other:?}"),
        }
        let redeem = redeem_invite(&actor, &church, existing.as_ref(), "m3".into(), "t1".into());
        match (already, redeem) {
            (true, Err(DomainError::AlreadyMember)) => {}
            (false, Ok(_)) => {}
            other => panic!("unexpected redeem: {other:?}"),
        }
    }

    #[test]
    fn us_prop_09_membership_decisions(
        governor in any::<bool>(),
        pending in any::<bool>(),
    ) {
        let role = if governor { "owner" } else { "member" };
        let viewer = viewer_of(
            User {
                id: "miriam".into(),
                name: "Miriam".into(),
                email: "miriam@ecclesia.test".into(),
                city: "Cedar Falls".into(),
                region: "Iowa".into(),
                bio: String::new(),
                created_at: "t0".into(),
            },
            vec![membership_of("miriam", "grace", role, "active")],
            vec![church_at("grace", "Cedar Falls", "Iowa")],
        );
        let status = if pending { "pending_request" } else { "active" };
        let target = membership_of("peter", "grace", "member", status);
        let church = church_at("grace", "Cedar Falls", "Iowa");
        let approve = approve_membership(&viewer, &target, &church);
        let decline = decline_membership(&viewer, &target, &church);
        match (governor, pending, approve) {
            (false, _, Err(DomainError::NotGovernor)) => {}
            (true, false, Err(DomainError::NothingPending)) => {}
            (true, true, Ok(_)) => {}
            other => panic!("unexpected approve: {other:?}"),
        }
        match (governor, pending, decline) {
            (false, _, Err(DomainError::NotGovernor)) => {}
            (true, false, Err(DomainError::NothingPending)) => {}
            (true, true, Ok(_)) => {}
            other => panic!("unexpected decline: {other:?}"),
        }
        let stranger = accept_invite(&viewer.user, &target);
        assert_eq!(stranger, Err(DomainError::NotInvitee));
        let peter = User {
            id: "peter".into(),
            name: "Peter".into(),
            email: "peter@ecclesia.test".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        };
        assert_eq!(
            accept_invite(&peter, &target),
            Err(DomainError::NothingPending)
        );
    }

    #[test]
    fn us_prop_10_visible_cards_obey_the_rule(
        scope in prop_oneof![Just(NeedScope::Church), Just(NeedScope::Neighboring), Just(NeedScope::Body)],
    ) {
        let grace = church_at("grace", "Cedar Falls", "Iowa");
        let luke = church_at("luke", "Cedar Falls", "Iowa");
        let viewer = viewer_of(
            User {
                id: "james".into(),
                name: "James".into(),
                email: "james@ecclesia.test".into(),
                city: "Cedar Falls".into(),
                region: "Iowa".into(),
                bio: String::new(),
                created_at: "t0".into(),
            },
            vec![membership_of("james", "luke", "member", "active")],
            vec![luke.clone()],
        );
        let open = ecclesia_sdk::prelude::NeedCard {
            id: "n1".into(),
            church_id: "grace".into(),
            church_name: "Grace".into(),
            church_city: "Cedar Falls".into(),
            church_region: "Iowa".into(),
            author_id: "miriam".into(),
            author_name: "Miriam".into(),
            title: "Meals".into(),
            body: "Tuesday".into(),
            gift_id: None,
            gift_name: None,
            scope: scope.as_str().into(),
            status: "open".into(),
            created_at: "t0".into(),
        };
        let churches = [grace.clone()];
        let cards = [open];
        for card in visible_need_cards(&viewer, &cards, &churches) {
            assert!(card.is_open());
            assert!(can_view_need(&viewer, card.sight(), &grace));
        }
    }

    #[test]
    fn us_prop_11_pair_memberships_stay_aligned(
        extra in any::<bool>(),
    ) {
        let churches = [
            church_at("grace", "Cedar Falls", "Iowa"),
            church_at("luke", "Cedar Falls", "Iowa"),
        ];
        let mut memberships = vec![membership_of("miriam", "grace", "owner", "active")];
        if extra {
            memberships.push(membership_of("miriam", "missing", "member", "active"));
        }
        let paired: Vec<_> = pair_memberships(&memberships, &churches).collect();
        for (church, membership) in &paired {
            assert_eq!(church.id, membership.church_id);
        }
        let ids = unique_church_ids(&memberships);
        let mut seen = std::collections::BTreeSet::new();
        for id in &ids {
            assert!(seen.insert(*id));
        }
    }

    #[test]
    fn us_prop_12_https_endpoint_stays_https(raw in junk()) {
        match https_endpoint(&raw) {
            Ok(url) => {
                assert!(url.starts_with("https://"));
                assert!(!url.chars().any(char::is_whitespace));
            }
            Err(DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
        }
        assert!(https_endpoint(&format!("http://{raw}")).is_err());
    }

    #[test]
    fn us_prop_13_word_gate_never_panics(a in junk(), b in junk()) {
        let _ = word_gate(&[&a, &b]);
        if a.to_ascii_lowercase().contains("worthless") || b.to_ascii_lowercase().contains("worthless") {
            assert_eq!(word_gate(&[&a, &b]), Posture::TearsDown);
        }
    }

    #[test]
    fn us_prop_14_session_round_trips(
        user in "[A-Za-z0-9_-]{1,40}",
        csrf in "[a-f0-9]{32}",
        secret in "[A-Za-z0-9]{8,32}",
    ) {
        let session = Session::signed_in(user.clone(), "u1".into(), csrf);
        let raw = session.encode(&secret);
        let decoded = Session::decode(&secret, &raw).expect("decode");
        assert_eq!(decoded.session_id, session.session_id);
        assert_eq!(decoded.csrf, session.csrf);
        assert_eq!(decoded.user_id, None);
        assert_eq!(Session::decode("other-secret", &raw), None);
        let tampered = raw.replacen(&user, "x", 1);
        if tampered != raw {
            assert_eq!(Session::decode(&secret, &tampered), None);
        }
    }

    #[test]
    fn us_prop_15_unknown_flash_stays_generic(code in junk()) {
        let ok = flash_from(Some(code.clone()), None).unwrap();
        let err = flash_from(None, Some(code.clone())).unwrap();
        match code.as_str() {
            "saved" => assert_eq!(ok.text(), "Saved."),
            "welcome" => assert_eq!(ok.text(), "Account created."),
            _ => {
                if !matches!(
                    code.as_str(),
                    "joined_request" | "invited" | "redeemed" | "approved" | "declined"
                        | "need_posted" | "applied" | "application_accepted" | "need_closed"
                        | "endorsed" | "endorsement_accepted" | "endorsement_declined"
                        | "gift_added" | "gift_removed" | "church_planted" | "invite_accepted"
                ) {
                    assert_eq!(ok.text(), "Done.");
                }
            }
        }
        match code.as_str() {
            "missing" => assert_eq!(err.text(), "Fill in the required fields."),
            "csrf" => assert_eq!(err.text(), "The form expired. Try again."),
            _ => {
                if !matches!(
                    code.as_str(),
                    "email" | "auth" | "not_found" | "forbidden" | "steward" | "not_yours"
                        | "self" | "already" | "not_member" | "scope" | "own_need" | "closed"
                        | "invite" | "pending" | "bad_email" | "tone" | "rate" | "miss"
                        | "mail" | "password"
                ) {
                    assert_eq!(err.text(), "That didn't work.");
                }
            }
        }
        assert!(!ok.text().contains("<script>"));
        assert!(!err.text().contains("<script>"));
    }
}
