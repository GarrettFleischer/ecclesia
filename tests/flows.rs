use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ecclesia::db::Db;
use ecclesia::http::{router, AppState};
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
    router(AppState { db })
}

async fn login(app: axum::Router, user_id: &str) -> (axum::Router, String) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/session")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("user_id={user_id}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    (app, cookie)
}

async fn get(app: axum::Router, cookie: &str, uri: &str) -> String {
    let response = app
        .oneshot(
            Request::get(uri)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn post(app: axum::Router, cookie: &str, uri: &str, body: &str) -> StatusCode {
    let response = app
        .oneshot(
            Request::post(uri)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

#[tokio::test]
async fn pastor_can_approve_a_join_request() {
    let app = app().await;
    let (app, miriam) = login(app, "user_miriam").await;
    let church = get(app.clone(), &miriam, "/churches/church_grace").await;
    assert!(church.contains("Peter Lang"));
    assert!(church.contains("Asked to join"));

    let status = post(
        app.clone(),
        &miriam,
        "/memberships/mem_grace_peter/approve",
        "",
    );
    assert_eq!(status.await, StatusCode::SEE_OTHER);

    let (app, peter) = login(app, "user_peter").await;
    let home = get(app, &peter, "/home").await;
    assert!(home.contains("Needs the body can carry"));
    assert!(home.contains("Prayer covering"));
}

#[tokio::test]
async fn neighboring_need_is_visible_across_the_valley() {
    let app = app().await;
    let (app, elena) = login(app, "user_elena").await;
    let home = get(app.clone(), &elena, "/home").await;
    assert!(home.contains("Spanish interpreter"));
    assert!(!home.contains("Meal train for the Okonkwo"));

    let (app, peter) = login(app, "user_peter").await;
    let home = get(app, &peter, "/home").await;
    assert!(!home.contains("Spanish interpreter"));
    assert!(home.contains("At the door"));
}

#[tokio::test]
async fn apply_form_keeps_the_submit_button_outside_the_textarea() {
    let app = app().await;
    let (app, elena) = login(app, "user_elena").await;
    let page = get(app.clone(), &elena, "/needs/need_spanish").await;
    let after_open = page
        .split_once("<textarea")
        .expect("apply textarea")
        .1;
    let (inside, rest) = after_open
        .split_once("</textarea>")
        .expect("textarea must be closed");
    assert!(
        !inside.contains("Apply to help"),
        "submit control was swallowed by an unclosed textarea: {inside}"
    );
    assert!(rest.contains(r#"<button class="btn" type="submit">Apply to help</button>"#));

    let status = post(
        app.clone(),
        &elena,
        "/needs/need_spanish/apply",
        "message=I+can+hold+both+languages+on+Thursday",
    );
    assert_eq!(status.await, StatusCode::SEE_OTHER);

    let again = get(app, &elena, "/needs/need_spanish").await;
    assert!(again.contains("You already offered"));
}

#[tokio::test]
async fn endorsement_is_not_public_until_accepted() {
    let app = app().await;
    let (app, ruth) = login(app, "user_ruth").await;
    let inbox = get(app.clone(), &ruth, "/inbox").await;
    assert!(inbox.contains("James Whitaker"));
    assert!(inbox.contains("Hospitality"));

    let before = get(app.clone(), &ruth, "/members/user_ruth").await;
    assert!(!before.contains("Endorsed by"));

    let status = post(
        app.clone(),
        &ruth,
        "/endorsements/end_james_ruth/accept",
        "",
    );
    assert_eq!(status.await, StatusCode::SEE_OTHER);

    let after = get(app, &ruth, "/members/user_ruth").await;
    assert!(after.contains("James Whitaker"));
}
