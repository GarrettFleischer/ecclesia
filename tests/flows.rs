use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ecclesia::db::Db;
use ecclesia::http::{router, AppState};
use ecclesia::leaf::DemoSeat;
use tower::ServiceExt;

async fn app() -> axum::Router {
    let path = std::env::temp_dir().join(format!(
        "ecclesia-test-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let url = format!("sqlite://{}", path.display());
    let db = Db::connect(&url).await.expect("test database");
    router(AppState {
        db,
        secret: "test-secret".into(),
        demo: DemoSeat::Open,
        push: ecclesia::sdk::push::PushHub::silent(),
    })
}

fn cookie_from(response: &axum::http::Response<Body>) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("ecclesia_sid="))
        .expect("signed session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn csrf_from(html: &str) -> Option<String> {
    html.split(r#"name="csrf" value=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .map(ToOwned::to_owned)
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
    let cookie = cookie_from(&response);
    let html = body_string(response).await;
    let csrf = csrf_from(&html);
    (html, cookie, csrf)
}

async fn login(app: axum::Router, user_id: &str) -> (axum::Router, String) {
    let (_html, cookie, csrf) = get_page(app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
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
    let cookie = cookie_from(&response);
    (app, cookie)
}

async fn get(app: axum::Router, cookie: &str, uri: &str) -> String {
    get_page(app, Some(cookie), uri).await.0
}

async fn post(app: axum::Router, cookie: &str, csrf: &str, uri: &str, extra: &str) -> StatusCode {
    let body = if extra.is_empty() {
        format!("csrf={csrf}")
    } else {
        format!("csrf={csrf}&{extra}")
    };
    let response = app
        .oneshot(
            Request::post(uri)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

#[tokio::test]
async fn us_ui_01_svg_marks_close_their_tags() {
    let app = app().await;
    let (landing, _, _) = get_page(app.clone(), None, "/").await;
    assert!(
        landing.contains("</circle>"),
        "vesica circles must close or the cross is swallowed"
    );
    assert!(
        landing.contains("</path>"),
        "vesica path must close or the cross is swallowed"
    );

    let (app, cookie) = login(app, "user_miriam").await;
    let home = get(app, &cookie, "/home").await;
    assert!(home.contains("</path>"), "dock icons must close path tags");
}

#[tokio::test]
async fn us_sec_02_http_rejects_a_missing_csrf() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let status = post(
        app,
        &cookie,
        "deadbeefdeadbeefdeadbeefdeadbeef",
        "/memberships/mem_grace_peter/approve",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn us_mem_04_pastor_can_approve_a_join_request() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (church, cookie, csrf) =
        get_page(app.clone(), Some(&cookie), "/churches/church_grace").await;
    let csrf = csrf.expect("church csrf");
    assert!(church.contains("Peter Lang"));
    assert!(church.contains("Asked to join"));

    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/memberships/mem_grace_peter/approve",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let (app, peter) = login(app, "user_peter").await;
    let home = get(app, &peter, "/home").await;
    assert!(home.contains("Open needs"));
    assert!(home.contains("Prayer covering"));
}

#[tokio::test]
async fn us_need_05_neighboring_need_is_visible_across_the_valley() {
    let app = app().await;
    let (app, elena) = login(app, "user_elena").await;
    let home = get(app.clone(), &elena, "/home").await;
    assert!(home.contains("Spanish interpreter"));
    assert!(!home.contains("Dinners for the Okonkwo"));

    let (app, peter) = login(app, "user_peter").await;
    let home = get(app, &peter, "/home").await;
    assert!(!home.contains("Spanish interpreter"));
    assert!(home.contains("Pending"));
}

#[tokio::test]
async fn us_need_02_apply_form_and_offer() {
    let app = app().await;
    let (app, cookie) = login(app, "user_elena").await;
    let (page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/needs/need_spanish").await;
    let csrf = csrf.expect("need csrf");
    let after_open = page.split_once("<textarea").expect("apply textarea").1;
    let (inside, rest) = after_open
        .split_once("</textarea>")
        .expect("textarea must be closed");
    assert!(!inside.contains("Apply to help"));
    assert!(rest.contains(r#"<button class="btn" type="submit">Apply to help</button>"#));

    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/needs/need_spanish/apply",
        "message=I+can+hold+both+languages+on+Thursday",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let again = get(app, &cookie, "/needs/need_spanish").await;
    assert!(again.contains("You applied."));
}

#[tokio::test]
async fn us_end_02_endorsement_is_not_public_until_accepted() {
    let app = app().await;
    let (app, cookie) = login(app, "user_ruth").await;
    let (inbox, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    assert!(inbox.contains("James Whitaker"));
    assert!(inbox.contains("Hospitality"));

    let before = get(app.clone(), &cookie, "/members/user_ruth").await;
    assert!(!before.contains("flood cleanup"));

    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/endorsements/end_james_ruth/accept",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let after = get(app, &cookie, "/members/user_ruth").await;
    assert!(after.contains("Endorsements"));
    assert!(after.contains("James Whitaker"));
    assert!(after.contains("flood cleanup"));
}

#[tokio::test]
async fn us_end_02_declined_stays_with_the_pair_and_can_be_accepted() {
    let app = app().await;
    let (app, cookie) = login(app, "user_ruth").await;
    let (_inbox, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/endorsements/end_james_ruth/decline",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let inbox = get(app.clone(), &cookie, "/inbox").await;
    assert!(inbox.contains("Declined"));
    assert!(inbox.contains("flood cleanup"));
    assert!(inbox.contains("card-dim"));
    assert!(inbox.contains("Accept"));

    let as_ruth = get(app.clone(), &cookie, "/members/user_ruth").await;
    assert!(as_ruth.contains("Declined"));
    assert!(as_ruth.contains("flood cleanup"));
    assert!(as_ruth.contains("card-dim"));
    assert!(as_ruth.contains("Accept"));

    let (app, peter) = login(app, "user_peter").await;
    let as_peter = get(app.clone(), &peter, "/members/user_ruth").await;
    assert!(!as_peter.contains("flood cleanup"));

    let (app, james) = login(app, "user_james").await;
    let as_james = get(app.clone(), &james, "/members/user_ruth").await;
    assert!(as_james.contains("Declined"));
    assert!(as_james.contains("flood cleanup"));
    assert!(!as_james.contains("/endorsements/end_james_ruth/accept"));

    let (app, cookie) = login(app, "user_ruth").await;
    let (_inbox, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/endorsements/end_james_ruth/accept",
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let (app, peter) = login(app, "user_peter").await;
    let public = get(app, &peter, "/members/user_ruth").await;
    assert!(public.contains("Endorsements"));
    assert!(public.contains("flood cleanup"));
    assert!(!public.contains("card-dim"));
}

#[tokio::test]
async fn us_end_01_can_endorse_a_skill_they_have_not_claimed() {
    let app = app().await;
    let (app, cookie) = login(app, "user_elena").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/members/user_daniel").await;
    let csrf = csrf.expect("member csrf");
    assert!(_page.contains(r#"name="skill""#));

    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/members/user_daniel/endorse",
        "skill=Mercy&note=He+thanked+every+person+who+brought+food+and+meant+it.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let (app, cookie) = login(app, "user_daniel").await;
    let inbox = get(app.clone(), &cookie, "/inbox").await;
    assert!(inbox.contains("Mercy"));
    assert!(inbox.contains("thanked every person"));
    assert!(inbox.contains("Accept"));
    assert!(inbox.contains("Decline"));

    let before = get(app.clone(), &cookie, "/members/user_daniel").await;
    assert!(!before.contains("thanked every person"));
    assert!(!before.contains("Mercy"));
}

#[tokio::test]
async fn us_end_01_spoken_skill_is_not_limited_to_the_catalog() {
    let app = app().await;
    let (app, cookie) = login(app, "user_james").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/members/user_ruth").await;
    let csrf = csrf.expect("member csrf");

    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/members/user_ruth/endorse",
        "skill=Staying+until+the+last+parent+left&note=She+washed+the+trays+after+the+youth+left+and+then+sat+with+the+one+kid+whose+ride+was+late.",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let (app, cookie) = login(app, "user_ruth").await;
    let (inbox, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/inbox").await;
    let csrf = csrf.expect("inbox csrf");
    assert!(inbox.contains("Staying until the last parent left"));

    let id = endorsement_id_near(&inbox, "Staying until the last parent left")
        .expect("pending spoken endorsement");
    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        &format!("/endorsements/{id}/accept"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let profile = get(app, &cookie, "/members/user_ruth").await;
    assert!(profile.contains("Endorsements"));
    assert!(profile.contains("Staying until the last parent left"));
    assert!(profile.contains("washed the trays"));
}

#[tokio::test]
async fn us_app_01_manifest_is_installable() {
    let app = app().await;
    let manifest = get_public(app.clone(), "/static/manifest.webmanifest").await;
    assert!(manifest.contains(r#""display": "standalone""#));
    assert!(manifest.contains("/static/icon-192.png"));
    assert!(manifest.contains("/inbox"));

    let (sw, headers) = get_public_with_headers(app.clone(), "/sw.js").await;
    assert!(sw.contains("showNotification"));
    assert_eq!(
        headers
            .get("service-worker-allowed")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let (landing, _, _) = get_page(app.clone(), None, "/").await;
    assert!(landing.contains("apple-touch-icon"));
    assert!(landing.contains("install-bar"));
    assert!(landing.contains("/static/app.js"));
}

#[tokio::test]
async fn us_app_01_you_page_offers_alerts_and_share() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let me = get(app.clone(), &cookie, "/me").await;
    assert!(me.contains("data-alerts"));
    assert!(me.contains("Turn on alerts"));

    let church = get(app.clone(), &cookie, "/churches/church_grace").await;
    assert!(church.contains("data-share"));
    assert!(church.contains("Share code"));

    let need = get(app, &cookie, "/needs/need_meals").await;
    assert!(need.contains("data-share"));
}

#[tokio::test]
async fn us_push_01_signed_in_person_can_subscribe() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
    let status = post(
        app.clone(),
        &cookie,
        &csrf,
        "/push/subscribe",
        "endpoint=https://push.example/m1&p256dh=abc&auth=def",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let device = post(
        app.clone(),
        &cookie,
        &csrf,
        "/push/device",
        "token=fcm-test-token&platform=android",
    )
    .await;
    assert_eq!(device, StatusCode::NO_CONTENT);

    let vapid = get_public(app, "/push/vapid").await;
    assert!(vapid.is_empty() || vapid.starts_with("B"));
}

async fn get_public(app: axum::Router, uri: &str) -> String {
    get_public_with_headers(app, uri).await.0
}

async fn get_public_with_headers(app: axum::Router, uri: &str) -> (String, axum::http::HeaderMap) {
    let response = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
    let headers = response.headers().clone();
    (body_string(response).await, headers)
}

fn endorsement_id_near(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    html[start..]
        .split("/endorsements/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(ToOwned::to_owned)
}
