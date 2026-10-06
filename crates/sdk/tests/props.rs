//! Property tests for pure SDK functions: SQL, sessions, bearers, search, and mail.

use ecclesia_sdk::bearer::{
    ACCESS_SECONDS, access_exp_unix, decode_access, encode_access, hash_refresh_wire,
};
use ecclesia_sdk::cache::keys_for_write;
use ecclesia_sdk::chat::chat_completions_url;
use ecclesia_sdk::church_search::rank_churches;
use ecclesia_sdk::db::{
    Driver, is_local_database_url, require_public_database_url, rewrite_placeholders,
};
use ecclesia_sdk::host::{HostKind, mail_env_from};
use ecclesia_sdk::judge::word_gate;
use ecclesia_sdk::password::{hash_token, score};
use ecclesia_sdk::prelude::{Church, Need, NeedShelf, Posture, Strength, User, Write};
use ecclesia_sdk::session::{CookieTransport, Session};
use ecclesia_sdk::web_push::audience;
use proptest::collection;
use proptest::prelude::*;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        ..ProptestConfig::default()
    }
}

fn sql_piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("?".into()),
        Just("'?'".into()),
        Just("''".into()),
        Just("'it''s'".into()),
        "[A-Za-z0-9_]{1,8}",
        Just(" = ".into()),
        Just(", ".into()),
    ]
}

fn church(id: &str, name: &str, address: &str) -> Church {
    Church {
        id: id.into(),
        name: name.into(),
        address: address.into(),
        latitude: 42.53,
        longitude: -92.45,
        country: "US".into(),
        description: String::new(),
        gathering: String::new(),
        ein: "12-3456789".into(),
        registry_state: "IA".into(),
        registry_number: "123456".into(),
        owner_id: "o".into(),
        invite_code: "code".into(),
        created_at: "t0".into(),
    }
}

fn user(id: &str) -> User {
    User {
        id: id.into(),
        first_name: "Ada".into(),
        last_name: "Lane".into(),
        email: format!("{id}@ecclesia.test"),
        bio: String::new(),
        created_at: "t0".into(),
        memberships: Vec::new(),
    }
}

fn need(church_id: &str) -> Need {
    Need {
        id: "n1".into(),
        church_id: church_id.into(),
        author_id: "ada".into(),
        title: "Meals".into(),
        body: "Tuesday".into(),
        gift_id: None,
        scope: "church".into(),
        status: "open".into(),
        created_at: "t0".into(),
        closed_at: None,
        praise: None,
        shelf: NeedShelf::Listed,
    }
}

fn assert_placeholder_rewrite(sql: &str, out: &str) {
    let mut input = sql.chars().peekable();
    let mut output = out.chars().peekable();
    let mut in_single = false;
    let mut n = 0u32;
    while let Some(ch) = input.next() {
        if ch == '\'' {
            assert_eq!(output.next(), Some('\''));
            if in_single {
                if input.peek() == Some(&'\'') {
                    input.next();
                    assert_eq!(output.next(), Some('\''));
                } else {
                    in_single = false;
                }
            } else {
                in_single = true;
            }
            continue;
        }
        if ch == '?' && !in_single {
            n += 1;
            assert_eq!(output.next(), Some('$'));
            for digit in n.to_string().chars() {
                assert_eq!(output.next(), Some(digit));
            }
            continue;
        }
        assert_eq!(output.next(), Some(ch));
    }
    assert_eq!(output.next(), None);
}

fn split_on_char(text: &str, index: usize) -> (&str, &str) {
    let mut at = text.len();
    for (seen, (byte, _)) in text.char_indices().enumerate() {
        if seen == index {
            at = byte;
            break;
        }
    }
    text.split_at(at)
}

fn leet(ch: char) -> char {
    match ch {
        'a' => '@',
        'e' => '3',
        'i' => '1',
        'o' => '0',
        's' => '$',
        't' => '7',
        other => other,
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn us_prop_sql_02_question_marks_become_postgres_numbers(parts in collection::vec(sql_piece(), 0..12)) {
        let sql = parts.concat();
        let once = rewrite_placeholders(&sql);
        assert_eq!(rewrite_placeholders(&once), once);
        assert_eq!(Driver::Sqlite.sql(&sql).as_ref(), sql);
        assert_eq!(Driver::Postgres.sql(&sql).as_ref(), once);
        assert_placeholder_rewrite(&sql, &once);
    }

    #[test]
    fn us_prop_sql_03_sqlite_is_local_and_remote_hosts_are_not(
        path in "[a-z0-9./_-]{0,24}",
        label in "[a-z]{1,12}",
        local in prop_oneof![Just("localhost"), Just("127.0.0.1"), Just("host.docker.internal")],
    ) {
        assert!(is_local_database_url(&format!("sqlite://{path}")));
        assert!(is_local_database_url(&format!("sqlite:{path}")));
        assert!(is_local_database_url(&format!("postgres://user:secret@{local}:5432/ecclesia")));
        prop_assume!(label != "localhost");
        assert!(!is_local_database_url(&format!("postgres://user:secret@{label}.neon.tech/ecclesia")));
        assert!(require_public_database_url(Some("postgres://neon.example/ecclesia")).is_ok());
        assert!(require_public_database_url(Some("sqlite://ecclesia.db")).is_err());
        assert!(require_public_database_url(None).is_err());
    }

    #[test]
    fn us_prop_session_01_signed_cookie_round_trips(
        session_id in "[A-Za-z0-9_-]{1,80}",
        csrf in "[0-9a-f]{32}",
        secret in "\\PC{0,32}",
        other in "\\PC{0,32}",
    ) {
        let session = Session::signed_in(session_id.clone(), "user-1".into(), csrf.clone());
        let raw = session.encode(&secret);
        let decoded = Session::decode(&secret, &raw).expect("decode");
        assert_eq!(decoded.session_id.as_deref(), Some(session_id.as_str()));
        assert_eq!(decoded.csrf, csrf);
        assert_eq!(decoded.user_id, None);
        assert!(decoded.check_csrf(&csrf));
        assert!(!decoded.check_csrf(""));
        prop_assume!(secret != other);
        assert_eq!(Session::decode(&other, &raw), None);
        let tampered = flip_last(&raw);
        assert_eq!(Session::decode(&secret, &tampered), None);
    }

    #[test]
    fn us_prop_session_02_decode_is_total(secret in "\\PC{0,16}", raw in "\\PC{0,120}") {
        match Session::decode(&secret, &raw) {
            None => {}
            Some(session) => {
                assert_eq!(session.csrf.len(), 32);
                assert!(session.csrf.chars().all(|ch| ch.is_ascii_hexdigit()));
                assert_eq!(session.user_id, None);
                let again = Session::decode(&secret, &session.encode(&secret)).expect("re-encode");
                assert_eq!(again.session_id, session.session_id);
                assert_eq!(again.csrf, session.csrf);
            }
        }
    }

    #[test]
    fn us_prop_session_03_csrf_matches_the_whole_token(
        csrf in "[0-9a-f]{32}",
        submitted in "\\PC{0,40}",
    ) {
        let session = Session::guest(csrf.clone());
        assert_eq!(session.check_csrf(&submitted), submitted == csrf);
        assert_eq!(
            CookieTransport::from_env_value(Some("1")),
            CookieTransport::Secure
        );
    }

    #[test]
    fn us_prop_session_04_cookie_transport(value in proptest::option::of("\\PC{0,16}")) {
        let transport = CookieTransport::from_env_value(value.as_deref());
        let secure = match value.as_deref().map(str::trim) {
            Some("1") => true,
            Some(text) if text.eq_ignore_ascii_case("true") => true,
            _ => false,
        };
        if secure {
            assert_eq!(transport, CookieTransport::Secure);
        } else {
            assert_eq!(transport, CookieTransport::Plain);
        }
    }

    #[test]
    fn us_prop_bearer_01_access_token_round_trips(
        session_id in "[A-Za-z0-9_-]{1,80}",
        exp in -1_000_000_000i64..2_000_000_000i64,
        secret in "\\PC{1,24}",
        other in "\\PC{1,24}",
    ) {
        let raw = encode_access(&secret, &session_id, exp);
        let (decoded_id, decoded_exp) = decode_access(&secret, &raw).expect("decode");
        assert_eq!(decoded_id, session_id);
        assert_eq!(decoded_exp, exp);
        prop_assume!(secret != other);
        assert_eq!(decode_access(&other, &raw), None);
        assert_eq!(decode_access(&secret, &flip_last(&raw)), None);
        assert_eq!(decode_access(&secret, &raw.replace(&session_id, &format!("{session_id}."))), None);
    }

    #[test]
    fn us_prop_bearer_02_decode_is_total(secret in "\\PC{0,16}", raw in "\\PC{0,160}") {
        if let Some((session_id, exp)) = decode_access(&secret, &raw) {
            assert!(!session_id.is_empty());
            assert!(!session_id.contains('.'));
            let again = decode_access(&secret, &encode_access(&secret, &session_id, exp)).expect("re-encode");
            assert_eq!(again.0, session_id);
            assert_eq!(again.1, exp);
        }
    }

    #[test]
    fn us_prop_bearer_03_expiry_is_fifteen_minutes(now in -1_000_000_000i64..1_800_000_000_000i64) {
        assert_eq!(access_exp_unix(now), now + ACCESS_SECONDS);
        assert_eq!(ACCESS_SECONDS, 900);
    }

    #[test]
    fn us_prop_token_01_hash_is_stable_hex(left in "\\PC{0,48}", right in "\\PC{0,48}") {
        let hashed = hash_token(&left);
        assert_eq!(hashed, hash_refresh_wire(&left));
        assert_eq!(hash_token(&left), hashed);
        assert_eq!(hashed.len(), 64);
        assert!(hashed.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()));
        prop_assume!(left != right);
        assert_ne!(hashed, hash_token(&right));
    }

    #[test]
    fn us_prop_tone_01_word_gate_folds_letters(
        letters in "[a-z]{0,24}",
        cut in 0usize..24,
    ) {
        let (head, tail) = split_on_char(&letters, cut);
        assert_eq!(word_gate(&[&letters]), word_gate(&[head, tail]));
        assert_eq!(word_gate(&[&letters]), word_gate(&[&letters.to_ascii_uppercase()]));
        let folded: String = letters.chars().map(leet).collect();
        assert_eq!(word_gate(&[&letters]), word_gate(&[&folded]));
        let spaced: String = "worthless".chars().flat_map(|ch| [ch, ' ']).collect();
        assert_eq!(word_gate(&[&spaced]), Posture::TearsDown);
        assert_eq!(word_gate(&["w0rthless"]), Posture::TearsDown);
        assert_eq!(word_gate(&[""]), Posture::Lifts);
    }

    #[test]
    fn us_prop_search_01_rank_respects_limit_and_case(
        name in "[A-Za-z][A-Za-z0-9]{0,20}",
        extras in collection::vec("[A-Za-z][A-Za-z0-9]{2,10}", 0..4),
        limit in 0usize..8,
    ) {
        let mut churches = vec![church("target", &name, "Cedar Falls")];
        for (index, extra) in extras.iter().enumerate() {
            if extra.eq_ignore_ascii_case(&name) {
                continue;
            }
            churches.push(church(&format!("id{index}"), extra, "Oak Street"));
        }
        assert!(rank_churches("   ", &churches, limit).is_empty());
        assert!(rank_churches(&name, &churches, 0).is_empty());
        let hits = rank_churches(&name, &churches, limit);
        assert!(hits.len() <= limit);
        let mut seen = Vec::new();
        for hit in &hits {
            assert!(churches.iter().any(|item| item.id == hit.id));
            assert!(!seen.contains(&hit.id));
            seen.push(hit.id.clone());
        }
        let padded = format!("  {name}  ");
        let folded = rank_churches(&name.to_ascii_uppercase(), &churches, limit);
        assert_eq!(hit_ids(&hits), hit_ids(&rank_churches(&padded, &churches, limit)));
        assert_eq!(hit_ids(&hits), hit_ids(&folded));
        if limit > 0 {
            assert_eq!(hits.first().map(|hit| hit.id.as_str()), Some("target"));
        }
    }

    #[test]
    fn us_prop_url_01_chat_and_push_origins(
        host in "[a-z]{1,12}",
        scheme in prop_oneof![Just("https"), Just("http")],
        slashes in 0usize..4,
    ) {
        let base = format!("{scheme}://{host}.example/v1");
        let with_slashes = format!("{base}{}", "/".repeat(slashes));
        let chat = chat_completions_url(&with_slashes);
        assert!(chat.ends_with("/chat/completions"));
        assert_eq!(chat_completions_url(&chat), chat);
        let endpoint = format!("{scheme}://{host}.example/push/v1");
        assert_eq!(audience(&endpoint).unwrap(), format!("{scheme}://{host}.example"));
    }

    #[test]
    fn us_prop_url_02_audience_is_total(raw in "\\PC{0,60}") {
        if let Ok(origin) = audience(&raw) {
            assert!(origin.contains("://"));
            let rest = origin.split_once("://").unwrap().1;
            assert!(!rest.is_empty());
            assert!(!rest.contains('/'));
        }
    }

    #[test]
    fn us_prop_mail_01_public_host_requires_mail_env(
        key in proptest::option::of("[a-z]{1,8}"),
        from in proptest::option::of("[a-z]{1,8}"),
        public_url in proptest::option::of("https://[a-z]{1,8}\\.test"),
        slashes in 0usize..4,
    ) {
        let listen = format!("http://127.0.0.1:43781{}", "/".repeat(slashes));
        let local = mail_env_from(HostKind::Local, key.clone(), from.clone(), public_url.clone(), &listen).unwrap();
        let expect_origin = public_url.clone().unwrap_or_else(|| "http://127.0.0.1:43781".into());
        assert_eq!(local.origin, expect_origin);
        assert_eq!(local.api_key, key);
        assert_eq!(local.from, from);
        match mail_env_from(HostKind::Public, key.clone(), from.clone(), public_url.clone(), &listen) {
            Ok(env) => {
                assert!(key.is_some() && from.is_some() && public_url.is_some());
                assert_eq!(env.api_key, key);
                assert_eq!(env.from, from);
                assert_eq!(env.origin, public_url.unwrap());
            }
            Err(_) => assert!(key.is_none() || from.is_none() || public_url.is_none()),
        }
    }

    #[test]
    fn us_prop_cache_01_write_keys_name_the_church(
        church_id in "[a-z0-9]{1,12}",
        next in "[a-z0-9]{1,12}",
        previous in proptest::option::of("[a-z0-9]{1,12}"),
    ) {
        let inserted = keys_for_write(&Write::InsertChurch(church(&church_id, &church_id, "100 Main Street")), None);
        assert_eq!(inserted, vec!["directory".to_string(), format!("church:{church_id}")]);
        let need_keys = keys_for_write(&Write::InsertNeed(need(&church_id)), None);
        assert_eq!(need_keys, vec![format!("church:{church_id}"), "directory".into()]);
        assert!(keys_for_write(&Write::InsertUser(user("ada")), previous.as_deref()).is_empty());
        let link = Write::UpsertMembership {
            user_id: "ada".into(),
            church_id: next.clone(),
            status: "pending".into(),
            role: "member".into(),
        };
        let keys = keys_for_write(&link, previous.as_deref());
        assert_eq!(keys.first().map(String::as_str), Some("directory"));
        assert!(keys.iter().any(|key| key == &format!("church:{next}")));
        if let Some(id) = &previous {
            if id != &next {
                assert!(keys.iter().any(|key| key == &format!("church:{id}")));
            }
        }
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
        let status_keys = keys_for_write(
            &Write::SetNeedStatus {
                id: "n1".into(),
                status: "closed",
                closed_at: None,
                praise: None,
            },
            previous.as_deref(),
        );
        match &previous {
            Some(id) => assert_eq!(status_keys, vec![format!("church:{id}"), "directory".into()]),
            None => assert!(status_keys.is_empty()),
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 12, max_shrink_iters: 64, ..ProptestConfig::default() })]

    #[test]
    fn us_prop_auth_01_password_score_stays_in_range(password in "\\PC{0,40}", name in "[A-Za-z]{0,12}", email in "[a-z]{0,12}@x.test") {
        let strength = score(&password, &name, &email);
        assert!(matches!(strength, Strength::TooGuessable | Strength::Acceptable));
        if password.is_empty() || password.chars().count() > 128 {
            assert_eq!(strength, Strength::TooGuessable);
        }
    }

    #[test]
    fn us_prop_auth_02_overlong_password_is_too_guessable(len in 129usize..160) {
        let password = "a".repeat(len);
        assert_eq!(score(&password, "Ada", "ada@x.test"), Strength::TooGuessable);
    }
}

fn flip_last(raw: &str) -> String {
    let mut chars: Vec<char> = raw.chars().collect();
    if let Some(last) = chars.last_mut() {
        *last = if *last == 'a' { 'b' } else { 'a' };
    }
    chars.into_iter().collect()
}

fn hit_ids<'a>(hits: &[&'a Church]) -> Vec<&'a str> {
    hits.iter().map(|church| church.id.as_str()).collect()
}
