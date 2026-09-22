use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use ecclesia_sdk::prelude::{OfferState, Viewer, VoiceKind, require_need_view, visible_offers};
use ecclesia_sdk::story;
use crate::views;

use super::context::{
    html, leaf_err, optional_gift_id, redirect_err, redirect_ok, signed_form, story_redirect,
    signed_in, unread, viewer_for, with_cookie,
};
use super::forms::{ApplyForm, CsrfForm, FlashQuery, NeedForm, NeedQuery};
use super::{AppError, AppState};

pub async fn need_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<NeedQuery>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let gifts = story::gift_catalog(&state.sdk).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::need_new(
            &viewer,
            &gifts,
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::NeedDraft::blank(query.church_id.as_deref().unwrap_or("")),
        )),
    ))
}

pub async fn create_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<NeedForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/needs/new").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.title, &form.body]) {
        let title = state.polish(VoiceKind::Need, &form.title).await;
        let body = state.polish(VoiceKind::Need, &form.body).await;
        let gifts = story::gift_catalog(&state.sdk).await?;
        let count = unread(&state.sdk.db, &viewer.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::need_new(
                &viewer,
                &gifts,
                count,
                None,
                &signed.session.csrf,
                &views::NeedDraft {
                    church_id: &form.church_id,
                    title: &title,
                    body: &body,
                    gift_id: &form.gift_id,
                    scope: &form.scope,
                    kind: views::DraftKind::Review,
                },
            )),
        ));
    }
    let gift = optional_gift_id(&form.gift_id);
    match story::post_need(
        &state.sdk,
        &viewer,
        &form.church_id,
        &form.title,
        &form.body,
        gift,
        &form.scope,
    )
    .await?
    {
        Ok(ok) => {
            let Some(need_id) = ok.need_id else {
                return Ok(with_cookie(
                    signed.jar,
                    redirect_err("/needs/new", "missing"),
                ));
            };
            let dest = format!("/needs/{need_id}");
            Ok(with_cookie(signed.jar, redirect_ok(&dest, "need_posted")))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/needs/new", error))),
    }
}

pub async fn need_show(
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
    paint_need(
        &state,
        signed.jar,
        viewer,
        &id,
        views::flash_from(flash.ok, flash.err),
        &signed.session.csrf,
        &views::OfferDraft::blank(),
    )
    .await
}

pub async fn apply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ApplyForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.message]) {
        let message = state.polish(VoiceKind::Offer, &form.message).await;
        return paint_need(
            &state,
            signed.jar,
            viewer,
            &id,
            None,
            &signed.session.csrf,
            &views::OfferDraft {
                message: &message,
                kind: views::DraftKind::Review,
            },
        )
        .await;
    }
    story_redirect(
        signed.jar,
        &dest,
        story::apply_to_need(&state.sdk, &viewer, &id, &form.message).await?,
        "applied",
    )
}

pub async fn close_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    story_redirect(
        signed.jar,
        &dest,
        story::close_need(&state.sdk, &viewer, &id).await?,
        "need_closed",
    )
}

pub async fn accept_application_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_application_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::accept_application(&state.sdk, &loaded.viewer, &id).await?,
        "application_accepted",
    )
}

pub async fn decline_application_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_application_decision(&state, jar, &id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::decline_application(&state.sdk, &loaded.viewer, &id).await?,
        "declined",
    )
}

struct ApplicationDecision {
    jar: CookieJar,
    viewer: Viewer,
    dest: String,
}

async fn load_application_decision(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    csrf: &str,
) -> Result<ApplicationDecision, Response> {
    let signed = signed_in(state, jar).await?;
    let viewer = match viewer_for(&state.sdk.db, signed.user).await {
        Ok(viewer) => viewer,
        Err(error) => return Err(error.into_response()),
    };
    let Some(application) = state
        .sdk
        .db
        .application(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(signed.jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/needs/{}", application.need_id);
    if !signed.session.check_csrf(csrf) {
        return Err(with_cookie(signed.jar, super::context::fail_csrf(&dest)));
    }
    Ok(ApplicationDecision {
        jar: signed.jar,
        viewer,
        dest,
    })
}

async fn paint_need(
    state: &AppState,
    jar: CookieJar,
    viewer: Viewer,
    id: &str,
    flash: Option<views::Flash>,
    csrf: &str,
    draft: &views::OfferDraft<'_>,
) -> Result<Response, AppError> {
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let Some(card) = state.sdk.db.need_card(id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::sorry_page(
                "We couldn't find that need.",
                views::SorrySeat::Member {
                    user: &viewer.user,
                    unread: count,
                    csrf,
                },
            )),
        ));
    };
    let Some(church) = state.sdk.db.church(&card.church_id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::sorry_page(
                "We couldn't find that church.",
                views::SorrySeat::Member {
                    user: &viewer.user,
                    unread: count,
                    csrf,
                },
            )),
        ));
    };
    if let Err(error) = require_need_view(&viewer, card.sight(), &church) {
        return Ok(with_cookie(
            jar,
            html(views::sorry_page(
                &error.to_string(),
                views::SorrySeat::Member {
                    user: &viewer.user,
                    unread: count,
                    csrf,
                },
            )),
        ));
    }
    let applications = state.sdk.db.applications_for_need(&card.id).await?;
    let offers: Vec<_> = visible_offers(&viewer, card.sight(), &applications).collect();
    let offer = OfferState::of_existing(applications.iter().find(|a| a.user_id == viewer.user.id));
    let help = ecclesia_sdk::prelude::can_apply(&viewer, card.sight(), &church);
    Ok(with_cookie(
        jar,
        html(views::need_show(
            &viewer, &card, &church, &offers, help, offer, count, flash, csrf, draft,
        )),
    ))
}
