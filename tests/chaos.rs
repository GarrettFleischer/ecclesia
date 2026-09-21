//! A person who does not read the labels. Empty submits, junk ids, XSS,
//! their own need, a church name of bangs. The app should stay up and speak
//! plainly.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use ecclesia::db::Db;
use ecclesia::http::{AppState, router};
use ecclesia::leaf::DemoSeat;
use tower::ServiceExt;

async fn app() -> axum::Router {
    let path = std::env::temp_dir().join(format!(
        "ecclesia-chaos-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Db::connect(&format!("sqlite://{}", path.display()))
        .await
        .expect("test database");
    router(AppState {
        db,
        secret: "test-secret".into(),
        cookie: ecclesia::sdk::session::CookieTransport::Plain,
        demo: DemoSeat::Open,
        push: ecclesia::sdk::push::PushHub::silent(),
        judge: ecclesia::sdk::judge::JudgeHub::word_gate(),
        refine: ecclesia::sdk::refine::RefineHub::silent(),
        gate: ecclesia::sdk::limit::RateGate::new(),
    })
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

async fn login(app: axum::Router, user_id: &str) -> (axum::Router, String) {
    let (_html, cookie, csrf) = get_ok(app.clone(), None, "/").await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/session")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("csrf={csrf}&user_id={user_id}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = try_cookie(&response).expect("login cookie");
    (app, cookie)
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
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

#[tokio::test]
async fn us_chaos_01_empty_post_gets_a_human_page() {
    let app = app().await;
    let response = post_response(app, None, "/register", String::new()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let html = body_string(response).await;
    assert!(html.contains("Fill in the required fields."));
    assert!(html.contains("Ecclesia"));
}

#[tokio::test]
async fn us_chaos_01_double_at_email_is_refused() {
    let app = app().await;
    let (_html, cookie, csrf) = get_ok(app.clone(), None, "/").await;
    let location = post_location(
        app,
        &cookie,
        &csrf,
        "/register",
        "name=Ada&email=ada@@nope.com&city=Waterloo&region=Iowa&bio=I+cook",
    )
    .await;
    assert!(location.contains("err=bad_email"), "got {location}");
}

#[tokio::test]
async fn us_chaos_01_script_name_does_not_run() {
    let app = app().await;
    let (_html, cookie, csrf) = get_ok(app.clone(), None, "/").await;
    let response = post_response(
        app.clone(),
        Some(&cookie),
        "/register",
        format!(
            "csrf={csrf}&name=%3Cscript%3Ealert(1)%3C/script%3E&email=escaped@nope.test&city=Waterloo&region=Iowa&bio=I+cook"
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = try_cookie(&response).expect("new seat");
    let (home, _, _) = get_ok(app, Some(&cookie), "/home").await;
    assert!(home.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!home.contains("<script>alert(1)</script>"));
}

#[tokio::test]
async fn us_chaos_02_bang_church_name_is_refused() {
    let app = app().await;
    let (app, cookie) = login(app, "user_keisha").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/churches/new").await;
    let location = post_location(
        app,
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
    let app = app().await;
    let (app, cookie) = login(app, "user_peter").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/needs/new").await;
    let location = post_location(
        app,
        &cookie,
        &csrf,
        "/needs",
        "church_id=church_grace&title=Need+five+dinners&body=Need+five+dinners+this+week&scope=church",
    )
    .await;
    assert!(location.contains("err=not_member"), "got {location}");
}

#[tokio::test]
async fn us_chaos_03_cannot_apply_to_your_own_need() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/needs/need_meals").await;
    let location = post_location(
        app,
        &cookie,
        &csrf,
        "/needs/need_meals/apply",
        "message=I+will+cook+my+own+dinners",
    )
    .await;
    assert!(location.contains("err=own_need"), "got {location}");
}

#[tokio::test]
async fn us_chaos_03_neighbor_cannot_close_a_need() {
    let app = app().await;
    let (app, cookie) = login(app, "user_elena").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/needs/need_spanish").await;
    let location = post_location(app, &cookie, &csrf, "/needs/need_spanish/close", "").await;
    assert!(location.contains("err=steward"), "got {location}");
}

#[tokio::test]
async fn us_chaos_04_cannot_endorse_yourself() {
    let app = app().await;
    let (app, cookie) = login(app, "user_ruth").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/members/user_ruth").await;
    let location = post_location(
        app,
        &cookie,
        &csrf,
        "/members/user_ruth/endorse",
        "skill=Hospitality&note=I+stayed+until+the+last+parent+came.",
    )
    .await;
    assert!(location.contains("err=self"), "got {location}");
}

#[tokio::test]
async fn us_chaos_05_junk_invite_and_missing_pages() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/churches").await;
    let location = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/invites/redeem",
        "code=zzzz-zzzz",
    )
    .await;
    assert!(location.contains("err=invite"), "got {location}");

    let (status, html, _, _) = get_any(app.clone(), Some(&cookie), "/asdf").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("That page doesn't exist."));

    let (status, html, _, _) = get_any(app, Some(&cookie), "/needs/nope").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("We couldn't find that need."));
}

#[tokio::test]
async fn us_chaos_06_double_approve_and_empty_profile() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/churches/church_grace").await;
    let first = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/memberships/mem_grace_peter/approve",
        "",
    )
    .await;
    assert!(first.contains("ok=approved"), "got {first}");
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/churches/church_grace").await;
    let second = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/memberships/mem_grace_peter/approve",
        "",
    )
    .await;
    assert!(second.contains("err=pending"), "got {second}");

    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/me").await;
    let empty = post_location(app, &cookie, &csrf, "/me", "name=&city=&region=&bio=").await;
    assert!(empty.contains("err=missing"), "got {empty}");
}

#[tokio::test]
async fn us_chaos_07_refine_junk_and_push_junk() {
    let app = app().await;
    let (_html, cookie, csrf) = get_ok(app.clone(), None, "/").await;
    let response = post_response(
        app.clone(),
        Some(&cookie),
        "/refine",
        format!("csrf={csrf}&kind=nope&text=hi"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = post_response(
        app.clone(),
        Some(&cookie),
        "/refine",
        format!("csrf={csrf}&kind=bio&text={}", "x".repeat(2001)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_ok(app.clone(), Some(&cookie), "/me").await;
    let response = post_response(
        app,
        Some(&cookie),
        "/push/subscribe",
        format!("csrf={csrf}&endpoint=javascript:alert(1)&p256dh=abc&auth=def"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
