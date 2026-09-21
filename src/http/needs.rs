use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::leaf::{
    accept_application, apply_to_need, close_need, decline_application, post_need, Application,
    CatalogPresence, Need, OfferState, PriorOffer, Viewer,
};
use crate::sdk::clock::{new_id, now_iso};
use crate::views;

use super::context::{
    apply_leaf_redirect, bind_session, fail_csrf, html, leaf_err, optional_gift_id, redirect_err,
    redirect_ok, require_user, unread, viewer_for, with_cookie,
};
use super::forms::{ApplyForm, CsrfForm, FlashQuery, NeedForm, NeedQuery};
use super::{AppError, AppState};

pub async fn need_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<NeedQuery>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let gifts = state.db.gifts().await?;
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::need_new(
            &viewer,
            &gifts,
            query.church_id.as_deref(),
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

pub async fn create_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<NeedForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf("/needs/new")));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let gift = optional_gift_id(&form.gift_id);
    let presence = gift_presence(&state, gift).await?;
    let effect = match post_need(
        &viewer,
        &form.church_id,
        &form.title,
        &form.body,
        gift,
        presence,
        &form.scope,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err("/needs/new", error))),
    };
    let Some(need_id) = effect.inserted_need_id() else {
        return Ok(with_cookie(jar, redirect_err("/needs/new", "missing")));
    };
    let dest = format!("/needs/{need_id}");
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "need_posted")))
}

async fn gift_presence(state: &AppState, gift: Option<&str>) -> Result<CatalogPresence, AppError> {
    match gift {
        Some(id) => Ok(CatalogPresence::of_lookup(state.db.gift(id).await?)),
        None => Ok(CatalogPresence::Listed),
    }
}

pub async fn need_show(
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
    let Some(card) = state.db.need_card(&id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("We couldn't find that need.")),
        ));
    };
    let Some(church) = state.db.church(&card.church_id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::error_page("We couldn't find that church.")),
        ));
    };
    if !crate::leaf::can_view_need(&viewer, card.sight(), &church) {
        return Ok(with_cookie(
            jar,
            html(views::error_page(
                "This need is only visible to its church.",
            )),
        ));
    }
    let applications = state.db.applications_for_need(&card.id).await?;
    let offer = OfferState::of_existing(applications.iter().find(|a| a.user_id == viewer.user.id));
    let help = crate::leaf::can_apply(&viewer, card.sight(), &church);
    let count = unread(&state.db, &viewer.user.id).await?;
    Ok(with_cookie(
        jar,
        html(views::need_show(
            &viewer,
            &card,
            &church,
            &applications,
            help,
            offer,
            count,
            views::flash_from(flash.ok, flash.err),
            &session.csrf,
        )),
    ))
}

pub async fn apply_need(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ApplyForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let Some(church) = state.db.church(&need.church_id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let prior =
        PriorOffer::of_existing(state.db.application_pair(&need.id, &viewer.user.id).await?);
    let effect = match apply_to_need(
        &viewer,
        &need,
        &church,
        prior,
        &form.message,
        new_id(),
        now_iso(),
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "applied")))
}

pub async fn close_need_http(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Response, AppError> {
    let dest = format!("/needs/{id}");
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, fail_csrf(&dest)));
    }
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Ok(with_cookie(jar, response)),
    };
    let viewer = viewer_for(&state.db, user).await?;
    let Some(need) = state.db.need(&id).await? else {
        return Ok(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    let effect = match close_need(&viewer, &need) {
        Ok(effect) => effect,
        Err(error) => return Ok(with_cookie(jar, leaf_err(&dest, error))),
    };
    state.db.apply(&effect).await?;
    Ok(with_cookie(jar, redirect_ok(&dest, "need_closed")))
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
    apply_leaf_redirect(
        &state.db,
        loaded.jar,
        &loaded.dest,
        accept_application(&loaded.viewer, &loaded.need, &loaded.application),
        "application_accepted",
    )
    .await
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
    apply_leaf_redirect(
        &state.db,
        loaded.jar,
        &loaded.dest,
        decline_application(&loaded.viewer, &loaded.need, &loaded.application),
        "declined",
    )
    .await
}

struct ApplicationDecision {
    jar: CookieJar,
    viewer: Viewer,
    need: Need,
    application: Application,
    dest: String,
}

async fn load_application_decision(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    csrf: &str,
) -> Result<ApplicationDecision, Response> {
    let (session, jar) = bind_session(jar, &state.secret);
    let user = match require_user(&state.db, &session).await {
        Ok(user) => user,
        Err(response) => return Err(with_cookie(jar, response)),
    };
    let viewer = match viewer_for(&state.db, user).await {
        Ok(viewer) => viewer,
        Err(error) => return Err(error.into_response()),
    };
    let Some(application) = state
        .db
        .application(id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(jar, redirect_err("/home", "not_found")));
    };
    let dest = format!("/needs/{}", application.need_id);
    if !session.check_csrf(csrf) {
        return Err(with_cookie(jar, fail_csrf(&dest)));
    }
    let Some(need) = state
        .db
        .need(&application.need_id)
        .await
        .map_err(|error| AppError::from(error).into_response())?
    else {
        return Err(with_cookie(jar, redirect_err(&dest, "not_found")));
    };
    Ok(ApplicationDecision {
        jar,
        viewer,
        need,
        application,
        dest,
    })
}
