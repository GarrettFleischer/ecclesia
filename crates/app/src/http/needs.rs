use axum::Json;
use axum::body::Body;
use axum::extract::{Form, FromRequest, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::views;
use ecclesia_sdk::media::{
    DeclaredFormat, MediaCaching, MediaDelivery, MediaError, MediaVariant, UploadLength,
    authorize_media_read, discard_staged, open_configured_store, stage_media, staged_ids,
};
use ecclesia_sdk::prelude::{
    AttachmentRef, NeedApproach, Place, Share, ShareKind, Viewer, VoiceKind, can_reply,
    can_view_need_near, coordinates, require_need_view,
};
use ecclesia_sdk::story::{self, ConversationError, StagedGrant, StagedMediaSight};

use super::context::{
    html, leaf_err, optional_gift_id, redirect_err, redirect_ok, signed_form, signed_in,
    story_redirect, unread, viewer_for, with_cookie,
};
use super::forms::{
    CsrfForm, FlashQuery, ImportNeedsForm, NeedForm, NeedQuery, ReplyForm, ShareMintForm,
};
use super::{AppError, AppState};

const STAGE_BYTE_LIMIT: usize = 12 * 1024 * 1024 + 256 * 1024;
const POST_BYTE_LIMIT: usize = 6 * 12 * 1024 * 1024 + 1024 * 1024;

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
            &[],
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
    request: Request,
) -> Result<Response, AppError> {
    let (parts, body) = request.into_parts();
    if is_multipart(&parts.headers) {
        let form = match read_multipart_body(&parts.headers, body, POST_BYTE_LIMIT).await {
            Ok(form) => form,
            Err(error) => return media_failure(&state, jar, "/needs/new", error).await,
        };
        return publish_need(&state, jar, NeedText::from_parts(&form), &form).await;
    }
    let request = Request::from_parts(parts, body);
    match Form::<NeedForm>::from_request(request, &state).await {
        Ok(Form(form)) => {
            publish_need(
                &state,
                jar,
                NeedText::from_urlencoded(&form),
                &IncomingForm::default(),
            )
            .await
        }
        Err(rejection) => Ok(rejection.into_response()),
    }
}

pub async fn need_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let place = shared_place(flash.lat.as_deref(), flash.lng.as_deref());
    let mark = need_mark(flash.ok.as_deref());
    paint_need(
        &state,
        jar,
        &id,
        views::flash_from(flash.ok, flash.err),
        mark,
        &views::OfferDraft::blank(),
        &[],
        place,
        flash.lat.as_deref(),
        flash.lng.as_deref(),
    )
    .await
}

pub async fn reply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    request: Request,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    accept_reply(&state, jar, &id, &dest, request, ReplyPost::Message).await
}

pub async fn complete_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    request: Request,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    accept_reply(&state, jar, &id, &dest, request, ReplyPost::Completion).await
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
            &id,
            None,
            views::NeedMark::None,
            &views::OfferDraft {
                message: &message,
                kind: views::DraftKind::Review,
                intent: views::ReplyIntent::Met,
            },
            &[],
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

pub async fn stage_media_http(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
) -> Result<Response, AppError> {
    let (parts, body) = request.into_parts();
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if !is_multipart(&parts.headers) {
        return Ok(with_cookie(
            signed.jar,
            json_sentence(StatusCode::BAD_REQUEST, "Fill in the required fields."),
        ));
    }
    let form = match read_multipart_body(&parts.headers, body, STAGE_BYTE_LIMIT).await {
        Ok(form) => form,
        Err(error) => return Ok(with_cookie(signed.jar, media_json(error))),
    };
    if !signed.session.check_csrf(field(&form, "csrf")) {
        return Ok(with_cookie(
            signed.jar,
            json_sentence(StatusCode::FORBIDDEN, "The form expired. Try again."),
        ));
    }
    let Some(file) = first_upload(&form) else {
        return Ok(with_cookie(
            signed.jar,
            json_sentence(StatusCode::BAD_REQUEST, "Fill in the required fields."),
        ));
    };
    match stage_bytes(
        &state.sdk.db,
        &signed.user.id,
        &file.content_type,
        &file.bytes,
    )
    .await
    {
        Ok(staged) => {
            let id = staged.id;
            Ok(with_cookie(
                signed.jar,
                Json(StagedBody {
                    full: format!("/media/{id}/full"),
                    thumb: format!("/media/{id}/thumb"),
                    width: staged.width,
                    height: staged.height,
                    id,
                }),
            ))
        }
        Err(error) => Ok(with_cookie(signed.jar, media_json(error))),
    }
}

pub async fn media_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((id, variant)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let Some(variant) = media_variant(&variant) else {
        return Ok(with_cookie(
            signed.jar,
            (StatusCode::NOT_FOUND, "Not found.").into_response(),
        ));
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let store = match open_configured_store() {
        Ok(store) => store,
        Err(error) => return Ok(with_cookie(signed.jar, media_plain(&error))),
    };
    match authorize_media_read(&state.sdk.db, &store, &viewer, &id, variant).await {
        Ok(delivery) => Ok(with_cookie(signed.jar, delivery_response(delivery))),
        Err(error) => Ok(with_cookie(signed.jar, media_plain(&error))),
    }
}

pub async fn media_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(target): Query<MediaTarget>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let csrf = csrf_from_request(&headers, &target.csrf);
    let fail = media_dest(&target);
    let signed = match signed_form(&state, jar, &csrf, &fail).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let need_id = target.need.trim();
    if !need_id.is_empty() {
        let viewer = viewer_for(&state.sdk.db, signed.user).await?;
        let dest = format!("/needs/{need_id}");
        return story_redirect(
            signed.jar,
            &dest,
            story::remove_need_photo(&state.sdk, &viewer, need_id, id.trim()).await?,
            "photo_removed",
        );
    }
    let reply_id = target.reply.trim();
    if !reply_id.is_empty() {
        let viewer = viewer_for(&state.sdk.db, signed.user).await?;
        let dest = reply_need_path(&state, reply_id).await?;
        return story_redirect(
            signed.jar,
            &dest,
            story::remove_reply_photo(&state.sdk, &viewer, reply_id, id.trim()).await?,
            "photo_removed",
        );
    }
    match discard_staged(&state.sdk.db, &signed.user.id, id.trim()).await {
        Ok(()) => Ok(with_cookie(signed.jar, StatusCode::NO_CONTENT)),
        Err(error) => Ok(with_cookie(signed.jar, media_json(error))),
    }
}

pub async fn need_events(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let place = shared_place(flash.lat.as_deref(), flash.lng.as_deref());
    let admitted = match admit_need(&state, jar, &id, place).await {
        Ok(admitted) => admitted,
        Err(response) => return Ok(response),
    };
    let after = last_event_id(&headers);
    let events = ecclesia_sdk::live::watch_need(&state.sdk, &id, after);
    let stream = events.map(notice_frame);
    Ok(with_cookie(admitted.jar, Sse::new(stream)))
}

pub async fn need_conversation(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let place = shared_place(flash.lat.as_deref(), flash.lng.as_deref());
    let admitted = match admit_need(&state, jar, &id, place).await {
        Ok(admitted) => admitted,
        Err(response) => return Ok(response),
    };
    let replies = load_replies(&state.sdk.db, &admitted.card.id).await?;
    let mut response = html(views::conversation_fragment(
        &admitted.card.id,
        &admitted.viewer.user.id,
        &replies,
        query_place(flash.lat.as_deref(), flash.lng.as_deref()),
    ))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(with_cookie(admitted.jar, response))
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

#[derive(Clone, Copy)]
enum ReplyPost {
    Message,
    Completion,
}

async fn accept_reply(
    state: &AppState,
    jar: CookieJar,
    need_id: &str,
    dest: &str,
    request: Request,
    post: ReplyPost,
) -> Result<Response, AppError> {
    let (parts, body) = request.into_parts();
    if is_multipart(&parts.headers) {
        let form = match read_multipart_body(&parts.headers, body, POST_BYTE_LIMIT).await {
            Ok(form) => form,
            Err(error) => return media_failure(state, jar, dest, error).await,
        };
        return publish_reply(
            state,
            jar,
            need_id,
            ReplyText::from_parts(&form),
            &form,
            post,
        )
        .await;
    }
    let request = Request::from_parts(parts, body);
    match Form::<ReplyForm>::from_request(request, state).await {
        Ok(Form(form)) => {
            publish_reply(
                state,
                jar,
                need_id,
                ReplyText::from_urlencoded(&form),
                &IncomingForm::default(),
                post,
            )
            .await
        }
        Err(rejection) => Ok(rejection.into_response()),
    }
}

async fn publish_need(
    state: &AppState,
    jar: CookieJar,
    text: NeedText,
    form: &IncomingForm,
) -> Result<Response, AppError> {
    let signed = match signed_form(state, jar, &text.csrf, "/needs/new").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let drafts = match photo_drafts(&state.sdk.db, &viewer.user.id, form).await {
        Ok(drafts) => drafts,
        Err(error) => return Ok(media_redirect(signed.jar, "/needs/new", error)),
    };
    if state.awaiting_review(&text.pass, &[&text.title, &text.body]) {
        let title = state.polish(VoiceKind::Need, &text.title).await;
        let body = state.polish(VoiceKind::Need, &text.body).await;
        let gifts = story::gift_catalog(&state.sdk).await?;
        let count = unread(&state.sdk.db, &viewer.user.id).await?;
        let kept = kept_photos(&drafts);
        return Ok(with_cookie(
            signed.jar,
            html(views::need_new(
                &viewer,
                &gifts,
                count,
                None,
                &signed.session.csrf,
                &views::NeedDraft {
                    church_id: &text.church_id,
                    title: &title,
                    body: &body,
                    gift_id: &text.gift_id,
                    scope: &text.scope,
                    kind: views::DraftKind::Review,
                },
                &kept,
            )),
        ));
    }
    let refs = attachment_refs(&drafts);
    let listed = match GrantList::load(&state.sdk.db, &viewer.user.id).await {
        Ok(listed) => listed,
        Err(error) => return Ok(media_redirect(signed.jar, "/needs/new", error)),
    };
    let grants = listed.grants();
    let sight = StagedMediaSight::granted(&viewer.user.id, &grants);
    match story::post_need_with_attachments(
        &state.sdk,
        &viewer,
        &text.church_id,
        &text.title,
        &text.body,
        optional_gift_id(&text.gift_id),
        &text.scope,
        &refs,
        &sight,
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
        Err(error) => Ok(with_cookie(
            signed.jar,
            conversation_redirect("/needs/new", error),
        )),
    }
}

async fn publish_reply(
    state: &AppState,
    jar: CookieJar,
    need_id: &str,
    text: ReplyText,
    form: &IncomingForm,
    post: ReplyPost,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{need_id}");
    let back = place_back(&dest, &text.lat, &text.lng);
    let signed = match signed_form(state, jar, &text.csrf, &back).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let drafts = match photo_drafts(&state.sdk.db, &viewer.user.id, form).await {
        Ok(drafts) => drafts,
        Err(error) => return Ok(media_redirect(signed.jar, &back, error)),
    };
    let post = posted_kind(post, &text.met);
    if state.awaiting_review(&text.pass, &[&text.body]) {
        let message = state.polish(VoiceKind::Reply, &text.body).await;
        let kept = kept_photos(&drafts);
        return paint_need(
            state,
            signed.jar,
            need_id,
            None,
            views::NeedMark::None,
            &views::OfferDraft {
                message: &message,
                kind: views::DraftKind::Review,
                intent: reply_intent(post),
            },
            &kept,
            shared_place(Some(&text.lat), Some(&text.lng)),
            Some(&text.lat),
            Some(&text.lng),
        )
        .await;
    }
    let refs = attachment_refs(&drafts);
    let listed = match GrantList::load(&state.sdk.db, &viewer.user.id).await {
        Ok(listed) => listed,
        Err(error) => return Ok(media_redirect(signed.jar, &back, error)),
    };
    let grants = listed.grants();
    let sight = StagedMediaSight::granted(&viewer.user.id, &grants);
    let place = shared_place(Some(&text.lat), Some(&text.lng));
    let result = match post {
        ReplyPost::Message => {
            story::reply_to_need_with_attachments(
                &state.sdk, &viewer, need_id, &text.body, place, &refs, &sight,
            )
            .await?
        }
        ReplyPost::Completion => {
            story::complete_need(&state.sdk, &viewer, need_id, &text.body, &refs, &sight).await?
        }
    };
    match result {
        Ok(_) => Ok(with_cookie(
            signed.jar,
            redirect_ok(&back, reply_flash(post)),
        )),
        Err(error) => Ok(with_cookie(signed.jar, conversation_redirect(&back, error))),
    }
}

fn posted_kind(post: ReplyPost, met: &str) -> ReplyPost {
    match post {
        ReplyPost::Message if met == "1" => ReplyPost::Completion,
        ReplyPost::Message | ReplyPost::Completion => post,
    }
}

fn reply_intent(post: ReplyPost) -> views::ReplyIntent {
    match post {
        ReplyPost::Message => views::ReplyIntent::Reply,
        ReplyPost::Completion => views::ReplyIntent::Met,
    }
}

fn reply_flash(post: ReplyPost) -> &'static str {
    match post {
        ReplyPost::Message => "replied",
        ReplyPost::Completion => "need_met",
    }
}

fn need_mark(ok: Option<&str>) -> views::NeedMark {
    match ok {
        Some("need_closed") | Some("need_met") => views::NeedMark::JustMet,
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

fn query_place<'a>(lat: Option<&'a str>, lng: Option<&'a str>) -> Option<(&'a str, &'a str)> {
    let pair = (lat?, lng?);
    if shared_place(Some(pair.0), Some(pair.1)).is_none() {
        return None;
    }
    Some(pair)
}

fn place_back(dest: &str, lat: &str, lng: &str) -> String {
    match shared_place(Some(lat), Some(lng)) {
        Some(place) => format!("{dest}?lat={}&lng={}", place.latitude, place.longitude),
        None => dest.to_string(),
    }
}

struct Admitted {
    jar: CookieJar,
    viewer: Viewer,
    csrf: String,
    card: ecclesia_sdk::prelude::NeedCard,
    church: ecclesia_sdk::prelude::Church,
    count: i64,
}

async fn admit_need(
    state: &AppState,
    jar: CookieJar,
    need_id: &str,
    place: Option<Place>,
) -> Result<Admitted, Response> {
    let signed = signed_in(state, jar).await?;
    let viewer = viewer_for(&state.sdk.db, signed.user)
        .await
        .map_err(IntoResponse::into_response)?;
    let count = unread(&state.sdk.db, &viewer.user.id)
        .await
        .map_err(IntoResponse::into_response)?;
    let csrf = signed.session.csrf.clone();
    let jar = signed.jar;
    let Some(card) = state
        .sdk
        .db
        .need_card(need_id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(member_sorry(
            jar,
            &viewer,
            count,
            &csrf,
            "We couldn't find that need.",
        ));
    };
    let Some(church) = state
        .sdk
        .db
        .church(&card.church_id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(member_sorry(
            jar,
            &viewer,
            count,
            &csrf,
            "We couldn't find that church.",
        ));
    };
    let member_view = require_need_view(&viewer, card.sight(), &church);
    let seen = member_view.is_ok()
        || place.is_some_and(|point| can_view_need_near(&viewer, card.sight(), &church, point));
    if !seen {
        let error = member_view.unwrap_err();
        return Err(member_sorry(jar, &viewer, count, &csrf, &error.to_string()));
    }
    Ok(Admitted {
        jar,
        viewer,
        csrf,
        card,
        church,
        count,
    })
}

fn member_sorry(
    jar: CookieJar,
    viewer: &Viewer,
    count: i64,
    csrf: &str,
    message: &str,
) -> Response {
    with_cookie(
        jar,
        html(views::sorry_page(
            message,
            views::SorrySeat::Member {
                user: &viewer.user,
                unread: count,
                csrf,
            },
        )),
    )
}

async fn paint_need(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    flash: Option<views::Flash>,
    mark: views::NeedMark,
    draft: &views::OfferDraft<'_>,
    kept: &[views::KeptPhoto<'_>],
    place: Option<Place>,
    lat: Option<&str>,
    lng: Option<&str>,
) -> Result<Response, AppError> {
    let admitted = match admit_need(state, jar, id, place).await {
        Ok(admitted) => admitted,
        Err(response) => return Ok(response),
    };
    let replies = load_replies(&state.sdk.db, &admitted.card.id).await?;
    let need_photos = state.sdk.db.need_attachments(&admitted.card.id).await?;
    let author_avatar = avatar_media_id(&state.sdk.db, &admitted.card.author_id).await?;
    let member_view = require_need_view(&admitted.viewer, admitted.card.sight(), &admitted.church);
    let approach = if member_view.is_ok() {
        NeedApproach::Membership
    } else if let Some(point) = place {
        NeedApproach::Near(point)
    } else {
        NeedApproach::Membership
    };
    let help = can_reply(
        &admitted.viewer,
        admitted.card.sight(),
        &admitted.church,
        approach,
    );
    let fields = query_place(lat, lng);
    let (share_url, share_mint) = share_link(&state.sdk.db, &admitted.card.id).await?;
    Ok(with_cookie(
        admitted.jar,
        html(views::need_show(
            &admitted.viewer,
            &admitted.card,
            &admitted.church,
            &need_photos,
            author_avatar.as_deref(),
            &replies,
            help,
            admitted.count,
            flash,
            &admitted.csrf,
            draft,
            kept,
            fields,
            &share_url,
            &share_mint,
            mark,
        )),
    ))
}

async fn load_replies(
    db: &ecclesia_sdk::Db,
    need_id: &str,
) -> Result<Vec<views::LoadedReply>, AppError> {
    let cards = db.need_replies(need_id).await?;
    let mut loaded = Vec::with_capacity(cards.len());
    for card in cards {
        let photos = db.reply_attachments(&card.id).await?;
        let avatar_media_id = avatar_media_id(db, &card.author_id).await?;
        loaded.push(views::LoadedReply {
            card,
            photos,
            avatar_media_id,
        });
    }
    Ok(loaded)
}

async fn avatar_media_id(db: &ecclesia_sdk::Db, user_id: &str) -> Result<Option<String>, AppError> {
    Ok(match db.avatar_reference(user_id).await? {
        Some(ecclesia_sdk::db::AvatarReference::Photo(id)) => Some(id),
        _ => None,
    })
}

async fn reply_need_path(state: &AppState, reply_id: &str) -> Result<String, AppError> {
    let Some(reply) = state.sdk.db.need_reply(reply_id).await? else {
        return Ok("/home".into());
    };
    Ok(format!("/needs/{}", reply.need_id))
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

fn notice_frame(event: ecclesia_sdk::live::WatchEvent) -> Result<Event, Infallible> {
    Ok(match event {
        ecclesia_sdk::live::WatchEvent::Reply(notice) => reply_event(notice),
        ecclesia_sdk::live::WatchEvent::Refresh => Event::default().event("refresh").data("{}"),
    })
}

fn reply_event(notice: ecclesia_sdk::live::ReplyNotice) -> Event {
    let payload = format!(r#"{{"reply_id":"{}"}}"#, json_escape(&notice.reply_id));
    Event::default().id(notice.stream_id).data(payload)
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn last_event_id(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("last-event-id")?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[derive(Serialize)]
struct StagedBody {
    id: String,
    full: String,
    thumb: String,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct PhotoError<'a> {
    error: &'a str,
}

#[derive(Deserialize, Default)]
pub(crate) struct MediaTarget {
    #[serde(default)]
    need: String,
    #[serde(default)]
    reply: String,
    #[serde(default)]
    csrf: String,
}

fn media_dest(target: &MediaTarget) -> String {
    let need = target.need.trim();
    if need.is_empty() {
        "/home".into()
    } else {
        format!("/needs/{need}")
    }
}

fn media_variant(value: &str) -> Option<MediaVariant> {
    match value {
        "full" => Some(MediaVariant::Full),
        "thumb" => Some(MediaVariant::Thumb),
        _ => None,
    }
}

fn media_status(error: &MediaError) -> StatusCode {
    match error {
        MediaError::StorageUnavailable | MediaError::Store => StatusCode::SERVICE_UNAVAILABLE,
        MediaError::NotFound => StatusCode::NOT_FOUND,
        MediaError::NotAllowed | MediaError::NotOwned => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_REQUEST,
    }
}

fn media_sentence(error: &MediaError) -> &'static str {
    views::photo_error_sentence(views::media_flash_code(error)).unwrap_or("That didn't work.")
}

fn media_json(error: MediaError) -> Response {
    (
        media_status(&error),
        Json(PhotoError {
            error: media_sentence(&error),
        }),
    )
        .into_response()
}

fn media_plain(error: &MediaError) -> Response {
    (media_status(error), media_sentence(error)).into_response()
}

fn media_redirect(jar: CookieJar, dest: &str, error: MediaError) -> Response {
    with_cookie(jar, redirect_err(dest, views::media_flash_code(&error)))
}

fn json_sentence(status: StatusCode, sentence: &'static str) -> Response {
    (status, Json(PhotoError { error: sentence })).into_response()
}

async fn media_failure(
    state: &AppState,
    jar: CookieJar,
    dest: &str,
    error: MediaError,
) -> Result<Response, AppError> {
    let signed = match signed_in(state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    Ok(media_redirect(signed.jar, dest, error))
}

fn conversation_redirect(path: &str, error: ConversationError) -> Redirect {
    match error {
        ConversationError::Domain(error) => leaf_err(path, error),
        ConversationError::UngrantedMedia { .. } => redirect_err(path, "not_owned"),
    }
}

fn delivery_response(delivery: MediaDelivery) -> Response {
    match delivery {
        MediaDelivery::Stream {
            bytes,
            content_type,
            caching,
        } => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
                (header::CACHE_CONTROL, cache_value(caching)),
            ],
            bytes,
        )
            .into_response(),
        MediaDelivery::Presigned { url, caching, .. } => {
            let mut response = Redirect::temporary(&url).into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, cache_value(caching));
            response
        }
    }
}

fn cache_value(caching: MediaCaching) -> HeaderValue {
    match caching {
        MediaCaching::NoStore => HeaderValue::from_static("no-store"),
        MediaCaching::Private { seconds } => {
            HeaderValue::from_str(&format!("private, max-age={seconds}"))
                .unwrap_or_else(|_| HeaderValue::from_static("private, max-age=300"))
        }
    }
}

struct NeedText {
    csrf: String,
    church_id: String,
    title: String,
    body: String,
    gift_id: String,
    scope: String,
    pass: String,
}

impl NeedText {
    fn from_urlencoded(form: &NeedForm) -> Self {
        Self {
            csrf: form.csrf.clone(),
            church_id: form.church_id.clone(),
            title: form.title.clone(),
            body: form.body.clone(),
            gift_id: form.gift_id.clone(),
            scope: form.scope.clone(),
            pass: form.pass.clone(),
        }
    }

    fn from_parts(form: &IncomingForm) -> Self {
        Self {
            csrf: field(form, "csrf").to_string(),
            church_id: field(form, "church_id").to_string(),
            title: field(form, "title").to_string(),
            body: field(form, "body").to_string(),
            gift_id: field(form, "gift_id").to_string(),
            scope: field(form, "scope").to_string(),
            pass: field(form, "pass").to_string(),
        }
    }
}

struct ReplyText {
    csrf: String,
    body: String,
    pass: String,
    lat: String,
    lng: String,
    met: String,
}

impl ReplyText {
    fn from_urlencoded(form: &ReplyForm) -> Self {
        Self {
            csrf: form.csrf.clone(),
            body: form.body.clone(),
            pass: form.pass.clone(),
            lat: form.lat.clone(),
            lng: form.lng.clone(),
            met: form.met.clone(),
        }
    }

    fn from_parts(form: &IncomingForm) -> Self {
        Self {
            csrf: field(form, "csrf").to_string(),
            body: field(form, "body").to_string(),
            pass: field(form, "pass").to_string(),
            lat: field(form, "lat").to_string(),
            lng: field(form, "lng").to_string(),
            met: field(form, "met").to_string(),
        }
    }
}

struct PhotoDraft {
    media_id: String,
    description: Option<String>,
}

pub(crate) struct GrantList {
    ids: Vec<String>,
}

impl GrantList {
    pub(crate) async fn load(db: &ecclesia_sdk::Db, owner_id: &str) -> Result<Self, MediaError> {
        Ok(Self {
            ids: staged_ids(db, owner_id).await?,
        })
    }

    pub(crate) fn grants(&self) -> Vec<StagedGrant<'_>> {
        let mut grants = Vec::with_capacity(self.ids.len());
        for id in &self.ids {
            grants.push(StagedGrant { media_id: id });
        }
        grants
    }
}

#[derive(Default)]
pub(crate) struct IncomingForm {
    pub fields: Vec<(String, String)>,
    pub files: Vec<IncomingFile>,
}

pub(crate) struct IncomingFile {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub(crate) fn field<'a>(form: &'a IncomingForm, name: &str) -> &'a str {
    form.fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

pub(crate) fn is_multipart(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("multipart/form-data")
        })
}

pub(crate) fn csrf_from_request(headers: &HeaderMap, fallback: &str) -> String {
    if let Some(value) = headers
        .get("x-csrf")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return value.to_string();
    }
    fallback.to_string()
}

pub(crate) async fn read_multipart_body(
    headers: &HeaderMap,
    body: Body,
    limit: usize,
) -> Result<IncomingForm, MediaError> {
    let bytes = take_body(body, limit).await?;
    parse_multipart(&bytes, &content_type_of(headers))
}

pub(crate) async fn one_staged_photo(
    db: &ecclesia_sdk::Db,
    owner_id: &str,
    form: &IncomingForm,
) -> Result<Option<String>, MediaError> {
    let drafts = photo_drafts(db, owner_id, form).await?;
    Ok(drafts.into_iter().next().map(|draft| draft.media_id))
}

async fn photo_drafts(
    db: &ecclesia_sdk::Db,
    owner_id: &str,
    form: &IncomingForm,
) -> Result<Vec<PhotoDraft>, MediaError> {
    let staged = staged_drafts(form);
    if !staged.is_empty() {
        return Ok(staged);
    }
    stage_uploaded(db, owner_id, form).await
}

pub(crate) async fn stage_bytes(
    db: &ecclesia_sdk::Db,
    owner_id: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<ecclesia_sdk::media::StagedMedia, MediaError> {
    if bytes.len() as u64 > ecclesia_sdk::media::MAX_UPLOAD_BYTES {
        return Err(MediaError::TooManyBytes);
    }
    let store = open_configured_store()?;
    stage_media(
        db,
        &store,
        owner_id,
        DeclaredFormat::from_mime(content_type),
        UploadLength::Declared(bytes.len() as u64),
        std::io::Cursor::new(bytes),
    )
    .await
}

pub(crate) const AVATAR_BYTE_LIMIT: usize = STAGE_BYTE_LIMIT;

fn kept_photos(drafts: &[PhotoDraft]) -> Vec<views::KeptPhoto<'_>> {
    let mut kept = Vec::with_capacity(drafts.len());
    for draft in drafts {
        kept.push(views::KeptPhoto {
            id: &draft.media_id,
            description: draft.description.as_deref().unwrap_or(""),
        });
    }
    kept
}

fn attachment_refs(drafts: &[PhotoDraft]) -> Vec<AttachmentRef<'_>> {
    let mut refs = Vec::with_capacity(drafts.len());
    for draft in drafts {
        refs.push(AttachmentRef {
            media_id: &draft.media_id,
            description: draft.description.as_deref(),
        });
    }
    refs
}

fn staged_drafts(form: &IncomingForm) -> Vec<PhotoDraft> {
    let mut drafts = Vec::new();
    let mut open = None;
    for (name, value) in &form.fields {
        match name.as_str() {
            "staged_id" => {
                if let Some(media_id) = open.take() {
                    drafts.push(PhotoDraft {
                        media_id,
                        description: None,
                    });
                }
                if let Some(media_id) = nonempty(value) {
                    open = Some(media_id.to_string());
                }
            }
            "description" => {
                if let Some(media_id) = open.take() {
                    drafts.push(PhotoDraft {
                        media_id,
                        description: blank_description(value),
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(media_id) = open {
        drafts.push(PhotoDraft {
            media_id,
            description: None,
        });
    }
    drafts
}

async fn stage_uploaded(
    db: &ecclesia_sdk::Db,
    owner_id: &str,
    form: &IncomingForm,
) -> Result<Vec<PhotoDraft>, MediaError> {
    let mut drafts = Vec::new();
    for file in &form.files {
        if file.name != "photos" && file.name != "photo" {
            continue;
        }
        let staged = stage_bytes(db, owner_id, &file.content_type, &file.bytes).await?;
        drafts.push(PhotoDraft {
            media_id: staged.id,
            description: None,
        });
    }
    Ok(drafts)
}

fn first_upload(form: &IncomingForm) -> Option<&IncomingFile> {
    form.files
        .iter()
        .find(|file| file.name == "photo" || file.name == "photos")
}

fn nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn blank_description(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn take_body(body: Body, limit: usize) -> Result<Vec<u8>, MediaError> {
    let bytes = axum::body::to_bytes(body, limit)
        .await
        .map_err(|_| MediaError::TooManyBytes)?;
    let mut owned = Vec::with_capacity(bytes.len());
    owned.extend_from_slice(&bytes);
    Ok(owned)
}

fn content_type_of(headers: &HeaderMap) -> String {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn parse_multipart(body: &[u8], content_type: &str) -> Result<IncomingForm, MediaError> {
    let boundary = boundary_of(content_type).ok_or(MediaError::Unreadable)?;
    let marker = format!("--{boundary}");
    let marker = marker.as_bytes();
    let mut form = IncomingForm::default();
    let mut rest = body;
    let Some(start) = find_subslice(rest, marker) else {
        return Err(MediaError::Unreadable);
    };
    rest = &rest[start + marker.len()..];
    loop {
        if rest.starts_with(b"--") {
            break;
        }
        if let Some(stripped) = rest.strip_prefix(b"\r\n") {
            rest = stripped;
        }
        let Some(next) = find_subslice(rest, marker) else {
            break;
        };
        let mut part = &rest[..next];
        if let Some(stripped) = part.strip_suffix(b"\r\n") {
            part = stripped;
        }
        push_part(&mut form, part)?;
        rest = &rest[next + marker.len()..];
    }
    Ok(form)
}

fn push_part(form: &mut IncomingForm, part: &[u8]) -> Result<(), MediaError> {
    let Some(split) = find_subslice(part, b"\r\n\r\n") else {
        return Ok(());
    };
    let headers = String::from_utf8_lossy(&part[..split]);
    let body = &part[split + 4..];
    let disposition = header_value(&headers, "content-disposition").unwrap_or("");
    let name = disposition_param(disposition, "name").unwrap_or("");
    if name.is_empty() {
        return Ok(());
    }
    if disposition_param(disposition, "filename").is_some() {
        if body.is_empty() {
            return Ok(());
        }
        if body.len() as u64 > ecclesia_sdk::media::MAX_UPLOAD_BYTES {
            return Err(MediaError::TooManyBytes);
        }
        let mut bytes = Vec::with_capacity(body.len());
        bytes.extend_from_slice(body);
        form.files.push(IncomingFile {
            name: name.to_string(),
            content_type: header_value(&headers, "content-type")
                .unwrap_or("")
                .trim()
                .to_string(),
            bytes,
        });
        return Ok(());
    }
    form.fields
        .push((name.to_string(), String::from_utf8_lossy(body).into_owned()));
    Ok(())
}

fn boundary_of(content_type: &str) -> Option<String> {
    for piece in content_type.split(';') {
        let piece = piece.trim();
        let Some((name, value)) = piece.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("boundary") {
            continue;
        }
        let raw = value.trim().trim_matches('"');
        if raw.is_empty() {
            return None;
        }
        return Some(raw.to_string());
    }
    None
}

fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    for line in headers.split("\r\n") {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(value.trim());
        }
    }
    None
}

fn disposition_param<'a>(header: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=");
    let start = header.find(&needle)? + needle.len();
    let rest = &header[start..];
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(&stripped[..end]);
    }
    Some(rest.split([';', ' ', '\r', '\n']).next().unwrap_or(""))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
