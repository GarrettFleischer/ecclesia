use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{
    Church, Viewer, VoiceKind, accept_invite, approve_membership, decline_membership,
    invite_member, parse_invite_email, plant_church, redeem_invite, request_join,
};
use crate::sdk::clock::{new_id, nonce4, now_iso};
use crate::views;

use super::context::{
    apply_leaf_redirect, church_directory, governor_ids, html, leaf_err, redirect_err, redirect_ok,
    signed_form, signed_in, unread, viewer_for, with_cookie,
};
use super::forms::{ChurchForm, CsrfForm, FlashQuery, InviteForm, RedeemForm};
use super::{AppError, AppState};

pub async fn churches_index(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let cards = church_directory(&state.db).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::churches_index(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &cards,
            count,
            &signed.session.csrf,
        )),
    ))
}

pub async fn church_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::church_new(
            &viewer,
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::ChurchDraft::blank(&viewer.user.city, &viewer.user.region),
        )),
    ))
}

pub async fn create_church(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChurchForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/churches/new").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if state.awaiting_review(&form.pass, &[&form.description]) {
        let description = state.polish(VoiceKind::Church, &form.description).await;
        let count = unread(&state.db, &signed.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::church_new(
                &viewer_for(&state.db, signed.user).await?,
                count,
                None,
                &signed.session.csrf,
                &views::ChurchDraft {
                    name: &form.name,
                    city: &form.city,
                    region: &form.region,
                    gathering: &form.gathering,
                    description: &description,
                    kind: views::DraftKind::Review,
                },
            )),
        ));
    }
    let posture = state
        .weigh(
            VoiceKind::Church,
            &[&form.name, &form.gathering, &form.description],
        )
        .await;
    let effect = match plant_church(
        &signed.user,
        &form.name,
        &form.city,
        &form.region,
        &form.description,
        &form.gathering,
        posture,
        new_id(),
        new_id(),
        &nonce4(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err("/churches/new", error))),
    };
    let Some(church_id) = effect.inserted_church_id() else {
        return Ok(with_cookie(
            signed.jar,
            redirect_err("/churches/new", "missing"),
        ));
    };
    let dest = format!("/churches/{church_id}");
    state.commit(&effect).await?;
    Ok(with_cookie(
        signed.jar,
        redirect_ok(&dest, "church_planted"),
    ))
}

pub async fn church_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(
            signed.jar,
            html(views::error_page("We couldn't find that church.")),
        ));
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let members = state.db.church_members(&church.id).await?;
    let needs = state.db.church_need_cards(&church.id).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::church_show(
            &viewer,
            &church,
            &members,
            &needs,
            views::flash_from(flash.ok, flash.err),
            count,
            &signed.session.csrf,
        )),
    ))
}

pub async fn join_church(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let dest = format!("/churches/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
    };
    let existing = state.db.membership_pair(&id, &signed.user.id).await?;
    let governors = governor_ids(&state.db, &id).await?;
    let effect = match request_join(
        &signed.user,
        &church,
        existing.as_ref(),
        &governors,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(
        signed.jar,
        redirect_ok(&dest, "joined_request"),
    ))
}

pub async fn invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<InviteForm>,
) -> Result<Response, AppError> {
    let dest = format!("/churches/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let Some(church) = state.db.church(&id).await? else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
    };
    let email = match parse_invite_email(&form.email) {
        Ok(email) => email,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    let Some(invitee) = state.db.user_by_email(&email).await? else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
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
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(signed.jar, redirect_ok(&dest, "invited")))
}

pub async fn redeem(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RedeemForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/churches").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(church) = state.db.church_by_invite(&form.code).await? else {
        return Ok(with_cookie(signed.jar, redirect_err("/churches", "invite")));
    };
    let existing = state
        .db
        .membership_pair(&church.id, &signed.user.id)
        .await?;
    let dest = format!("/churches/{}", church.id);
    let effect = match redeem_invite(
        &signed.user,
        &church,
        existing.as_ref(),
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    if !effect.writes.is_empty() {
        state.commit(&effect).await?;
    }
    Ok(with_cookie(signed.jar, redirect_ok(&dest, "redeemed")))
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
        &state,
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
        &state,
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
    let signed = signed_in(state, jar).await?;
    let Some(target) = state
        .db
        .membership(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(signed.jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/churches/{}", target.church_id);
    if !signed.session.check_csrf(csrf) {
        return Err(with_cookie(signed.jar, super::context::fail_csrf(&dest)));
    }
    let viewer = viewer_for(&state.db, signed.user)
        .await
        .map_err(|error| error.into_response())?;
    let Some(church) = state
        .db
        .church(&target.church_id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
    };
    Ok(MembershipDecision {
        jar: signed.jar,
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
    let signed = match signed_form(&state, jar, &form.csrf, "/home").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(target) = state.db.membership(&id).await? else {
        return Ok(with_cookie(signed.jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/churches/{}", target.church_id);
    let effect = match accept_invite(&signed.user, &target) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(
        signed.jar,
        redirect_ok(&dest, "invite_accepted"),
    ))
}
