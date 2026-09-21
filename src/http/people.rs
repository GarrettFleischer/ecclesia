use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{
    CatalogPresence, Endorsement, EndorsementQueue, GiftOnProfile, SkillSource, User, VoiceKind,
    accept_endorsement, add_gift, decline_endorsement, endorse, pair_memberships, remove_gift,
    update_profile,
};
use crate::sdk::clock::{new_id, now_iso};
use crate::views;

use super::context::{
    apply_leaf_redirect, churches_by_place, churches_for_memberships, html, leaf_err, redirect_err,
    redirect_ok, signed_form, signed_in, unread, viewer_for, with_cookie,
};
use super::forms::{CsrfForm, EndorseForm, FlashQuery, GiftForm, ProfileForm};
use super::{AppError, AppState};

pub async fn member_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let Some(person) = state.db.user(&id).await? else {
        let count = unread(&state.db, &viewer.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::sorry_page(
                "We couldn't find that person.",
                views::SorrySeat::Member {
                    user: &viewer.user,
                    unread: count,
                    csrf: &signed.session.csrf,
                },
            )),
        ));
    };
    let memberships = state.db.memberships_for_user(&person.id).await?;
    let churches = churches_for_memberships(&state.db, &memberships).await?;
    let paired: Vec<_> = pair_memberships(&memberships, &churches).collect();
    let gifts = state.db.member_gifts(&person.id).await?;
    let endorsements = state.db.accepted_endorsements_for(&person.id).await?;
    let declined = state.db.declined_endorsements_for(&person.id).await?;
    let catalog = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::member_show(
            &viewer,
            &person,
            &paired,
            &gifts,
            &endorsements,
            &declined,
            &catalog,
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::EndorseDraft::blank(),
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
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(person) = state.db.user(&id).await? else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
    };
    let catalog = state.db.gifts().await?;
    let skill = match SkillSource::from_catalog(&catalog, &form.skill) {
        Ok(skill) => skill,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    let queue = EndorsementQueue::of_existing(
        state
            .db
            .pending_endorsement(&signed.user.id, &id, skill.display())
            .await?,
    );
    if state.awaiting_review(&form.pass, &[&form.note]) {
        let note = state.polish(VoiceKind::Endorsement, &form.note).await;
        return paint_member(
            &state,
            signed.jar,
            viewer_for(&state.db, signed.user).await?,
            &person,
            None,
            &signed.session.csrf,
            &views::EndorseDraft {
                skill: skill.display(),
                note: &note,
                kind: views::DraftKind::Review,
            },
        )
        .await;
    }
    let posture = state
        .weigh(VoiceKind::Endorsement, &[skill.display(), &form.note])
        .await;
    let effect = match endorse(
        &signed.user,
        &person,
        skill,
        queue,
        &form.note,
        posture,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(signed.jar, redirect_ok(&dest, "endorsed")))
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
    let signed = signed_form(state, jar, csrf, "/inbox").await?;
    let Some(endorsement) = state
        .db
        .endorsement(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(signed.jar, redirect_err("/inbox", "not_found")));
    };
    let gift_ids = state
        .db
        .gift_ids_for(&signed.user.id)
        .await
        .map_err(|error| AppError::from(error).into_response())?;
    let held = GiftOnProfile::of_ids(&gift_ids, &endorsement.gift_id);
    Ok(EndorsementDecision {
        jar: signed.jar,
        user: signed.user,
        endorsement,
        held,
    })
}

pub async fn inbox(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let pending = state.db.pending_endorsements_for(&viewer.user.id).await?;
    let declined = state.db.declined_endorsements_for(&viewer.user.id).await?;
    let notes = state.db.notifications(&viewer.user.id).await?;
    state.db.mark_notifications_read(&viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::inbox(
            &viewer,
            &pending,
            &declined,
            &notes,
            0,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
        )),
    ))
}

pub async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let gifts = state.db.member_gifts(&viewer.user.id).await?;
    let catalog = state.db.gifts().await?;
    let memberships: Vec<_> = pair_memberships(&viewer.memberships, &viewer.churches).collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::me(
            &viewer,
            &gifts,
            &catalog,
            &memberships,
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::ProfileDraft {
                name: &viewer.user.name,
                city: &viewer.user.city,
                region: &viewer.user.region,
                bio: &viewer.user.bio,
                kind: views::DraftKind::Blank,
            },
            &views::GiftDraft::blank(),
        )),
    ))
}

pub async fn update_me(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ProfileForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if state.awaiting_review(&form.pass, &[&form.bio]) {
        let bio = state.polish(VoiceKind::Bio, &form.bio).await;
        let viewer = viewer_for(&state.db, signed.user).await?;
        return paint_me(
            &state,
            signed.jar,
            &viewer,
            None,
            &signed.session.csrf,
            &views::ProfileDraft {
                name: &form.name,
                city: &form.city,
                region: &form.region,
                bio: &bio,
                kind: views::DraftKind::Review,
            },
            &views::GiftDraft::blank(),
        )
        .await;
    }
    let posture = state.weigh(VoiceKind::Bio, &[&form.name, &form.bio]).await;
    let effect = match update_profile(
        &signed.user.id,
        &form.name,
        &form.city,
        &form.region,
        &form.bio,
        posture,
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(signed.jar, redirect_ok("/me", "saved")))
}

pub async fn add_gift_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<GiftForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if state.awaiting_review(&form.pass, &[&form.note]) {
        let note = state.polish(VoiceKind::GiftNote, &form.note).await;
        let viewer = viewer_for(&state.db, signed.user).await?;
        return paint_me(
            &state,
            signed.jar,
            &viewer,
            None,
            &signed.session.csrf,
            &views::ProfileDraft {
                name: &viewer.user.name,
                city: &viewer.user.city,
                region: &viewer.user.region,
                bio: &viewer.user.bio,
                kind: views::DraftKind::Blank,
            },
            &views::GiftDraft {
                gift_id: &form.gift_id,
                note: &note,
                kind: views::DraftKind::Review,
            },
        )
        .await;
    }
    let presence = CatalogPresence::of_lookup(state.db.gift(&form.gift_id).await?);
    let posture = state.weigh(VoiceKind::GiftNote, &[&form.note]).await;
    let effect = match add_gift(
        &signed.user.id,
        &form.gift_id,
        presence,
        &form.note,
        posture,
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(signed.jar, redirect_ok("/me", "gift_added")))
}

pub async fn remove_gift_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let effect = match remove_gift(&signed.user.id, &id) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    };
    state.commit(&effect).await?;
    Ok(with_cookie(signed.jar, redirect_ok("/me", "gift_removed")))
}

pub async fn the_body(State(state): State<AppState>, jar: CookieJar) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.db, signed.user).await?;
    let groups = churches_by_place(&state.db).await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::the_body(
            &viewer,
            &groups,
            count,
            &signed.session.csrf,
        )),
    ))
}

pub async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        html(views::error_page("That page doesn't exist.")),
    )
}

async fn paint_member(
    state: &AppState,
    jar: CookieJar,
    viewer: crate::leaf::Viewer,
    person: &User,
    flash: Option<views::Flash>,
    csrf: &str,
    draft: &views::EndorseDraft<'_>,
) -> Result<Response, AppError> {
    let memberships = state.db.memberships_for_user(&person.id).await?;
    let churches = churches_for_memberships(&state.db, &memberships).await?;
    let paired: Vec<_> = pair_memberships(&memberships, &churches).collect();
    let gifts = state.db.member_gifts(&person.id).await?;
    let endorsements = state.db.accepted_endorsements_for(&person.id).await?;
    let declined = state.db.declined_endorsements_for(&person.id).await?;
    let catalog = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::member_show(
            &viewer,
            person,
            &paired,
            &gifts,
            &endorsements,
            &declined,
            &catalog,
            count,
            flash,
            csrf,
            draft,
        )),
    ))
}

async fn paint_me(
    state: &AppState,
    jar: CookieJar,
    viewer: &crate::leaf::Viewer,
    flash: Option<views::Flash>,
    csrf: &str,
    profile: &views::ProfileDraft<'_>,
    gift: &views::GiftDraft<'_>,
) -> Result<Response, AppError> {
    let gifts = state.db.member_gifts(&viewer.user.id).await?;
    let catalog = state.db.gifts().await?;
    let memberships: Vec<_> = pair_memberships(&viewer.memberships, &viewer.churches).collect();
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::me(
            viewer,
            &gifts,
            &catalog,
            &memberships,
            count,
            flash,
            csrf,
            profile,
            gift,
        )),
    ))
}
