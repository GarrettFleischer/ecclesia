use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{
    accept_endorsement, add_gift, decline_endorsement, endorse, remove_gift, update_profile,
    CatalogPresence, Endorsement, EndorsementQueue, GiftOnProfile, SkillSource, User,
};
use crate::sdk::clock::{new_id, now_iso};
use crate::views;

use super::context::{
    apply_leaf_redirect, bind_session, churches_by_place, churches_paired_with, fail_csrf, html,
    leaf_err, redirect_err, redirect_ok, require_user, unread, viewer_for, with_cookie,
};
use super::forms::{CsrfForm, EndorseForm, FlashQuery, GiftForm, ProfileForm};
use super::{AppError, AppState};

pub async fn member_show(
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
            html(views::error_page("We couldn't find that person.")),
        ));
    };
    let memberships = state.db.memberships_for_user(&person.id).await?;
    let churches = churches_paired_with(&state.db, &memberships).await?;
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

pub async fn endorse_member(
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
    let catalog = state.db.gifts().await?;
    let skill = match SkillSource::from_catalog(&catalog, &form.skill) {
        Ok(skill) => skill,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    let queue = EndorsementQueue::of_existing(
        state
            .db
            .pending_endorsement(&user.id, &id, skill.display())
            .await?,
    );
    let effect = match endorse(
        &user,
        &person,
        skill,
        queue,
        &form.note,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "endorsed")))
}

pub async fn accept_endorsement_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_endorsement_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    apply_leaf_redirect(
        &state,
        loaded.jar,
        "/inbox",
        accept_endorsement(&loaded.user, &loaded.endorsement, loaded.held),
        "endorsement_accepted",
    )
    .await
}

pub async fn decline_endorsement_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_endorsement_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    apply_leaf_redirect(
        &state,
        loaded.jar,
        "/inbox",
        decline_endorsement(&loaded.user, &loaded.endorsement),
        "endorsement_declined",
    )
    .await
}

struct EndorsementDecision {
    jar: CookieJar,
    user: User,
    endorsement: Endorsement,
    held: GiftOnProfile,
}

async fn load_endorsement_decision(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    csrf: &str,
) -> Result<EndorsementDecision, Response> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(csrf) {
        return Err(with_cookie(jar, fail_csrf("/inbox")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Err(with_cookie(jar, response)),
    };
    let Some(endorsement) = state
        .db
        .endorsement(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(jar, redirect_err("/inbox", "not_found")));
    };
    let gift_ids = state
        .db
        .gift_ids_for(&user.id)
        .await
        .map_err(|error| AppError::from(error).into_response())?;
    let held = GiftOnProfile::of_ids(&gift_ids, &endorsement.gift_id);
    Ok(EndorsementDecision {
        jar,
        user,
        endorsement,
        held,
    })
}

pub async fn inbox(
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

pub async fn me(
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
    let memberships = churches_paired_with(&state.db, &viewer.memberships).await?;
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

pub async fn update_me(
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
    state.commit(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "saved")))
}

pub async fn add_gift_http(
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
    let presence = CatalogPresence::of_lookup(state.db.gift(&form.gift_id).await?);
    let effect = match add_gift(&user.id, &form.gift_id, presence, &form.note) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/me", error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "gift_added")))
}

pub async fn remove_gift_http(
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
    state.commit(&effect).await?;
    Ok(with_cookie(jar, redirect_ok("/me", "gift_removed")))
}

pub async fn the_body(State(state): State<AppState>, jar: CookieJar) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let groups = churches_by_place(&state.db).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::the_body(&viewer, &groups, count, &session.csrf)),
    ))
}

pub async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        html(views::error_page("That page doesn't exist.")),
    )
}
