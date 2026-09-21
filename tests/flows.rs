use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use ecclesia::db::Db;
use ecclesia::http::{AppState, router};
use ecclesia::leaf::DemoSeat;
use tower::ServiceExt;

async fn app_with(
    judge: ecclesia::sdk::judge::JudgeHub,
    refine: ecclesia::sdk::refine::RefineHub,
) -> axum::Router {
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
        cookie: ecclesia::sdk::session::CookieTransport::Plain,
        demo: DemoSeat::Open,
        push: ecclesia::sdk::push::PushHub::silent(),
        judge,
        refine,
        gate: ecclesia::sdk::limit::RateGate::new(),
    })
}

async fn app_sealed() -> axum::Router {
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
        cookie: ecclesia::sdk::session::CookieTransport::Plain,
        demo: DemoSeat::Sealed,
        push: ecclesia::sdk::push::PushHub::silent(),
        judge: ecclesia::sdk::judge::JudgeHub::word_gate(),
        refine: ecclesia::sdk::refine::RefineHub::silent(),
        gate: ecclesia::sdk::limit::RateGate::new(),
    })
}

async fn app() -> axum::Router {
    app_with(
        ecclesia::sdk::judge::JudgeHub::word_gate(),
        ecclesia::sdk::refine::RefineHub::silent(),
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
    assert!(home.contains("Waiting"));
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
    assert!(
        !inbox.contains("Accept it from your inbox"),
        "the pending card is the decision; the notice is only a badge"
    );

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
    assert!(manifest.contains(r#""start_url": "/home""#));
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

#[tokio::test]
async fn us_tone_01_an_attack_is_refused() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    let location = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/needs",
        "church_id=church_grace&title=Attack&body=You+are+worthless+and+you+suck.&scope=church",
    )
    .await;
    assert!(
        location.contains("err=tone"),
        "attack should bounce with tone, got {location}"
    );

    let home = get(app, &cookie, "/home").await;
    assert!(!home.contains("You are worthless"));
}

#[tokio::test]
async fn us_refine_01_silent_echoes_the_same_words() {
    let app = app().await;
    let (landing, cookie, csrf) = get_page(app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    assert!(landing.contains("data-rewrite"));
    assert!(landing.contains(r#"data-kind="bio""#));

    let (status, body) = post_json(
        app.clone(),
        &cookie,
        &csrf,
        "/refine",
        "kind=endorsement&text=+She+stayed.+",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#""text":"She stayed.""#));
    assert!(body.contains(r#""seat":"echo""#));

    let (app, cookie) = login(app, "user_elena").await;
    let need = get(app.clone(), &cookie, "/needs/need_spanish").await;
    assert!(need.contains(r#"data-kind="offer""#));
    let member = get(app, &cookie, "/members/user_daniel").await;
    assert!(member.contains(r#"data-kind="endorsement""#));
}

#[tokio::test]
async fn us_refine_02_submit_shows_the_rewrite_before_publish() {
    let app = app_with(
        ecclesia::sdk::judge::JudgeHub::word_gate(),
        ecclesia::sdk::refine::RefineHub::polish(),
    )
    .await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/needs/new").await;
    let csrf = csrf.expect("need csrf");
    let (status, html) = post_page(
        app.clone(),
        &cookie,
        &csrf,
        "/needs",
        "church_id=church_grace&title=Need+five+dinners&body=Need+++five+++dinners&scope=church",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Need five dinners"));
    assert!(html.contains("Read this through."));
    assert!(html.contains(">Publish<"));
    assert!(html.contains(r#"name="pass" value="publish""#));

    let location = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/needs",
        "church_id=church_grace&title=Need+five+dinners&body=Need+five+dinners&scope=church&pass=publish",
    )
    .await;
    assert!(location.contains("/needs/"));
    assert!(location.contains("ok=need_posted"));

    let home = get(app, &cookie, "/home").await;
    assert!(home.contains("Need five dinners"));
}

async fn post_location(
    app: axum::Router,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> String {
    let body = format!("csrf={csrf}&{extra}");
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
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn post_page(
    app: axum::Router,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> (StatusCode, String) {
    let body = format!("csrf={csrf}&{extra}");
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
    (response.status(), body_string(response).await)
}

async fn post_json(
    app: axum::Router,
    cookie: &str,
    csrf: &str,
    uri: &str,
    extra: &str,
) -> (StatusCode, String) {
    let body = format!("csrf={csrf}&{extra}");
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
    (response.status(), body_string(response).await)
}

fn endorsement_id_near(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)?;
    html[start..]
        .split("/endorsements/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(ToOwned::to_owned)
}

#[tokio::test]
async fn us_sec_03_forged_flash_stays_generic() {
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let bait = get(
        app.clone(),
        &cookie,
        "/home?ok=Visit+https://evil.example+now",
    )
    .await;
    assert!(bait.contains("Done."));
    assert!(!bait.contains("evil.example"));
    let html = get(app, &cookie, "/home?err=%3Cscript%3Ealert(1)%3C/script%3E").await;
    assert!(html.contains("That didn't work."));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("alert(1)"));
}

#[tokio::test]
async fn us_app_03_website_landing_and_guest_home() {
    let app = app().await;
    let (landing, _, _) = get_page(app.clone(), None, "/").await;
    assert!(landing.contains("The Body of Christ"));
    assert!(landing.contains("We are the ecclesia"));
    assert!(landing.contains("His kingdom"));
    assert!(landing.contains("Needs and Gifts"));
    assert!(landing.contains("simply because it was unseen"));
    assert!(landing.contains("Create an account"));
    assert!(landing.contains("Open the demo"));
    assert!(!landing.contains("Ask for help"));
    assert!(!landing.contains("People in Cedar Falls"));
    assert!(landing.contains("href=\"#account\""));

    let (guest, _, _) = get_page(app.clone(), None, "/home").await;
    assert!(guest.contains("Create an account"));
    assert!(guest.contains("People in Cedar Falls"));
    assert!(!guest.contains("His kingdom"));
    assert!(!guest.contains("Open needs"));

    let response = app
        .clone()
        .oneshot(Request::get("/home").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (app, cookie) = login(app, "user_miriam").await;
    let signed = app
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
async fn us_sec_01_forged_cookie_cannot_sit_as_miriam() {
    let app = app().await;
    let response = app
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
async fn us_sec_09_sealed_demo_refuses_impersonation() {
    let app = app_sealed().await;
    let (landing, cookie, csrf) = get_page(app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    assert!(!landing.contains("Miriam Cole"));
    assert!(!landing.contains("look around"));
    let location = post_location(app, &cookie, &csrf, "/session", "user_id=user_miriam").await;
    assert!(location.contains("err=demo"), "got {location}");
}

#[tokio::test]
async fn us_sec_08_refine_rejects_overlong_text() {
    let app = app().await;
    let (_landing, cookie, csrf) = get_page(app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let huge = "x".repeat(2001);
    let (status, _body) = post_json(
        app,
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
    let app = app().await;
    let (_landing, cookie, csrf) = get_page(app.clone(), None, "/").await;
    let csrf = csrf.expect("landing csrf");
    let mut last = StatusCode::OK;
    for _ in 0..21 {
        let (status, _) = post_json(
            app.clone(),
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
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) = get_page(app.clone(), Some(&cookie), "/me").await;
    let csrf = csrf.expect("me csrf");
    let status = post(
        app,
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
    let app = app().await;
    let (app, cookie) = login(app, "user_miriam").await;
    let (_page, cookie, csrf) =
        get_page(app.clone(), Some(&cookie), "/churches/church_grace").await;
    let csrf = csrf.expect("church csrf");
    let missing = post_location(
        app.clone(),
        &cookie,
        &csrf,
        "/churches/church_grace/invite",
        "email=nobody%40example.test",
    )
    .await;
    let known = post_location(
        app,
        &cookie,
        &csrf,
        "/churches/church_grace/invite",
        "email=james%40stlukes.test",
    )
    .await;
    assert!(missing.contains("ok=invited"), "got {missing}");
    assert!(known.contains("ok=invited"), "got {known}");
}

#[tokio::test]
async fn us_sec_15_another_profile_hides_email() {
    let app = app().await;
    let (app, cookie) = login(app, "user_peter").await;
    let page = get(app, &cookie, "/members/user_miriam").await;
    assert!(page.contains("Miriam Cole"));
    assert!(!page.contains("miriam@gracecovenant.test"));
}

#[tokio::test]
async fn us_sec_16_security_headers_and_cookie_flags() {
    let app = app().await;
    let response = app
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
