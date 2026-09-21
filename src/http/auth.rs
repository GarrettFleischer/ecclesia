use axum::extract::{Form, Query, State};
use axum::response::{Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{may_impersonate, register, EmailAvailability};
use crate::sdk::clock::{new_id, now_iso};
use crate::sdk::session::{self, Session};
use crate::views;

use super::context::{
    bind_session, fail_csrf, html, leaf_err, load_user, memberships_with_churches, redirect_ok,
    require_user, unread, viewer_for, with_cookie,
};
use super::forms::{CsrfForm, FlashQuery, RegisterForm, SessionForm};
use super::{AppError, AppState};

pub async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if load_user(&state.db, &session).await?.is_some() && flash.err.is_none() && flash.ok.is_none()
    {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    let users = demo_people(&state).await?;
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

async fn demo_people(state: &AppState) -> Result<Vec<crate::leaf::User>, AppError> {
    match state.demo {
        crate::leaf::DemoSeat::Open => Ok(state.db.demo_users().await?),
        crate::leaf::DemoSeat::Sealed => Ok(Vec::new()),
    }
}

pub async fn start_session(
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
        return Ok(with_cookie(
            jar,
            super::context::redirect_err("/", "not_found"),
        ));
    };
    let next = Session::signed_in(user.id, session::fresh_csrf());
    Ok(with_cookie(
        session::put(jar, &state.secret, &next),
        Redirect::to("/home"),
    ))
}

pub async fn logout(
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

pub async fn register_user(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/")));
    }
    let availability = EmailAvailability::of_existing(state.db.user_by_email(&form.email).await?);
    let effect = match register(
        &form.name,
        &form.email,
        &form.city,
        &form.region,
        &form.bio,
        availability,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/", error))),
    };
    let Some(user_id) = effect.inserted_user_id() else {
        return Ok(with_cookie(
            jar,
            super::context::redirect_err("/", "missing"),
        ));
    };
    let user_id = user_id.to_owned();
    state.commit(&effect).await?;
    let next = Session::signed_in(user_id, session::fresh_csrf());
    Ok(with_cookie(
        session::put(jar, &state.secret, &next),
        redirect_ok("/home", "welcome"),
    ))
}

pub async fn home(
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
    let pending = memberships_with_churches(&state.db, viewer.pending_memberships()).await?;
    let needs = state.db.all_need_cards().await?;
    let churches = state.db.churches().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::home(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &pending,
            &needs,
            &churches,
            count,
            &session.csrf,
        )),
    ))
}
