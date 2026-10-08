//! Property tests for church rules. Inputs are values a test can build.
//! A story returns `Ok` or `DomainError`. It does not panic.

use ecclesia_domain::{
    ADDRESS_MAX, ApplicationStatus, Church, ChurchLinkStatus, DomainError, Effect,
    EmailAvailability, EndorsementStatus, GATHERING_MAX, Gift, GiftOnProfile, MembershipRole,
    NAME_MAX, Need, NeedCard, NeedScope, NeedShelf, NeedStatus, SkillSource, VoiceKind, VoicePass,
    can_apply, can_endorse, can_view_need, churches_are_neighbors, churches_with_counts,
    coordinates, count_for, device_token, distance_km, group_churches_by_place, https_endpoint,
    invite_code_for, is_need_steward, link_after_approval, nearby_km, need_fields, normalize_email,
    note_field, notice_each_governor, optional_note, optional_text, parse_invite_email,
    person_fields, profile_fields, push_key, push_platform, require_need_view, require_text,
    rewrite_text, sample, skill_field, visible_need_cards,
};
use proptest::collection;
use proptest::prelude::*;

const TITLE_MAX: usize = 120;
const BODY_MAX: usize = 2000;
const NOTE_MAX: usize = 600;
const BIO_MAX: usize = 800;
const HTTPS_MAX: usize = 2048;
const PUSH_KEY_MAX: usize = 256;
const DEVICE_TOKEN_MAX: usize = 512;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        ..ProptestConfig::default()
    }
}

fn junk() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("   ".into()),
        Just("\t\n".into()),
        "[A-Za-z0-9 ._+-]{0,24}",
        "\\PC{0,40}",
        (0usize..90).prop_map(|n| "n".repeat(n)),
    ]
}

fn globe() -> impl Strategy<Value = (f64, f64)> {
    (-90.0f64..90.0f64, -180.0f64..180.0f64)
}

fn link_status() -> impl Strategy<Value = ChurchLinkStatus> {
    prop_oneof![
        Just(ChurchLinkStatus::Pending),
        Just(ChurchLinkStatus::Invited),
        Just(ChurchLinkStatus::Active),
    ]
}

fn role() -> impl Strategy<Value = MembershipRole> {
    prop_oneof![
        Just(MembershipRole::Owner),
        Just(MembershipRole::Steward),
        Just(MembershipRole::Member),
    ]
}

fn noise_char() -> impl Strategy<Value = char> {
    prop_oneof![Just(' '), Just('-'), Just('!'), Just('\u{e9}')]
}

fn with_noise(raw: &str, noise: char) -> String {
    let mut out = String::new();
    out.push(noise);
    for ch in raw.chars() {
        out.push(ch);
        out.push(noise);
    }
    out
}

fn need_record(scope: &str, status: &str, author_id: &str) -> Need {
    Need {
        id: "need".into(),
        church_id: "grace".into(),
        author_id: author_id.into(),
        title: "Meals".into(),
        body: "Tuesday".into(),
        gift_id: None,
        scope: scope.into(),
        status: status.into(),
        created_at: "t0".into(),
        closed_at: None,
        praise: None,
        shelf: NeedShelf::Listed,
    }
}

fn need_card(scope: &str, status: &str, author_id: &str) -> NeedCard {
    NeedCard {
        id: "n1".into(),
        church_id: "grace".into(),
        church_name: "Grace".into(),
        church_address: "100 Main Street".into(),
        author_id: author_id.into(),
        author_name: "Miriam".into(),
        title: "Meals".into(),
        body: "Tuesday".into(),
        gift_id: None,
        gift_name: None,
        scope: scope.into(),
        status: status.into(),
        created_at: "t0".into(),
        praise: None,
        shelf: NeedShelf::Listed,
    }
}

fn church_named(id: &str, address: &str) -> Church {
    let mut church = sample::church(id);
    church.address = address.into();
    church
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn us_prop_geo_01_distance_is_a_metric(
        a in globe(),
        b in globe(),
        c in globe(),
    ) {
        let ab = distance_km(a.0, a.1, b.0, b.1);
        let ba = distance_km(b.0, b.1, a.0, a.1);
        let ac = distance_km(a.0, a.1, c.0, c.1);
        let bc = distance_km(b.0, b.1, c.0, c.1);
        let bound = 6371.0 * std::f64::consts::PI + 1e-4;
        assert!(ab.is_finite() && ab >= 0.0 && ab <= bound);
        assert!((ab - ba).abs() <= 1e-6);
        assert!(distance_km(a.0, a.1, a.0, a.1).abs() <= 1e-9);
        assert!(distance_km(a.0, a.1, a.0, a.1 + 360.0).abs() <= 1e-6);
        assert!(ac <= ab + bc + 1e-3);
    }

    #[test]
    fn us_prop_geo_02_neighbors_follow_distance(
        a in globe(),
        b in globe(),
        same_id in any::<bool>(),
    ) {
        let left = sample::church_at("grace", a.0, a.1);
        let right_id = if same_id { "grace" } else { "luke" };
        let right = sample::church_at(right_id, b.0, b.1);
        let expect = left.id != right.id && distance_km(a.0, a.1, b.0, b.1) <= nearby_km();
        assert_eq!(churches_are_neighbors(&left, &right), expect);
        assert_eq!(
            churches_are_neighbors(&left, &right),
            churches_are_neighbors(&right, &left)
        );
    }

    #[test]
    fn us_prop_val_01_text_trims_or_refuses(raw in junk(), max in 1usize..80) {
        match require_text(&raw, max) {
            Ok(text) => {
                assert_eq!(text, raw.trim());
                assert!(!text.is_empty());
                assert!(text.chars().count() <= max);
                assert_eq!(require_text(&text, max).unwrap(), text);
            }
            Err(DomainError::InvalidInput) => {
                let trimmed = raw.trim();
                assert!(trimmed.is_empty() || trimmed.chars().count() > max);
            }
            Err(other) => panic!("{other:?}"),
        }
        match optional_text(&raw, max) {
            Ok(text) => {
                assert_eq!(text, raw.trim());
                assert!(text.chars().count() <= max);
            }
            Err(DomainError::InvalidInput) => {
                assert!(raw.trim().chars().count() > max);
            }
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_val_02_email_is_one_address(raw in junk()) {
        assert_eq!(parse_invite_email(&raw), normalize_email(&raw));
        match normalize_email(&raw) {
            Ok(email) => {
                assert_eq!(email.chars().filter(|ch| *ch == '@').count(), 1);
                assert_eq!(email, email.to_lowercase());
                assert!(!email.chars().any(char::is_whitespace));
                assert_eq!(normalize_email(&email).unwrap(), email);
                let (local, domain) = email.split_once('@').unwrap();
                assert!(!local.is_empty());
                assert!(domain.contains('.'));
                assert!(!domain.contains(".."));
            }
            Err(DomainError::InvalidEmail | DomainError::InvalidInput) => {}
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_val_03_person_fields_compose(
        first in junk(),
        last in junk(),
        email in junk(),
    ) {
        let first_r = require_text(&first, NAME_MAX);
        let last_r = require_text(&last, NAME_MAX);
        let email_r = normalize_email(&email);
        match person_fields(&first, &last, &email) {
            Ok((stored_first, stored_last, stored_email)) => {
                assert_eq!(stored_first, first_r.unwrap());
                assert_eq!(stored_last, last_r.unwrap());
                assert_eq!(stored_email, email_r.unwrap());
            }
            Err(error) => {
                assert!(first_r.is_err() || last_r.is_err() || email_r.is_err());
                assert!(matches!(
                    error,
                    DomainError::InvalidInput | DomainError::InvalidEmail
                ));
            }
        }
    }

    #[test]
    fn us_prop_val_04_profile_fields_compose(first in junk(), last in junk(), bio in junk()) {
        let first_r = require_text(&first, NAME_MAX);
        let last_r = require_text(&last, NAME_MAX);
        let bio_r = optional_text(&bio, BIO_MAX);
        match profile_fields(&first, &last, &bio) {
            Ok(stored) => {
                assert_eq!(
                    stored,
                    (first_r.unwrap(), last_r.unwrap(), bio_r.unwrap())
                );
            }
            Err(DomainError::InvalidInput) => {
                assert!(first_r.is_err() || last_r.is_err() || bio_r.is_err());
            }
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_val_05_coordinates_stay_on_the_globe(latitude in any::<f64>(), longitude in any::<f64>()) {
        let on_globe = latitude.is_finite()
            && longitude.is_finite()
            && (-90.0..=90.0).contains(&latitude)
            && (-180.0..=180.0).contains(&longitude);
        match coordinates(latitude, longitude) {
            Ok((stored_lat, stored_lng)) => {
                assert!(on_globe);
                assert_eq!(stored_lat, latitude);
                assert_eq!(stored_lng, longitude);
            }
            Err(DomainError::InvalidInput) => assert!(!on_globe),
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_val_06_church_and_need_fields_compose(
        name in junk(),
        address in junk(),
        description in junk(),
        gathering in junk(),
        title in junk(),
        body in junk(),
        scope in "[a-z]{0,12}",
        latitude in any::<f64>(),
        longitude in any::<f64>(),
    ) {
        let name_r = require_text(&name, TITLE_MAX);
        let address_r = require_text(&address, ADDRESS_MAX);
        let description_r = require_text(&description, BODY_MAX);
        let gathering_r = optional_text(&gathering, GATHERING_MAX);
        let coord_r = coordinates(latitude, longitude);
        let name_has_letter = name_r
            .as_ref()
            .ok()
            .is_some_and(|stored| stored.chars().any(|ch| ch.is_ascii_alphanumeric()));
        let church_ok = name_has_letter
            && address_r.is_ok()
            && description_r.is_ok()
            && gathering_r.is_ok()
            && coord_r.is_ok();
        match ecclesia_domain::church_fields(
            &name,
            &address,
            latitude,
            longitude,
            &description,
            &gathering,
        ) {
            Ok((stored_name, stored_address, stored_lat, stored_lng, stored_description, stored_gathering)) => {
                assert!(church_ok);
                assert_eq!(stored_name, name_r.unwrap());
                assert_eq!(stored_address, address_r.unwrap());
                assert_eq!((stored_lat, stored_lng), coord_r.unwrap());
                assert_eq!(stored_description, description_r.unwrap());
                assert_eq!(stored_gathering, gathering_r.unwrap());
            }
            Err(DomainError::InvalidInput) => assert!(!church_ok),
            Err(other) => panic!("{other:?}"),
        }

        let title_r = require_text(&title, TITLE_MAX);
        let body_r = require_text(&body, BODY_MAX);
        let scope_r = NeedScope::parse(&scope);
        match need_fields(&title, &body, &scope) {
            Ok((stored_title, stored_body, stored_scope)) => {
                assert_eq!(stored_title, title_r.unwrap());
                assert_eq!(stored_body, body_r.unwrap());
                assert_eq!(stored_scope, scope_r.unwrap());
            }
            Err(DomainError::InvalidInput) => {
                assert!(title_r.is_err() || body_r.is_err() || scope_r.is_none());
            }
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_val_07_notes_and_push_fields(raw in junk()) {
        assert_eq!(note_field(&raw), require_text(&raw, NOTE_MAX));
        assert_eq!(skill_field(&raw), require_text(&raw, TITLE_MAX));
        assert_eq!(optional_note(&raw), optional_text(&raw, NOTE_MAX));
        assert_eq!(rewrite_text(&raw), optional_text(&raw, BODY_MAX));
        assert_eq!(device_token(&raw), require_text(&raw, DEVICE_TOKEN_MAX));
        match require_text(&raw, PUSH_KEY_MAX) {
            Ok(text) if text.chars().all(push_key_char) => {
                assert_eq!(push_key(&raw).unwrap(), text);
            }
            _ => assert_eq!(push_key(&raw), Err(DomainError::InvalidInput)),
        }
        match raw.trim() {
            "web" | "ios" | "android" => assert_eq!(push_platform(&raw).unwrap(), raw.trim()),
            _ => assert_eq!(push_platform(&raw), Err(DomainError::InvalidInput)),
        }
    }

    #[test]
    fn us_prop_val_08_https_endpoint_stays_https(raw in junk()) {
        match https_endpoint(&raw) {
            Ok(url) => {
                let rest = url.strip_prefix("https://").unwrap();
                assert!(!rest.is_empty());
                assert!(!url.chars().any(endpoint_forbidden));
                assert!(url.chars().count() <= HTTPS_MAX);
                assert_eq!(url, raw.trim());
                assert_eq!(https_endpoint(&url).unwrap(), url);
            }
            Err(DomainError::InvalidInput) => {
                let trimmed = raw.trim();
                let rest = trimmed.strip_prefix("https://").unwrap_or("");
                let refused = trimmed.is_empty()
                    || trimmed.chars().count() > HTTPS_MAX
                    || !trimmed.starts_with("https://")
                    || rest.is_empty()
                    || trimmed.chars().any(endpoint_forbidden);
                assert!(refused);
            }
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_code_01_invite_code_ignores_noise(
        name in "[A-Za-z0-9]{0,12}",
        nonce in "[A-Za-z0-9]{0,12}",
        noise in noise_char(),
    ) {
        let plain = invite_code_for(&name, &nonce);
        assert_eq!(plain, invite_code_for(&with_noise(&name, noise), &with_noise(&nonce, noise)));
        assert_eq!(plain, invite_code_for(&name.to_ascii_uppercase(), &nonce.to_ascii_uppercase()));
        assert_eq!(plain, plain.to_ascii_lowercase());
        assert_eq!(plain.chars().count(), 6);
        assert!(plain.chars().all(|ch| "23456789abcdefghjkmnpqrstuvwxyz".contains(ch)));
    }

    #[test]
    fn us_prop_dir_01_counts_and_places_keep_every_church(
        ids in collection::vec("[a-z]{1,6}", 1..5),
        addresses in collection::vec("[A-Za-z0-9 ]{1,18}", 1..5),
        members in collection::vec(0i64..20, 1..5),
        needs in collection::vec(0i64..10, 1..5),
    ) {
        let len = ids.len().min(addresses.len()).min(members.len()).min(needs.len());
        prop_assume!(
            ids[..len]
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == len
        );
        prop_assume!(
            addresses[..len]
                .iter()
                .map(|address| address.trim())
                .collect::<std::collections::HashSet<_>>()
                .len()
                == len
        );
        let churches: Vec<Church> = (0..len)
            .map(|index| church_named(&ids[index], addresses[index].trim()))
            .collect();
        let counts: Vec<(String, i64, i64)> = (0..len)
            .map(|index| (ids[index].clone(), members[index], needs[index]))
            .collect();
        assert_eq!(count_for(&counts, "missing"), (0, 0));
        if len > 0 {
            assert_eq!(count_for(&counts, &ids[0]), (members[0], needs[0]));
        }
        let cards = churches_with_counts(churches, &counts);
        assert_eq!(cards.len(), len);
        for (index, card) in cards.iter().enumerate() {
            assert_eq!(card.0.id, ids[index]);
            assert_eq!((card.1, card.2), (members[index], needs[index]));
        }
        let groups = group_churches_by_place(cards.iter().cloned());
        let mut seen = 0;
        let mut places = Vec::new();
        for group in &groups {
            assert!(!group.is_empty());
            let address = group[0].0.address.clone();
            assert!(!places.contains(&address));
            places.push(address.clone());
            for card in group {
                assert_eq!(card.0.address, address);
                seen += 1;
            }
        }
        assert_eq!(seen, len);
    }

    #[test]
    fn us_prop_enum_01_tokens_round_trip(token in "[a-z_]{0,16}") {
        for scope in [NeedScope::Church, NeedScope::Neighboring, NeedScope::Body] {
            assert_eq!(NeedScope::parse(scope.as_str()), Some(scope));
        }
        for status in [NeedStatus::Open, NeedStatus::Closed] {
            assert_eq!(NeedStatus::parse(status.as_str()), Some(status));
        }
        for status in [ApplicationStatus::Pending, ApplicationStatus::Accepted, ApplicationStatus::Declined] {
            assert_eq!(ApplicationStatus::parse(status.as_str()), Some(status));
        }
        for status in [EndorsementStatus::Pending, EndorsementStatus::Accepted, EndorsementStatus::Declined] {
            assert_eq!(EndorsementStatus::parse(status.as_str()), Some(status));
        }
        for kind in [VoiceKind::Need, VoiceKind::Offer, VoiceKind::Endorsement, VoiceKind::GiftNote, VoiceKind::Bio, VoiceKind::Church, VoiceKind::Prayer, VoiceKind::Reply, VoiceKind::Praise] {
            assert_eq!(VoiceKind::parse(kind.as_str()), Some(kind));
        }
        match NeedScope::parse(&token) {
            Some(scope) => assert_eq!(scope.as_str(), token),
            None => assert!(!matches!(token.as_str(), "church" | "neighboring" | "body")),
        }
        let pass = VoicePass::parse(&token);
        assert_eq!(VoicePass::parse(pass.as_str()), pass);
        if token.trim() == "publish" {
            assert_eq!(pass, VoicePass::Publish);
        } else {
            assert_eq!(pass, VoicePass::Review);
        }
    }

    #[test]
    fn us_prop_enum_02_named_states(existing in proptest::option::of(any::<u8>()), role in role(), status in link_status()) {
        if existing.is_some() {
            assert_eq!(
                EmailAvailability::of_existing(existing),
                EmailAvailability::Taken
            );
        } else {
            assert_eq!(
                EmailAvailability::of_existing(existing),
                EmailAvailability::Free
            );
        }
        assert_eq!(role.can_govern(), matches!(role, MembershipRole::Owner | MembershipRole::Steward));
        assert_eq!(status.is_waiting(), matches!(status, ChurchLinkStatus::Pending | ChurchLinkStatus::Invited));
        match link_after_approval(status) {
            Ok(ChurchLinkStatus::Active) => assert!(status.is_waiting()),
            Err(DomainError::NothingPending) => assert!(!status.is_waiting()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_prop_enum_03_gift_presence(
        ids in collection::vec("[a-z]{0,8}", 0..6),
        gift_id in "[a-z]{0,8}",
    ) {
        let named = !gift_id.is_empty() && ids.iter().any(|id| id == &gift_id);
        let presence = GiftOnProfile::of_ids(&ids, &gift_id);
        if named {
            assert_eq!(presence, GiftOnProfile::Named);
        } else {
            assert_eq!(presence, GiftOnProfile::Absent);
        }
    }

    #[test]
    fn us_prop_rule_01_endorse_refuses_the_same_person(from in "[a-z]{0,8}", to in "[a-z]{0,8}") {
        if from == to {
            assert_eq!(can_endorse(&from, &to), Err(DomainError::SelfAction));
        } else {
            assert_eq!(can_endorse(&from, &to), Ok(()));
        }
    }

    #[test]
    fn us_prop_rule_02_sight_agrees_with_apply(
        role_name in prop_oneof![Just("owner"), Just("steward"), Just("member"), Just("visitor")],
        status_name in prop_oneof![Just("active"), Just("pending"), Just("invited"), Just("gone")],
        scope in prop_oneof![Just("church"), Just("neighboring"), Just("body"), Just("wide")],
        need_status in prop_oneof![Just("open"), Just("closed"), Just("later")],
        same_church in any::<bool>(),
        author in any::<bool>(),
        near in prop_oneof![Just(0u8), Just(1u8), Just(2u8)],
        listed in any::<bool>(),
    ) {
        let grace = sample::church_at("grace", 42.5349, -92.4453);
        let (lat, lng) = match near {
            0 => (42.5349, -92.4453),
            1 => (42.4928, -92.3426),
            _ => (30.2672, -97.7431),
        };
        let home_id = if same_church { "grace" } else { "luke" };
        let home = sample::church_at(home_id, lat, lng);
        let person = sample::user_in_church("ada", home_id, role_name, status_name);
        let viewer = sample::viewer_of(person, Some(home));
        let author_id = if author { "ada" } else { "miriam" };
        let need = need_record(scope, need_status, author_id);
        let sight = need.sight();
        let allowed = can_view_need(&viewer, sight, &grace);
        assert_eq!(require_need_view(&viewer, sight, &grace).is_ok(), allowed);
        assert_eq!(
            is_need_steward(&viewer, sight),
            viewer.user.id == need.author_id || viewer.can_govern(&need.church_id)
        );
        if viewer.user.id == need.author_id || viewer.can_govern(&need.church_id) {
            assert!(allowed);
        }
        if !need.is_open() {
            assert_eq!(can_apply(&viewer, sight, &grace), Err(DomainError::NeedClosed));
        } else if viewer.user.id == need.author_id {
            assert_eq!(can_apply(&viewer, sight, &grace), Err(DomainError::OwnNeed));
        }
        if can_apply(&viewer, sight, &grace).is_ok() {
            assert!(allowed);
            assert!(viewer.is_active_anywhere());
        }
        let card = need_card(scope, need_status, author_id);
        let churches = [grace.clone()];
        let church_list: &[Church] = if listed { &churches } else { &[] };
        let cards = [card];
        let visible: Vec<&NeedCard> = visible_need_cards(&viewer, &cards, church_list).collect();
        let expect = listed && cards[0].is_open() && can_view_need(&viewer, cards[0].sight(), &grace);
        assert_eq!(visible.len() == 1, expect);
    }

    #[test]
    fn us_prop_skill_01_catalog_match_ignores_case_and_space(
        gap in prop_oneof![Just(""), Just(" "), Just("  ")],
    ) {
        let catalog = [Gift {
            id: "gift_hospitality".into(),
            name: "Hospitality".into(),
            category: "care".into(),
        }];
        let typed = format!("hos{gap}pitality");
        let source = SkillSource::from_catalog(&catalog, &typed).unwrap();
        assert_eq!(source.gift_id(), "gift_hospitality");
        assert_eq!(source.display(), "Hospitality");
        assert_eq!(
            SkillSource::from_catalog(&catalog, "   "),
            Err(DomainError::InvalidInput)
        );
        let spoken = SkillSource::from_catalog(&catalog, "organ").unwrap();
        assert_eq!(spoken.display(), "organ");
        assert_eq!(spoken.gift_id(), "");
    }

    #[test]
    fn us_prop_notice_01_each_governor_gets_one_notice(ids in collection::vec("[a-z]{1,8}", 0..6)) {
        let mut effect = Effect {
            writes: Vec::new(),
            notices: Vec::new(),
        };
        notice_each_governor(&mut effect, &ids, "join", "A member is waiting".into(), "Review the request.", "/church");
        assert_eq!(effect.notices.len(), ids.len());
        for (draft, id) in effect.notices.iter().zip(ids.iter()) {
            assert_eq!(draft.user_id, *id);
            assert_eq!(draft.kind, "join");
            assert_eq!(draft.href, "/church");
        }
    }

    #[test]
    fn us_prop_err_01_flash_codes_are_tokens(index in 0usize..34) {
        let error = domain_error(index);
        let code = error.flash_code();
        assert!(!code.is_empty());
        assert!(code.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'));
        assert!(!code.contains(' '));
    }
}

fn push_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '=' | '+' | '/')
}

fn endpoint_forbidden(ch: char) -> bool {
    ch.is_whitespace() || ch == '<' || ch == '>' || ch == '"'
}

fn domain_error(index: usize) -> DomainError {
    match index {
        0 => DomainError::SelfAction,
        1 => DomainError::NotInTheBody,
        2 => DomainError::OutsideChurch,
        3 => DomainError::OutsideNeighborhood,
        4 => DomainError::NeedClosed,
        5 => DomainError::AlreadyApplied,
        6 => DomainError::OwnNeed,
        7 => DomainError::NotGovernor,
        8 => DomainError::NotSteward,
        9 => DomainError::NotRecipient,
        10 => DomainError::NotInvitee,
        11 => DomainError::NothingPending,
        12 => DomainError::AlreadyMember,
        13 => DomainError::DuplicateEndorsement,
        14 => DomainError::InvalidInput,
        15 => DomainError::InvalidEmail,
        16 => DomainError::EmailTaken,
        17 => DomainError::UnknownGift,
        18 => DomainError::NotFound,
        19 => DomainError::WeakPassword,
        20 => DomainError::TearsDown,
        21 => DomainError::AlreadyMarked,
        22 => DomainError::NotAuthor,
        23 => DomainError::PrayerAnswered,
        24 => DomainError::NoChurch,
        25 => DomainError::InvalidEin,
        26 => DomainError::InvalidRegistry,
        27 => DomainError::InvalidPostal,
        28 => DomainError::InvalidService,
        29 => DomainError::PastorHoldsChurch,
        30 => DomainError::AlreadyPastor,
        31 => DomainError::NeedOpen,
        32 => DomainError::NeedArchived,
        33 => DomainError::ChurchStillOpen,
        _ => DomainError::OutsideChurch,
    }
}
