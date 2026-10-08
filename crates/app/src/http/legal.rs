use axum::extract::{Query, State};
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;

use crate::views;

use super::context::{bind_session, html, with_cookie};
use super::forms::FlashQuery;
use super::{AppError, AppState};

pub async fn privacy(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    let markup = views::privacy(views::flash_from(flash.ok, flash.err), &session.csrf);
    Ok(with_cookie(jar, html(markup)))
}

pub async fn terms(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    let markup = views::terms(views::flash_from(flash.ok, flash.err), &session.csrf);
    Ok(with_cookie(jar, html(markup)))
}

pub async fn give(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state).await?;
    let markup = views::give(views::flash_from(flash.ok, flash.err), &session.csrf);
    Ok(with_cookie(jar, html(markup)))
}
