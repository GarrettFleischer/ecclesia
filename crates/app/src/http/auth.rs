use axum::extract::{Form, Path, Query, State};
use axum::response::{Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::prelude::VoiceKind;
use ecclesia_sdk::session::{self, Session};
use ecclesia_sdk::story::{self, MailOrigin, StoryOk};

use crate::views;

use super::context::{
    ClientKey, bind_session, device_meta, fail_csrf, html, leaf_err, load_user, redirect_err,
    redirect_ok, unread, viewer_for, with_cookie,
};
use super::forms::{
    CsrfForm, FlashQuery, PasswordForm, RegisterForm, ResetCompleteForm, SessionForm, TokenQuery,
};
use super::{AppError, AppState};

pub async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if load_user(&state.sdk.db, &session).await?.is_some()
        && flash.err.is_none()
        && flash.ok.is_none()
    {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    Ok(with_cookie(
        jar,
        html(views::landing(
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
            &views::RegisterDraft::blank(),
        )),
    ))
}

pub async fn start_session(
    State(state): State<AppState>,
    who: ClientKey,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    if session.user_id.is_some() {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Session, &who.0).await {
        return Ok(with_cookie(jar, redirect_err("/", "rate")));
    }
    match story::sign_in(
        &state.sdk,
        &form.email,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(put_signed(&state, jar, &ok), Redirect::to("/home"))),
        Err(_) => Ok(with_cookie(jar, redirect_err("/", "miss"))),
    }
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    if let Some(id) = session.session_id.as_deref() {
        story::logout(&state.sdk, id).await?;
    }
    let guest = Session::guest(session::fresh_csrf());
    Ok(with_cookie(state.put_session(jar, &guest), Redirect::to("/")))
}

pub async fn logout_all(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    if let Some(user_id) = session.user_id.as_deref() {
        story::logout_all(&state.sdk, user_id).await?;
    }
    let guest = Session::guest(session::fresh_csrf());
    Ok(with_cookie(state.put_session(jar, &guest), Redirect::to("/")))
}

pub async fn revoke_session(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    let Some(user_id) = session.user_id.as_deref() else {
        return Ok(with_cookie(jar, redirect_err("/", "miss")));
    };
    match story::revoke_session(&state.sdk, user_id, &id).await? {
        Ok(_) => {}
        Err(_) => return Ok(with_cookie(jar, redirect_err("/me", "miss"))),
    }
    if session.session_id.as_deref() == Some(id.as_str()) {
        let guest = Session::guest(session::fresh_csrf());
        return Ok(with_cookie(state.put_session(jar, &guest), Redirect::to("/")));
    }
    Ok(with_cookie(jar, Redirect::to("/me")))
}

pub async fn register_user(
    State(state): State<AppState>,
    who: ClientKey,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Register, &who.0).await {
        return Ok(with_cookie(jar, redirect_err("/", "rate")));
    }
    if state.awaiting_review(&form.pass, &[&form.bio]) {
        let bio = state.polish(VoiceKind::Bio, &form.bio).await;
        return Ok(with_cookie(
            jar,
            html(views::landing(
                None,
                &session.csrf,
                &views::RegisterDraft {
                    name: &form.name,
                    email: &form.email,
                    city: &form.city,
                    region: &form.region,
                    bio: &bio,
                    kind: views::DraftKind::Review,
                },
            )),
        ));
    }
    match story::register(
        &state.sdk,
        &form.name,
        &form.email,
        &form.city,
        &form.region,
        &form.bio,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(
            put_signed(&state, jar, &ok),
            redirect_ok("/home", "welcome"),
        )),
        Err(error) => Ok(with_cookie(jar, leaf_err("/", error))),
    }
}

pub async fn request_link(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    request_mail(&state, who, jar, form, MailAsk::Magic).await
}

pub async fn request_reset(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    request_mail(&state, who, jar, form, MailAsk::Reset).await
}

enum MailAsk {
    Magic,
    Reset,
}

async fn request_mail(
    state: &AppState,
    who: ClientKey,
    jar: CookieJar,
    form: SessionForm,
    ask: MailAsk,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    if session.user_id.is_some() {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Session, &who.0).await {
        return Ok(with_cookie(jar, redirect_err("/", "mail")));
    }
    if let Ok(email) = ecclesia_sdk::prelude::normalize_email(&form.email)
        && let RateDecision::Refuse = state.sdk.gate.decide_mail(&email).await
    {
        return Ok(with_cookie(jar, redirect_err("/", "mail")));
    }
    let origin = MailOrigin {
        origin: state.origin.clone(),
    };
    match ask {
        MailAsk::Magic => {
            let _ = story::request_magic(&state.sdk, &form.email, &origin).await?;
        }
        MailAsk::Reset => {
            let _ = story::request_reset(&state.sdk, &form.email, &origin).await?;
        }
    }
    Ok(with_cookie(jar, redirect_err("/", "mail")))
}

pub async fn consume_link(
    State(state): State<AppState>,
    who: ClientKey,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if session.user_id.is_some() {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    match story::consume_magic(
        &state.sdk,
        &id,
        query.t.as_deref().unwrap_or(""),
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(put_signed(&state, jar, &ok), Redirect::to("/home"))),
        Err(_) => Ok(with_cookie(jar, redirect_err("/", "miss"))),
    }
}

pub async fn reset_form(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if session.user_id.is_some() {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    let secret = query.t.as_deref().unwrap_or("");
    if !story::reset_form_ok(&state.sdk, &id, secret).await? {
        return Ok(with_cookie(jar, redirect_err("/", "miss")));
    }
    Ok(with_cookie(
        jar,
        html(views::reset_password(&session.csrf, &id, secret)),
    ))
}

pub async fn complete_reset(
    State(state): State<AppState>,
    who: ClientKey,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ResetCompleteForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    match story::complete_reset(
        &state.sdk,
        &id,
        &form.t,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(put_signed(&state, jar, &ok), Redirect::to("/home"))),
        Err(error) if error.flash_code() == "password" => {
            Ok(with_cookie(jar, leaf_err(&format!("/session/reset/{id}"), error)))
        }
        Err(_) => Ok(with_cookie(jar, redirect_err("/", "miss"))),
    }
}

pub async fn change_password(
    State(state): State<AppState>,
    who: ClientKey,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Form(form): Form<PasswordForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    let Some(user) = load_user(&state.sdk.db, &session).await? else {
        return Ok(with_cookie(jar, redirect_err("/", "miss")));
    };
    match story::change_password(
        &state.sdk,
        &user,
        &form.current,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(
            put_signed(&state, jar, &ok),
            redirect_ok("/me", "saved"),
        )),
        Err(error) if error.flash_code() == "password" => {
            Ok(with_cookie(jar, leaf_err("/me", error)))
        }
        Err(_) => Ok(with_cookie(jar, redirect_err("/me", "miss"))),
    }
}

pub async fn home(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    match load_user(&state.sdk.db, &session).await? {
        Some(user) => member_home(state, jar, session, user, flash).await,
        None => Ok(with_cookie(
            jar,
            html(views::landing(
                views::flash_from(flash.ok, flash.err),
                &session.csrf,
                &views::RegisterDraft::blank(),
            )),
        )),
    }
}

async fn member_home(
    state: AppState,
    jar: CookieJar,
    session: Session,
    user: ecclesia_sdk::prelude::User,
    flash: FlashQuery,
) -> Result<Response, AppError> {
    let viewer = viewer_for(&state.sdk.db, user).await?;
    let pending: Vec<_> =
        ecclesia_sdk::prelude::pair_memberships(viewer.pending_memberships(), &viewer.churches)
            .collect();
    let page = story::home_needs(&state.sdk, &viewer, flash.after.as_deref()).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::home(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &pending,
            &page.cards,
            &page.churches,
            page.next_cursor.as_deref(),
            count,
            &session.csrf,
        )),
    ))
}

fn put_signed(state: &AppState, jar: CookieJar, ok: &StoryOk) -> CookieJar {
    let (Some(session_id), Some(user_id), Some(csrf)) =
        (ok.session_id.clone(), ok.user_id.clone(), ok.csrf.clone())
    else {
        return jar;
    };
    state.put_session(jar, &Session::signed_in(session_id, user_id, csrf))
}
