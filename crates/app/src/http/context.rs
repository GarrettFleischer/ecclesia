use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::net::SocketAddr;

use ecclesia_sdk::db::Db;
use ecclesia_sdk::prelude::{
    Church, DomainError, Membership, User, Viewer, unique_church_ids,
};
use ecclesia_sdk::session::Session;
use ecclesia_sdk::story::StoryOk;

use super::AppError;

pub struct SignedIn {
    pub session: Session,
    pub jar: CookieJar,
    pub user: User,
}

pub fn html(markup: maud::Markup) -> Html<String> {
    Html(markup.into_string())
}

pub struct ClientKey(pub String);

impl<S> FromRequestParts<S> for ClientKey
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(who_from_parts(parts)))
    }
}

fn who_from_parts(parts: &Parts) -> String {
    match parts.extensions.get::<ConnectInfo<SocketAddr>>() {
        Some(ConnectInfo(addr)) => addr.ip().to_string(),
        None => String::from("local"),
    }
}

pub async fn bind_session(
    jar: CookieJar,
    state: &super::AppState,
) -> Result<(Session, CookieJar), AppError> {
    match ecclesia_sdk::session::from_jar(&state.secret, &jar) {
        ecclesia_sdk::session::JarSession::Known(parsed) => {
            let had_id = parsed.session_id.is_some();
            let live = ecclesia_sdk::story::resolve_session(&state.sdk, parsed).await?;
            if had_id && live.session_id.is_none() {
                let jar = state.put_session(jar, &live);
                Ok((live, jar))
            } else {
                Ok((live, jar))
            }
        }
        ecclesia_sdk::session::JarSession::Minted(session) => {
            let jar = state.put_session(jar, &session);
            Ok((session, jar))
        }
    }
}

pub fn device_meta(headers: &axum::http::HeaderMap, who: &ClientKey) -> ecclesia_sdk::story::DeviceMeta {
    let ip = headers
        .get("fly-client-ip")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or(who.0.as_str())
        .to_string();
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    ecclesia_sdk::story::DeviceMeta { user_agent, ip }
}

pub async fn signed_in(state: &super::AppState, jar: CookieJar) -> Result<SignedIn, Response> {
    let (session, jar) = bind_session(jar, state)
        .await
        .map_err(IntoResponse::into_response)?;
    match require_user(&state.sdk.db, &session).await {
        Ok(user) => Ok(SignedIn { session, jar, user }),
        Err(response) => Err(with_cookie(jar, response)),
    }
}

pub async fn signed_form(
    state: &super::AppState,
    jar: CookieJar,
    csrf: &str,
    fail_path: &str,
) -> Result<SignedIn, Response> {
    let signed = signed_in(state, jar).await?;
    if !signed.session.check_csrf(csrf) {
        return Err(with_cookie(signed.jar, fail_csrf(fail_path)));
    }
    Ok(signed)
}

pub fn fail_csrf(path: &str) -> Redirect {
    redirect_err(path, "csrf")
}

pub fn redirect_ok(path: &str, code: &str) -> Redirect {
    Redirect::to(&flash_location(path, "ok", code))
}

pub fn redirect_err(path: &str, code: &str) -> Redirect {
    Redirect::to(&flash_location(path, "err", code))
}

fn flash_location(path: &str, key: &str, code: &str) -> String {
    format!("{}?{key}={code}", same_origin_path(path))
}

fn same_origin_path(path: &str) -> &str {
    if path.starts_with('/') && !path.starts_with("//") && !path.contains('\\') {
        path
    } else {
        "/"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_sec_17_redirects_stay_on_this_origin() {
        assert_eq!(same_origin_path("/churches/grace"), "/churches/grace");
        assert_eq!(same_origin_path("//evil.example"), "/");
        assert_eq!(same_origin_path("https://evil.example"), "/");
        assert_eq!(same_origin_path("/\\evil"), "/");
    }
}

pub fn leaf_err(path: &str, error: DomainError) -> Redirect {
    redirect_err(path, error.flash_code())
}

pub fn story_redirect(
    jar: CookieJar,
    dest: &str,
    result: Result<StoryOk, DomainError>,
    ok: &str,
) -> Result<Response, AppError> {
    match result {
        Ok(_) => Ok(with_cookie(jar, redirect_ok(dest, ok))),
        Err(error) => Ok(with_cookie(jar, leaf_err(dest, error))),
    }
}

pub fn with_cookie(jar: CookieJar, body: impl IntoResponse) -> Response {
    (jar, body).into_response()
}

pub async fn load_user(db: &Db, session: &Session) -> Result<Option<User>, AppError> {
    let Some(id) = session.user_id.as_deref() else {
        return Ok(None);
    };
    Ok(db.user(id).await?)
}

pub async fn require_user(db: &Db, session: &Session) -> Result<User, Response> {
    match load_user(db, session).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(Redirect::to("/home?err=auth").into_response()),
        Err(error) => Err(error.into_response()),
    }
}

pub async fn viewer_for(db: &Db, user: User) -> Result<Viewer, AppError> {
    let memberships = db.memberships_for_user(&user.id).await?;
    let churches = db.churches_for_user(&user.id).await?;
    let gift_ids = db.gift_ids_for(&user.id).await?;
    Ok(Viewer {
        user,
        memberships,
        churches,
        gift_ids,
    })
}

pub async fn unread(db: &Db, user_id: &str) -> Result<i64, AppError> {
    Ok(db.unread_count(user_id).await?)
}

pub async fn churches_for_memberships(
    db: &Db,
    memberships: &[Membership],
) -> Result<Vec<Church>, AppError> {
    let ids = unique_church_ids(memberships);
    Ok(db.churches_with_ids(&ids).await?)
}

pub fn optional_gift_id(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub async fn security_headers(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    attach_security_headers(response.headers_mut());
    response
}

pub async fn soften_form_errors(request: axum::extract::Request, next: Next) -> Response {
    let response = next.run(request).await;
    if response.status() != StatusCode::UNPROCESSABLE_ENTITY {
        return response;
    }
    (
        StatusCode::BAD_REQUEST,
        html(crate::views::error_page("Fill in the required fields.")),
    )
        .into_response()
}

fn attach_security_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; script-src 'self'; worker-src 'self'; connect-src 'self'; style-src 'self' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        header::HeaderName::from_static("x-permitted-cross-domain-policies"),
        HeaderValue::from_static("none"),
    );
}
