//! JSON session endpoints for native and API clients.

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use ecclesia_sdk::limit::RateKind;
use ecclesia_sdk::prelude::DomainError;
use ecclesia_sdk::story::{api_logout_bearer, api_me_profile, refresh_api, sign_in_api};
use serde::{Deserialize, Serialize};

use super::context::{ClientKey, device_meta};
use super::AppState;

#[derive(Deserialize)]
pub struct SignInBody {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshBody {
    pub refresh_token: String,
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    token_type: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/session", post(api_sign_in))
        .route("/session/refresh", post(api_refresh))
        .route("/session/logout", post(api_logout))
        .route("/me", get(api_me))
        .fallback(api_not_found)
}

pub async fn api_not_found() -> Response {
    json_error(StatusCode::NOT_FOUND, "miss")
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response
}

fn json_error(status: StatusCode, code: &'static str) -> Response {
    no_store((status, Json(ErrorBody { error: code })).into_response())
}

fn server_error() -> Response {
    json_error(StatusCode::INTERNAL_SERVER_ERROR, "error")
}

fn token_response(tokens: ecclesia_sdk::story::ApiSessionTokens) -> Response {
    no_store(
        (
            StatusCode::OK,
            Json(TokenResponse {
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                expires_in: tokens.expires_in,
                token_type: "Bearer",
            }),
        )
            .into_response(),
    )
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    let trimmed = value.trim();
    let (scheme, token) = trimmed.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub async fn api_sign_in(
    State(state): State<AppState>,
    who: ClientKey,
    headers: HeaderMap,
    body: Result<Json<SignInBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "missing"),
    };
    if body.email.trim().is_empty() || body.password.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "missing");
    }
    if let ecclesia_sdk::limit::RateDecision::Refuse =
        state.decide_rate(RateKind::Session, &who.0).await
    {
        return json_error(StatusCode::TOO_MANY_REQUESTS, "rate");
    }
    let device = device_meta(&headers, &who);
    match sign_in_api(
        &state.sdk,
        &state.secret,
        &body.email,
        &body.password,
        &device,
    )
    .await
    {
        Ok(Ok(tokens)) => token_response(tokens),
        Ok(Err(DomainError::NotFound)) => json_error(StatusCode::UNAUTHORIZED, "miss"),
        Ok(Err(_)) => json_error(StatusCode::UNAUTHORIZED, "miss"),
        Err(error) => {
            tracing::error!("api sign in: {error:#}");
            server_error()
        }
    }
}

pub async fn api_refresh(
    State(state): State<AppState>,
    who: ClientKey,
    body: Result<Json<RefreshBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "missing"),
    };
    if body.refresh_token.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "missing");
    }
    if let ecclesia_sdk::limit::RateDecision::Refuse =
        state.decide_rate(RateKind::Session, &who.0).await
    {
        return json_error(StatusCode::TOO_MANY_REQUESTS, "rate");
    }
    match refresh_api(&state.sdk, &state.secret, &body.refresh_token).await {
        Ok(Some(tokens)) => token_response(tokens),
        Ok(None) => json_error(StatusCode::UNAUTHORIZED, "miss"),
        Err(error) => {
            tracing::error!("api refresh: {error:#}");
            server_error()
        }
    }
}

pub async fn api_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return json_error(StatusCode::UNAUTHORIZED, "miss");
    };
    match api_logout_bearer(&state.sdk, &state.secret, token).await {
        Ok(true) => no_store(StatusCode::NO_CONTENT.into_response()),
        Ok(false) => json_error(StatusCode::UNAUTHORIZED, "miss"),
        Err(error) => {
            tracing::error!("api logout: {error:#}");
            server_error()
        }
    }
}

pub async fn api_me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return json_error(StatusCode::UNAUTHORIZED, "miss");
    };
    match api_me_profile(&state.sdk, &state.secret, token).await {
        Ok(Some(profile)) => no_store((StatusCode::OK, Json(profile)).into_response()),
        Ok(None) => json_error(StatusCode::UNAUTHORIZED, "miss"),
        Err(error) => {
            tracing::error!("api me: {error:#}");
            server_error()
        }
    }
}
