use axum::extract::{Form, Path, Query, State};
use axum::response::{Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::prelude::{Church, DomainError, User};
use ecclesia_sdk::session::{self, Session};
use ecclesia_sdk::story::{self, MailOrigin, StoryOk};

use crate::views;

use super::context::{
    ClientKey, bind_session, device_meta, fail_csrf, html, leaf_err, load_user, redirect_err,
    redirect_ok, signed_home, unread, viewer_for, with_cookie,
};
use super::forms::{
    CsrfForm, FlashQuery, PasswordForm, RegisterForm, RegisterQuery, ResetCompleteForm,
    SessionForm, TokenQuery,
};
use super::{AppError, AppState};

async fn redirect_signed(state: &AppState, user_id: Option<&str>) -> Result<Redirect, AppError> {
    let dest = match user_id {
        Some(id) => match state.sdk.db.user(id).await? {
            Some(user) => signed_home(&user),
            None => "/churches/join",
        },
        None => "/churches/join",
    };
    Ok(Redirect::to(dest))
}

pub async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if flash.err.is_none() && flash.ok.is_none() {
        if let Some(user) = load_user(&state.sdk.db, &session).await? {
            return Ok(with_cookie(jar, Redirect::to(signed_home(&user))));
        }
    }
    Ok(with_cookie(
        jar,
        html(views::landing(
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

pub async fn register_form(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Query(query): Query<RegisterQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if let Some(user) = load_user(&state.sdk.db, &session).await? {
        let code = query.code.trim();
        if code.is_empty() {
            return Ok(with_cookie(jar, Redirect::to(signed_home(&user))));
        }
        return enter_with_code(&state, &who.0, jar, &user, code, signed_home(&user)).await;
    }
    paint_register(
        &state,
        jar,
        &session.csrf,
        views::flash_from(query.ok, query.err),
        &views::RegisterDraft {
            first_name: "",
            last_name: "",
            email: "",
            code: query.code.trim(),
        },
    )
    .await
}

pub async fn join_link(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Path(code): Path<String>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if let Some(user) = load_user(&state.sdk.db, &session).await? {
        return enter_with_code(&state, &who.0, jar, &user, &code, signed_home(&user)).await;
    }
    let dest = format!("/register?code={}", views::escape_segment(code.trim()));
    Ok(with_cookie(jar, Redirect::to(&dest)))
}

pub async fn sign_in_form(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    guest_auth_form(state, jar, flash, |csrf, flash| {
        views::sign_in_page(flash, csrf, "")
    })
    .await
}

pub async fn magic_link_form(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    guest_auth_form(state, jar, flash, |csrf, flash| {
        views::magic_link_page(flash, csrf)
    })
    .await
}

pub async fn forgot_password_form(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    guest_auth_form(state, jar, flash, |csrf, flash| {
        views::forgot_password_page(flash, csrf)
    })
    .await
}

async fn guest_auth_form(
    state: AppState,
    jar: CookieJar,
    flash: FlashQuery,
    page: impl FnOnce(&str, Option<views::Flash>) -> maud::Markup,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    if let Some(user) = load_user(&state.sdk.db, &session).await? {
        return Ok(with_cookie(jar, Redirect::to(signed_home(&user))));
    }
    let markup = page(&session.csrf, views::flash_from(flash.ok, flash.err));
    Ok(with_cookie(jar, html(markup)))
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
        return Ok(with_cookie(jar, fail_csrf("/session/new")));
    }
    if session.user_id.is_some() {
        return Ok(with_cookie(
            jar,
            redirect_signed(&state, session.user_id.as_deref()).await?,
        ));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Session, &who.0).await {
        return Ok(with_cookie(jar, redirect_err("/session/new", "rate")));
    }
    match story::sign_in(
        &state.sdk,
        &form.email,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(
            put_signed(&state, jar, &ok),
            redirect_signed(&state, ok.user_id.as_deref()).await?,
        )),
        Err(_) => Ok(with_cookie(
            jar,
            html(views::sign_in_page(
                views::flash_from(None, Some("miss".into())),
                &session.csrf,
                &form.email,
            )),
        )),
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
    Ok(with_cookie(
        state.put_session(jar, &guest),
        Redirect::to("/"),
    ))
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
    Ok(with_cookie(
        state.put_session(jar, &guest),
        Redirect::to("/"),
    ))
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
        return Ok(with_cookie(
            state.put_session(jar, &guest),
            Redirect::to("/"),
        ));
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
    let back = register_return(form.code.trim());
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&back)));
    }
    if let Some(user) = load_user(&state.sdk.db, &session).await? {
        let code = form.code.trim();
        if code.is_empty() {
            return Ok(with_cookie(jar, Redirect::to(signed_home(&user))));
        }
        return enter_with_code(&state, &who.0, jar, &user, code, signed_home(&user)).await;
    }
    let draft = views::RegisterDraft {
        first_name: &form.first_name,
        last_name: &form.last_name,
        email: &form.email,
        code: form.code.trim(),
    };
    if !draft.code.is_empty() && church_named(&state, draft.code).await?.is_none() {
        return paint_register(&state, jar, &session.csrf, None, &draft).await;
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Register, &who.0).await {
        return Ok(with_cookie(jar, redirect_err(&back, "rate")));
    }
    match story::register(
        &state.sdk,
        &form.first_name,
        &form.last_name,
        &form.email,
        &form.password,
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => {
            let jar = put_signed(&state, jar, &ok);
            let code = form.code.trim();
            if code.is_empty() {
                return Ok(with_cookie(jar, redirect_ok("/churches/join", "welcome")));
            }
            let Some(user_id) = ok.user_id.as_deref() else {
                return Ok(with_cookie(jar, redirect_ok("/churches/join", "welcome")));
            };
            let Some(user) = state.sdk.db.user(user_id).await? else {
                return Ok(with_cookie(jar, redirect_ok("/churches/join", "welcome")));
            };
            enter_with_code(&state, &who.0, jar, &user, code, "/churches/join").await
        }
        Err(error) if error.flash_code() == "password" => {
            paint_register(
                &state,
                jar,
                &session.csrf,
                views::flash_from(None, Some(error.flash_code().to_string())),
                &draft,
            )
            .await
        }
        Err(error) => Ok(with_cookie(jar, leaf_err(&back, error))),
    }
}

pub async fn request_link(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    request_mail(&state, who, jar, form, MailAsk::Magic, "/session/link/new").await
}

pub async fn request_reset(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    request_mail(&state, who, jar, form, MailAsk::Reset, "/session/reset/new").await
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
    back: &'static str,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, state).await?;
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(back)));
    }
    if session.user_id.is_some() {
        return Ok(with_cookie(
            jar,
            redirect_signed(state, session.user_id.as_deref()).await?,
        ));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Session, &who.0).await {
        return Ok(with_cookie(jar, redirect_err(back, "mail")));
    }
    if let Ok(email) = ecclesia_sdk::prelude::normalize_email(&form.email)
        && let RateDecision::Refuse = state.sdk.gate.decide_mail(&email).await
    {
        return Ok(with_cookie(jar, redirect_err(back, "mail")));
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
    Ok(with_cookie(jar, redirect_err(back, "mail")))
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
        return Ok(with_cookie(
            jar,
            redirect_signed(&state, session.user_id.as_deref()).await?,
        ));
    }
    match story::consume_magic(
        &state.sdk,
        &id,
        query.t.as_deref().unwrap_or(""),
        &device_meta(&headers, &who),
    )
    .await?
    {
        Ok(ok) => Ok(with_cookie(
            put_signed(&state, jar, &ok),
            redirect_signed(&state, ok.user_id.as_deref()).await?,
        )),
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
        return Ok(with_cookie(
            jar,
            redirect_signed(&state, session.user_id.as_deref()).await?,
        ));
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
        Ok(ok) => Ok(with_cookie(
            put_signed(&state, jar, &ok),
            redirect_signed(&state, ok.user_id.as_deref()).await?,
        )),
        Err(error) if error.flash_code() == "password" => Ok(with_cookie(
            jar,
            leaf_err(&format!("/session/reset/{id}"), error),
        )),
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
            html(views::guest_home(
                views::flash_from(flash.ok, flash.err),
                &session.csrf,
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
    let page = story::home_needs(&state.sdk, &viewer, flash.after.as_deref()).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::home(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &page.cards,
            &page.churches,
            page.next_cursor.as_deref(),
            count,
            &session.csrf,
        )),
    ))
}

fn register_return(code: &str) -> String {
    let code = code.trim();
    if code.is_empty() {
        "/register".to_string()
    } else {
        format!("/register?code={}", views::escape_segment(code))
    }
}

async fn church_named(state: &AppState, code: &str) -> Result<Option<Church>, AppError> {
    let code = code.trim();
    if code.is_empty() {
        return Ok(None);
    }
    Ok(state.sdk.db.church_by_invite(code).await?)
}

async fn paint_register(
    state: &AppState,
    jar: CookieJar,
    csrf: &str,
    flash: Option<views::Flash>,
    draft: &views::RegisterDraft<'_>,
) -> Result<Response, AppError> {
    let asked = draft.code.trim();
    let church = church_named(state, asked).await?;
    let code = if church.is_some() { asked } else { "" };
    let flash = if church.is_none() && !asked.is_empty() && flash.is_none() {
        views::flash_from(None, Some("invite".into()))
    } else {
        flash
    };
    let draft = views::RegisterDraft {
        first_name: draft.first_name,
        last_name: draft.last_name,
        email: draft.email,
        code,
    };
    Ok(with_cookie(
        jar,
        html(views::register_page(flash, csrf, &draft, church.as_ref())),
    ))
}

async fn enter_with_code(
    state: &AppState,
    who: &str,
    jar: CookieJar,
    user: &User,
    code: &str,
    fail: &str,
) -> Result<Response, AppError> {
    if let RateDecision::Refuse = state.decide_rate(RateKind::Redeem, who).await {
        return Ok(with_cookie(jar, redirect_err(fail, "rate")));
    }
    let Some(church) = church_named(state, code).await? else {
        return Ok(with_cookie(jar, redirect_err(fail, "invite")));
    };
    match story::redeem_invite(&state.sdk, user, code.trim()).await? {
        Ok(_) => {
            let dest = format!("/churches/{}", church.id);
            Ok(with_cookie(jar, redirect_ok(&dest, "redeemed")))
        }
        Err(DomainError::AlreadyMember) => {
            let dest = format!("/churches/{}", church.id);
            Ok(with_cookie(jar, Redirect::to(&dest)))
        }
        Err(error) => Ok(with_cookie(jar, leaf_err(fail, error))),
    }
}

fn put_signed(state: &AppState, jar: CookieJar, ok: &StoryOk) -> CookieJar {
    let (Some(session_id), Some(user_id), Some(csrf)) =
        (ok.session_id.clone(), ok.user_id.clone(), ok.csrf.clone())
    else {
        return jar;
    };
    state.put_session(jar, &Session::signed_in(session_id, user_id, csrf))
}
