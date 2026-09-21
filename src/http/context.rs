use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::db::Db;
use crate::leaf::{
    churches_with_counts, group_churches_by_place, unique_church_ids, Church, ChurchCard,
    DomainError, Effect, Membership, PlaceGroup, User, Viewer,
};
use crate::sdk::session::{self, Session};

use super::AppError;

pub struct SignedIn {
    pub session: Session,
    pub jar: CookieJar,
    pub user: User,
}

pub fn html(markup: maud::Markup) -> Html<String> {
    Html(markup.into_string())
}

pub fn bind_session(jar: CookieJar, secret: &str) -> (Session, CookieJar) {
    let session = session::from_jar(secret, &jar);
    let jar = session::put(jar, secret, &session);
    (session, jar)
}

pub async fn signed_in(state: &super::AppState, jar: CookieJar) -> Result<SignedIn, Response> {
    let (session, jar) = bind_session(jar, &state.secret);
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
    Redirect::to(&format!("{path}?err=csrf"))
}

pub fn redirect_ok(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?ok={code}"))
}

pub fn redirect_err(path: &str, code: &str) -> Redirect {
    Redirect::to(&format!("{path}?err={code}"))
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
}
