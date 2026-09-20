use axum::extract::{Form, Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::db::Db;
use crate::leaf::{
    accept_invite, add_gift, apply_to_need, close_need, decide_application, decide_endorsement,
    decide_membership, endorse, invite_member, may_impersonate, parse_invite_email, plant_church,
    post_need, redeem_invite, register, remove_gift, request_join, update_profile,
    visible_need_cards, Church, DomainError, Need, User, Viewer,
};
use crate::sdk::clock::{new_id, nonce4, now_iso};
use crate::sdk::session::{self, Session};
use crate::views;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub secret: String,
    pub demo: bool,
}

struct AppError(anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("{0:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(views::error_page("The server stumbled. Try again.").into_string()),
        )
            .into_response()
    }
}

#[derive(Deserialize, Default)]
struct FlashQuery {
    ok: Option<String>,
    err: Option<String>,
}

#[derive(Deserialize)]
struct SessionForm {
    csrf: String,
    user_id: String,
}

#[derive(Deserialize)]
struct CsrfForm {
    csrf: String,
}

#[derive(Deserialize)]
struct RegisterForm {
    csrf: String,
    name: String,
    email: String,
    city: String,
    region: String,
    #[serde(default)]
    bio: String,
}

#[derive(Deserialize)]
struct ChurchForm {
    csrf: String,
    name: String,
    city: String,
    region: String,
    #[serde(default)]
    gathering: String,
    description: String,
}

#[derive(Deserialize)]
struct InviteForm {
    csrf: String,
    email: String,
}

#[derive(Deserialize)]
struct RedeemForm {
    csrf: String,
    code: String,
}

#[derive(Deserialize)]
struct NeedForm {
    csrf: String,
    church_id: String,
    title: String,
    body: String,
    #[serde(default)]
    gift_id: String,
    scope: String,
}

#[derive(Deserialize)]
struct NeedQuery {
    church_id: Option<String>,
}

#[derive(Deserialize)]
struct ApplyForm {
    csrf: String,
    message: String,
}

#[derive(Deserialize)]
struct EndorseForm {
    csrf: String,
    gift_id: String,
    note: String,
}

#[derive(Deserialize)]
struct ProfileForm {
    csrf: String,
    name: String,
    city: String,
    region: String,
    #[serde(default)]
    bio: String,
}

#[derive(Deserialize)]
struct GiftForm {
    csrf: String,
    gift_id: String,
    #[serde(default)]
    note: String,
}

pub async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ecclesia=info,tower_http=info".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ecclesia.db".into());
    let secret = std::env::var("ECCLESIA_SECRET").unwrap_or_else(|_| {
        tracing::warn!("ECCLESIA_SECRET is unset; signing cookies with the local development key");
        "dev-only-change-me".into()
    });
    let demo = std::env::var("ECCLESIA_DEMO")
        .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    let db = Db::connect(&database_url).await?;
    let state = AppState { db, secret, demo };

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let app = router(state).nest_service("/static", ServeDir::new(static_dir));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(43781);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("ecclesia listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(landing))
        .route("/session", post(start_session))
        .route("/session/logout", post(logout))
        .route("/register", post(register_user))
        .route("/home", get(home))
        .route("/churches", get(churches_index).post(create_church))
        .route("/churches/new", get(church_new))
        .route("/churches/{id}", get(church_show))
        .route("/churches/{id}/join", post(join_church))
        .route("/churches/{id}/invite", post(invite))
        .route("/invites/redeem", post(redeem))
        .route("/memberships/{id}/approve", post(approve_membership))
        .route("/memberships/{id}/decline", post(decline_membership))
        .route("/memberships/{id}/accept-invite", post(accept_invite_http))
        .route("/needs/new", get(need_new))
        .route("/needs", post(create_need))
        .route("/needs/{id}", get(need_show))
        .route("/needs/{id}/apply", post(apply_need))
        .route("/needs/{id}/close", post(close_need_http))
        .route("/applications/{id}/accept", post(accept_application))
        .route("/applications/{id}/decline", post(decline_application))
        .route("/members/{id}", get(member_show))
        .route("/members/{id}/endorse", post(endorse_member))
        .route("/endorsements/{id}/accept", post(accept_endorsement))
        .route("/endorsements/{id}/decline", post(decline_endorsement))
        .route("/inbox", get(inbox))
        .route("/me", get(me).post(update_me))
        .route("/me/gifts", post(add_gift_http))
        .route("/me/gifts/{id}/remove", post(remove_gift_http))
        .route("/the-body", get(the_body))
        .fallback(fallback)
        .layer(middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn security_headers(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; style-src 'self' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; frame-ancestors 'none'",
        ),
    );
    response
}

fn html(markup: maud::Markup) -> Html<String> {
    Html(markup.into_string())
}

fn bind_session(jar: CookieJar, secret: &str) -> (Session, CookieJar) {
    let session = session::from_jar(secret, &jar);
    let jar = session::put(jar, secret, &session);
    (session, jar)
}

fn fail_csrf(path: &str) -> Redirect {
    Redirect::to(&format!("{path}?err=csrf"))
}

fn redirect_ok(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?ok={code}"))
}

fn redirect_err(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?err={code}"))
}

fn leaf_err(path: &str, error: DomainError) -> Redirect {
    redirect_err(path, error.flash_code())
}

async fn load_user(db: &Db, session: &Session) -> Result<Option<User>, AppError> {
    let Some(id) = session.user_id.as_deref() else {
        return Ok(None);
    };
    Ok(db.user(id).await?)
}

async fn require_user(db: &Db, session: &Session) -> Result<User, Response> {
    match load_user(db, session).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(Redirect::to("/?err=auth").into_response()),
        Err(error) => Err(error.into_response()),
    }
}

async fn viewer_for(db: &Db, user: User) -> Result<Viewer, AppError> {
    let memberships = db.memberships_for_user(&user.id).await?;
    let churches = db.churches_for_user(&user.id).await?;
    let gift_ids = db.gift_ids_for(&user.id).await?;
    Ok(Viewer {
        user,
        memberships,
        churches,
        gift_ids,
    })
}

async fn unread(db: &Db, user_id: &str) -> Result<i64, AppError> {
    Ok(db.unread_count(user_id).await?)
}

fn with_cookie(jar: CookieJar, body: impl IntoResponse) -> Response {
    (jar, body).into_response()
}

async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if load_user(&state.db, &session).await?.is_some() && flash.err.is_none() && flash.ok.is_none()
    {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    let users = if state.demo {
        state.db.demo_users().await?
    } else {
        vec![]
    };
    Ok(with_cookie(
        jar,
        html(views::landing(
            &users,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
            state.demo,
        )),
    ))
}

async fn start_session(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    if let Err(error) = may_impersonate(state.demo) {
        return Ok(with_cookie(jar, leaf_err("/", error)));
    }
    let Some(user) = state.db.user(form.user_id.trim()).await? else {
        return Ok(with_cookie(jar, redirect_err("/", "not_found")));
    };
    let next = Session::signed_in(user.id, session::fresh_csrf());
    Ok(with_cookie(
        session::put(jar, &state.secret, &next),
        Redirect::to("/home"),
    ))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    Ok(with_cookie(session::clear(jar), Redirect::to("/")))
}

async fn register_user(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    let taken = state.db.user_by_email(&form.email).await?.is_some();
    let effect = match register(
        &form.name,
        &form.email,
        &form.city,
        &form.region,
        &form.bio,
        taken,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/", error))),
    };
    let user_id = match effect.writes.first() {
        Some(crate::leaf::Write::InsertUser(user)) => user.id.clone(),
        _ => return Ok(with_cookie(jar, redirect_err("/", "missing"))),
    };
    state.db.apply(&effect).await?;
    let next = Session::signed_in(user_id, session::fresh_csrf());
    Ok(with_cookie(
        session::put(jar, &state.secret, &next),
        redirect_ok("/home", "welcome"),
    ))
}

async fn home(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let mut pending = Vec::new();
    for membership in viewer
        .memberships
        .iter()
        .filter(|m| m.status == "pending_request" || m.status == "pending_invite")
    {
        if let Some(church) = state.db.church(&membership.church_id).await? {
            pending.push((membership.clone(), church));
        }
    }
    let cards = state.db.all_need_cards().await?;
    let churches = state.db.churches().await?;
    let visible: Vec<_> = visible_need_cards(&viewer, &cards, &churches)
        .into_iter()
        .cloned()
        .collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::home(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &pending,
            &visible,
            count,
            &session.csrf,
        )),
    ))
}

async fn churches_index(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let churches = state.db.churches().await?;
    let counts = state.db.counts_for_churches().await?;
    let cards: Vec<(Church, i64, i64)> = churches
        .into_iter()
        .map(|church| {
            let (members, needs) = counts
                .iter()
                .find(|(id, _, _)| *id == church.id)
                .map(|(_, m, n)| (*m, *n))
                .unwrap_or((0, 0));
            (church, members, needs)
        })
        .collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::churches_index(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &cards,
            count,
            &session.csrf,
        )),
    ))
}

async fn church_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::church_new(
            &viewer,
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn create_church(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChurchForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/churches/new")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let effect = match plant_church(
        &user,
        &form.name,
        &form.city,
        &form.region,
        &form.description,
        &form.gathering,
        new_id(),
        new_id(),
        &nonce4(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/churches/new", error))),
    };
    let church_id = match effect.writes.first() {
        Some(crate::leaf::Write::InsertChurch(church)) => church.id.clone(),
        _ => return Ok(with_cookie(jar, redirect_err("/churches/new", "missing"))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(&format!("/churches/{church_id}"), "church_planted"),
    ))
}

async fn church_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("That church is not here.")),
        ));
    };
    let viewer = viewer_for(&state.db, user).await?;
    let members = state.db.church_members(&church.id).await?;
    let needs = state.db.church_need_cards(&church.id).await?;
    let visible: Vec<_> = visible_need_cards(&viewer, &needs, std::slice::from_ref(&church))
        .into_iter()
        .cloned()
        .collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::church_show(
            &viewer,
            &church,
            &members,
            &visible,
            views::flash_from(flash.ok, flash.err),
            count,
            &session.csrf,
        )),
    ))
}

async fn join_church(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let dest = format!("/churches/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let existing = state.db.membership_pair(&id, &user.id).await?;
    let governors = state.db.governors(&id).await?;
    let governor_ids: Vec<String> = governors.into_iter().map(|g| g.id).collect();
    let effect = match request_join(
        &user,
        &church,
        existing.as_ref(),
        &governor_ids,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "joined_request")))
}

async fn invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<InviteForm>,
) -> Result<Response, AppError> {
    let dest = format!("/churches/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let email = match parse_invite_email(&form.email) {
        Ok(email) => email,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    let Some(invitee) = state.db.user_by_email(&email).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let existing = state.db.membership_pair(&id, &invitee.id).await?;
    let effect = match invite_member(
        &viewer,
        &church,
        &invitee,
        existing.as_ref(),
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "invited")))
}

async fn redeem(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RedeemForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/churches")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(church) = state.db.church_by_invite(&form.code).await? else {
        return Ok(with_cookie(jar, redirect_err("/churches", "invite")));
    };
    let existing = state.db.membership_pair(&church.id, &user.id).await?;
    let dest = format!("/churches/{}", church.id);
    let effect = match redeem_invite(&user, &church, existing.as_ref(), new_id(), now_iso()) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    if !effect.writes.is_empty() {
        state.db.apply(&effect).await?;
    }
    Ok(with_cookie(jar, redirect_ok(&dest, "redeemed")))
}

async fn approve_membership(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_membership_http(state, jar, id, true, form.csrf).await
}

async fn decline_membership(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_membership_http(state, jar, id, false, form.csrf).await
}

async fn decide_membership_http(
    state: AppState,
    jar: CookieJar,
    id: String,
    approve: bool,
    csrf: String,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(target) = state.db.membership(&id).await? else {
        return Ok(with_cookie(jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/churches/{}", target.church_id);
    if !session.check_csrf(&csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let viewer = viewer_for(&state.db, user).await?;
    let Some(church) = state.db.church(&target.church_id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let effect = match decide_membership(&viewer, &target, &church, approve) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(&dest, if approve { "approved" } else { "declined" }),
    ))
}

async fn accept_invite_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/home")));
    }
    let Some(target) = state.db.membership(&id).await? else {
        return Ok(with_cookie(jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/churches/{}", target.church_id);
    let effect = match accept_invite(&user, &target) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "invite_accepted")))
}

async fn need_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<NeedQuery>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let churches: Vec<Church> = viewer.active_churches().cloned().collect();
    let gifts = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::need_new(
            &viewer,
            &churches,
            &gifts,
            query.church_id.as_deref(),
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn create_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<NeedForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/needs/new")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let gift = if form.gift_id.trim().is_empty() {
        None
    } else {
        Some(form.gift_id.as_str())
    };
    let gift_exists = match gift {
        Some(id) => state.db.gift(id).await?.is_some(),
        None => true,
    };
    let effect = match post_need(
        &viewer,
        &form.church_id,
        &form.title,
        &form.body,
        gift,
        gift_exists,
        &form.scope,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/needs/new", error))),
    };
    let need_id = match effect.writes.first() {
        Some(crate::leaf::Write::InsertNeed(need)) => need.id.clone(),
        _ => return Ok(with_cookie(jar, redirect_err("/needs/new", "missing"))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(&format!("/needs/{need_id}"), "need_posted"),
    ))
}

async fn need_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(card) = state.db.need_card(&id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("That need is not here.")),
        ));
    };
    let Some(church) = state.db.church(&card.church_id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("That church is not here.")),
        ));
    };
    let need = Need::from_card(&card);
    if !crate::leaf::can_view_need(&viewer, &need, &church) {
        return Ok(with_cookie(
            jar,
            html(views::error_page("This need stays with another household.")),
        ));
    }
    let applications = state.db.applications_for_need(&need.id).await?;
    let already = applications.iter().any(|a| a.user_id == viewer.user.id);
    let help = crate::leaf::can_apply(&viewer, &need, &church);
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::need_show(
            &viewer,
            &card,
            &church,
            &applications,
            help,
            already,
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn apply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ApplyForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let Some(church) = state.db.church(&need.church_id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let already = state
        .db
        .application_pair(&need.id, &viewer.user.id)
        .await?
        .is_some();
    let effect = match apply_to_need(
        &viewer,
        &need,
        &church,
        already,
        &form.message,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "applied")))
}

async fn close_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let effect = match close_need(&viewer, &need) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "need_closed")))
}

async fn accept_application(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_application_http(state, jar, id, true, form.csrf).await
}

async fn decline_application(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_application_http(state, jar, id, false, form.csrf).await
}

async fn decide_application_http(
    state: AppState,
    jar: CookieJar,
    id: String,
    accept: bool,
    csrf: String,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(application) = state.db.application(&id).await? else {
        return Ok(with_cookie(jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/needs/{}", application.need_id);
    if !session.check_csrf(&csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let Some(need) = state.db.need(&application.need_id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let effect = match decide_application(&viewer, &need, &application, accept) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(
            &dest,
            if accept {
                "application_accepted"
            } else {
                "declined"
            },
        ),
    ))
}

async fn member_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(person) = state.db.user(&id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("That person is not here.")),
        ));
    };
    let memberships = state.db.memberships_for_user(&person.id).await?;
    let mut churches = Vec::new();
    for membership in memberships {
        if let Some(church) = state.db.church(&membership.church_id).await? {
            churches.push((church, membership));
        }
    }
    let gifts = state.db.member_gifts(&person.id).await?;
    let endorsements = state.db.accepted_endorsements_for(&person.id).await?;
    let catalog = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::member_show(
            &viewer,
            &person,
            &churches,
            &gifts,
            &endorsements,
            &catalog,
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn endorse_member(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<EndorseForm>,
) -> Result<Response, AppError> {
    let dest = format!("/members/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(person) = state.db.user(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let gift = state.db.gift(&form.gift_id).await?;
    let gift_exists = gift.is_some();
    let gift_name = gift
        .as_ref()
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "a gift".into());
    let pending = state
        .db
        .pending_endorsement(&user.id, &id, &form.gift_id)
        .await?
        .is_some();
    let effect = match endorse(
        &user,
        &person,
        &form.gift_id,
        gift_exists,
        pending,
        &form.note,
        &gift_name,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "endorsed")))
}

async fn accept_endorsement(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_endorsement_http(state, jar, id, true, form.csrf).await
}

async fn decline_endorsement(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    decide_endorsement_http(state, jar, id, false, form.csrf).await
}

async fn decide_endorsement_http(
    state: AppState,
    jar: CookieJar,
    id: String,
    accept: bool,
    csrf: String,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&csrf) {
        return Ok(with_cookie(jar, fail_csrf("/inbox")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let Some(endorsement) = state.db.endorsement(&id).await? else {
        return Ok(with_cookie(jar, redirect_err("/inbox", "not_found")));
    };
    let gift_name = state
        .db
        .gift(&endorsement.gift_id)
        .await?
        .map(|g| g.name)
        .unwrap_or_else(|| "a gift".into());
    let effect = match decide_endorsement(&user, &endorsement, accept, &gift_name) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/inbox", error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(
            "/inbox",
            if accept {
                "endorsement_accepted"
            } else {
                "endorsement_declined"
            },
        ),
    ))
}

async fn inbox(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let pending = state.db.pending_endorsements_for(&viewer.user.id).await?;
    let notes = state.db.notifications(&viewer.user.id).await?;
    state.db.mark_notifications_read(&viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::inbox(
            &viewer,
            &pending,
            &notes,
            0,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let gifts = state.db.member_gifts(&viewer.user.id).await?;
    let catalog = state.db.gifts().await?;
    let mut memberships = Vec::new();
    for membership in &viewer.memberships {
        if let Some(church) = state.db.church(&membership.church_id).await? {
            memberships.push((church, membership.clone()));
        }
    }
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::me(
            &viewer,
            &gifts,
            &catalog,
            &memberships,
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

async fn update_me(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ProfileForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let effect = match update_profile(&user.id, &form.name, &form.city, &form.region, &form.bio) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/me", error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "saved")))
}

async fn add_gift_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<GiftForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let exists = state.db.gift(&form.gift_id).await?.is_some();
    let effect = match add_gift(&user.id, &form.gift_id, exists, &form.note) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/me", error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "gift_added")))
}

async fn remove_gift_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let effect = match remove_gift(&user.id, &id) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/me", error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "gift_removed")))
}

async fn the_body(State(state): State<AppState>, jar: CookieJar) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let churches = state.db.churches().await?;
    let counts = state.db.counts_for_churches().await?;
    let mut groups: Vec<(String, Vec<(Church, i64, i64)>)> = Vec::new();
    for church in churches {
        let place = format!("{}, {}", church.city, church.region);
        let (members, needs) = counts
            .iter()
            .find(|(id, _, _)| *id == church.id)
            .map(|(_, m, n)| (*m, *n))
            .unwrap_or((0, 0));
        if let Some((_, list)) = groups.iter_mut().find(|(name, _)| *name == place) {
            list.push((church, members, needs));
        } else {
            groups.push((place, vec![(church, members, needs)]));
        }
    }
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::the_body(&viewer, &groups, count)),
    ))
}

async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        html(views::error_page("That page is not in this house.")),
    )
}
