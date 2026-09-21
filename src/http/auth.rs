use axum::extract::{Form, Query, State};
use axum::response::{Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{EmailAvailability, VoiceKind, may_impersonate, pair_memberships, register};
use crate::sdk::clock::{new_id, now_iso};
use crate::sdk::session::{self, Session};
use crate::views;

use super::context::{
    ClientKey, bind_session, fail_csrf, html, leaf_err, load_user, redirect_err, redirect_ok,
    unread, viewer_for, with_cookie,
};
use super::forms::{CsrfForm, FlashQuery, RegisterForm, SessionForm};
use super::{AppError, AppState};
use crate::sdk::limit::{RateDecision, RateKind};

pub async fn landing(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    if load_user(&state.db, &session).await?.is_some() && flash.err.is_none() && flash.ok.is_none()
    {
        return Ok(with_cookie(jar, Redirect::to("/home")));
    }
    Ok(with_cookie(
        jar,
        html(views::landing(
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
            state.demo,
            &views::RegisterDraft::blank(),
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
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<SessionForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/home")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Session, &who.0) {
        return Ok(with_cookie(jar, redirect_err("/home", "rate")));
    }
    if let Err(error) = may_impersonate(state.demo) {
        return Ok(with_cookie(jar, leaf_err("/home", error)));
    }
    let Some(user) = state.db.user(form.user_id.trim()).await? else {
        return Ok(with_cookie(
            jar,
            super::context::redirect_err("/home", "not_found"),
        ));
    };
    let next = Session::signed_in(user.id, session::fresh_csrf());
    Ok(with_cookie(
        state.put_session(jar, &next),
        Redirect::to("/home"),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/me")));
    }
    Ok(with_cookie(state.clear_session(jar), Redirect::to("/")))
}

pub async fn register_user(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/home")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Register, &who.0) {
        return Ok(with_cookie(jar, redirect_err("/home", "rate")));
    }
    if state.awaiting_review(&form.pass, &[&form.bio]) {
        let bio = state.polish(VoiceKind::Bio, &form.bio).await;
        let users = demo_people(&state).await?;
        return Ok(with_cookie(
            jar,
            html(views::guest_home(
                &users,
                None,
                &session.csrf,
                state.demo,
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
    let availability = EmailAvailability::of_existing(state.db.user_by_email(&form.email).await?);
    let posture = state.weigh(VoiceKind::Bio, &[&form.name, &form.bio]).await;
    let effect = match register(
        &form.name,
        &form.email,
        &form.city,
        &form.region,
        &form.bio,
        availability,
        posture,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/home", error))),
    };
    let Some(user_id) = effect.inserted_user_id() else {
        return Ok(with_cookie(
            jar,
            super::context::redirect_err("/home", "missing"),
        ));
    };
    let user_id = user_id.to_owned();
    state.commit(&effect).await?;
    let next = Session::signed_in(user_id, session::fresh_csrf());
    Ok(with_cookie(
        state.put_session(jar, &next),
        redirect_ok("/home", "welcome"),
    ))
}

pub async fn home(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    match load_user(&state.db, &session).await? {
        Some(user) => member_home(state, jar, session, user, flash).await,
        None => {
            let users = demo_people(&state).await?;
            Ok(with_cookie(
                jar,
                html(views::guest_home(
                    &users,
                    views::flash_from(flash.ok, flash.err),
                    &session.csrf,
                    state.demo,
                    &views::RegisterDraft::blank(),
                )),
            ))
        }
    }
}

async fn member_home(
    state: AppState,
    jar: CookieJar,
    session: Session,
    user: crate::leaf::User,
    flash: FlashQuery,
) -> Result<Response, AppError> {
    let viewer = viewer_for(&state.db, user).await?;
    let pending: Vec<_> =
        pair_memberships(viewer.pending_memberships(), &viewer.churches).collect();
    let needs = state.db.open_need_cards().await?;
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
