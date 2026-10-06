use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::views;
use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::prelude::{
    Place, Viewer, VoiceKind, VoicePass, postal_address, service_schedule,
};
use ecclesia_sdk::story;

use super::context::{
    ClientKey, SignedIn, html, leaf_err, redirect_err, redirect_ok, signed_form, signed_in,
    story_redirect, unread, viewer_for, with_cookie,
};
use super::forms::{
    ChurchForm, ChurchPost, CsrfForm, FlashQuery, JoinChurchForm, JoinQuery, RedeemForm,
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
    let gathering = match service_schedule(&service_days(&form), &service_times(&form)) {
        Ok(gathering) => gathering,
        Err(error) => {
            let draft = church_draft(&form, &form.description, submitted_kind(&form.pass));
            return render_church_form(
                &state,
                signed,
                views::flash_from(None, Some(error.flash_code().to_string())),
                &draft,
            )
            .await;
        }
    };
    if state.awaiting_review(
        &form.pass,
        &[
            &form.name,
            &form.address_line1,
            &form.city,
            &gathering,
            &form.description,
        ],
    ) {
        let description = state.polish(VoiceKind::Church, &form.description).await;
        let draft = church_draft(&form, &description, views::DraftKind::Review);
        return render_church_form(&state, signed, None, &draft).await;
    }
    let address = match postal_address(
        &form.address_line1,
        &form.address_line2,
        &form.city,
        &form.address_state,
        &form.postal_code,
    ) {
        Ok(address) => address,
        Err(error) => {
            let draft = church_draft(&form, &form.description, submitted_kind(&form.pass));
            return render_church_form(
                &state,
                signed,
                views::flash_from(None, Some(error.flash_code().to_string())),
                &draft,
            )
            .await;
        }
    };
    let place = match state.sdk.locate_address(&address).await {
        Ok(Some(place)) => place,
        Ok(None) => {
            let draft = church_draft(&form, &form.description, submitted_kind(&form.pass));
            return render_church_form(
                &state,
                signed,
                views::flash_from(None, Some("address".into())),
                &draft,
            )
            .await;
        }
        Err(error) => {
            tracing::warn!("address lookup failed: {error:#}");
            let draft = church_draft(&form, &form.description, submitted_kind(&form.pass));
            return render_church_form(
                &state,
                signed,
                views::flash_from(None, Some("address".into())),
                &draft,
            )
            .await;
        }
    };
    let ok = match story::plant_church(
        &state.sdk,
        &signed.user,
        &form.name,
        &address,
        place.latitude,
        place.longitude,
        &form.description,
        &gathering,
        &form.ein,
        &form.registry_state,
        &form.registry_number,
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
    let movable = state
        .sdk
        .db
        .open_needs_from_closed_churches(&viewer.user.id)
        .await?;
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
            &movable,
            views::flash_for(flash.ok, flash.err, Some(page.church.name.as_str())),
            count,
            &signed.session.csrf,
            &state.origin,
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
    body: String,
) -> Result<Response, AppError> {
    let form = super::forms::invite_form(&body);
    let dest = format!("/churches/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    story_redirect(
        signed.jar,
        &dest,
        story::invite_members(
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
        story::approve_membership(&state.sdk, &loaded.viewer, &church_id, &user_id).await?,
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
        story::decline_membership(&state.sdk, &loaded.viewer, &church_id, &user_id).await?,
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
    Form(form): Form<ChurchPost>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/home").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    match story::accept_invite(&state.sdk, &signed.user, &form.church_id).await? {
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
    let church_name = viewer.churches.first().map(|church| church.name.as_str());
    let mut flash = views::flash_for(query.ok.clone(), query.err.clone(), church_name);
    let place = match read_coords(&query.lat, &query.lng) {
        Err(()) => {
            if flash.is_none() {
                flash = views::flash_from(None, Some("missing".to_string()));
            }
            None
        }
        Ok(Some((latitude, longitude))) => Some(Place {
            latitude,
            longitude,
        }),
        Ok(None) => None,
    };
    let hits = match story::join_finder(
        &state.sdk,
        &viewer
            .user
            .memberships
            .iter()
            .map(|link| link.church_id.as_str())
            .collect::<Vec<_>>(),
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
    let movable = state
        .sdk
        .db
        .open_needs_from_closed_churches(&viewer.user.id)
        .await?;
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
            &movable,
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

fn church_draft<'a>(
    form: &'a ChurchForm,
    description: &'a str,
    kind: views::DraftKind,
) -> views::ChurchDraft<'a> {
    views::ChurchDraft {
        name: &form.name,
        address_line1: &form.address_line1,
        address_line2: &form.address_line2,
        city: &form.city,
        address_state: &form.address_state,
        postal_code: &form.postal_code,
        ein: &form.ein,
        registry_state: &form.registry_state,
        registry_number: &form.registry_number,
        service_days: service_days(form),
        service_times: service_times(form),
        description,
        kind,
    }
}

fn service_days(form: &ChurchForm) -> [&str; 8] {
    [
        form.service_day_0.as_str(),
        form.service_day_1.as_str(),
        form.service_day_2.as_str(),
        form.service_day_3.as_str(),
        form.service_day_4.as_str(),
        form.service_day_5.as_str(),
        form.service_day_6.as_str(),
        form.service_day_7.as_str(),
    ]
}

fn service_times(form: &ChurchForm) -> [&str; 8] {
    [
        form.service_time_0.as_str(),
        form.service_time_1.as_str(),
        form.service_time_2.as_str(),
        form.service_time_3.as_str(),
        form.service_time_4.as_str(),
        form.service_time_5.as_str(),
        form.service_time_6.as_str(),
        form.service_time_7.as_str(),
    ]
}

fn submitted_kind(pass: &str) -> views::DraftKind {
    match VoicePass::parse(pass) {
        VoicePass::Publish => views::DraftKind::Review,
        VoicePass::Review => views::DraftKind::Blank,
    }
}

async fn render_church_form(
    state: &AppState,
    signed: SignedIn,
    flash: Option<views::Flash>,
    draft: &views::ChurchDraft<'_>,
) -> Result<Response, AppError> {
    let csrf = signed.session.csrf.clone();
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::church_new(&viewer, count, flash, &csrf, draft)),
    ))
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
