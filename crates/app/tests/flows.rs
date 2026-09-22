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
        db,
        judge,
        refine,
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
    assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
    let next_cookie = try_cookie_from(&response)
        .or_else(|| cookie.map(ToOwned::to_owned))
        .expect("session cookie");
    let html = body_string(response).await;
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

async fn register(world: &World, name: &str, email: &str) -> String {
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let body = format!(
        "csrf={csrf}&name={}&email={}&city=Cedar+Falls&region=Iowa&bio=I+cook&password={}&pass=publish",
        enc(name),
        enc(email),
        enc(PASS)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/register", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "register {email}");
    cookie_from(&response)
}

async fn sign_in(world: &World, email: &str) -> String {
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let body = format!(
        "csrf={csrf}&email={}&password={}",
        enc(email),
        enc(PASS)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/session", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "sign in {email}");
    cookie_from(&response)
}

async fn plant(world: &World, cookie: &str, name: &str) -> (String, String) {
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(cookie), "/churches/new").await;
    let csrf = csrf.expect("church csrf");
    let body = format!(
        "csrf={csrf}&name={}&city=Cedar+Falls&region=Iowa&gathering=Sunday+at+10.&description=A+church+on+Main+Street.&pass=publish",
        enc(name)
    );
    let response = post_form(world.app.clone(), Some(&cookie), "/churches", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "plant {name}");
    let location = location_of(&response);
    let church_id = id_from_location(&location, "/churches/");
    (try_cookie_from(&response).unwrap_or(cookie), church_id)
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
    let (_page, cookie, csrf) =
        get_page(world.app.clone(), Some(cookie), &format!("/churches/{church_id}")).await;
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
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "post need {title}");
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

fn membership_id(html: &str) -> String {
    html.split("/memberships/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .expect("membership id")
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
    let home = get(&world, &cookie, "/home").await;
    assert!(home.contains("Open needs"));
    assert!(home.contains("Miriam"));

    let again = sign_in(&world, "miriam@grace.test").await;
    let home = get(&world, &again, "/home").await;
    assert!(home.contains("Open needs"));
}

#[tokio::test]
async fn us_auth_04_unknown_email_is_quiet() {
    let world = app().await;
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let miss = post_location(
        &world,
        &cookie,
        &csrf,
        "/session",
        "email=nobody%40x.test&password=nope",
    )
    .await;
    assert!(miss.contains("err=miss"), "got {miss}");
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
async fn us_auth_01_weak_password_stays_on_landing() {
    let world = app().await;
    let (_html, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let location = post_location(
        &world,
        &cookie,
        &csrf,
        "/register",
        "name=Cara+Nguyen&email=cara@verify.test&city=Cedar+Falls&region=Iowa&bio=I+cook&password=password&pass=publish",
    )
    .await;
    assert!(location.contains("err=password"), "got {location}");
    assert!(world
        .sdk
        .db
        .user_by_email("cara@verify.test")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn us_auth_02_register_cookie_is_v2() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let raw = cookie
        .strip_prefix("ecclesia_sid=")
        .expect("cookie name");
    let parts: Vec<_> = raw.split('.').collect();
    assert_eq!(parts.first().copied(), Some("v2"));
    assert_eq!(parts.len(), 4);
    assert!(!parts[1].is_empty());
}

#[tokio::test]
async fn us_auth_03_logout_all_returns_to_landing() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
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
    let (_me, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
    let _ = post(&world, &cookie, &csrf, "/session/logout", "").await;
    let (_landing, guest, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let body = format!(
        "csrf={csrf}&email=miriam@grace.test&password={}",
        enc(PASS)
    );
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
        sessions.iter().map(|row| row.ip.as_str()).collect::<Vec<_>>()
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
    assert_eq!(location_of(&response), "/home");
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
    let (church, miriam, csrf) =
        get_page(world.app.clone(), Some(&miriam), &format!("/churches/{church_id}")).await;
    let csrf = csrf.expect("church csrf");
    assert!(church.contains("Peter Lang"));
    assert!(church.contains("Asked to join"));
    let mem = membership_id(&church);
    let status = post(
        &world,
        &miriam,
        &csrf,
        &format!("/memberships/{mem}/approve"),
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
async fn us_need_02_apply_form_and_offer() {
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
    let (page, cookie, csrf) =
        get_page(world.app.clone(), Some(&elena), &format!("/needs/{need_id}")).await;
    let csrf = csrf.expect("need csrf");
    let after_open = page.split_once("<textarea").expect("apply textarea").1;
    let (inside, rest) = after_open
        .split_once("</textarea>")
        .expect("textarea must be closed");
    assert!(!inside.contains("Apply to help"));
    assert!(rest.contains(r#"<button class="btn" type="submit">Apply to help</button>"#));

    let status = post(
        &world,
        &cookie,
        &csrf,
        &format!("/needs/{need_id}/apply"),
        "message=I+can+hold+both+languages+on+Thursday",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let again = get(&world, &cookie, &format!("/needs/{need_id}")).await;
    assert!(again.contains("You applied."));
}

#[tokio::test]
async fn us_end_02_endorsement_is_not_public_until_accepted() {
    let world = app().await;
    let _ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let (_page, james, csrf) =
        get_page(world.app.clone(), Some(&james), &format!("/members/{ruth_id}")).await;
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
    let _ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let _james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let _peter = register(&world, "Peter Lang", "peter@grace.test").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let james = sign_in(&world, "james@stlukes.test").await;
    let (_page, james, csrf) =
        get_page(world.app.clone(), Some(&james), &format!("/members/{ruth_id}")).await;
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
    let _daniel = register(&world, "Daniel Cole", "daniel@grace.test").await;
    let elena = register(&world, "Elena Vasquez", "elena@mercy.test").await;
    let daniel_id = user_id(&world, "daniel@grace.test").await;
    let (_page, cookie, csrf) =
        get_page(world.app.clone(), Some(&elena), &format!("/members/{daniel_id}")).await;
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
    let _ruth = register(&world, "Ruth Alvarez", "ruth@grace.test").await;
    let james = register(&world, "James Whitaker", "james@stlukes.test").await;
    let ruth_id = user_id(&world, "ruth@grace.test").await;
    let (_page, cookie, csrf) =
        get_page(world.app.clone(), Some(&james), &format!("/members/{ruth_id}")).await;
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
    assert!(landing.contains("/static/app.js"));
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
    assert!(church.contains("data-share"));
    assert!(church.contains("Share code"));

    let need = get(&world, &cookie, &format!("/needs/{need_id}")).await;
    assert!(need.contains("data-share"));
}

#[tokio::test]
async fn us_push_01_signed_in_person_can_subscribe() {
    let world = app().await;
    let cookie = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
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
    let (landing, cookie, csrf) = get_page(world.app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    assert!(landing.contains("data-rewrite"));
    assert!(landing.contains(r#"data-kind="bio""#));

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
    assert!(need.contains(r#"data-kind="offer""#));
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
        "/home?ok=Visit+https://evil.example+now",
    )
    .await;
    assert!(bait.contains("Done."));
    assert!(!bait.contains("evil.example"));
    let html = get(&world, &cookie, "/home?err=%3Cscript%3Ealert(1)%3C/script%3E").await;
    assert!(html.contains("That didn't work."));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("alert(1)"));
}

#[tokio::test]
async fn us_app_03_website_landing_and_guest_home() {
    let world = app().await;
    let (landing, _, _) = get_page(world.app.clone(), None, "/").await;
    assert!(landing.contains("The Body of Christ"));
    assert!(landing.contains("We are the ecclesia"));
    assert!(landing.contains("His kingdom"));
    assert!(landing.contains("Needs and Gifts"));
    assert!(landing.contains("simply because it was unseen"));
    assert!(landing.contains("Create an account"));
    assert!(landing.contains("Sign in"));
    assert!(landing.contains("Email me a link"));
    assert!(landing.contains("Forgot password"));
    assert!(!landing.contains("Ask for help"));
    assert!(!landing.contains("People in Cedar Falls"));
    assert!(!landing.contains("Open the demo"));
    assert!(landing.contains("href=\"#account\""));

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
    assert_eq!(location, "/home");
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
    let (_page, cookie, csrf) = get_page(world.app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
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
    let (_page, cookie, csrf) =
        get_page(world.app.clone(), Some(&cookie), &format!("/churches/{church_id}")).await;
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
    let _miriam = register(&world, "Miriam Cole", "miriam@grace.test").await;
    let peter = register(&world, "Peter Lang", "peter@grace.test").await;
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
