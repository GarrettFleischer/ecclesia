use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::views;
use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::prelude::{Place, Viewer, VoiceKind};
use ecclesia_sdk::story;

use super::context::{
    ClientKey, html, leaf_err, redirect_err, redirect_ok, signed_form, signed_in, story_redirect,
    unread, viewer_for, with_cookie,
};
use super::forms::{
    ChurchForm, CsrfForm, FlashQuery, InviteForm, JoinChurchForm, JoinQuery, RedeemForm,
};
use super::{AppError, AppState};

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
            &views::ChurchDraft::blank(),
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
    if state.awaiting_review(
        &form.pass,
        &[
            &form.name,
            &form.address,
            &form.gathering,
            &form.description,
        ],
    ) {
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
                    address: &form.address,
                    latitude: &form.latitude,
                    longitude: &form.longitude,
                    gathering: &form.gathering,
                    description: &description,
                    kind: views::DraftKind::Review,
                },
            )),
        ));
    }
    let (Some(latitude), Some(longitude)) =
        (parse_coord(&form.latitude), parse_coord(&form.longitude))
    else {
        return Ok(with_cookie(
            signed.jar,
            redirect_err("/churches/new", "missing"),
        ));
    };
    let ok = match story::plant_church(
        &state.sdk,
        &signed.user,
        &form.name,
        &form.address,
        latitude,
        longitude,
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
            &page.answered,
            page.next_need_cursor.as_deref(),
            page.next_member_cursor.as_deref(),
            views::flash_for(flash.ok, flash.err, Some(page.church.name.as_str())),
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
            let dest = format!("/churches/{}", ok.church_id.as_deref().unwrap_or_default());
            Ok(with_cookie(signed.jar, redirect_ok(&dest, "redeemed")))
        }
        Err(ecclesia_sdk::prelude::DomainError::NotFound) => {
            Ok(with_cookie(signed.jar, redirect_err("/churches", "invite")))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/churches", error))),
    }
}

pub async fn approve_membership_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((church_id, user_id)): Path<(String, String)>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_membership_decision(&state, jar, &church_id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::approve_membership(&state.sdk, &loaded.viewer, &user_id).await?,
        "approved",
    )
}

pub async fn decline_membership_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((church_id, user_id)): Path<(String, String)>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let loaded = match load_membership_decision(&state, jar, &church_id, &form.csrf).await {
        Ok(loaded) => loaded,
        Err(response) => return Ok(response),
    };
    story_redirect(
        loaded.jar,
        &loaded.dest,
        story::decline_membership(&state.sdk, &loaded.viewer, &user_id).await?,
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
    church_id: &str,
    csrf: &str,
) -> Result<MembershipDecision, Response> {
    let signed = signed_in(state, jar).await?;
    let dest = format!("/churches/{church_id}");
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
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/home").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    match story::accept_invite(&state.sdk, &signed.user).await? {
        Ok(ok) => {
            let dest = format!("/churches/{}", ok.church_id.as_deref().unwrap_or_default());
            Ok(with_cookie(
                signed.jar,
                redirect_ok(&dest, "invite_accepted"),
            ))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/home", error))),
    }
}

pub async fn join_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<JoinQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let church_name = viewer.church.as_ref().map(|church| church.name.as_str());
    let mut flash = views::flash_for(query.ok.clone(), query.err.clone(), church_name);
    let place = match read_coords(&query.lat, &query.lng) {
        Err(()) => {
            if flash.is_none() {
                flash = views::flash_from(None, Some("missing".to_string()));
            }
            None
        }
        Ok(Some((latitude, longitude))) => Some(Place { latitude, longitude }),
        Ok(None) => None,
    };
    let hits = match story::join_finder(
        &state.sdk,
        viewer.user.church_id.as_deref(),
        place,
        &query.q,
    )
    .await?
    {
        Ok(hits) => hits,
        Err(_) => {
            if flash.is_none() {
                flash = views::flash_from(None, Some("missing".to_string()));
            }
            Vec::new()
        }
    };
    Ok(with_cookie(
        signed.jar,
        html(views::join_church_page(
            &viewer,
            flash,
            count,
            &signed.session.csrf,
            &hits,
            &query.q,
            query.lat.trim(),
            query.lng.trim(),
        )),
    ))
}

pub async fn join_by_search(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<JoinChurchForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/churches/join").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    match story::request_join(&state.sdk, &signed.user, &form.church_id).await? {
        Ok(ok) => {
            let id = ok
                .church_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .unwrap_or(form.church_id.as_str());
            let dest = format!("/churches/{id}");
            Ok(with_cookie(
                signed.jar,
                redirect_ok(&dest, "joined_request"),
            ))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/churches/join", error))),
    }
}

pub async fn join_by_code(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<RedeemForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/churches/join").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if let RateDecision::Refuse = state.decide_rate(RateKind::Redeem, &who.0).await {
        return Ok(with_cookie(
            signed.jar,
            redirect_err("/churches/join", "rate"),
        ));
    }
    match story::redeem_invite(&state.sdk, &signed.user, &form.code).await? {
        Ok(ok) => {
            let Some(id) = ok.church_id.as_deref().filter(|id| !id.is_empty()) else {
                return Ok(with_cookie(
                    signed.jar,
                    redirect_ok("/churches/join", "redeemed"),
                ));
            };
            let dest = format!("/churches/{id}");
            Ok(with_cookie(signed.jar, redirect_ok(&dest, "redeemed")))
        }
        Err(ecclesia_sdk::prelude::DomainError::NotFound) => Ok(with_cookie(
            signed.jar,
            redirect_err("/churches/join", "invite"),
        )),
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/churches/join", error))),
    }
}

fn parse_coord(raw: &str) -> Option<f64> {
    let value = raw.trim().parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn read_coords(lat: &str, lng: &str) -> Result<Option<(f64, f64)>, ()> {
    let lat = lat.trim();
    let lng = lng.trim();
    if lat.is_empty() && lng.is_empty() {
        return Ok(None);
    }
    match (lat.parse::<f64>(), lng.parse::<f64>()) {
        (Ok(lat), Ok(lng)) if lat.is_finite() && lng.is_finite() => Ok(Some((lat, lng))),
        _ => Err(()),
    }
}
