use axum::extract::{Form, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::views;
use ecclesia_sdk::prelude::{SkillSource, User, VoiceKind};
use ecclesia_sdk::story;

use super::context::{
    bind_session, html, leaf_err, load_user, member_churches, redirect_err, redirect_ok,
    signed_form, signed_in, story_redirect, unread, viewer_for, with_cookie,
};
use super::forms::{
    ChurchPost, CsrfForm, EndorseForm, FlashQuery, GiftForm, ProfileForm, TransferForm,
};
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let Some(person) = state.sdk.db.user(&id).await? else {
        let count = unread(&state.sdk.db, &viewer.user.id).await?;
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
    let churches = member_churches(&state.sdk.db, &person).await?;
    let gifts = state.sdk.db.member_gifts(&person.id).await?;
    let endorsements = state.sdk.db.accepted_endorsements_for(&person.id).await?;
    let declined = state.sdk.db.declined_endorsements_for(&person.id).await?;
    let catalog = story::gift_catalog(&state.sdk).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let avatar = avatar_media_id(&state.sdk.db, &person.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::member_show(
            &viewer,
            &person,
            avatar.as_deref(),
            &churches,
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
    let Some(person) = state.sdk.db.user(&id).await? else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_found")));
    };
    let catalog = story::gift_catalog(&state.sdk).await?;
    let skill = match SkillSource::from_catalog(&catalog, &form.skill) {
        Ok(skill) => skill,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    };
    if state.awaiting_review(&form.pass, &[&form.note]) {
        let note = state.polish(VoiceKind::Endorsement, &form.note).await;
        return paint_member(
            &state,
            signed.jar,
            viewer_for(&state.sdk.db, signed.user).await?,
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
    story_redirect(
        signed.jar,
        &dest,
        story::endorse(&state.sdk, &signed.user, &id, skill, &form.note).await?,
        "endorsed",
    )
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
    story_redirect(
        loaded.jar,
        "/inbox",
        story::accept_endorsement(&state.sdk, &loaded.user, &id).await?,
        "endorsement_accepted",
    )
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
    story_redirect(
        loaded.jar,
        "/inbox",
        story::decline_endorsement(&state.sdk, &loaded.user, &id).await?,
        "endorsement_declined",
    )
}

struct EndorsementDecision {
    jar: CookieJar,
    user: User,
}

async fn load_endorsement_decision(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    csrf: &str,
) -> Result<EndorsementDecision, Response> {
    let signed = signed_form(state, jar, csrf, "/inbox").await?;
    if state
        .sdk
        .db
        .endorsement(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
        .is_none()
    {
        return Err(with_cookie(signed.jar, redirect_err("/inbox", "not_found")));
    }
    Ok(EndorsementDecision {
        jar: signed.jar,
        user: signed.user,
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let pending = state
        .sdk
        .db
        .pending_endorsements_for(&viewer.user.id)
        .await?;
    let declined = state
        .sdk
        .db
        .declined_endorsements_for(&viewer.user.id)
        .await?;
    let notes = state.sdk.db.notifications(&viewer.user.id).await?;
    state
        .sdk
        .db
        .mark_notifications_read(&viewer.user.id)
        .await?;
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let gifts = state.sdk.db.member_gifts(&viewer.user.id).await?;
    let catalog = story::gift_catalog(&state.sdk).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let devices = state.sdk.db.sessions_for_user(&viewer.user.id).await?;
    let movable = state
        .sdk
        .db
        .open_needs_from_closed_churches(&viewer.user.id)
        .await?;
    let avatar = avatar_media_id(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::me(
            &viewer,
            avatar.as_deref(),
            &gifts,
            &catalog,
            &devices,
            signed.session.session_id.as_deref(),
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::ProfileDraft {
                first_name: &viewer.user.first_name,
                last_name: &viewer.user.last_name,
                bio: &viewer.user.bio,
                kind: views::DraftKind::Blank,
            },
            &views::GiftDraft::blank(),
            &movable,
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
    if state.awaiting_review(&form.pass, &[&form.first_name, &form.last_name, &form.bio]) {
        let bio = state.polish(VoiceKind::Bio, &form.bio).await;
        let viewer = viewer_for(&state.sdk.db, signed.user).await?;
        return paint_me(
            &state,
            signed.jar,
            &viewer,
            None,
            &signed.session.csrf,
            &views::ProfileDraft {
                first_name: &form.first_name,
                last_name: &form.last_name,
                bio: &bio,
                kind: views::DraftKind::Review,
            },
            &views::GiftDraft::blank(),
        )
        .await;
    }
    story_redirect(
        signed.jar,
        "/me",
        story::update_profile(
            &state.sdk,
            &signed.user.id,
            &form.first_name,
            &form.last_name,
            &form.bio,
        )
        .await?,
        "saved",
    )
}

pub async fn leave_church_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChurchPost>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let staying = signed
        .user
        .memberships
        .iter()
        .any(|link| link.church_id != form.church_id);
    match story::leave_church(&state.sdk, &signed.user, &form.church_id).await? {
        Ok(_) => {
            let dest = if staying { "/me" } else { "/churches/join" };
            Ok(with_cookie(signed.jar, redirect_ok(dest, "left")))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    }
}

pub async fn close_church_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ChurchPost>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let staying = signed
        .user
        .memberships
        .iter()
        .any(|link| link.church_id != form.church_id);
    match story::close_church(&state.sdk, &signed.user, &form.church_id).await? {
        Ok(_) => {
            let dest = if staying { "/home" } else { "/churches/join" };
            Ok(with_cookie(signed.jar, redirect_ok(dest, "closed")))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    }
}

pub async fn transfer_church_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<TransferForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    match story::transfer_church(&state.sdk, &signed.user, &form.church_id, &form.email).await? {
        Ok(_) => Ok(with_cookie(signed.jar, redirect_ok("/me", "transferred"))),
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/me", error))),
    }
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
        let viewer = viewer_for(&state.sdk.db, signed.user).await?;
        return paint_me(
            &state,
            signed.jar,
            &viewer,
            None,
            &signed.session.csrf,
            &views::ProfileDraft {
                first_name: &viewer.user.first_name,
                last_name: &viewer.user.last_name,
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
    story_redirect(
        signed.jar,
        "/me",
        story::add_gift(&state.sdk, &signed.user.id, &form.gift_id, &form.note).await?,
        "gift_added",
    )
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
    story_redirect(
        signed.jar,
        "/me",
        story::remove_gift(&state.sdk, &signed.user.id, &id).await?,
        "gift_removed",
    )
}

pub async fn fallback(State(state): State<AppState>, jar: CookieJar) -> Response {
    let Ok((session, jar)) = bind_session(jar, &state).await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            html(views::error_page(
                "Something broke on our end. Try again in a moment.",
            )),
        )
            .into_response();
    };
    match load_user(&state.sdk.db, &session).await {
        Ok(Some(user)) => signed_missing(&state, jar, user, &session.csrf).await,
        _ => (
            StatusCode::NOT_FOUND,
            html(views::error_page("That page doesn't exist.")),
        )
            .into_response(),
    }
}

async fn signed_missing(state: &AppState, jar: CookieJar, user: User, csrf: &str) -> Response {
    let count = unread(&state.sdk.db, &user.id).await.unwrap_or(0);
    (
        StatusCode::NOT_FOUND,
        with_cookie(
            jar,
            html(views::sorry_page(
                "That page doesn't exist.",
                views::SorrySeat::Member {
                    user: &user,
                    unread: count,
                    csrf,
                },
            )),
        ),
    )
        .into_response()
}

async fn paint_member(
    state: &AppState,
    jar: CookieJar,
    viewer: ecclesia_sdk::prelude::Viewer,
    person: &User,
    flash: Option<views::Flash>,
    csrf: &str,
    draft: &views::EndorseDraft<'_>,
) -> Result<Response, AppError> {
    let churches = member_churches(&state.sdk.db, person).await?;
    let gifts = state.sdk.db.member_gifts(&person.id).await?;
    let endorsements = state.sdk.db.accepted_endorsements_for(&person.id).await?;
    let declined = state.sdk.db.declined_endorsements_for(&person.id).await?;
    let catalog = story::gift_catalog(&state.sdk).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let avatar = avatar_media_id(&state.sdk.db, &person.id).await?;
    Ok(with_cookie(
        jar,
        html(views::member_show(
            &viewer,
            person,
            avatar.as_deref(),
            &churches,
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
    viewer: &ecclesia_sdk::prelude::Viewer,
    flash: Option<views::Flash>,
    csrf: &str,
    profile: &views::ProfileDraft<'_>,
    gift: &views::GiftDraft<'_>,
) -> Result<Response, AppError> {
    let gifts = state.sdk.db.member_gifts(&viewer.user.id).await?;
    let catalog = story::gift_catalog(&state.sdk).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let devices = state.sdk.db.sessions_for_user(&viewer.user.id).await?;
    let movable = state
        .sdk
        .db
        .open_needs_from_closed_churches(&viewer.user.id)
        .await?;
    let avatar = avatar_media_id(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::me(
            viewer,
            avatar.as_deref(),
            &gifts,
            &catalog,
            &devices,
            None,
            count,
            flash,
            csrf,
            profile,
            gift,
            &movable,
        )),
    ))
}

#[derive(Deserialize, Default)]
pub(crate) struct AvatarDeleteQuery {
    #[serde(default)]
    csrf: String,
}

pub async fn replace_avatar_http(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
) -> Result<Response, AppError> {
    let (parts, body) = request.into_parts();
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if !super::needs::is_multipart(&parts.headers) {
        return Ok(with_cookie(signed.jar, redirect_err("/me", "missing")));
    }
    let form = match super::needs::read_multipart_body(
        &parts.headers,
        body,
        super::needs::AVATAR_BYTE_LIMIT,
    )
    .await
    {
        Ok(form) => form,
        Err(error) => {
            return Ok(with_cookie(
                signed.jar,
                redirect_err("/me", views::media_flash_code(&error)),
            ));
        }
    };
    if !signed
        .session
        .check_csrf(super::needs::field(&form, "csrf"))
    {
        return Ok(with_cookie(signed.jar, redirect_err("/me", "csrf")));
    }
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let media_id = match super::needs::one_staged_photo(&state.sdk.db, &viewer.user.id, &form).await
    {
        Ok(Some(id)) => id,
        Ok(None) => return Ok(with_cookie(signed.jar, redirect_err("/me", "missing"))),
        Err(error) => {
            return Ok(with_cookie(
                signed.jar,
                redirect_err("/me", views::media_flash_code(&error)),
            ));
        }
    };
    let listed = match super::needs::GrantList::load(&state.sdk.db, &viewer.user.id).await {
        Ok(listed) => listed,
        Err(error) => {
            return Ok(with_cookie(
                signed.jar,
                redirect_err("/me", views::media_flash_code(&error)),
            ));
        }
    };
    let grants = listed.grants();
    let sight = ecclesia_sdk::story::StagedMediaSight::granted(&viewer.user.id, &grants);
    match story::replace_avatar_photo(&state.sdk, &viewer, &media_id, &sight).await? {
        Ok(_) => Ok(with_cookie(signed.jar, redirect_ok("/me", "photo_updated"))),
        Err(ecclesia_sdk::story::ConversationError::Domain(error)) => {
            Ok(with_cookie(signed.jar, leaf_err("/me", error)))
        }
        Err(ecclesia_sdk::story::ConversationError::UngrantedMedia { .. }) => {
            Ok(with_cookie(signed.jar, redirect_err("/me", "not_owned")))
        }
    }
}

pub async fn remove_avatar_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<AvatarDeleteQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let csrf = super::needs::csrf_from_request(&headers, &query.csrf);
    let signed = match signed_form(&state, jar, &csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    story_redirect(
        signed.jar,
        "/me",
        story::remove_avatar_photo(&state.sdk, &viewer).await?,
        "photo_removed",
    )
}

async fn avatar_media_id(
    db: &ecclesia_sdk::db::Db,
    user_id: &str,
) -> Result<Option<String>, AppError> {
    Ok(match db.avatar_reference(user_id).await? {
        Some(ecclesia_sdk::db::AvatarReference::Photo(id)) => Some(id),
        _ => None,
    })
}
