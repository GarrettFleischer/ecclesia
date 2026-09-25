//! A person who does not read the labels. Empty submits, junk ids, XSS,
//! their own need, a church name of bangs. The app should stay up and speak
//! plainly.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use ecclesia::http::{AppState, router};
use ecclesia_sdk::db::Db;
use tower::ServiceExt;

const PASS: &str = "Thursday dinners at six oclock";

static TEST_DB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone)]
struct World {
    app: axum::Router,
    sdk: ecclesia_sdk::Sdk,
}

async fn app() -> World {
    let path = std::env::temp_dir().join(format!(
        "ecclesia-chaos-{}-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEST_DB.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let db = Db::connect(&format!("sqlite://{}", path.display()))
        .await
        .expect("test database");
    let sdk = ecclesia_sdk::Sdk::assemble(
        db,
        ecclesia_sdk::judge::JudgeHub::word_gate(),
        ecclesia_sdk::refine::RefineHub::silent(),
        ecclesia_sdk::push::PushHub::silent(),
        ecclesia_sdk::Cache::memory(),
    );
    World {
        app: router(AppState {
            sdk: sdk.clone(),
            secret: "test-secret".into(),
            cookie: ecclesia_sdk::session::CookieTransport::Plain,
            origin: "http://127.0.0.1:43781".into(),
        }),
        sdk,
    }
}

fn try_cookie(response: &axum::http::Response<Body>) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("ecclesia_sid="))
        .map(|value| value.split(';').next().unwrap().to_string())
}

fn csrf_from(html: &str) -> Option<String> {
    let start = html.find(r#"name="csrf""#)?;
    let window = html.get(start..)?.get(..120).unwrap_or(&html[start..]);
    for key in ["value=\"", "content=\""] {
        if let Some(at) = window.find(key) {
            let rest = &window[at + key.len()..];
            let token = rest.split('"').next()?;
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn get_ok(app: axum::Router, cookie: Option<&str>, uri: &str) -> (String, String, String) {
    let (status, html, cookie, csrf) = get_any(app, cookie, uri).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}");
    (html, cookie, csrf.expect("csrf"))
}

async fn get_any(
    app: axum::Router,
    cookie: Option<&str>,
    uri: &str,
) -> (StatusCode, String, String, Option<String>) {
    let mut request = Request::get(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let next_cookie = try_cookie(&response)
        .or_else(|| cookie.map(ToOwned::to_owned))
        .unwrap_or_default();
    let html = body_string(response).await;
    let csrf = csrf_from(&html);
    (status, html, next_cookie, csrf)
}

fn enc(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b' ' => out.push('+'),
            b'&' | b'=' | b'%' => out.push_str(&format!("%{byte:02X}")),
            _ => out.push(byte as char),
        }
    }
    out
}

async fn register(world: &World, name: &str, email: &str) -> String {
    let (_html, cookie, csrf) = get_ok(world.app.clone(), None, "/register").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/register",
        format!(
            "csrf={csrf}&name={}&email={}&city=Cedar+Falls&region=Iowa&bio=I+cook&password={}",
            enc(name),
            enc(email),
            enc(PASS)
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    try_cookie(&response).expect("register cookie")
}

async fn plant(world: &World, cookie: &str, name: &str) -> (String, String) {
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(cookie), "/churches/new").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/churches",
        format!(
            "csrf={csrf}&name={}&city=Cedar+Falls&region=Iowa&gathering=Sunday+at+10.&description=A+church+on+Main+Street.",
            enc(name)
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = location_of(&response);
    let church_id = location
        .split("/churches/")
        .nth(1)
        .and_then(|rest| rest.split(['?', '/']).next())
        .expect("church id")
        .to_string();
    (try_cookie(&response).unwrap_or(cookie), church_id)
}

async fn post_need(
    world: &World,
    cookie: &str,
    church_id: &str,
    title: &str,
    body: &str,
) -> String {
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(cookie), "/needs/new").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/needs",
        format!(
            "csrf={csrf}&church_id={}&title={}&body={}&scope=church",
            enc(church_id),
            enc(title),
            enc(body)
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    location_of(&response)
        .split("/needs/")
        .nth(1)
        .and_then(|rest| rest.split(['?', '/']).next())
        .expect("need id")
        .to_string()
}

fn location_of(response: &axum::http::Response<Body>) -> String {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn post_response(
    app: axum::Router,
    cookie: Option<&str>,
    uri: &str,
    body: String,
) -> axum::http::Response<Body> {
    let mut request =
        Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    app.oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

async fn post_location(
    app: axum::Router,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> String {
    let body = if extra.is_empty() {
        format!("csrf={csrf}")
    } else {
        format!("csrf={csrf}&{extra}")
    };
    let response = post_response(app, Some(cookie), uri, body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "POST {uri}");
    location_of(&response)
}

#[tokio::test]
async fn us_chaos_01_empty_post_gets_a_human_page() {
    let world = app().await;
    let response = post_response(world.app, None, "/register", String::new()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let html = body_string(response).await;
    assert!(html.contains("Fill in the required fields."));
    assert!(html.contains("Ecclesia"));
}

#[tokio::test]
async fn us_chaos_01_double_at_email_is_refused() {
    let world = app().await;
    let (_html, cookie, csrf) = get_ok(world.app.clone(), None, "/register").await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        "/register",
        &format!(
            "name=Ada&email=ada@@nope.com&city=Waterloo&region=Iowa&bio=I+cook&password={}",
            enc(PASS)
        ),
    )
    .await;
    assert!(location.contains("err=bad_email"), "got {location}");
}

#[tokio::test]
async fn us_chaos_01_script_name_does_not_run() {
    let world = app().await;
    let (_html, cookie, csrf) = get_ok(world.app.clone(), None, "/register").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/register",
        format!(
            "csrf={csrf}&name=%3Cscript%3Ealert(1)%3C/script%3E&email=escaped@nope.test&city=Waterloo&region=Iowa&bio=I+cook&password={}",
            enc(PASS)
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = try_cookie(&response).expect("new cookie");
    let (home, _, _) = get_ok(world.app, Some(&cookie), "/home").await;
    assert!(home.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!home.contains("<script>alert(1)</script>"));
}

#[tokio::test]
async fn us_chaos_02_bang_church_name_is_refused() {
    let world = app().await;
    let cookie = register(&world, "Keisha Ward", "keisha@mercy.test").await;
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&cookie), "/churches/new").await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        "/churches",
        "name=!!!&city=Waterloo&region=Iowa&description=A+table+in+the+north+end.&gathering=",
    )
    .await;
    assert!(location.contains("err=missing"), "got {location}");
}

#[tokio::test]
async fn us_chaos_02_pending_member_cannot_post_a_need() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_miriam, church_id) = plant(&world, &miriam, "Grace Covenant").await;
    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&peter),
        &format!("/churches/{church_id}"),
    )
    .await;
    let _ = post_location(
        world.app.clone(),
        &cookie,
        &csrf,
        &format!("/churches/{church_id}/join"),
        "",
    )
    .await;
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&cookie), "/needs/new").await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        "/needs",
        &format!(
            "church_id={church_id}&title=Need+five+dinners&body=Need+five+dinners+this+week&scope=church"
        ),
    )
    .await;
    assert!(location.contains("err=not_member"), "got {location}");
}

#[tokio::test]
async fn us_chaos_03_cannot_apply_to_your_own_need() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, church_id) = plant(&world, &cookie, "Grace Covenant").await;
    let need_id = post_need(
        &world,
        &cookie,
        &church_id,
        "Dinners for the Okonkwo family",
        "Five dinners this week.",
    )
    .await;
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&cookie),
        &format!("/needs/{need_id}"),
    )
    .await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        &format!("/needs/{need_id}/apply"),
        "message=I+will+cook+my+own+dinners",
    )
    .await;
    assert!(location.contains("err=own_need"), "got {location}");
}

#[tokio::test]
async fn us_chaos_03_neighbor_cannot_close_a_need() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&miriam), "/needs/new").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/needs",
        format!(
            "csrf={csrf}&church_id={grace}&title=Spanish+interpreter&body=Hold+both+languages+on+Thursday.&scope=neighboring"
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let need_id = location_of(&response)
        .split("/needs/")
        .nth(1)
        .and_then(|rest| rest.split(['?', '/']).next())
        .expect("need id")
        .to_string();

    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let (_elena, _) = plant(&world, &elena, "New Mercy").await;
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&elena),
        &format!("/needs/{need_id}"),
    )
    .await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        &format!("/needs/{need_id}/close"),
        "",
    )
    .await;
    assert!(location.contains("err=steward"), "got {location}");
}

#[tokio::test]
async fn us_chaos_04_cannot_endorse_yourself() {
    let world = app().await;
    let cookie = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let ruth_id = world
        .sdk
        .db
        .user_by_email("ruth@grace.test")
        .await
        .unwrap()
        .unwrap()
        .id;
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&cookie),
        &format!("/members/{ruth_id}"),
    )
    .await;
    let location = post_location(
        world.app,
        &cookie,
        &csrf,
        &format!("/members/{ruth_id}/endorse"),
        "skill=Hospitality&note=I+stayed+until+the+last+parent+came.",
    )
    .await;
    assert!(location.contains("err=self"), "got {location}");
}

#[tokio::test]
async fn us_chaos_05_junk_invite_and_missing_pages() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&cookie), "/churches").await;
    let location = post_location(
        world.app.clone(),
        &cookie,
        &csrf,
        "/invites/redeem",
        "code=zzzz-zzzz",
    )
    .await;
    assert!(location.contains("err=invite"), "got {location}");

    let (status, html, _, _) = get_any(world.app.clone(), Some(&cookie), "/asdf").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("That page doesn't exist."));

    let (status, html, _, _) = get_any(world.app, Some(&cookie), "/needs/nope").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("We couldn't find that need."));
}

#[tokio::test]
async fn us_chaos_06_double_approve_and_empty_profile() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, church_id) = plant(&world, &miriam, "Grace Covenant").await;
    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&peter),
        &format!("/churches/{church_id}"),
    )
    .await;
    let _ = post_location(
        world.app.clone(),
        &cookie,
        &csrf,
        &format!("/churches/{church_id}/join"),
        "",
    )
    .await;
    let (church, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&miriam),
        &format!("/churches/{church_id}"),
    )
    .await;
    let mem = church
        .split("/memberships/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .expect("membership id")
        .to_string();
    let first = post_location(
        world.app.clone(),
        &cookie,
        &csrf,
        &format!("/memberships/{mem}/approve"),
        "",
    )
    .await;
    assert!(first.contains("ok=approved"), "got {first}");
    let (_page, cookie, csrf) = get_ok(
        world.app.clone(),
        Some(&cookie),
        &format!("/churches/{church_id}"),
    )
    .await;
    let second = post_location(
        world.app.clone(),
        &cookie,
        &csrf,
        &format!("/memberships/{mem}/approve"),
        "",
    )
    .await;
    assert!(second.contains("err=pending"), "got {second}");

    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&cookie), "/me").await;
    let empty = post_location(world.app, &cookie, &csrf, "/me", "name=&city=&region=&bio=").await;
    assert!(empty.contains("err=missing"), "got {empty}");
}

#[tokio::test]
async fn us_chaos_07_refine_junk_and_push_junk() {
    let world = app().await;
    let (_html, cookie, csrf) = get_ok(world.app.clone(), None, "/").await;
    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/refine",
        format!("csrf={csrf}&kind=nope&text=hi"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = post_response(
        world.app.clone(),
        Some(&cookie),
        "/refine",
        format!("csrf={csrf}&kind=bio&text={}", "x".repeat(2001)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_page, cookie, csrf) = get_ok(world.app.clone(), Some(&cookie), "/me").await;
    let response = post_response(
        world.app,
        Some(&cookie),
        "/push/subscribe",
        format!("csrf={csrf}&endpoint=javascript:alert(1)&p256dh=abc&auth=def"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
