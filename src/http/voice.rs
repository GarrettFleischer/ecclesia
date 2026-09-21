//! Optional rewrite. CSRF is enough; guests write a bio before they have a seat.

use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;

use crate::leaf::VoiceKind;

use super::context::{bind_session, with_cookie};
use super::forms::RefineForm;
use super::{AppError, AppState};

#[derive(Serialize)]
struct RefineReply {
    text: String,
    seat: &'static str,
}

pub async fn refine_words(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RefineForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state.secret);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(
            jar,
            (
                StatusCode::FORBIDDEN,
                Json(RefineReply {
                    text: String::new(),
                    seat: "echo",
                }),
            ),
        ));
    }
    let Some(kind) = VoiceKind::parse(form.kind.trim()) else {
        return Ok(with_cookie(
            jar,
            (
                StatusCode::BAD_REQUEST,
                Json(RefineReply {
                    text: form.text,
                    seat: "echo",
                }),
            ),
        ));
    };
    let text = match state.refine.rewrite(kind, &form.text).await {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!("rewrite failed: {error:#}");
            form.text.trim().to_string()
        }
    };
    let seat = if state.refine.is_live() {
        "live"
    } else {
        "echo"
    };
    Ok(with_cookie(jar, Json(RefineReply { text, seat })))
}
