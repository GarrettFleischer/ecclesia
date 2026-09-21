use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::net::SocketAddr;

use crate::db::Db;
use crate::leaf::{
    Church, ChurchCard, DomainError, Effect, Membership, PlaceGroup, User, Viewer,
    churches_with_counts, group_churches_by_place, unique_church_ids,
};
use crate::sdk::session::Session;

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

pub fn bind_session(jar: CookieJar, state: &super::AppState) -> (Session, CookieJar) {
    match crate::sdk::session::from_jar(&state.secret, &jar) {
        crate::sdk::session::JarSession::Known(session) => (session, jar),
        crate::sdk::session::JarSession::Minted(session) => {
            let jar = state.put_session(jar, &session);
            (session, jar)
        }
    }
}

pub async fn signed_in(state: &super::AppState, jar: CookieJar) -> Result<SignedIn, Response> {
    let (session, jar) = bind_session(jar, state);
    match require_user(&state.db, &session).await {
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

pub async fn apply_leaf_redirect(
    state: &super::AppState,
    jar: CookieJar,
    dest: &str,
    effect: Result<Effect, DomainError>,
    ok: &str,
) -> Result<Response, AppError> {
    match effect {
        Ok(effect) => {
            state.commit(&effect).await?;
            Ok(with_cookie(jar, redirect_ok(dest, ok)))
        }
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
        Ok(None) => Err(Redirect::to("/?err=auth").into_response()),
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

pub async fn church_directory(db: &Db) -> Result<Vec<ChurchCard>, AppError> {
    let churches = db.churches().await?;
    let counts = db.counts_for_churches().await?;
    Ok(churches_with_counts(churches, &counts))
}

pub async fn churches_by_place(db: &Db) -> Result<Vec<PlaceGroup>, AppError> {
    let cards = church_directory(db).await?;
    Ok(group_churches_by_place(cards))
}

pub async fn governor_ids(db: &Db, church_id: &str) -> Result<Vec<String>, AppError> {
    Ok(db.governor_ids(church_id).await?)
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
