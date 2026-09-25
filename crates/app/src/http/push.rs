use axum::extract::{Form, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use std::path::PathBuf;

use ecclesia_sdk::limit::{RateDecision, RateKind};
use ecclesia_sdk::prelude::DomainError;
use ecclesia_sdk::story;

use super::context::{ClientKey, signed_form, with_cookie};
use super::forms::{PushDeviceForm, PushSubscribeForm, PushUnsubscribeForm};
use super::{AppError, AppState};

pub async fn service_worker() -> Response {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static/sw.js");
    match std::fs::read_to_string(path) {
        Ok(body) => (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/javascript; charset=utf-8"),
                ),
                (
                    header::HeaderName::from_static("service-worker-allowed"),
                    HeaderValue::from_static("/"),
                ),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            ],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn vapid_public(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; charset=utf-8")],
        state.sdk.push.public_key.clone(),
    )
}

pub async fn subscribe(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<PushSubscribeForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if let RateDecision::Refuse = state.decide_rate(RateKind::Push, &who.0).await {
        return Ok(with_cookie(signed.jar, StatusCode::TOO_MANY_REQUESTS));
    }
    match story::subscribe_push(
        &state.sdk,
        &signed.user.id,
        &form.endpoint,
        &form.p256dh,
        &form.auth,
    )
    .await?
    {
        Ok(_) => Ok(with_cookie(signed.jar, StatusCode::NO_CONTENT)),
        Err(DomainError::InvalidInput) => Ok(with_cookie(signed.jar, StatusCode::BAD_REQUEST)),
        Err(_) => Ok(with_cookie(signed.jar, StatusCode::BAD_REQUEST)),
    }
}

pub async fn unsubscribe(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<PushUnsubscribeForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if let RateDecision::Refuse = state.decide_rate(RateKind::Push, &who.0).await {
        return Ok(with_cookie(signed.jar, StatusCode::TOO_MANY_REQUESTS));
    }
    match story::unsubscribe_push(&state.sdk, &signed.user.id, &form.endpoint).await? {
        Ok(_) => Ok(with_cookie(signed.jar, StatusCode::NO_CONTENT)),
        Err(_) => Ok(with_cookie(signed.jar, StatusCode::BAD_REQUEST)),
    }
}

pub async fn register_device(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<PushDeviceForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/me").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    if let RateDecision::Refuse = state.decide_rate(RateKind::Push, &who.0).await {
        return Ok(with_cookie(signed.jar, StatusCode::TOO_MANY_REQUESTS));
    }
    match story::register_device(&state.sdk, &signed.user.id, &form.token, &form.platform).await? {
        Ok(_) => Ok(with_cookie(signed.jar, StatusCode::NO_CONTENT)),
        Err(_) => Ok(with_cookie(signed.jar, StatusCode::BAD_REQUEST)),
    }
}
