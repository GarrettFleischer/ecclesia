use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{
    accept_invite, approve_membership, decline_membership, invite_member, parse_invite_email,
    plant_church, redeem_invite, request_join, visible_need_cards, Church, Viewer,
};
use crate::sdk::clock::{new_id, nonce4, now_iso};
use crate::views;

use super::context::{
    apply_leaf_redirect, bind_session, church_directory, fail_csrf, governor_ids, html, leaf_err,
    redirect_err, redirect_ok, require_user, unread, viewer_for, with_cookie,
};
use super::forms::{ChurchForm, CsrfForm, FlashQuery, InviteForm, RedeemForm};
use super::{AppError, AppState};

pub async fn churches_index(
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
    let cards = church_directory(&state.db).await?;
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

pub async fn church_new(
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

pub async fn create_church(
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
    let Some(church_id) = effect.inserted_church_id() else {
        return Ok(with_cookie(jar, redirect_err("/churches/new", "missing")));
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(
        jar,
        redirect_ok(&format!("/churches/{church_id}"), "church_planted"),
    ))
}

pub async fn church_show(
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
    let visible = owned_church_needs(&viewer, &needs, &church);
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

fn owned_church_needs(
    viewer: &crate::leaf::Viewer,
    needs: &[crate::leaf::NeedCard],
    church: &Church,
) -> Vec<crate::leaf::NeedCard> {
    visible_need_cards(viewer, needs, std::slice::from_ref(church))
        .into_iter()
        .cloned()
        .collect()
}

pub async fn join_church(
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
    let governors = governor_ids(&state.db, &id).await?;
    let effect = match request_join(
        &user,
        &church,
        existing.as_ref(),
        &governors,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "joined_request")))
}

pub async fn invite(
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

pub async fn redeem(
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

pub async fn approve_membership_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_membership_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    apply_leaf_redirect(
        &state.db,
        loaded.jar,
        &loaded.dest,
        approve_membership(&loaded.viewer, &loaded.target, &loaded.church),
        "approved",
    )
    .await
}

pub async fn decline_membership_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_membership_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    apply_leaf_redirect(
        &state.db,
        loaded.jar,
        &loaded.dest,
        decline_membership(&loaded.viewer, &loaded.target, &loaded.church),
        "declined",
    )
    .await
}

struct MembershipDecision {
    jar: CookieJar,
    viewer: Viewer,
    target: crate::leaf::Membership,
    church: Church,
    dest: String,
}

async fn load_membership_decision(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    csrf: &str,
) -> Result<MembershipDecision, Response> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Err(with_cookie(jar, response)),
    };
    let Some(target) = state
        .db
        .membership(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/churches/{}", target.church_id);
    if !session.check_csrf(csrf) {
        return Err(with_cookie(jar, fail_csrf(&dest)));
    }
    let viewer = viewer_for(&state.db, user)
        .await
        .map_err(|error| error.into_response())?;
    let Some(church) = state
        .db
        .church(&target.church_id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    Ok(MembershipDecision {
        jar,
        viewer,
        target,
        church,
        dest,
    })
}

pub async fn accept_invite_http(
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
