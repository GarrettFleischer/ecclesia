use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use ecclesia_sdk::prelude::{Viewer, VoiceKind};
use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::story;
use crate::views;

use super::context::{
    ClientKey, html, leaf_err, redirect_err, story_redirect,
    redirect_ok, signed_form, signed_in, unread, viewer_for, with_cookie,
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let page = story::church_directory(&state.sdk, flash.after.as_deref()).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::churches_index(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &page.cards,
            page.next_cursor.as_deref(),
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
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
        let count = unread(&state.sdk.db, &signed.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::church_new(
                &viewer_for(&state.sdk.db, signed.user).await?,
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
    let ok = match story::plant_church(
        &state.sdk,
        &signed.user,
        &form.name,
        &form.city,
        &form.region,
        &form.description,
        &form.gathering,
    )
    .await?
    {
        Ok(ok) => ok,
        Err(error) => return Ok(with_cookie(signed.jar, leaf_err("/churches/new", error))),
    };
    let Some(church_id) = ok.church_id else {
        return Ok(with_cookie(
            signed.jar,
            redirect_err("/churches/new", "missing"),
        ));
    };
    let dest = format!("/churches/{church_id}");
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
    let Some(page) = story::church_show(
        &state.sdk,
        &id,
        flash.after.as_deref(),
        flash.members_after.as_deref(),
    )
    .await?
    else {
        let count = unread(&state.sdk.db, &signed.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::sorry_page(
                "We couldn't find that church.",
                views::SorrySeat::Member {
                    user: &signed.user,
                    unread: count,
                    csrf: &signed.session.csrf,
                },
            )),
        ));
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::church_show(
            &viewer,
            &page.church,
            &page.members,
            &page.needs,
            page.next_need_cursor.as_deref(),
            page.next_member_cursor.as_deref(),
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
    story_redirect(
        signed.jar,
        &dest,
        story::request_join(&state.sdk, &signed.user, &id).await?,
        "joined_request",
    )
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
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    story_redirect(
        signed.jar,
        &dest,
        story::invite_member(
            &state.sdk,
            &viewer,
            &id,
            &form.email,
            &story::MailOrigin {
                origin: state.origin.clone(),
            },
        )
        .await?,
        "invited",
    )
}

pub async fn redeem(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<RedeemForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/churches").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if let RateDecision::Refuse = state.decide_rate(RateKind::Redeem, &who.0).await {
        return Ok(with_cookie(signed.jar, redirect_err("/churches", "rate")));
    }
    match story::redeem_invite(&state.sdk, &signed.user, &form.code).await? {
        Ok(ok) => {
            let dest = format!(
                "/churches/{}",
                ok.church_id.as_deref().unwrap_or_default()
            );
            Ok(with_cookie(signed.jar, redirect_ok(&dest, "redeemed")))
        }
        Err(ecclesia_sdk::prelude::DomainError::NotFound) => {
            Ok(with_cookie(signed.jar, redirect_err("/churches", "invite")))
        }
        Err(error) => {
            Ok(with_cookie(signed.jar, leaf_err("/churches", error)))
        }
    }
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
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::approve_membership(&state.sdk, &loaded.viewer, &id).await?,
        "approved",
    )
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
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::decline_membership(&state.sdk, &loaded.viewer, &id).await?,
        "declined",
    )
}

struct MembershipDecision {
    jar: CookieJar,
    viewer: Viewer,
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
        .sdk
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
    let viewer = viewer_for(&state.sdk.db, signed.user)
        .await
        .map_err(|error| error.into_response())?;
    Ok(MembershipDecision {
        jar: signed.jar,
        viewer,
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
    match story::accept_invite(&state.sdk, &signed.user, &id).await? {
        Ok(ok) => {
            let dest = format!(
                "/churches/{}",
                ok.church_id.as_deref().unwrap_or_default()
            );
            Ok(with_cookie(
                signed.jar,
                redirect_ok(&dest, "invite_accepted"),
            ))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/home", error))),
    }
}
