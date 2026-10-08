use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use ecclesia::http::{AppState, router};
use ecclesia_sdk::db::Db;
use image::ImageEncoder;
use tower::ServiceExt;

const PASS: &str = "Thursday dinners at six oclock";

static TEST_DB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone)]
struct World {
    app: axum::Router,
    sdk: ecclesia_sdk::Sdk,
}

async fn app_with(
    judge: ecclesia_sdk::judge::JudgeHub,
    refine: ecclesia_sdk::refine::RefineHub,
) -> World {
    let path = std::env::temp_dir().join(format!(
        "ecclesia-test-{}-{}-{}.db",
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
        db.clone(),
        judge,
        refine,
        ecclesia_sdk::push::PushHub::silent(),
        ecclesia_sdk::Cache::memory(),
    )
    .with_fixture_places();
    sdk.db.seed_grace_church().await.expect("seed church");
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

async fn app() -> World {
    app_with(
        ecclesia_sdk::judge::JudgeHub::word_gate(),
        ecclesia_sdk::refine::RefineHub::silent(),
    )
    .await
}

fn cookie_from(response: &axum::http::Response<Body>) -> String {
    try_cookie_from(response).expect("signed session cookie")
}

fn try_cookie_from(response: &axum::http::Response<Body>) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("ecclesia_sid="))
        .map(|value| value.split(';').next().unwrap().to_string())
}

fn csrf_from(html: &str) -> Option<String> {
    html.split(r#"name="csrf" value=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .map(ToOwned::to_owned)
        .or_else(|| {
            html.split(r#"name="csrf" content=""#)
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .map(ToOwned::to_owned)
        })
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn get_page(
    app: axum::Router,
    cookie: Option<&str>,
    uri: &str,
) -> (String, String, Option<String>) {
    let mut request = Request::get(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let next_cookie = try_cookie_from(&response).or_else(|| cookie.map(ToOwned::to_owned));
    let html = body_string(response).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}\n{html}");
    let next_cookie = next_cookie.expect("session cookie");
    let csrf = csrf_from(&html);
    (html, next_cookie, csrf)
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

async fn post_form(
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

fn split_name(name: &str) -> (&str, &str) {
    name.rsplit_once(' ').unwrap_or((name, "Lane"))
}

async fn register(world: &World, name: &str, email: &str) -> String {
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/register").await;
    let csrf = csrf.expect("register csrf");
    let (first, last) = split_name(name);
    let body = format!(
        "csrf={csrf}&first_name={}&last_name={}&email={}&password={}",
        enc(first),
        enc(last),
        enc(email),
        enc(PASS)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/register", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "register {email}");
    cookie_from(&response)
}

async fn sign_in(world: &World, email: &str) -> String {
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/session/new").await;
    let csrf = csrf.expect("sign in csrf");
    let body = format!("csrf={csrf}&email={}&password={}", enc(email), enc(PASS));
    let response = post_form(world.app.clone(), Some(&cookie), "/session", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "sign in {email}");
    cookie_from(&response)
}

async fn plant(world: &World, cookie: &str, name: &str) -> (String, String) {
    plant_at(world, cookie, name, "42.5349", "-92.4453").await
}

async fn plant_at(
    world: &World,
    cookie: &str,
    name: &str,
    latitude: &str,
    longitude: &str,
) -> (String, String) {
    let (page, cookie, csrf) = get_page(world.app.clone(), Some(cookie), "/churches/new").await;
    assert!(page.contains("Register a new church"));
    assert!(page.contains(">Line 1"));
    assert!(page.contains(">Line 2"));
    assert!(page.contains("autocomplete=\"address-line1\""));
    assert!(page.contains("autocomplete=\"address-level2\""));
    assert!(page.contains("autocomplete=\"postal-code\""));
    assert!(page.contains("type=\"time\""));
    assert!(page.contains("Add a service"));
    assert!(!page.contains("name=\"latitude\""));
    let csrf = csrf.expect("church csrf");
    let body = format!(
        "csrf={csrf}&name={}&{}&ein=12-3456789&registry_state=IA&registry_number=123456&service_day_0=Sunday&service_time_0=10:00&description=A+church+on+Main+Street.&pass=publish",
        enc(name),
        address_fields(latitude, longitude)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/churches", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "plant {name}");
    let location = location_of(&response);
    let church_id = id_from_location(&location, "/churches/");
    let church = world
        .sdk
        .db
        .church(&church_id)
        .await
        .expect("church")
        .expect("row");
    let expect_lat: f64 = latitude.parse().expect("latitude");
    let expect_lng: f64 = longitude.parse().expect("longitude");
    assert_eq!(church.gathering, "Sunday at 10 a.m.");
    assert_eq!(church.ein, "12-3456789");
    assert_eq!(church.registry_state, "IA");
    assert_eq!(church.registry_number, "123456");
    assert!((church.latitude - expect_lat).abs() < 0.0001);
    assert!((church.longitude - expect_lng).abs() < 0.0001);
    (try_cookie_from(&response).unwrap_or(cookie), church_id)
}

fn address_fields(latitude: &str, longitude: &str) -> &'static str {
    match (latitude, longitude) {
        ("30.2672", "-97.7431") => {
            "address-line1=100+Congress+Avenue&address-level2=Austin&address-level1=TX&postal-code=78701"
        }
        ("42.4928", "-92.3426") => {
            "address-line1=200+Commercial+Street&address-level2=Waterloo&address-level1=IA&postal-code=50701"
        }
        _ => {
            "address-line1=100+Main+Street&address-level2=Cedar+Falls&address-level1=IA&postal-code=50613"
        }
    }
}

fn location_of(response: &axum::http::Response<Body>) -> String {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn id_from_location(location: &str, prefix: &str) -> String {
    location
        .split(prefix)
        .nth(1)
        .and_then(|rest| rest.split(['?', '/']).next())
        .expect("id in location")
        .to_string()
}

async fn join(world: &World, cookie: &str, church_id: &str) -> String {
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(cookie), "/churches/join").await;
    let csrf = csrf.expect("join csrf");
    let response = post_form(
        world.app.clone(),
        Some(&cookie),
        &format!("/churches/{church_id}/join"),
        format!("csrf={csrf}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    try_cookie_from(&response).unwrap_or(cookie)
}

async fn post_need(
    world: &World,
    cookie: &str,
    church_id: &str,
    title: &str,
    body: &str,
    scope: &str,
) -> (String, String) {
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(cookie), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    let extra = format!(
        "csrf={csrf}&church_id={}&title={}&body={}&scope={}&pass=publish",
        enc(church_id),
        enc(title),
        enc(body),
        enc(scope)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/needs", extra).await;
    assert_eq!(
        response.status(),
        StatusCode::SEE_OTHER,
        "post need {title}"
    );
    let location = location_of(&response);
    let need_id = id_from_location(&location, "/needs/");
    (try_cookie_from(&response).unwrap_or(cookie), need_id)
}

async fn user_id(world: &World, email: &str) -> String {
    world
        .sdk
        .db
        .user_by_email(email)
        .await
        .expect("lookup")
        .expect(email)
        .id
}

async fn get(world: &World, cookie: &str, uri: &str) -> String {
    get_page(world.app.clone(), Some(cookie), uri).await.0
}

async fn post(world: &World, cookie: &str, csrf: &str, uri: &str, extra: &str) -> StatusCode {
    let body = if extra.is_empty() {
        format!("csrf={csrf}")
    } else {
        format!("csrf={csrf}&{extra}")
    };
    let response = post_form(world.app.clone(), Some(cookie), uri, body).await;
    response.status()
}

async fn post_location(world: &World, cookie: &str, csrf: &str, uri: &str, extra: &str) -> String {
    let body = format!("csrf={csrf}&{extra}");
    let response = post_form(world.app.clone(), Some(cookie), uri, body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    location_of(&response)
}

async fn post_page(
    world: &World,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> (StatusCode, String) {
    let body = format!("csrf={csrf}&{extra}");
    let response = post_form(world.app.clone(), Some(cookie), uri, body).await;
    (response.status(), body_string(response).await)
}

async fn post_json(
    world: &World,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> (StatusCode, String) {
    post_page(world, cookie, csrf, uri, extra).await
}

fn waiting_user_id(html: &str, church_id: &str) -> String {
    let marker = format!("/churches/{church_id}/members/");
    html.split(&marker)
        .nth(1)
        .and_then(|rest| rest.split(['/', '"', '?']).next())
        .expect("waiting user id")
        .to_string()
}

fn endorsement_id_near(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    html[start..]
        .split("/endorsements/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(ToOwned::to_owned)
}

async fn get_public(world: &World, uri: &str) -> String {
    get_public_with_headers(world, uri).await.0
}

async fn get_public_with_headers(world: &World, uri: &str) -> (String, axum::http::HeaderMap) {
    let response = world
        .app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
    let headers = response.headers().clone();
    (body_string(response).await, headers)
}

#[tokio::test]
async fn us_ui_01_svg_marks_close_their_tags() {
    let world = app().await;
    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(
        landing.contains("</circle>"),
        "vesica circles must close or the cross is swallowed"
    );
    assert!(
        landing.contains("</path>"),
        "vesica path must close or the cross is swallowed"
    );

    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, _) = plant(&world, &cookie, "Grace Covenant").await;
    let home = get(&world, &cookie, "/home").await;
    assert!(home.contains("</path>"), "dock icons must close path tags");
}

#[tokio::test]
async fn us_sec_02_http_rejects_a_missing_csrf() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let status = post(
        &world,
        &cookie,
        "deadbeefdeadbeefdeadbeefdeadbeef",
        "/session/logout",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn us_auth_01_register_then_sign_in() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let join = get(&world, &cookie, "/churches/join").await;
    assert!(join.contains("Find your church"));
    assert!(join.contains("Miriam"));
    assert!(!join.contains("Open needs"));
    assert!(!join.contains("class=\"dock\""));

    let again = sign_in(&world, "miriam@grace.test").await;
    let join = get(&world, &again, "/churches/join").await;
    assert!(join.contains("Find your church"));
    assert!(!join.contains("class=\"dock\""));
}

#[tokio::test]
async fn us_church_04_leave_returns_to_the_join_step() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam-leave@grace.test").await;
    let (_miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let peter = register(&world, "Peter Lane", "peter-leave@grace.test").await;
    let peter = join(&world, &peter, &grace).await;
    let (me, peter, csrf) = get_page(world.app.clone(), Some(&peter), "/me").await;
    assert!(me.contains("Leave Grace Covenant"));
    assert!(me.contains("Grace Covenant"));
    assert!(me.contains("Waiting"));
    assert!(!me.contains("Find your church"));
    assert!(!me.contains("Register a new church"));
    let location = post_location(
        &world,
        &peter,
        &csrf.expect("me csrf"),
        "/me/church/leave",
        &format!("church_id={grace}"),
    )
    .await;
    assert!(
        location.starts_with("/churches/join?ok=left"),
        "leave should open the join step, got {location}"
    );
    let join = get(&world, &peter, &location).await;
    assert!(join.contains("Find your church"));
    assert!(join.contains("class=\"steps\""));
    assert!(join.contains("Left."));
    assert!(join.contains("Sign out"));
    assert!(!join.contains("class=\"dock\""));
    assert!(!join.contains("Leave Grace Covenant"));
    let user = world
        .sdk
        .db
        .user_by_email("peter-leave@grace.test")
        .await
        .unwrap()
        .unwrap();
    assert!(user.memberships.is_empty());

    for path in ["/home", "/me", "/pray", "/inbox"] {
        let response = world
            .app
            .clone()
            .oneshot(
                Request::get(path)
                    .header(header::COOKIE, &peter)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "{path}");
        assert_eq!(location_of(&response), "/churches/join", "{path}");
    }
}

#[tokio::test]
async fn us_church_05_pastor_closes_or_names_the_next_pastor() {
    let world = app().await;
    let ada = register(&world, "Ada Pastor", "ada-hold@grace.test").await;
    let (ada, grace) = plant(&world, &ada, "Grace Covenant").await;
    let (me, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/me").await;
    assert!(me.contains("Close Grace Covenant"));
    assert!(me.contains("Transfer Grace Covenant"));
    assert!(me.contains("Next pastor"));
    assert!(!me.contains("Leave Grace Covenant"));
    let csrf = csrf.expect("me csrf");
    let denied = post_location(
        &world,
        &ada,
        &csrf,
        "/me/church/leave",
        &format!("church_id={grace}"),
    )
    .await;
    assert!(
        denied.contains("err=pastor"),
        "a pastor stays with the church, got {denied}"
    );

    let peter = register(&world, "Peter Lane", "peter-hold@grace.test").await;
    let location = post_location(
        &world,
        &ada,
        &csrf,
        "/me/church/transfer",
        &format!("church_id={grace}&email=peter-hold%40grace.test"),
    )
    .await;
    assert!(
        location.starts_with("/me?ok=transferred"),
        "transfer should stay on the profile, got {location}"
    );
    let me = get(&world, &ada, &location).await;
    assert!(me.contains("Transferred."));
    assert!(me.contains("Leave Grace Covenant"));
    assert!(!me.contains("Close Grace Covenant"));

    let (peter_page, peter, peter_csrf) = get_page(world.app.clone(), Some(&peter), "/me").await;
    assert!(peter_page.contains("Close Grace Covenant"));
    assert!(peter_page.contains("Pastor"));
    let church = world.sdk.db.church(&grace).await.unwrap().unwrap();
    let peter_user = world
        .sdk
        .db
        .user_by_email("peter-hold@grace.test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(church.owner_id, peter_user.id);
    let invite = church.invite_code.clone();

    let location = post_location(
        &world,
        &peter,
        &peter_csrf.expect("peter csrf"),
        "/me/church/close",
        &format!("church_id={grace}"),
    )
    .await;
    assert!(
        location.starts_with("/churches/join?ok=closed"),
        "close should open the join step, got {location}"
    );
    let join = get(&world, &peter, &location).await;
    assert!(join.contains("Closed."));
    assert!(world.sdk.db.church(&grace).await.unwrap().is_none());
    assert!(
        world
            .sdk
            .db
            .church_by_invite(&invite)
            .await
            .unwrap()
            .is_none()
    );
    let (kept, deleted_at) = world.sdk.db.stored_church(&grace).await.unwrap().unwrap();
    assert_eq!(kept.id, grace);
    assert!(deleted_at.is_some());
    let ada_user = world
        .sdk
        .db
        .user_by_email("ada-hold@grace.test")
        .await
        .unwrap()
        .unwrap();
    assert!(ada_user.memberships.is_empty());
    let peter_user = world
        .sdk
        .db
        .user_by_email("peter-hold@grace.test")
        .await
        .unwrap()
        .unwrap();
    assert!(peter_user.memberships.is_empty());
}

#[tokio::test]
async fn us_church_06_closed_church_needs_can_move() {
    let world = app().await;
    let ada = register(&world, "Ada Pastor", "ada-move@grace.test").await;
    let (ada, grace) = plant(&world, &ada, "Grace Covenant").await;
    let (ada, need_id) = post_need(
        &world,
        &ada,
        &grace,
        "Dinners for the Cole family",
        "Tuesday and Thursday.",
        "church",
    )
    .await;
    let peter = register(&world, "Peter Lane", "peter-move@grace.test").await;
    let peter = join(&world, &peter, &grace).await;

    let (me, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/me").await;
    assert!(me.contains("Close Grace Covenant"));
    let location = post_location(
        &world,
        &ada,
        &csrf.expect("me csrf"),
        "/me/church/close",
        &format!("church_id={grace}"),
    )
    .await;
    assert!(
        location.starts_with("/churches/join?ok=closed"),
        "a pastor with no other church picks another, got {location}"
    );
    let join = get(&world, &ada, &location).await;
    assert!(join.contains("Find your church"));
    assert!(join.contains("Open needs from Grace Covenant"));
    assert!(join.contains("Dinners for the Cole family"));
    assert!(!join.contains("Move open needs"));
    assert!(world.sdk.db.need(&need_id).await.unwrap().is_none());

    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/home")
                .header(header::COOKIE, &peter)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), "/churches/join");
    let peter_join = get(&world, &peter, "/churches/join").await;
    assert!(!peter_join.contains("Dinners for the Cole family"));

    let (ada, hope) = plant(&world, &ada, "Hope Chapel").await;
    let home = get(&world, &ada, "/home").await;
    assert!(home.contains("Open needs from Grace Covenant"));
    assert!(home.contains("Dinners for the Cole family"));
    assert!(home.contains("Move open needs to Hope Chapel"));
    let (home, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/home").await;
    assert!(home.contains("Move open needs to Hope Chapel"));
    let location = post_location(
        &world,
        &ada,
        &csrf.expect("home csrf"),
        "/needs/import",
        &format!("source_church_id={grace}&church_id={hope}"),
    )
    .await;
    assert!(
        location.starts_with("/home?ok=moved"),
        "moved needs stay on home, got {location}"
    );
    let home = get(&world, &ada, &location).await;
    assert!(home.contains("Moved."));
    assert!(home.contains("Dinners for the Cole family"));
    assert!(!home.contains("Open needs from Grace Covenant"));
    let moved = world.sdk.db.need(&need_id).await.unwrap().unwrap();
    assert_eq!(moved.church_id, hope);
    let church = get(&world, &ada, &format!("/churches/{hope}")).await;
    assert!(church.contains("Dinners for the Cole family"));
}

#[tokio::test]
async fn us_church_07_a_second_church_keeps_the_member_and_the_needs() {
    let world = app().await;
    let ada = register(&world, "Ada Pastor", "ada-stay@grace.test").await;
    let (ada, grace) = plant(&world, &ada, "Grace Covenant").await;
    let (ada, need_id) = post_need(
        &world,
        &ada,
        &grace,
        "Rides on Sunday",
        "Two seats after the service.",
        "church",
    )
    .await;
    let (ada, hope) = plant(&world, &ada, "Hope Chapel").await;
    let peter = register(&world, "Peter Lane", "peter-stay@grace.test").await;
    let peter = join(&world, &peter, &grace).await;

    let (me, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/me").await;
    assert!(me.contains("Close Grace Covenant"));
    let location = post_location(
        &world,
        &ada,
        &csrf.expect("me csrf"),
        "/me/church/close",
        &format!("church_id={grace}"),
    )
    .await;
    assert!(
        location.starts_with("/home?ok=closed"),
        "a pastor who still has a church stays, got {location}"
    );
    let home = get(&world, &ada, &location).await;
    assert!(home.contains("Closed."));
    assert!(home.contains("Open needs from Grace Covenant"));
    assert!(home.contains("Rides on Sunday"));
    assert!(home.contains("Move open needs to Hope Chapel"));
    let you = get(&world, &ada, "/me").await;
    assert!(you.contains("Open needs from Grace Covenant"));
    assert!(you.contains("Move open needs to Hope Chapel"));

    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/home")
                .header(header::COOKIE, &peter)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), "/churches/join");

    let (home, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/home").await;
    assert!(home.contains("Move open needs to Hope Chapel"));
    let location = post_location(
        &world,
        &ada,
        &csrf.expect("home csrf"),
        "/needs/import",
        &format!("source_church_id={grace}&church_id={hope}"),
    )
    .await;
    assert!(location.starts_with("/home?ok=moved"), "{location}");
    let moved = world.sdk.db.need(&need_id).await.unwrap().unwrap();
    assert_eq!(moved.church_id, hope);
    let you = get(&world, &ada, "/me").await;
    assert!(!you.contains("Open needs from Grace Covenant"));
}

#[tokio::test]
async fn us_auth_01_join_search_finds_grace() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam-search@grace.test").await;
    let page = get(&world, &cookie, "/churches/join?q=grace").await;
    assert!(page.contains("Name or city"));
    assert!(page.contains("Cedar Falls"));
    assert!(page.contains("/static/join.js?v=6"));
    assert!(page.contains("/static/app.js?v=33"));
    let shell = get_public(&world, "/static/app.js").await;
    assert!(shell.contains("ecclesia-place"));
    assert!(shell.contains("print-code.css"));
    assert!(shell.contains("placeSettled"));
    assert!(shell.contains("Notification.requestPermission"));
    let finder = get_public(&world, "/static/join.js").await;
    assert!(finder.contains("ecclesia-place"));
    assert!(finder.contains("placeSettled"));
    assert!(page.contains("Grace Fellowship"));
    let by_city = get(&world, &cookie, "/churches/join?q=Cedar+Falls").await;
    assert!(by_city.contains("Grace Fellowship"));
    assert!(by_city.contains("seed_grace"));
    let nearby = get(&world, &cookie, "/churches/join?lat=42.5349&lng=-92.4453").await;
    assert!(nearby.contains("Nearby"));
    assert!(nearby.contains("Grace Fellowship"));
    assert!(page.contains("seed_grace"));
    assert!(page.contains("data-scan-code"));
    assert!(page.contains("Scan church code"));
    assert!(page.contains("data-join-query"));
    assert!(!page.contains("Search churches"));
    assert!(!page.contains("Have a code"));
    assert!(!page.contains("Invite code"));
    assert!(!page.contains("class=\"dock\""));
    assert!(page.contains("Sign out"));

    let home = world
        .app
        .clone()
        .oneshot(
            Request::get("/home")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(home.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&home), "/churches/join");

    let (_page, cookie, csrf) =
        get_page(world.app.clone(), Some(&cookie), "/churches/join?q=Grace").await;
    let csrf = csrf.expect("join csrf");
    let location = post_location(
        &world,
        &cookie,
        &csrf,
        "/churches/join",
        "church_id=seed_grace",
    )
    .await;
    assert!(
        location.starts_with("/churches/seed_grace"),
        "join should open the church, got {location}"
    );
    let church = get(
        &world,
        &cookie,
        location.split('?').next().unwrap_or(&location),
    )
    .await;
    assert!(church.contains("Your request to join Grace Fellowship has been sent."));
    assert!(church.contains("class=\"dock\""));
}

#[tokio::test]
async fn us_auth_01_join_lists_churches_near_you() {
    let world = app().await;
    let planter = register(&world, "Ada Pastor", "ada-plant@grace.test").await;
    let (planter, _) = plant_at(&world, &planter, "River Church", "42.4928", "-92.3426").await;
    let (_planter, _) = plant_at(&world, &planter, "Far Chapel", "30.2672", "-97.7431").await;
    let seeker = register(&world, "No Church", "seeker-near@grace.test").await;

    let near = get(&world, &seeker, "/churches/join?lat=42.5349&lng=-92.4453").await;
    let grace = near.find("<h3>Grace Fellowship</h3>").expect("grace");
    let river = near.find("<h3>River Church</h3>").expect("river");
    assert!(grace < river, "nearest church should come first");
    assert!(near.contains("<h2>Nearby</h2>"));
    assert!(near.contains("Ask to join"));
    assert!(!near.contains("<h3>Far Chapel</h3>"));
    assert!(!near.contains("Search churches"));
    assert!(!near.contains("Closest church"));

    let far = get(&world, &seeker, "/churches/join?lat=30.2672&lng=-97.7431").await;
    assert!(far.contains("<h3>Far Chapel</h3>"));
    assert!(!far.contains("<h3>Grace Fellowship</h3>"));
    assert!(!far.contains("<h3>River Church</h3>"));

    let named = get(&world, &seeker, "/churches/join?q=River").await;
    assert!(named.contains("River Church"));
    assert!(!named.contains("<h2>Nearby</h2>"));
}

#[tokio::test]
async fn us_auth_04_unknown_email_is_quiet() {
    let world = app().await;
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let (status, page) = post_page(
        &world,
        &cookie,
        &csrf,
        "/session",
        "email=nobody%40x.test&password=nope",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Try again."));
    assert!(page.contains("value=\"nobody@x.test\""));
    assert!(!page.contains("value=\"nope\""));
    let mail = post_location(
        &world,
        &cookie,
        &csrf,
        "/session/link",
        "email=nobody%40x.test",
    )
    .await;
    assert!(mail.contains("err=mail"), "got {mail}");
}

#[tokio::test]
async fn us_auth_01_weak_password_stays_on_register() {
    let world = app().await;
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/register").await;
    let csrf = csrf.expect("register csrf");
    let (status, page) = post_page(
        &world,
        &cookie,
        &csrf,
        "/register",
        "first_name=Cara&last_name=Nguyen&email=cara@verify.test&password=password",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Pick a stronger password."));
    assert!(page.contains("value=\"Cara\""));
    assert!(page.contains("value=\"Nguyen\""));
    assert!(page.contains("value=\"cara@verify.test\""));
    assert!(!page.contains("value=\"password\""));
    assert!(
        world
            .sdk
            .db
            .user_by_email("cara@verify.test")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn us_auth_01_churchless_unknown_code_creates_no_account() {
    let world = app().await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), None, "/register").await;
    let csrf = csrf.expect("register csrf");
    let (status, page) = post_page(
        &world,
        &cookie,
        &csrf,
        "/register",
        "first_name=Ada&last_name=Lovelace&email=ghost-code@example.test&password=Thursday%20dinners%20at%20six%20oclock&code=not-a-church",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("No church has that code."));
    assert!(page.contains("Find your church"));
    assert!(!page.contains("name=\"code\""));
    assert!(
        world
            .sdk
            .db
            .user_by_email("ghost-code@example.test")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn us_auth_01_churchless_weak_password_keeps_the_invited_church() {
    let world = app().await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), None, "/register?code=GRACESEED").await;
    let csrf = csrf.expect("register csrf");
    let (status, page) = post_page(
        &world,
        &cookie,
        &csrf,
        "/register",
        "first_name=Ada&last_name=Lovelace&email=weak-code@example.test&password=password&code=GRACESEED",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Pick a stronger password."));
    assert!(page.contains("Grace Fellowship"));
    assert!(page.contains("Join Grace Fellowship"));
    assert!(page.contains("value=\"Ada\""));
    assert!(
        world
            .sdk
            .db
            .user_by_email("weak-code@example.test")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn us_auth_01_churchless_pages_stay_on_the_church_step() {
    let world = app().await;
    let cookie = register(&world, "No Church", "nochurch@example.test").await;
    for path in ["/home", "/me", "/churches", "/inbox", "/the-body"] {
        let response = world
            .app
            .clone()
            .oneshot(
                Request::get(path)
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "{path}");
        assert_eq!(location_of(&response), "/churches/join", "{path}");
    }
    let (join, _, _) = get_page(world.app.clone(), Some(&cookie), "/churches/join").await;
    assert!(join.contains("Name or city"));
    assert!(join.contains("Scan church code"));
    assert!(join.contains("data-join-query"));
    assert!(!join.contains("Search churches"));
    assert!(!join.contains("Invite code"));
    assert!(!join.contains("class=\"dock\""));
}

#[tokio::test]
async fn us_auth_01_churchless_lowercase_invite_opens_that_church() {
    let world = app().await;
    let response = world
        .app
        .clone()
        .oneshot(Request::get("/join/graceseed").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let (page, _, _) = get_page(world.app.clone(), None, &location_of(&response)).await;
    assert!(page.contains("Grace Fellowship"));
    assert!(page.contains("Join Grace Fellowship"));
}

#[tokio::test]
async fn us_auth_01_churchless_signed_in_register_follows_the_church() {
    let world = app().await;
    let cookie = register(&world, "Return Home", "return-home@example.test").await;
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/register")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), "/churches/join");

    let (cookie, _) = plant(&world, &cookie, "Grace Covenant").await;
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/register")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), "/home");
}

#[tokio::test]
async fn us_auth_01_churchless_register_hides_the_password() {
    let world = app().await;
    register(&world, "Hash Check", "hash-check@example.test").await;
    let user = world
        .sdk
        .db
        .user_by_email("hash-check@example.test")
        .await
        .unwrap()
        .expect("user");
    assert!(user.memberships.is_empty());
    let hash = world
        .sdk
        .db
        .user_password_hash(&user.id)
        .await
        .unwrap()
        .expect("hash");
    assert!(hash.starts_with("$argon2"));
    assert!(!hash.contains(PASS));
}

#[tokio::test]
async fn us_auth_01_invite_link_joins_during_signup() {
    let world = app().await;
    let response = world
        .app
        .clone()
        .oneshot(Request::get("/join/GRACESEED").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = location_of(&response);
    assert!(
        location.starts_with("/register?code="),
        "guest invite should open You, got {location}"
    );

    let (page, cookie, csrf) = get_page(world.app.clone(), None, &location).await;
    let csrf = csrf.expect("register csrf");
    assert!(
        page.contains("Grace Fellowship"),
        "invite page missed the church at {location}"
    );
    assert!(page.contains("Join Grace Fellowship"));
    assert!(page.contains("100 Main Street"));
    assert!(!page.contains("Search by name"));
    assert!(!page.contains("class=\"steps\""));

    let body = format!(
        "csrf={csrf}&first_name=Ada&last_name=Lovelace&email=ada-qr@example.test&password={}&code=GRACESEED",
        enc(PASS)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/register", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = location_of(&response);
    assert!(
        location.starts_with("/churches/seed_grace"),
        "signup with a code should open that church, got {location}"
    );
    assert!(location.contains("ok=redeemed"));
    let cookie = cookie_from(&response);
    let (church, _, _) = get_page(world.app.clone(), Some(&cookie), &location).await;
    assert!(church.contains("You're in."));
    assert!(church.contains("class=\"dock\""));

    let member = register(&world, "Bea Ng", "bea-qr@example.test").await;
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/join/GRACESEED")
                .header(header::COOKIE, &member)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = location_of(&response);
    assert!(
        location.starts_with("/churches/seed_grace"),
        "a signed in person should join from the code, got {location}"
    );

    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/join/missing-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let location = location_of(&response);
    let (page, _, _) = get_page(world.app.clone(), None, &location).await;
    assert!(page.contains("No church has that code."));
    assert!(page.contains("Find your church"));
    assert!(!page.contains("name=\"code\""));
}

#[tokio::test]
async fn us_auth_02_register_cookie_is_v2() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let raw = cookie.strip_prefix("ecclesia_sid=").expect("cookie name");
    let parts: Vec<_> = raw.split('.').collect();
    assert_eq!(parts.first().copied(), Some("v2"));
    assert_eq!(parts.len(), 4);
    assert!(!parts[1].is_empty());
}

#[tokio::test]
async fn us_auth_03_logout_all_returns_to_landing() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, _) = plant(&world, &cookie, "Grace Covenant").await;
    let (me, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
    assert!(me.contains("This device"));
    assert!(me.contains("Devices"));
    let location = post_location(&world, &cookie, &csrf, "/session/logout-all", "").await;
    assert_eq!(location, "/");
    let home = get(&world, &cookie, "/home").await;
    assert!(home.contains("Create an account"));
    assert!(!home.contains("Open needs"));
}

#[tokio::test]
async fn us_auth_02_session_ip_uses_fly_client_ip() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_join, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/churches/join").await;
    let csrf = csrf.expect("join csrf");
    let _ = post(&world, &cookie, &csrf, "/session/logout", "").await;
    let (_landing, guest, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let body = format!("csrf={csrf}&email=miriam@grace.test&password={}", enc(PASS));
    let response = world
        .app
        .clone()
        .oneshot(
            Request::post("/session")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, guest)
                .header("fly-client-ip", "203.0.113.9")
                .header("x-forwarded-for", "198.51.100.7, 203.0.113.9")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let user = world
        .sdk
        .db
        .user_by_email("miriam@grace.test")
        .await
        .unwrap()
        .expect("miriam");
    let sessions = world.sdk.db.sessions_for_user(&user.id).await.unwrap();
    assert!(
        sessions.iter().any(|row| row.ip == "203.0.113.9"),
        "ips {:?}",
        sessions
            .iter()
            .map(|row| row.ip.as_str())
            .collect::<Vec<_>>()
    );
    assert!(sessions.iter().all(|row| row.ip != "198.51.100.7"));
}

#[tokio::test]
async fn us_mail_08_signed_in_magic_get_does_not_consume() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    ecclesia_sdk::story::request_magic(
        &world.sdk,
        "miriam@grace.test",
        &ecclesia_sdk::story::MailOrigin {
            origin: "http://127.0.0.1:43781".into(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let rows = world.sdk.db.pending_outbox().await.unwrap();
    let mail = rows
        .iter()
        .rev()
        .find(|row| row.kind == "mail")
        .expect("mail");
    let href = mail
        .payload
        .split("http://127.0.0.1:43781")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("local href");
    let tail = href.rsplit('/').next().expect("id");
    let id = tail.split("?t=").next().expect("id");
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get(href)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), "/churches/join");
    let live = world.sdk.db.live_magic(id).await.unwrap().expect("token");
    assert!(live.consumed_at.is_none());
}

#[tokio::test]
async fn us_mem_04_pastor_can_approve_a_join_request() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, church_id) = plant(&world, &miriam, "Grace Covenant").await;
    let (_miriam, _) = post_need(
        &world,
        &miriam,
        &church_id,
        "Prayer covering",
        "Pray for the Okonkwo family this week.",
        "church",
    )
    .await;

    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let peter = join(&world, &peter, &church_id).await;
    let (church, miriam, csrf) = get_page(
        world.app.clone(),
        Some(&miriam),
        &format!("/churches/{church_id}"),
    )
    .await;
    let csrf = csrf.expect("church csrf");
    assert!(church.contains("Peter Lang"));
    assert!(church.contains("Asked to join"));
    let peter_id = waiting_user_id(&church, &church_id);
    let status = post(
        &world,
        &miriam,
        &csrf,
        &format!("/churches/{church_id}/members/{peter_id}/approve"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let home = get(&world, &peter, "/home").await;
    assert!(home.contains("Open needs"));
    assert!(home.contains("Prayer covering"));
}

#[tokio::test]
async fn us_need_05_neighboring_need_is_visible_across_the_valley() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (miriam, _) = post_need(
        &world,
        &miriam,
        &grace,
        "Spanish interpreter",
        "We need someone who can hold both languages on Thursday.",
        "neighboring",
    )
    .await;
    let (_miriam, _) = post_need(
        &world,
        &miriam,
        &grace,
        "Dinners for the Okonkwo family",
        "Five dinners this week.",
        "church",
    )
    .await;

    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let (_elena, _) = plant(&world, &elena, "New Mercy").await;
    let home = get(&world, &elena, "/home").await;
    assert!(home.contains("Spanish interpreter"));
    assert!(!home.contains("Dinners for the Okonkwo"));

    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let _peter = join(&world, &peter, &grace).await;
    let home = get(&world, &peter, "/home").await;
    assert!(!home.contains("Spanish interpreter"));
    assert!(home.contains("Waiting"));
}

#[tokio::test]
async fn us_need_02_public_reply() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (_miriam, need_id) = post_need(
        &world,
        &miriam,
        &grace,
        "Spanish interpreter",
        "We need someone who can hold both languages on Thursday.",
        "neighboring",
    )
    .await;

    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let (_elena, _) = plant(&world, &elena, "New Mercy").await;
    let (page, cookie, csrf) = get_page(
        world.app.clone(),
        Some(&elena),
        &format!("/needs/{need_id}"),
    )
    .await;
    let csrf = csrf.expect("need csrf");
    let after_open = page.split_once("<textarea").expect("reply textarea").1;
    let (inside, rest) = after_open
        .split_once("</textarea>")
        .expect("textarea must be closed");
    assert!(!inside.contains("Reply"));
    assert!(rest.contains(r#"<button class="btn" type="submit">Post reply</button>"#));
    assert!(!page.contains("This need has been met"));

    let sent = post_form(
        world.app.clone(),
        Some(&cookie),
        &format!("/needs/{need_id}/replies"),
        format!("csrf={csrf}&body=I+can+hold+both+languages+on+Thursday&pass=publish"),
    )
    .await;
    assert_eq!(sent.status(), StatusCode::SEE_OTHER);
    let location = location_of(&sent);
    assert!(location.contains("ok=replied"), "got {location}");

    let again = get(&world, &cookie, &format!("/needs/{need_id}")).await;
    assert!(again.contains("I can hold both languages on Thursday"));
    assert!(again.contains("Elena Vasquez"));
    assert!(again.contains("/members/"));
    let flashed = get(&world, &cookie, &location).await;
    assert!(flashed.contains("Reply posted."));
}

#[tokio::test]
async fn us_end_02_endorsement_is_not_public_until_accepted() {
    let world = app().await;
    let ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let (_ruth, _) = plant(&world, &ruth, "Grace Covenant").await;
    let james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let (james, _) = plant(&world, &james, "New Mercy").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let (_page, james, csrf) = get_page(
        world.app.clone(),
        Some(&james),
        &format!("/members/{ruth_id}"),
    )
    .await;
    let csrf = csrf.expect("member csrf");
    let status = post(
        &world,
        &james,
        &csrf,
        &format!("/members/{ruth_id}/endorse"),
        "skill=Hospitality&note=Ruth+fed+40+of+our+teenagers+after+the+flood+cleanup+in+May.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let ruth = sign_in(&world, "ruth@grace.test").await;
    let (inbox, cookie, csrf) = get_page(world.app.clone(), Some(&ruth), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    assert!(inbox.contains("James Whitaker"));
    assert!(inbox.contains("Hospitality"));
    assert!(
        !inbox.contains("Accept it from your inbox"),
        "the pending card is the decision; the notice is only a badge"
    );

    let before = get(&world, &cookie, &format!("/members/{ruth_id}")).await;
    assert!(!before.contains("flood cleanup"));
    let id = endorsement_id_near(&inbox, "Hospitality").expect("pending endorsement");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/endorsements/{id}/accept"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let after = get(&world, &cookie, &format!("/members/{ruth_id}")).await;
    assert!(after.contains("Endorsements"));
    assert!(after.contains("James Whitaker"));
    assert!(after.contains("flood cleanup"));
}

#[tokio::test]
async fn us_end_02_declined_stays_with_the_pair_and_can_be_accepted() {
    let world = app().await;
    let ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let (_ruth, _) = plant(&world, &ruth, "Grace Covenant").await;
    let james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let (_james, _) = plant(&world, &james, "New Mercy").await;
    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let (_peter, _) = plant(&world, &peter, "Hope Chapel").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let james = sign_in(&world, "james@stlukes.test").await;
    let (_page, james, csrf) = get_page(
        world.app.clone(),
        Some(&james),
        &format!("/members/{ruth_id}"),
    )
    .await;
    let csrf = csrf.expect("member csrf");
    let status = post(
        &world,
        &james,
        &csrf,
        &format!("/members/{ruth_id}/endorse"),
        "skill=Hospitality&note=Ruth+fed+40+after+the+flood+cleanup+in+May.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let ruth = sign_in(&world, "ruth@grace.test").await;
    let (inbox, cookie, csrf) = get_page(world.app.clone(), Some(&ruth), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    let id = endorsement_id_near(&inbox, "Hospitality").expect("pending endorsement");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/endorsements/{id}/decline"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let inbox = get(&world, &cookie, "/inbox").await;
    assert!(inbox.contains("Declined"));
    assert!(inbox.contains("flood cleanup"));
    assert!(inbox.contains("card-dim"));
    assert!(inbox.contains("Accept"));

    let as_ruth = get(&world, &cookie, &format!("/members/{ruth_id}")).await;
    assert!(as_ruth.contains("Declined"));
    assert!(as_ruth.contains("flood cleanup"));
    assert!(as_ruth.contains("card-dim"));
    assert!(as_ruth.contains("Accept"));

    let peter = sign_in(&world, "peter@grace.test").await;
    let as_peter = get(&world, &peter, &format!("/members/{ruth_id}")).await;
    assert!(!as_peter.contains("flood cleanup"));

    let james = sign_in(&world, "james@stlukes.test").await;
    let as_james = get(&world, &james, &format!("/members/{ruth_id}")).await;
    assert!(as_james.contains("Declined"));
    assert!(as_james.contains("flood cleanup"));
    assert!(!as_james.contains(&format!("/endorsements/{id}/accept")));

    let ruth = sign_in(&world, "ruth@grace.test").await;
    let (_inbox, cookie, csrf) = get_page(world.app.clone(), Some(&ruth), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/endorsements/{id}/accept"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let peter = sign_in(&world, "peter@grace.test").await;
    let public = get(&world, &peter, &format!("/members/{ruth_id}")).await;
    assert!(public.contains("Endorsements"));
    assert!(public.contains("flood cleanup"));
    assert!(!public.contains("card-dim"));
}

#[tokio::test]
async fn us_end_01_can_endorse_a_skill_they_have_not_claimed() {
    let world = app().await;
    let daniel = register(&world, "Daniel Cole", "daniel@grace.test").await;
    let (_daniel, _) = plant(&world, &daniel, "Grace Covenant").await;
    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let (elena, _) = plant(&world, &elena, "New Mercy").await;
    let daniel_id = user_id(&world, "daniel@grace.test").await;
    let (_page, cookie, csrf) = get_page(
        world.app.clone(),
        Some(&elena),
        &format!("/members/{daniel_id}"),
    )
    .await;
    let csrf = csrf.expect("member csrf");
    assert!(_page.contains(r#"name="skill""#));

    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/members/{daniel_id}/endorse"),
        "skill=Mercy&note=He+thanked+every+person+who+brought+food+and+meant+it.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let daniel = sign_in(&world, "daniel@grace.test").await;
    let inbox = get(&world, &daniel, "/inbox").await;
    assert!(inbox.contains("Mercy"));
    assert!(inbox.contains("thanked every person"));
    assert!(inbox.contains("Accept"));
    assert!(inbox.contains("Decline"));

    let before = get(&world, &daniel, &format!("/members/{daniel_id}")).await;
    assert!(!before.contains("thanked every person"));
    assert!(!before.contains("Mercy"));
}

#[tokio::test]
async fn us_end_01_spoken_skill_is_not_limited_to_the_catalog() {
    let world = app().await;
    let ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let (_ruth, _) = plant(&world, &ruth, "Grace Covenant").await;
    let james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let (james, _) = plant(&world, &james, "New Mercy").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let (_page, cookie, csrf) = get_page(
        world.app.clone(),
        Some(&james),
        &format!("/members/{ruth_id}"),
    )
    .await;
    let csrf = csrf.expect("member csrf");

    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/members/{ruth_id}/endorse"),
        "skill=Staying+until+the+last+parent+left&note=She+washed+the+trays+after+the+youth+left+and+then+sat+with+the+one+kid+whose+ride+was+late.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let ruth = sign_in(&world, "ruth@grace.test").await;
    let (inbox, cookie, csrf) = get_page(world.app.clone(), Some(&ruth), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    assert!(inbox.contains("Staying until the last parent left"));

    let id = endorsement_id_near(&inbox, "Staying until the last parent left")
        .expect("pending spoken endorsement");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/endorsements/{id}/accept"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let profile = get(&world, &cookie, &format!("/members/{ruth_id}")).await;
    assert!(profile.contains("Endorsements"));
    assert!(profile.contains("Staying until the last parent left"));
    assert!(profile.contains("washed the trays"));
}

#[tokio::test]
async fn us_app_01_manifest_is_installable() {
    let world = app().await;
    let manifest = get_public(&world, "/static/manifest.webmanifest").await;
    assert!(manifest.contains(r#""display": "standalone""#));
    assert!(manifest.contains(r#""start_url": "/home""#));
    assert!(manifest.contains("/static/icon-192.png"));
    assert!(manifest.contains("/inbox"));

    let (sw, headers) = get_public_with_headers(&world, "/sw.js").await;
    assert!(sw.contains("showNotification"));
    assert_eq!(
        headers
            .get("service-worker-allowed")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(landing.contains("apple-touch-icon"));
    assert!(landing.contains("install-bar"));
    assert!(landing.contains("install-bar-ios"));
    assert!(landing.contains("install-help"));
    assert!(landing.contains("ios-install-iphone-share.svg"));
    assert!(landing.contains("/static/app.js"));
    assert!(landing.contains("site-guest"));
}

#[tokio::test]
async fn us_app_01_guest_shell_css_respects_safe_area_for_install_bar() {
    let world = app().await;
    let css = get_public(&world, "/static/app.css").await;
    assert!(
        css.contains("body.site-guest") && css.contains("padding-top: env(safe-area-inset-top"),
        "guest body should clear the notch"
    );
    assert!(
        css.contains(".install-bar")
            && css.contains("position: fixed")
            && css.contains("top: calc(env(safe-area-inset-top"),
        "install bar should float below the safe area without shifting the page"
    );
}

#[tokio::test]
async fn us_app_03_auth_routes_carry_one_form_each() {
    let world = app().await;
    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(
        !landing.contains(r#"action="/register""#) && !landing.contains(r#"action="/session""#),
        "homepage should not embed account forms"
    );

    let (register, _, _) = get_page(world.app.clone(), None, "/register").await;
    assert_eq!(register.matches(r#"action="/register""#).count(), 1);
    assert!(register.contains("<h1>You</h1>"));
    assert!(register.contains("Find your church"));
    assert!(register.contains("class=\"steps\""));
    assert!(register.contains("sheet-auth"));
    assert!(register.contains("site-account"));
    assert!(!register.contains("install-bar"));
    assert!(register.contains("href=\"/session/new\""));
    assert!(register.contains("data-auth=\"register\""));
    assert!(!register.contains("We are the ecclesia"));

    let (sign_in, _, _) = get_page(world.app.clone(), None, "/session/new").await;
    assert!(sign_in.contains(r#"action="/session""#));
    assert!(sign_in.contains("<h1>Sign in</h1>"));
    assert!(sign_in.contains("data-password-toggle"));
    assert!(sign_in.contains("data-auth=\"sign-in\""));
    assert!(sign_in.contains("href=\"/register\""));
    assert!(!sign_in.contains(r#"action="/register""#));
    assert!(!sign_in.contains("install-bar"));

    let (link, _, _) = get_page(world.app.clone(), None, "/session/link/new").await;
    assert_eq!(link.matches(r#"action="/session/link""#).count(), 1);
    assert!(!link.contains(r#"action="/session""#));

    let (reset, _, _) = get_page(world.app.clone(), None, "/session/reset/new").await;
    assert_eq!(reset.matches(r#"action="/session/reset""#).count(), 1);
}

#[tokio::test]
async fn us_app_01_you_page_offers_alerts_and_share() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, church_id) = plant(&world, &cookie, "Grace Covenant").await;
    let (cookie, need_id) = post_need(
        &world,
        &cookie,
        &church_id,
        "Dinners for the Okonkwo family",
        "Five dinners this week.",
        "church",
    )
    .await;
    let me = get(&world, &cookie, "/me").await;
    assert!(me.contains("data-alerts"));
    assert!(me.contains("Turn on alerts"));

    let church = get(&world, &cookie, &format!("/churches/{church_id}")).await;
    let row = world
        .sdk
        .db
        .church(&church_id)
        .await
        .expect("church")
        .expect("row");
    let join = format!("http://127.0.0.1:43781/join/{}", row.invite_code);
    assert!(church.contains("data-print-qr"));
    assert!(church.contains("Print code"));
    assert!(church.contains("data-invite-email"));
    assert!(church.contains("data-invite-drop"));
    assert!(church.contains("Open file"));
    assert!(church.contains("invite-qr"));
    assert!(church.contains(&format!("data-join=\"{join}\"")));
    assert!(church.contains("Send invite"));
    assert!(!church.contains("Show code"));
    assert!(!church.contains("class=\"code\""));
    assert!(!church.contains("data-share"));

    let need = get(&world, &cookie, &format!("/needs/{need_id}")).await;
    let share = world
        .sdk
        .db
        .share_for_target(ecclesia_sdk::prelude::ShareKind::Need, &need_id)
        .await
        .expect("share")
        .expect("code");
    assert!(need.contains(&format!("data-share-url=\"/s/{}\"", share.code)));
    assert!(need.contains("data-share"));

    let response = world
        .app
        .clone()
        .oneshot(
            Request::get(format!("/s/{}", share.code))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&response), format!("/needs/{need_id}"));

    let missing = world
        .app
        .clone()
        .oneshot(Request::get("/s/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::OK);
    let missing_html = body_string(missing).await;
    assert!(missing_html.contains("We couldn't find that link."));
}

#[tokio::test]
async fn us_push_01_signed_in_person_can_subscribe() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/churches/join").await;
    let csrf = csrf.expect("join csrf");
    let status = post(
        &world,
        &cookie,
        &csrf,
        "/push/subscribe",
        "endpoint=https://push.example/m1&p256dh=abc&auth=def",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let device = post(
        &world,
        &cookie,
        &csrf,
        "/push/device",
        "token=fcm-test-token&platform=android",
    )
    .await;
    assert_eq!(device, StatusCode::NO_CONTENT);

    let vapid = get_public(&world, "/push/vapid").await;
    assert!(vapid.is_empty() || vapid.starts_with("B"));
}

#[tokio::test]
async fn us_tone_01_an_attack_is_refused() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, church_id) = plant(&world, &cookie, "Grace Covenant").await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    let location = post_location(
        &world,
        &cookie,
        &csrf,
        "/needs",
        &format!(
            "church_id={church_id}&title=Attack&body=You+are+worthless+and+you+suck.&scope=church"
        ),
    )
    .await;
    assert!(
        location.contains("err=tone"),
        "attack should bounce with tone, got {location}"
    );

    let home = get(&world, &cookie, "/home").await;
    assert!(!home.contains("You are worthless"));
}

#[tokio::test]
async fn us_refine_01_silent_echoes_the_same_words() {
    let world = app().await;
    let (register_page, cookie, csrf) = get_page(world.app.clone(), None, "/register").await;
    let csrf = csrf.expect("register csrf");
    assert!(!register_page.contains("data-rewrite"));
    assert!(register_page.contains(r#"name="first_name""#));
    assert!(register_page.contains("data-password-toggle"));

    let (status, body) = post_json(
        &world,
        &cookie,
        &csrf,
        "/refine",
        "kind=endorsement&text=+She+stayed.+",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#""text":"She stayed.""#));
    assert!(body.contains(r#""seat":"echo""#));

    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (_miriam, need_id) = post_need(
        &world,
        &miriam,
        &grace,
        "Spanish interpreter",
        "We need someone who can hold both languages on Thursday.",
        "neighboring",
    )
    .await;
    let _daniel = register(&world, "Daniel Cole", "daniel@grace.test").await;
    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let (_elena, _) = plant(&world, &elena, "New Mercy").await;
    let need = get(&world, &elena, &format!("/needs/{need_id}")).await;
    assert!(need.contains(r#"data-kind="reply""#));
    let daniel_id = user_id(&world, "daniel@grace.test").await;
    let member = get(&world, &elena, &format!("/members/{daniel_id}")).await;
    assert!(member.contains(r#"data-kind="endorsement""#));
}

#[tokio::test]
async fn us_refine_02_submit_shows_the_rewrite_before_publish() {
    let world = app_with(
        ecclesia_sdk::judge::JudgeHub::word_gate(),
        ecclesia_sdk::refine::RefineHub::polish(),
    )
    .await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, church_id) = plant(&world, &cookie, "Grace Covenant").await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    let (status, html) = post_page(
        &world,
        &cookie,
        &csrf,
        "/needs",
        &format!(
            "church_id={church_id}&title=Need+five+dinners&body=Need+++five+++dinners&scope=church"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Need five dinners"));
    assert!(html.contains("Read this through."));
    assert!(html.contains(">Publish<"));
    assert!(html.contains(r#"name="pass" value="publish""#));

    let location = post_location(
        &world,
        &cookie,
        &csrf,
        "/needs",
        &format!(
            "church_id={church_id}&title=Need+five+dinners&body=Need+five+dinners&scope=church&pass=publish"
        ),
    )
    .await;
    assert!(location.contains("/needs/"));
    assert!(location.contains("ok=need_posted"));

    let home = get(&world, &cookie, "/home").await;
    assert!(home.contains("Need five dinners"));
}

#[tokio::test]
async fn us_sec_03_forged_flash_stays_generic() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let bait = get(
        &world,
        &cookie,
        "/churches/join?ok=Visit+https://evil.example+now",
    )
    .await;
    assert!(bait.contains("Done."));
    assert!(!bait.contains("evil.example"));
    let html = get(
        &world,
        &cookie,
        "/churches/join?err=%3Cscript%3Ealert(1)%3C/script%3E",
    )
    .await;
    assert!(html.contains("That didn't work."));
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(!html.contains("alert(1)"));
}

#[tokio::test]
async fn us_app_03_website_landing_and_guest_home() {
    let world = app().await;
    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(landing.contains("The Body of Christ"));
    let title = landing.find("The Body of Christ").expect("title");
    let account = landing.find("Create an account").expect("account");
    let narrative = landing
        .find("The church was a community before it was an organization.")
        .expect("narrative");
    assert!(title < account);
    assert!(account < narrative);
    let acts = landing.find("Acts 2:44-45").expect("acts 2");
    assert_eq!(landing.matches("Acts 2:44-45").count(), 1);
    assert!(acts < narrative);
    assert!(landing.contains("The Church Takes Care of Its Own"));
    assert!(landing.contains("See how they love one another."));
    assert!(landing.contains("It was part of the mission."));
    assert!(landing.contains("Ride home after surgery"));
    assert!(landing.contains("I can pick you up Thursday."));
    assert!(landing.contains("I haven't seen the Brennans in a while"));
    assert!(landing.contains("My wife has been very sick. I've been home with her."));
    assert!(!landing.contains("Help while my wife is sick"));
    assert!(landing.contains("Prayer matters"));
    assert!(landing.contains("My daughter hasn't spoken to us in six months."));
    assert!(!landing.contains("Please pray"));
    assert!(!landing.contains("please pray"));
    assert!(landing.contains("They just need to find each other."));
    assert!(landing.contains("One Body"));
    assert!(!landing.contains("Ecclesia is free for churches and their members."));
    assert!(landing.contains("Bear one another\u{2019}s burdens"));
    assert!(landing.contains("There was not a needy person among them."));
    assert!(landing.contains("1 Corinthians 12:27"));
    assert!(!landing.contains("steeples-wide.webp"));
    assert!(!landing.contains("steeples-tall.webp"));
    assert!(landing.contains("linda-handrail-before.webp"));
    assert!(landing.contains("church-map"));
    assert!(landing.contains("Hope Chapel"));
    assert!(landing.contains("Handrail for my front steps"));
    assert!(landing.contains("Licensed contractor"));
    assert!(landing.contains("Roof repair"));
    assert!(!landing.contains("The church already has most of what it needs"));
    assert!(!landing.contains("A family needs a crib."));
    assert!(!landing.contains("She posts a simple request"));
    assert!(!landing.contains("Within the hour, three people respond."));
    assert!(!landing.contains("A member asks"));
    assert!(!landing.contains("A Need, Answered Together"));
    assert!(!landing.contains("data-photo-viewer"));
    assert!(!landing.contains("data-photo-open"));
    assert!(landing.contains("/static/app.css?v=48"));
    assert!(landing.contains("landing-phones"));
    assert!(!landing.contains("<input"));
    assert!(landing.contains("Create an account"));
    assert!(landing.contains("Sign in"));
    assert!(!landing.contains("Email me a link"));
    assert!(!landing.contains("Forgot password"));
    assert!(!landing.contains("Ask for help"));
    assert!(!landing.contains("People in Cedar Falls"));
    assert!(!landing.contains("Open the demo"));
    assert!(landing.contains("href=\"/register\""));
    assert!(landing.contains("href=\"/session/new\""));
    assert!(!landing.contains("method=\"post\" action=\"/register\""));

    let (sign_in, _, _) = get_page(world.app.clone(), None, "/session/new").await;
    assert!(sign_in.contains("Email me a link"));
    assert!(sign_in.contains("Forgot password"));

    let (guest, _, _) = get_page(world.app.clone(), None, "/home").await;
    assert!(guest.contains("Create an account"));
    assert!(guest.contains("Sign in"));
    assert!(!guest.contains("Open needs"));

    let response = world
        .app
        .clone()
        .oneshot(Request::get("/home").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let signed = world
        .app
        .clone()
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signed.status(), StatusCode::SEE_OTHER);
    let location = signed
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/churches/join");
}

#[tokio::test]
async fn us_app_04_privacy_and_terms_pages() {
    let world = app().await;
    let (privacy, _, _) = get_page(world.app.clone(), None, "/privacy").await;
    assert!(privacy.contains("<h1"));
    assert!(privacy.contains("Privacy"));
    assert!(privacy.contains("mailto:support@ecclesiatogether.org"));
    assert!(privacy.contains("18 and older"));
    assert!(privacy.contains("Neon stores the database in Oregon."));
    assert!(privacy.contains(r#"class="site-footer""#));

    let (terms, _, _) = get_page(world.app.clone(), None, "/terms").await;
    assert!(terms.contains("Terms"));
    assert!(terms.contains("You're 18 or older."));
    assert!(terms.contains("Ecclesia is free for churches and members."));
}

#[tokio::test]
async fn us_app_04_landing_footer_links() {
    let world = app().await;
    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(landing.contains(r#"class="site-footer""#));
    assert!(landing.contains(r#"href="/privacy""#));
    assert!(landing.contains(r#"href="/terms""#));
    assert!(landing.contains("mailto:support@ecclesiatogether.org"));
    assert!(landing.contains("ESV Text Edition: 2025"));
    let main_end = landing.find("</main>").expect("main closes");
    let footer = landing.find(r#"class="site-footer""#).expect("site footer");
    assert!(footer > main_end, "footer follows main");
    let footer_html = &landing[footer..];
    assert!(footer_html.contains(r#"href="/give""#));
    assert!(footer_html.contains(">Give<"));

    let (give, _, _) = get_page(world.app.clone(), None, "/give").await;
    assert!(give.contains("<h1"));
    assert!(give.contains("Giving is not set up yet."));
    assert!(give.contains("not tax-deductible until Ecclesia is a recognized charity."));
    assert!(!give.contains("<form"));
    assert!(!give.contains("<input"));
}

#[tokio::test]
async fn us_app_04_churchless_member_opens_privacy() {
    let world = app().await;
    let cookie = register(&world, "No Church", "legal@example.test").await;
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/privacy")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let give = world
        .app
        .clone()
        .oneshot(
            Request::get("/give")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(give.status(), StatusCode::OK);
}

#[tokio::test]
async fn us_sec_01_forged_cookie_cannot_sit_as_anyone() {
    let world = app().await;
    let response = world
        .app
        .clone()
        .oneshot(
            Request::get("/home")
                .header(
                    header::COOKIE,
                    "ecclesia_sid=v1.user_miriam.aabbccddeeff00112233445566778899.00",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("Create an account"));
    assert!(!html.contains("Open needs"));
    assert!(!html.contains("class=\"who-name\""));
}

#[tokio::test]
async fn us_sec_08_refine_rejects_overlong_text() {
    let world = app().await;
    let (_landing, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let huge = "x".repeat(2001);
    let (status, _body) = post_json(
        &world,
        &cookie,
        &csrf,
        "/refine",
        &format!("kind=bio&text={huge}"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn us_sec_10_refine_budget_returns_429() {
    let world = app().await;
    let (_landing, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let mut last = StatusCode::OK;
    for _ in 0..21 {
        let (status, _) = post_json(
            &world,
            &cookie,
            &csrf,
            "/refine",
            "kind=bio&text=She+stayed.",
        )
        .await;
        last = status;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn us_sec_07_push_rejects_a_plain_http_endpoint() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/churches/join").await;
    let csrf = csrf.expect("join csrf");
    let status = post(
        &world,
        &cookie,
        &csrf,
        "/push/subscribe",
        "endpoint=http://push.example/m1&p256dh=abc&auth=def",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn us_sec_14_invite_does_not_reveal_missing_email() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (cookie, church_id) = plant(&world, &cookie, "Grace Covenant").await;
    let _james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let (_page, cookie, csrf) = get_page(
        world.app.clone(),
        Some(&cookie),
        &format!("/churches/{church_id}"),
    )
    .await;
    let csrf = csrf.expect("church csrf");
    let missing = post_location(
        &world,
        &cookie,
        &csrf,
        &format!("/churches/{church_id}/invite"),
        "email=nobody%40example.test",
    )
    .await;
    let known = post_location(
        &world,
        &cookie,
        &csrf,
        &format!("/churches/{church_id}/invite"),
        "email=james%40stlukes.test",
    )
    .await;
    assert!(missing.contains("ok=invited"), "got {missing}");
    assert!(known.contains("ok=invited"), "got {known}");
}

#[tokio::test]
async fn us_sec_15_another_profile_hides_email() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_miriam, _) = plant(&world, &miriam, "Grace Covenant").await;
    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let (peter, _) = plant(&world, &peter, "New Mercy").await;
    let miriam_id = user_id(&world, "miriam@grace.test").await;
    let page = get(&world, &peter, &format!("/members/{miriam_id}")).await;
    assert!(page.contains("Miriam Cole"));
    assert!(!page.contains("miriam@grace.test"));
}

#[tokio::test]
async fn us_sec_16_security_headers_and_cookie_flags() {
    let world = app().await;
    let response = world
        .app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-frame-options")
            .and_then(|value| value.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    let csp = headers
        .get("content-security-policy")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    let cookie = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("ecclesia_sid="))
        .expect("session cookie");
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(
        !cookie.to_ascii_lowercase().contains("secure"),
        "plain transport must not set Secure: {cookie}"
    );
}

async fn post_json_api(app: axum::Router, uri: &str, body: &str) -> axum::http::Response<Body> {
    app.oneshot(
        Request::post(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn us_api_01_json_sign_in_refresh_and_me() {
    let world = app_with(
        ecclesia_sdk::judge::JudgeHub::silent(),
        ecclesia_sdk::refine::RefineHub::silent(),
    )
    .await;
    let email = format!(
        "api-{}@example.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let cookie = register(&world, "Api Member", &email).await;
    let cookie_only = world
        .app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cookie_only.status(), StatusCode::UNAUTHORIZED);

    let api_sign_in = post_json_api(
        world.app.clone(),
        "/api/session",
        &format!(r#"{{"email":"{email}","password":"{pass}"}}"#, pass = PASS),
    )
    .await;
    assert_eq!(api_sign_in.status(), StatusCode::OK);
    assert!(try_cookie_from(&api_sign_in).is_none());
    let sign_body = body_string(api_sign_in).await;
    let tokens: serde_json::Value = serde_json::from_str(&sign_body).unwrap();
    let access = tokens["access_token"].as_str().expect("access");
    let refresh = tokens["refresh_token"].as_str().expect("refresh");
    assert_eq!(tokens["token_type"].as_str(), Some("Bearer"));
    assert_eq!(tokens["expires_in"].as_i64(), Some(900));

    let me = world
        .app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let cache = me
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok());
    assert_eq!(cache, Some("no-store"));
    let profile: serde_json::Value = serde_json::from_str(&body_string(me).await).unwrap();
    assert_eq!(profile["email"].as_str(), Some(email.as_str()));
    assert_eq!(profile["first_name"].as_str(), Some("Api"));
    assert_eq!(profile["last_name"].as_str(), Some("Member"));
    assert_eq!(profile["memberships"].as_array().map(Vec::len), Some(0));
    assert!(profile["id"].as_str().is_some());

    let rotated = post_json_api(
        world.app.clone(),
        "/api/session/refresh",
        &format!(r#"{{"refresh_token":"{refresh}"}}"#),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated_body: serde_json::Value =
        serde_json::from_str(&body_string(rotated).await).unwrap();
    let refresh2 = rotated_body["refresh_token"].as_str().expect("refresh2");
    let access2 = rotated_body["access_token"].as_str().expect("access2");
    assert_ne!(refresh2, refresh);

    let stale = post_json_api(
        world.app.clone(),
        "/api/session/refresh",
        &format!(r#"{{"refresh_token":"{refresh}"}}"#),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::UNAUTHORIZED);

    let cookie = sign_in(&world, &email).await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/churches/join").await;
    let csrf = csrf.expect("me csrf");
    let logout_all = post_form(
        world.app.clone(),
        Some(&cookie),
        "/session/logout-all",
        format!("csrf={csrf}"),
    )
    .await;
    assert_eq!(logout_all.status(), StatusCode::SEE_OTHER);
    let me_after = world
        .app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header(header::AUTHORIZATION, format!("Bearer {access2}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn us_near_01_nearby_stays_empty_until_a_point_is_shared() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (_miriam, _) = post_need(
        &world,
        &miriam,
        &grace,
        "Church dinners",
        "Five dinners this week.",
        "church",
    )
    .await;
    let traveler = register(&world, "Ada Lovelace", "ada@austin.test").await;
    let (traveler, _) = plant_at(&world, &traveler, "Austin Chapel", "30.2672", "-97.7431").await;

    let empty = get(&world, &traveler, "/nearby").await;
    assert!(empty.contains("Share where you are"));
    assert!(!empty.contains("Church dinners"));

    let here = get(&world, &traveler, "/nearby?lat=42.5349&lng=-92.4453").await;
    assert!(here.contains("Church dinners"));

    let away = get(&world, &traveler, "/nearby?lat=30.2672&lng=-97.7431").await;
    assert!(!away.contains("Church dinners"));

    let home = get(&world, &traveler, "/home").await;
    assert!(!home.contains("Church dinners"));
}

#[tokio::test]
async fn us_need_07_a_traveler_can_reply_when_the_church_is_near() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let (_miriam, need_id) = post_need(
        &world,
        &miriam,
        &grace,
        "Church dinners",
        "Five dinners this week.",
        "church",
    )
    .await;
    let traveler = register(&world, "Ada Lovelace", "ada@austin.test").await;
    let (traveler, _) = plant_at(&world, &traveler, "Austin Chapel", "30.2672", "-97.7431").await;

    let (page, cookie, csrf) = get_page(
        world.app.clone(),
        Some(&traveler),
        &format!("/needs/{need_id}?lat=42.5349&lng=-92.4453"),
    )
    .await;
    assert!(page.contains("Church dinners"));
    let csrf = csrf.expect("need csrf");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/needs/{need_id}/replies"),
        "body=I+can+bring+dinner+Thursday.&lat=42.5349&lng=-92.4453&pass=publish",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let thread = get(
        &world,
        &cookie,
        &format!("/needs/{need_id}?lat=42.5349&lng=-92.4453"),
    )
    .await;
    assert!(thread.contains("I can bring dinner Thursday."));
    assert!(thread.contains("Ada Lovelace"));
    assert!(thread.contains("/members/"));

    let refused = post_form(
        world.app.clone(),
        Some(&cookie),
        &format!("/needs/{need_id}/replies"),
        format!("csrf={csrf}&body=Too+far+to+help.&lat=30.2672&lng=-97.7431&pass=publish"),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::SEE_OTHER);
    let location = location_of(&refused);
    assert!(location.contains("err=scope"), "got {location}");
}

#[tokio::test]
async fn us_pray_02_church_prayers_come_before_the_shared_point() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let elena = register(&world, "Elena Vasquez", "elena@grace.test").await;
    let elena = join(&world, &elena, &grace).await;
    let (church, miriam, csrf) = get_page(
        world.app.clone(),
        Some(&miriam),
        &format!("/churches/{grace}"),
    )
    .await;
    let elena_id = waiting_user_id(&church, &grace);
    let status = post(
        &world,
        &miriam,
        &csrf.expect("church csrf"),
        &format!("/churches/{grace}/members/{elena_id}/approve"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let james = register(&world, "James Wright", "james@mercy.test").await;
    let (james, mercy) = plant_at(&world, &james, "Mercy Chapel", "42.4928", "-92.3426").await;
    let ada = register(&world, "Ada Lovelace", "ada@austin.test").await;
    let (ada, austin) = plant_at(&world, &ada, "Austin Chapel", "30.2672", "-97.7431").await;

    let (_miriam, church_prayer) =
        post_prayer(&world, &miriam, &grace, "Surgery on Thursday.", "signed").await;
    let (_james, mercy_prayer) =
        post_prayer(&world, &james, &mercy, "Mercy roof prayer.", "signed").await;
    let (_ada, austin_prayer) =
        post_prayer(&world, &ada, &austin, "Austin far prayer.", "signed").await;

    let deck = get(&world, &elena, "/pray").await;
    assert!(deck.contains("Surgery on Thursday."));
    assert!(deck.contains("Grace Covenant"));
    assert!(!deck.contains("Your church"));
    assert!(deck.contains("Pray for churches where you are."));
    assert!(deck.contains("href=\"/nearby\""));
    assert!(deck.contains("Not now"));
    assert!(deck.contains("data-pray-toast"));
    assert!(!deck.contains("Pray where you are"));
    assert!(!deck.contains("Share where you are"));
    assert!(!deck.contains("Mercy roof prayer."));
    assert!(!deck.contains("Austin far prayer."));

    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&elena), "/pray").await;
    let csrf = csrf.expect("pray csrf");
    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/prayers/{church_prayer}/next"),
        "pass=publish",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let after = get(&world, &elena, "/pray").await;
    assert!(after.contains("Mercy roof prayer."));
    assert!(after.contains("Mercy Chapel"));
    assert!(after.contains("Pray for churches where you are."));
    assert!(!after.contains("Surgery on Thursday."));
    assert!(!after.contains("Austin far prayer."));
    assert!(!after.contains("Share where you are"));
    assert!(!after.contains("Surrounding church"));

    let ignored = get(&world, &elena, "/pray?lat=30.2672&lng=-97.7431").await;
    assert!(ignored.contains("Mercy roof prayer."));
    assert!(ignored.contains("Mercy Chapel"));
    assert!(!ignored.contains("Austin far prayer."));
    assert!(!ignored.contains("Surrounding church"));

    let nearby = get(&world, &elena, "/nearby?lat=42.5349&lng=-92.4453").await;
    assert!(nearby.contains("Mercy roof prayer."));
    assert!(!nearby.contains("Surgery on Thursday."));
    assert!(!nearby.contains("Austin far prayer."));

    let empty_nearby = get(&world, &elena, "/nearby").await;
    assert!(empty_nearby.contains("Share where you are"));
    assert!(empty_nearby.contains("Share location"));
    assert!(!empty_nearby.contains("Austin far prayer."));

    let away = get(&world, &elena, "/nearby?lat=30.2672&lng=-97.7431").await;
    assert!(away.contains("Austin far prayer."));
    assert!(!away.contains("Share location"));
    assert!(!away.contains("Mercy roof prayer."));

    let marked = post_location(
        &world,
        &cookie,
        &csrf,
        &format!("/prayers/{mercy_prayer}/next"),
        "pass=publish",
    )
    .await;
    assert_eq!(marked, "/pray?ok=next");

    let done = get(&world, &elena, "/pray").await;
    assert!(done.contains("You've prayed through today's requests."));
    assert!(done.contains("Pray for churches where you are."));
    assert!(!done.contains("Mercy roof prayer."));
    assert!(!done.contains("Austin far prayer."));

    let refused = post_location(
        &world,
        &cookie,
        &csrf,
        &format!("/prayers/{austin_prayer}/next"),
        "pass=publish",
    )
    .await;
    assert!(refused.contains("err=scope"), "got {refused}");

    let moved = post_location(
        &world,
        &cookie,
        &csrf,
        &format!("/prayers/{austin_prayer}/next"),
        "lat=30.2672&lng=-97.7431&pass=publish",
    )
    .await;
    assert!(moved.contains("lat=30.2672"), "got {moved}");
    assert!(moved.contains("ok=next"), "got {moved}");
}

#[tokio::test]
async fn us_pray_01_an_unnamed_prayer_shows_no_author() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let miriam_id = user_id(&world, "miriam@grace.test").await;
    let elena = register(&world, "Elena Vasquez", "elena@grace.test").await;
    let elena = join(&world, &elena, &grace).await;
    let (church, miriam, csrf) = get_page(
        world.app.clone(),
        Some(&miriam),
        &format!("/churches/{grace}"),
    )
    .await;
    let elena_id = waiting_user_id(&church, &grace);
    post(
        &world,
        &miriam,
        &csrf.expect("church csrf"),
        &format!("/churches/{grace}/members/{elena_id}/approve"),
        "",
    )
    .await;

    let (_miriam, prayer_id) =
        post_prayer(&world, &miriam, &grace, "Surgery on Thursday.", "unnamed").await;
    let card = get(&world, &elena, &format!("/prayers/{prayer_id}")).await;
    assert!(card.contains("Surgery on Thursday."));
    assert!(!card.contains("Miriam Cole"));
    assert!(!card.contains("Grace Covenant"));
    assert!(!card.contains("No name"));
    assert!(!card.contains(&format!("/members/{miriam_id}")));
    assert!(!card.contains(&format!("/churches/{grace}")));

    let ada = register(&world, "Ada Lovelace", "ada-pool@austin.test").await;
    let (ada, _austin) = plant_at(&world, &ada, "Austin Chapel", "30.2672", "-97.7431").await;
    let deck = get(&world, &ada, "/pray").await;
    assert!(deck.contains("Surgery on Thursday."));
    assert!(!deck.contains("Grace Covenant"));
    assert!(!deck.contains("No name"));
    assert!(deck.contains("aria-label=\"Pray\""));
    assert!(deck.contains("aria-pressed=\"false\""));
    assert!(deck.contains(&format!("action=\"/prayers/{prayer_id}/pray\"")));
    assert!(!deck.contains("You're the first to pray for this."));
    assert!(!deck.contains("pray-count"));
    assert!(!deck.contains("I prayed"));
    let (_page, ada, csrf) = get_page(world.app.clone(), Some(&ada), "/pray").await;
    let prayed = post(
        &world,
        &ada,
        &csrf.expect("pray csrf"),
        &format!("/prayers/{prayer_id}/pray"),
        "pass=publish",
    )
    .await;
    assert_eq!(prayed, StatusCode::SEE_OTHER);
    let shown = get(&world, &ada, &format!("/prayers/{prayer_id}")).await;
    assert!(!shown.contains("You're the first to pray for this."));
    assert!(!shown.contains("other people have prayed"));
    assert!(!shown.contains("pray-count"));
    assert!(shown.contains("aria-label=\"Pray\""));
    assert!(shown.contains("aria-pressed=\"true\""));
    assert!(shown.contains("is-pressed"));
    assert!(!shown.contains(&format!("action=\"/prayers/{prayer_id}/pray\"")));
    assert!(!shown.contains("I prayed"));
}

async fn post_prayer(
    world: &World,
    cookie: &str,
    church_id: &str,
    body: &str,
    byline: &str,
) -> (String, String) {
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(cookie), "/prayers/new").await;
    let csrf = csrf.expect("prayer csrf");
    let extra = format!(
        "csrf={csrf}&church_id={}&body={}&byline={}&pass=publish",
        enc(church_id),
        enc(body),
        enc(byline)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/prayers", extra).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "post prayer");
    let prayer_id = id_from_location(&location_of(&response), "/prayers/");
    (try_cookie_from(&response).unwrap_or(cookie), prayer_id)
}

#[tokio::test]
async fn us_media_01_need_reply_completion_and_audience() {
    let world = app().await;
    let miriam = register(&world, "Miriam Cole", "miriam-media@grace.test").await;
    let (miriam, grace) = plant(&world, &miriam, "Grace Covenant").await;
    let jpeg = tiny_jpeg();

    let (page, miriam, csrf) = get_page(world.app.clone(), Some(&miriam), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    assert!(page.contains("Add photos"));
    assert!(page.contains("Up to five photos. JPEG, PNG, or WebP."));
    let posted = post_multipart(
        world.app.clone(),
        &miriam,
        "/needs",
        &[
            ("csrf", &csrf),
            ("church_id", &grace),
            ("title", "Handrail for the front steps"),
            ("body", "The old railing came loose last winter."),
            ("scope", "church"),
            ("pass", "publish"),
        ],
        &[("photos", "steps.jpg", "image/jpeg", jpeg.as_slice())],
    )
    .await;
    assert_eq!(posted.status(), StatusCode::SEE_OTHER);
    let location = location_of(&posted);
    assert!(location.contains("ok=need_posted"), "{location}");
    let miriam = try_cookie_from(&posted).unwrap_or(miriam);
    let need_id = id_from_location(&location, "/needs/");

    let need = get(&world, &miriam, &format!("/needs/{need_id}")).await;
    assert!(need.contains("photo-large"));
    assert!(need.contains("Add a reply"));
    assert!(need.contains("Mark this need met"));
    assert!(need.contains("How was the need met?"));
    let media_id = media_id_from(&need);
    let photo = authed_get(
        world.app.clone(),
        &miriam,
        &format!("/media/{media_id}/full"),
    )
    .await;
    assert_eq!(photo.0, StatusCode::OK);
    assert_eq!(
        photo
            .1
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/webp")
    );
    assert!(photo.2.starts_with(b"RIFF"));

    let (need, miriam, csrf) = get_page(
        world.app.clone(),
        Some(&miriam),
        &format!("/needs/{need_id}"),
    )
    .await;
    let csrf = csrf.expect("reply csrf");
    assert!(need.contains(">MC<"));
    let replied = post_multipart(
        world.app.clone(),
        &miriam,
        &format!("/needs/{need_id}/replies"),
        &[
            ("csrf", &csrf),
            ("body", "I can measure the steps Thursday afternoon."),
            ("pass", "publish"),
        ],
        &[("photos", "measure.jpg", "image/jpeg", jpeg.as_slice())],
    )
    .await;
    assert_eq!(replied.status(), StatusCode::SEE_OTHER);
    assert!(location_of(&replied).contains("ok=replied"));
    let miriam = try_cookie_from(&replied).unwrap_or(miriam);

    let fragment = get(&world, &miriam, &format!("/needs/{need_id}/conversation")).await;
    assert!(fragment.contains("I can measure the steps Thursday afternoon."));
    assert!(fragment.contains("data-reply="));
    assert!(fragment.contains("data-conversation"));

    let events = authed_get(
        world.app.clone(),
        &miriam,
        &format!("/needs/{need_id}/events"),
    )
    .await;
    assert_eq!(events.0, StatusCode::OK);
    assert_eq!(
        events
            .1
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );

    let (need, miriam, csrf) = get_page(
        world.app.clone(),
        Some(&miriam),
        &format!("/needs/{need_id}"),
    )
    .await;
    assert!(need.contains("I can measure the steps Thursday afternoon."));
    let csrf = csrf.expect("complete csrf");
    let completed = post_multipart(
        world.app.clone(),
        &miriam,
        &format!("/needs/{need_id}/complete"),
        &[
            ("csrf", &csrf),
            ("body", "The new handrail is installed and ready to use."),
            ("pass", "publish"),
        ],
        &[],
    )
    .await;
    assert_eq!(completed.status(), StatusCode::SEE_OTHER);
    let done = location_of(&completed);
    assert!(done.contains("ok=need_met"), "{done}");
    let miriam = try_cookie_from(&completed).unwrap_or(miriam);
    let met = get(&world, &miriam, &done).await;
    assert!(met.contains("Need met."));
    assert!(met.contains("The new handrail is installed and ready to use."));
    assert!(met.contains("Met"));

    let elena = register(&world, "Elena Vasquez", "elena-media@mercy.test").await;
    let (_elena, _) = plant(&world, &elena, "New Mercy").await;
    let hidden = authed_get(
        world.app.clone(),
        &elena,
        &format!("/media/{media_id}/full"),
    )
    .await;
    assert_eq!(hidden.0, StatusCode::FORBIDDEN);
    let hidden_body = String::from_utf8_lossy(&hidden.2);
    assert!(hidden_body.contains("That photo couldn't be attached. Add it again."));
    let outside = get(&world, &elena, &format!("/needs/{need_id}/conversation")).await;
    assert!(!outside.contains("I can measure the steps Thursday afternoon."));
    let stream = authed_get(
        world.app.clone(),
        &elena,
        &format!("/needs/{need_id}/events"),
    )
    .await;
    assert_ne!(
        stream
            .1
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );

    let you = get(&world, &miriam, "/me").await;
    assert!(you.contains("Profile photo"));
    assert!(you.contains("avatar-xl"));
    assert!(you.contains(">MC<"));

    let (fresh, fresh_cookie, fresh_csrf) =
        get_page(world.app.clone(), Some(&miriam), "/needs/new").await;
    let fresh_csrf = fresh_csrf.expect("sixth csrf");
    assert!(fresh.contains("Photos"));
    let mut files = Vec::new();
    for index in 0..6 {
        files.push((
            "photos",
            format!("board-{index}.jpg"),
            "image/jpeg",
            jpeg.as_slice(),
        ));
    }
    let file_refs: Vec<(&str, &str, &str, &[u8])> = files
        .iter()
        .map(|(name, filename, kind, bytes)| (*name, filename.as_str(), *kind, *bytes))
        .collect();
    let refused = post_multipart(
        world.app.clone(),
        &fresh_cookie,
        "/needs",
        &[
            ("csrf", &fresh_csrf),
            ("church_id", &grace),
            ("title", "Six loose boards on the porch"),
            (
                "body",
                "Each one needs a photo before anyone can see the job.",
            ),
            ("scope", "church"),
            ("pass", "publish"),
        ],
        &file_refs,
    )
    .await;
    assert_eq!(refused.status(), StatusCode::SEE_OTHER);
    let refusal = location_of(&refused);
    assert!(refusal.contains("err=attachments"), "{refusal}");
    let shown_cookie = try_cookie_from(&refused).unwrap_or(fresh_cookie);
    let shown = get(&world, &shown_cookie, &refusal).await;
    assert!(shown.contains("You can add up to five photos."));
    let home = get(&world, &miriam, "/home").await;
    assert!(!home.contains("Six loose boards on the porch"));
}

fn tiny_jpeg() -> Vec<u8> {
    let image = image::RgbImage::from_pixel(8, 8, image::Rgb([40, 90, 70]));
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 80)
        .write_image(image.as_raw(), 8, 8, image::ExtendedColorType::Rgb8)
        .expect("jpeg");
    encoded
}

fn media_id_from(html: &str) -> String {
    html.split("/media/")
        .nth(1)
        .and_then(|rest| rest.split(['/', '"', '?']).next())
        .expect("media id")
        .to_string()
}

async fn post_multipart(
    app: axum::Router,
    cookie: &str,
    uri: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &str, &str, &[u8])],
) -> axum::http::Response<Body> {
    let boundary = "ecclesia-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    for (name, filename, content_type, bytes) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    app.oneshot(
        Request::post(uri)
            .header(header::COOKIE, cookie)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn authed_get(
    app: axum::Router,
    cookie: &str,
    uri: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        app.oneshot(
            Request::get(uri)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .expect("response")
    .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = match tokio::time::timeout(
        std::time::Duration::from_secs(1),
        axum::body::to_bytes(response.into_body(), 64 * 1024),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes.to_vec(),
        _ => Vec::new(),
    };
    (status, headers, bytes)
}
