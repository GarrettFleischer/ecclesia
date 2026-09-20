use axum::extract::{Form, FromRef, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::db::Db;
use crate::domain::{
    can_apply, can_decide_membership, can_endorse, can_view_need, invite_code_for,
    next_membership_after_decision, Church, DomainError, MembershipStatus, NeedScope, User, Viewer,
};
use crate::views;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
}

impl FromRef<AppState> for Db {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
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
    user_id: String,
}

#[derive(Deserialize)]
struct RegisterForm {
    name: String,
    email: String,
    city: String,
    region: String,
    #[serde(default)]
    bio: String,
}

#[derive(Deserialize)]
struct ChurchForm {
    name: String,
    city: String,
    region: String,
    #[serde(default)]
    gathering: String,
    description: String,
}

#[derive(Deserialize)]
struct InviteForm {
    email: String,
}

#[derive(Deserialize)]
struct RedeemForm {
    code: String,
}

#[derive(Deserialize)]
struct NeedForm {
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
    message: String,
}

#[derive(Deserialize)]
struct EndorseForm {
    gift_id: String,
    note: String,
}

#[derive(Deserialize)]
struct ProfileForm {
    name: String,
    city: String,
    region: String,
    #[serde(default)]
    bio: String,
}

#[derive(Deserialize)]
struct GiftForm {
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
    let db = Db::connect(&database_url).await?;
    let state = AppState { db };

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
        .route("/register", post(register))
        .route("/home", get(home))
        .route("/churches", get(churches_index).post(create_church))
        .route("/churches/new", get(church_new))
        .route("/churches/{id}", get(church_show))
        .route("/churches/{id}/join", post(request_join))
        .route("/churches/{id}/invite", post(invite_member))
        .route("/invites/redeem", post(redeem_invite))
        .route("/memberships/{id}/approve", post(approve_membership))
        .route("/memberships/{id}/decline", post(decline_membership))
        .route("/memberships/{id}/accept-invite", post(accept_invite))
        .route("/needs/new", get(need_new))
        .route("/needs", post(create_need))
        .route("/needs/{id}", get(need_show))
        .route("/needs/{id}/apply", post(apply_need))
        .route("/needs/{id}/close", post(close_need))
        .route("/applications/{id}/accept", post(accept_application))
        .route("/applications/{id}/decline", post(decline_application))
        .route("/members/{id}", get(member_show))
        .route("/members/{id}/endorse", post(endorse_member))
        .route("/endorsements/{id}/accept", post(accept_endorsement))
        .route("/endorsements/{id}/decline", post(decline_endorsement))
        .route("/inbox", get(inbox))
        .route("/me", get(me).post(update_me))
        .route("/me/gifts", post(add_gift))
        .route("/me/gifts/{id}/remove", post(remove_gift))
        .route("/the-body", get(the_body))
        .fallback(fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn html(markup: maud::Markup) -> Html<String> {
    Html(markup.into_string())
}

fn signed_in(jar: &CookieJar) -> Option<String> {
    jar.get("ecclesia_uid")
        .map(|cookie| cookie.value().to_string())
}

fn put_session(jar: CookieJar, user_id: &str) -> CookieJar {
    let cookie = Cookie::build(("ecclesia_uid", user_id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::days(365))
        .build();
    jar.add(cookie)
}

fn clear_session(jar: CookieJar) -> CookieJar {
    let cookie = Cookie::build(("ecclesia_uid", ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(0))
        .build();
    jar.add(cookie)
}

async fn load_user(db: &Db, jar: &CookieJar) -> Result<Option<User>, AppError> {
    let Some(id) = signed_in(jar) else {
        return Ok(None);
    };
    Ok(db.user(&id).await?)
}

async fn require_user(db: &Db, jar: &CookieJar) -> Result<User, Response> {
    match load_user(db, jar).await {
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

fn redirect_ok(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?ok={code}"))
}

fn redirect_err(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?err={code}"))
}

async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    if load_user(&state.db, &jar).await?.is_some() && flash.err.is_none() && flash.ok.is_none() {
        return Ok(Redirect::to("/home").into_response());
    }
    let users = state.db.demo_users().await?;
    Ok(html(views::landing(
        &users,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn start_session(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    let Some(user) = state.db.user(form.user_id.trim()).await? else {
        return Ok(redirect_err("/", "not_found").into_response());
    };
    Ok((put_session(jar, &user.id), Redirect::to("/home")).into_response())
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    (clear_session(jar), Redirect::to("/"))
}

async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    if form.name.trim().is_empty()
        || form.email.trim().is_empty()
        || form.city.trim().is_empty()
        || form.region.trim().is_empty()
    {
        return Ok(redirect_err("/", "missing").into_response());
    }
    if state.db.user_by_email(&form.email).await?.is_some() {
        return Ok(redirect_err("/", "email").into_response());
    }
    let user = state
        .db
        .create_user(&form.name, &form.email, &form.city, &form.region, &form.bio)
        .await?;
    Ok((put_session(jar, &user.id), redirect_ok("/home", "welcome")).into_response())
}

async fn home(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
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
    let all = state.db.all_need_cards().await?;
    let churches = state.db.churches().await?;
    let visible: Vec<_> = all
        .into_iter()
        .filter(|card| {
            let Some(church) = churches.iter().find(|c| c.id == card.church_id) else {
                return false;
            };
            let need = crate::domain::Need {
                id: card.id.clone(),
                church_id: card.church_id.clone(),
                author_id: card.author_id.clone(),
                title: card.title.clone(),
                body: card.body.clone(),
                gift_id: card.gift_id.clone(),
                scope: card.scope.clone(),
                status: card.status.clone(),
                created_at: card.created_at.clone(),
            };
            card.is_open() && can_view_need(&viewer, &need, church)
        })
        .collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(html(views::home(
        &viewer,
        views::flash_from(flash.ok, flash.err),
        &pending,
        &visible,
        count,
    ))
    .into_response())
}

async fn churches_index(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
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
    Ok(html(views::churches_index(
        &viewer,
        views::flash_from(flash.ok, flash.err),
        &cards,
        count,
    ))
    .into_response())
}

async fn church_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(html(views::church_new(
        &viewer,
        count,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn create_church(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChurchForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    if form.name.trim().is_empty() || form.description.trim().is_empty() {
        return Ok(redirect_err("/churches/new", "missing").into_response());
    }
    let church = state
        .db
        .create_church(
            &user,
            &form.name,
            &form.city,
            &form.region,
            &form.description,
            &form.gathering,
            &invite_code_for(&form.name),
        )
        .await?;
    Ok(redirect_ok(&format!("/churches/{}", church.id), "church_planted").into_response())
}

async fn church_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let Some(church) = state.db.church(&id).await? else {
        return Ok(html(views::error_page("That church is not here.")).into_response());
    };
    let viewer = viewer_for(&state.db, user).await?;
    let members = state.db.church_members(&church.id).await?;
    let needs = state.db.church_need_cards(&church.id).await?;
    let visible: Vec<_> = needs
        .into_iter()
        .filter(|card| {
            let need = crate::domain::Need {
                id: card.id.clone(),
                church_id: card.church_id.clone(),
                author_id: card.author_id.clone(),
                title: card.title.clone(),
                body: card.body.clone(),
                gift_id: card.gift_id.clone(),
                scope: card.scope.clone(),
                status: card.status.clone(),
                created_at: card.created_at.clone(),
            };
            can_view_need(&viewer, &need, &church)
        })
        .collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(html(views::church_show(
        &viewer,
        &church,
        &members,
        &visible,
        views::flash_from(flash.ok, flash.err),
        count,
    ))
    .into_response())
}

async fn request_join(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let dest = format!("/churches/{id}");
    let Some(church) = state.db.church(&id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if let Some(existing) = state.db.membership_pair(&id, &user.id).await? {
        return Ok(redirect_err(
            &dest,
            if existing.is_active() {
                "already"
            } else {
                "already"
            },
        )
        .into_response());
    }
    state
        .db
        .insert_membership(&id, &user.id, "member", "pending_request")
        .await?;
    for governor in state.db.governors(&id).await? {
        state
            .db
            .notify(
                &governor.id,
                "join_request",
                &format!("{} asked to join {}", user.name, church.name),
                "Approve them from the church page if they belong in this household.",
                &dest,
            )
            .await?;
    }
    Ok(redirect_ok(&dest, "joined_request").into_response())
}

async fn invite_member(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<InviteForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let dest = format!("/churches/{id}");
    let viewer = viewer_for(&state.db, user).await?;
    let Some(membership) = viewer.membership_in(&id) else {
        return Ok(redirect_err(&dest, "forbidden").into_response());
    };
    if let Err(error) = can_decide_membership(membership) {
        return Ok(redirect_err(&dest, domain_code(&error)).into_response());
    }
    let Some(church) = state.db.church(&id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    let Some(invitee) = state.db.user_by_email(&form.email).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if state.db.membership_pair(&id, &invitee.id).await?.is_some() {
        return Ok(redirect_err(&dest, "already").into_response());
    }
    let created = state
        .db
        .insert_membership(&id, &invitee.id, "member", "pending_invite")
        .await?;
    state
        .db
        .notify(
            &invitee.id,
            "invite",
            &format!("{} invited you to {}", viewer.user.name, church.name),
            "Accept from home or the church page. They already want you in.",
            &format!("/churches/{id}"),
        )
        .await?;
    let _ = created;
    Ok(redirect_ok(&dest, "invited").into_response())
}

async fn redeem_invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RedeemForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let Some(church) = state.db.church_by_invite(&form.code).await? else {
        return Ok(redirect_err("/churches", "invite").into_response());
    };
    if let Some(existing) = state.db.membership_pair(&church.id, &user.id).await? {
        if existing.status == "pending_invite" {
            return Ok(redirect_ok(&format!("/churches/{}", church.id), "redeemed").into_response());
        }
        return Ok(redirect_err(&format!("/churches/{}", church.id), "already").into_response());
    }
    state
        .db
        .insert_membership(&church.id, &user.id, "member", "pending_invite")
        .await?;
    Ok(redirect_ok(&format!("/churches/{}", church.id), "redeemed").into_response())
}

async fn approve_membership(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_membership(state, jar, id, true).await
}

async fn decline_membership(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_membership(state, jar, id, false).await
}

async fn decide_membership(
    state: AppState,
    jar: CookieJar,
    id: String,
    approve: bool,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let Some(target) = state.db.membership(&id).await? else {
        return Ok(redirect_err("/home", "not_found").into_response());
    };
    let dest = format!("/churches/{}", target.church_id);
    let viewer = viewer_for(&state.db, user).await?;
    let Some(actor) = viewer.membership_in(&target.church_id) else {
        return Ok(redirect_err(&dest, "forbidden").into_response());
    };
    if let Err(error) = can_decide_membership(actor) {
        return Ok(redirect_err(&dest, domain_code(&error)).into_response());
    }
    let Some(current) = target.status() else {
        return Ok(redirect_err(&dest, "pending").into_response());
    };
    let next = match next_membership_after_decision(current, approve) {
        Ok(status) => status,
        Err(error) => return Ok(redirect_err(&dest, domain_code(&error)).into_response()),
    };
    state
        .db
        .set_membership_status(&target.id, next.as_str())
        .await?;
    if let Some(church) = state.db.church(&target.church_id).await? {
        let title = if approve {
            format!("You are in at {}", church.name)
        } else {
            format!("{} could not receive you just now", church.name)
        };
        state
            .db
            .notify(
                &target.user_id,
                "membership",
                &title,
                if approve {
                    "Your gifts can now meet the needs of this household."
                } else {
                    "You can ask again later, or look for another household."
                },
                &dest,
            )
            .await?;
    }
    Ok(redirect_ok(&dest, if approve { "approved" } else { "declined" }).into_response())
}

async fn accept_invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let Some(target) = state.db.membership(&id).await? else {
        return Ok(redirect_err("/home", "not_found").into_response());
    };
    if target.user_id != user.id || target.status() != Some(MembershipStatus::PendingInvite) {
        return Ok(redirect_err("/home", "forbidden").into_response());
    }
    state
        .db
        .set_membership_status(&target.id, MembershipStatus::Active.as_str())
        .await?;
    Ok(redirect_ok(
        &format!("/churches/{}", target.church_id),
        "invite_accepted",
    )
    .into_response())
}

async fn need_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<NeedQuery>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let churches: Vec<Church> = viewer.active_churches().cloned().collect();
    let gifts = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(html(views::need_new(
        &viewer,
        &churches,
        &gifts,
        query.church_id.as_deref(),
        count,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn create_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<NeedForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    if !viewer.is_active_in(&form.church_id) {
        return Ok(redirect_err("/needs/new", "not_member").into_response());
    }
    if form.title.trim().is_empty() || form.body.trim().is_empty() {
        return Ok(redirect_err("/needs/new", "missing").into_response());
    }
    if NeedScope::parse(&form.scope).is_none() {
        return Ok(redirect_err("/needs/new", "missing").into_response());
    }
    let gift = if form.gift_id.trim().is_empty() {
        None
    } else {
        Some(form.gift_id.as_str())
    };
    let need = state
        .db
        .create_need(
            &form.church_id,
            &viewer.user.id,
            &form.title,
            &form.body,
            gift,
            &form.scope,
        )
        .await?;
    Ok(redirect_ok(&format!("/needs/{}", need.id), "need_posted").into_response())
}

async fn need_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(card) = state.db.need_card(&id).await? else {
        return Ok(html(views::error_page("That need is not here.")).into_response());
    };
    let Some(church) = state.db.church(&card.church_id).await? else {
        return Ok(html(views::error_page("That church is not here.")).into_response());
    };
    let need = crate::domain::Need {
        id: card.id.clone(),
        church_id: card.church_id.clone(),
        author_id: card.author_id.clone(),
        title: card.title.clone(),
        body: card.body.clone(),
        gift_id: card.gift_id.clone(),
        scope: card.scope.clone(),
        status: card.status.clone(),
        created_at: card.created_at.clone(),
    };
    if !can_view_need(&viewer, &need, &church) {
        return Ok(
            html(views::error_page("This need stays with another household.")).into_response(),
        );
    }
    let applications = state.db.applications_for_need(&need.id).await?;
    let already = applications.iter().any(|a| a.user_id == viewer.user.id);
    let help = can_apply(&viewer, &need, &church);
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(html(views::need_show(
        &viewer,
        &card,
        &church,
        &applications,
        help,
        already,
        count,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn apply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ApplyForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let dest = format!("/needs/{id}");
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    let Some(church) = state.db.church(&need.church_id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if let Err(error) = can_apply(&viewer, &need, &church) {
        return Ok(redirect_err(&dest, domain_code(&error)).into_response());
    }
    if state
        .db
        .application_pair(&need.id, &viewer.user.id)
        .await?
        .is_some()
    {
        return Ok(redirect_err(&dest, "already").into_response());
    }
    if form.message.trim().is_empty() {
        return Ok(redirect_err(&dest, "missing").into_response());
    }
    state
        .db
        .create_application(&need.id, &viewer.user.id, &form.message)
        .await?;
    state
        .db
        .notify(
            &need.author_id,
            "application",
            &format!("{} offered to help: {}", viewer.user.name, need.title),
            "Receive them from the need if this is the right pair of hands.",
            &dest,
        )
        .await?;
    Ok(redirect_ok(&dest, "applied").into_response())
}

async fn close_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let dest = format!("/needs/{id}");
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if viewer.user.id != need.author_id && !viewer.can_govern(&need.church_id) {
        return Ok(redirect_err(&dest, "forbidden").into_response());
    }
    state.db.set_need_status(&need.id, "closed").await?;
    Ok(redirect_ok(&dest, "need_closed").into_response())
}

async fn accept_application(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_application(state, jar, id, true).await
}

async fn decline_application(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_application(state, jar, id, false).await
}

async fn decide_application(
    state: AppState,
    jar: CookieJar,
    id: String,
    accept: bool,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(application) = state.db.application(&id).await? else {
        return Ok(redirect_err("/home", "not_found").into_response());
    };
    let dest = format!("/needs/{}", application.need_id);
    let Some(need) = state.db.need(&application.need_id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if viewer.user.id != need.author_id && !viewer.can_govern(&need.church_id) {
        return Ok(redirect_err(&dest, "forbidden").into_response());
    }
    if application.status != "pending" {
        return Ok(redirect_err(&dest, "pending").into_response());
    }
    state
        .db
        .set_application_status(
            &application.id,
            if accept { "accepted" } else { "declined" },
        )
        .await?;
    state
        .db
        .notify(
            &application.user_id,
            "application",
            &format!(
                "{} {}",
                need.title,
                if accept {
                    "received your offer"
                } else {
                    "could not receive this offer"
                }
            ),
            if accept {
                "Go be the hands."
            } else {
                "Thank you for offering. Another need will come."
            },
            &dest,
        )
        .await?;
    Ok(redirect_ok(
        &dest,
        if accept {
            "application_accepted"
        } else {
            "declined"
        },
    )
    .into_response())
}

async fn member_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(person) = state.db.user(&id).await? else {
        return Ok(html(views::error_page("That person is not here.")).into_response());
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
    Ok(html(views::member_show(
        &viewer,
        &person,
        &churches,
        &gifts,
        &endorsements,
        &catalog,
        count,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn endorse_member(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<EndorseForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let dest = format!("/members/{id}");
    if let Err(error) = can_endorse(&user.id, &id) {
        return Ok(redirect_err(&dest, domain_code(&error)).into_response());
    }
    let Some(person) = state.db.user(&id).await? else {
        return Ok(redirect_err(&dest, "not_found").into_response());
    };
    if state.db.gift(&form.gift_id).await?.is_none() {
        return Ok(redirect_err(&dest, "missing").into_response());
    }
    if state
        .db
        .pending_endorsement(&user.id, &id, &form.gift_id)
        .await?
        .is_some()
    {
        return Ok(redirect_err(&dest, "already").into_response());
    }
    if form.note.trim().is_empty() {
        return Ok(redirect_err(&dest, "missing").into_response());
    }
    let endorsement = state
        .db
        .create_endorsement(&user.id, &id, &form.gift_id, &form.note)
        .await?;
    let gift_name = state
        .db
        .gift(&form.gift_id)
        .await?
        .map(|g| g.name)
        .unwrap_or_else(|| "a gift".into());
    state
        .db
        .notify(
            &person.id,
            "endorsement",
            &format!("{} endorsed you for {gift_name}", user.name),
            "Accept it onto your profile from Inbox, or decline.",
            "/inbox",
        )
        .await?;
    let _ = endorsement;
    Ok(redirect_ok(&dest, "endorsed").into_response())
}

async fn accept_endorsement(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_endorsement(state, jar, id, true).await
}

async fn decline_endorsement(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    decide_endorsement(state, jar, id, false).await
}

async fn decide_endorsement(
    state: AppState,
    jar: CookieJar,
    id: String,
    accept: bool,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let Some(endorsement) = state.db.endorsement(&id).await? else {
        return Ok(redirect_err("/inbox", "not_found").into_response());
    };
    if endorsement.to_user_id != user.id || endorsement.status != "pending" {
        return Ok(redirect_err("/inbox", "forbidden").into_response());
    }
    state
        .db
        .set_endorsement_status(
            &endorsement.id,
            if accept { "accepted" } else { "declined" },
        )
        .await?;
    if accept {
        state
            .db
            .add_member_gift(&user.id, &endorsement.gift_id, "")
            .await?;
    }
    let gift_name = state
        .db
        .gift(&endorsement.gift_id)
        .await?
        .map(|g| g.name)
        .unwrap_or_else(|| "a gift".into());
    state
        .db
        .notify(
            &endorsement.from_user_id,
            "endorsement",
            &format!(
                "{} {} your endorsement for {gift_name}",
                user.name,
                if accept { "received" } else { "declined" }
            ),
            if accept {
                "It is on their profile now."
            } else {
                "They chose not to wear it. That is theirs to decide."
            },
            &format!("/members/{}", user.id),
        )
        .await?;
    Ok(redirect_ok(
        "/inbox",
        if accept {
            "endorsement_accepted"
        } else {
            "endorsement_declined"
        },
    )
    .into_response())
}

async fn inbox(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let pending = state.db.pending_endorsements_for(&viewer.user.id).await?;
    let notes = state.db.notifications(&viewer.user.id).await?;
    state.db.mark_notifications_read(&viewer.user.id).await?;
    Ok(html(views::inbox(
        &viewer,
        &pending,
        &notes,
        0,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
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
    Ok(html(views::me(
        &viewer,
        &gifts,
        &catalog,
        &memberships,
        count,
        views::flash_from(flash.ok, flash.err),
    ))
    .into_response())
}

async fn update_me(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ProfileForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    if form.name.trim().is_empty() {
        return Ok(redirect_err("/me", "missing").into_response());
    }
    state
        .db
        .update_user(&user.id, &form.name, &form.city, &form.region, &form.bio)
        .await?;
    Ok(redirect_ok("/me", "saved").into_response())
}

async fn add_gift(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<GiftForm>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    if state.db.gift(&form.gift_id).await?.is_none() {
        return Ok(redirect_err("/me", "missing").into_response());
    }
    state
        .db
        .add_member_gift(&user.id, &form.gift_id, &form.note)
        .await?;
    Ok(redirect_ok("/me", "gift_added").into_response())
}

async fn remove_gift(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
    };
    state.db.remove_member_gift(&user.id, &id).await?;
    Ok(redirect_ok("/me", "gift_removed").into_response())
}

async fn the_body(State(state): State<AppState>, jar: CookieJar) -> Result<Response, AppError> {
    let user = match require_user(&state.db, &jar).await {
        Ok(user) => user,
        Err(response) => return Ok(response),
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
    Ok(html(views::the_body(&viewer, &groups, count)).into_response())
}

async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        html(views::error_page("That page is not in this house.")),
    )
}

fn domain_code(error: &DomainError) -> &'static str {
    match error {
        DomainError::SelfAction => "self",
        DomainError::NotInTheBody => "not_member",
        DomainError::OutsideChurch | DomainError::OutsideNeighborhood => "scope",
        DomainError::NeedClosed => "closed",
        DomainError::AlreadyApplied
        | DomainError::AlreadyMember
        | DomainError::DuplicateEndorsement => "already",
        DomainError::OwnNeed => "own_need",
        DomainError::NotGovernor => "forbidden",
        DomainError::NothingPending => "pending",
    }
}
