use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::views;
use ecclesia_sdk::prelude::{
    NeedApproach, Place, Share, ShareKind, Viewer, VoiceKind, can_reply, can_view_need_near,
    coordinates, require_need_view,
};
use ecclesia_sdk::story;

use super::context::{
    html, leaf_err, optional_gift_id, redirect_err, redirect_ok, signed_form, signed_in,
    story_redirect, unread, viewer_for, with_cookie,
};
use super::forms::{
    CsrfForm, FlashQuery, ImportNeedsForm, NeedForm, NeedQuery, ReplyForm, ShareMintForm,
};
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

pub async fn import_needs_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ImportNeedsForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/home").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    story_redirect(
        signed.jar,
        "/home",
        story::import_open_needs(
            &state.sdk,
            &signed.user,
            &form.source_church_id,
            &form.church_id,
        )
        .await?,
        "moved",
    )
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
    let mark = need_mark(flash.ok.as_deref());
    paint_need(
        &state,
        signed.jar,
        viewer,
        &id,
        views::flash_from(flash.ok, flash.err),
        mark,
        &signed.session.csrf,
        &views::OfferDraft::blank(),
        shared_place(flash.lat.as_deref(), flash.lng.as_deref()),
        flash.lat.as_deref(),
        flash.lng.as_deref(),
    )
    .await
}

pub async fn reply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ReplyForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.body]) {
        let message = state.polish(VoiceKind::Reply, &form.body).await;
        return paint_need(
            &state,
            signed.jar,
            viewer,
            &id,
            None,
            views::NeedMark::None,
            &signed.session.csrf,
            &views::OfferDraft {
                message: &message,
                kind: views::DraftKind::Review,
                intent: views::ReplyIntent::Reply,
            },
            shared_place(Some(&form.lat), Some(&form.lng)),
            Some(&form.lat),
            Some(&form.lng),
        )
        .await;
    }
    let back = place_back(&dest, &form.lat, &form.lng);
    story_redirect(
        signed.jar,
        &back,
        story::reply_to_need(
            &state.sdk,
            &viewer,
            &id,
            &form.body,
            shared_place(Some(&form.lat), Some(&form.lng)),
        )
        .await?,
        "replied",
    )
}

pub async fn close_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ReplyForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.body]) {
        let message = state.polish(VoiceKind::Praise, &form.body).await;
        return paint_need(
            &state,
            signed.jar,
            viewer,
            &id,
            None,
            views::NeedMark::None,
            &signed.session.csrf,
            &views::OfferDraft {
                message: &message,
                kind: views::DraftKind::Review,
                intent: views::ReplyIntent::Met,
            },
            shared_place(Some(&form.lat), Some(&form.lng)),
            Some(&form.lat),
            Some(&form.lng),
        )
        .await;
    }
    story_redirect(
        signed.jar,
        &dest,
        story::close_need(&state.sdk, &viewer, &id, &form.body).await?,
        "need_closed",
    )
}

pub async fn reopen_need_http(
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
        story::reopen_need(&state.sdk, &viewer, &id).await?,
        "need_reopened",
    )
}

fn need_mark(ok: Option<&str>) -> views::NeedMark {
    match ok {
        Some("need_closed") => views::NeedMark::JustMet,
        Some("need_reopened") => views::NeedMark::Reopened,
        _ => views::NeedMark::None,
    }
}

fn shared_place(lat: Option<&str>, lng: Option<&str>) -> Option<Place> {
    let latitude = lat?.parse().ok()?;
    let longitude = lng?.parse().ok()?;
    coordinates(latitude, longitude)
        .ok()
        .map(|(latitude, longitude)| Place {
            latitude,
            longitude,
        })
}

fn place_back(dest: &str, lat: &str, lng: &str) -> String {
    match shared_place(Some(lat), Some(lng)) {
        Some(place) => format!("{dest}?lat={}&lng={}", place.latitude, place.longitude),
        None => dest.to_string(),
    }
}

async fn paint_need(
    state: &AppState,
    jar: CookieJar,
    viewer: Viewer,
    id: &str,
    flash: Option<views::Flash>,
    mark: views::NeedMark,
    csrf: &str,
    draft: &views::OfferDraft<'_>,
    place: Option<Place>,
    lat: Option<&str>,
    lng: Option<&str>,
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
    let member_view = require_need_view(&viewer, card.sight(), &church);
    let seen = member_view.is_ok()
        || place.is_some_and(|point| can_view_need_near(&viewer, card.sight(), &church, point));
    if !seen {
        let error = member_view.unwrap_err();
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
    let approach = if member_view.is_ok() {
        NeedApproach::Membership
    } else if let Some(point) = place {
        NeedApproach::Near(point)
    } else {
        NeedApproach::Membership
    };
    let help = can_reply(&viewer, card.sight(), &church, approach);
    let replies = state.sdk.db.need_replies(&card.id).await?;
    let fields = match (place, lat, lng) {
        (Some(_), Some(lat), Some(lng)) => Some((lat, lng)),
        _ => None,
    };
    let (share_url, share_mint) = share_link(&state.sdk.db, &card.id).await?;
    Ok(with_cookie(
        jar,
        html(views::need_show(
            &viewer,
            &card,
            &church,
            &replies,
            help,
            count,
            flash,
            csrf,
            draft,
            fields,
            &share_url,
            &share_mint,
            mark,
        )),
    ))
}

pub async fn share_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ShareMintForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    match story::share_need(
        &state.sdk,
        &viewer,
        &id,
        shared_place(Some(&form.lat), Some(&form.lng)),
    )
    .await?
    {
        Ok(path) => Ok(with_cookie(signed.jar, path)),
        Err(error) => Ok(with_cookie(signed.jar, leaf_err(&dest, error))),
    }
}

pub async fn open_share(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Response, AppError> {
    let Some(share) = state.sdk.db.share_by_code(&code).await? else {
        return Ok(missing_link());
    };
    let Some(path) = share_location(&share) else {
        return Ok(missing_link());
    };
    Ok(Redirect::to(&path).into_response())
}

async fn share_link(
    db: &ecclesia_sdk::db::Db,
    need_id: &str,
) -> Result<(String, String), AppError> {
    let Some(share) = db.share_for_target(ShareKind::Need, need_id).await? else {
        return Ok((String::new(), format!("/needs/{need_id}/share")));
    };
    Ok((format!("/s/{}", share.code), String::new()))
}

fn share_location(share: &Share) -> Option<String> {
    match ShareKind::parse(&share.kind)? {
        ShareKind::Need => Some(format!("/needs/{}", share.target_id)),
    }
}

fn missing_link() -> Response {
    html(views::error_page("We couldn't find that link.")).into_response()
}
