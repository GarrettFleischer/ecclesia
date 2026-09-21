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
    assert!(!before.contains("From others"));
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
    assert!(after.contains("From others"));
    assert!(after.contains("James Whitaker"));
    assert!(after.contains("flood cleanup"));
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
    assert!(inbox.contains("Publish"));

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
    assert!(profile.contains("From others"));
    assert!(profile.contains("Staying until the last parent left"));
    assert!(profile.contains("washed the trays"));
}

fn endorsement_id_near(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    html[start..]
        .split("/endorsements/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(ToOwned::to_owned)
}
